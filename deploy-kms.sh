#!/bin/bash

# KMS一键部署脚本 - 支持Mock-TEE和QEMU-TEE两个版本
# One-key deployment script for KMS - supports both Mock-TEE and QEMU-TEE versions

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KMS_DIR="$SCRIPT_DIR/kms"
CLOUDFLARED_PID_FILE="/tmp/cloudflared.pid"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

show_help() {
    cat <<EOF
KMS部署脚本 - 支持Mock-TEE和QEMU-TEE版本

用法: $0 [选项] <命令>

命令:
  mock-deploy     部署Mock-TEE版本（快速测试）
  qemu-deploy     部署QEMU-TEE版本（真实TEE环境）
  test-all        测试所有API端点
  status          检查部署状态
  stop            停止所有服务
  clean           清理部署环境

选项:
  -p, --port PORT    指定API服务端口 (默认: 8080)
  -t, --tunnel       启用Cloudflare Tunnel公网访问
  -h, --help         显示此帮助信息

示例:
  $0 mock-deploy -t              # 部署Mock版本并启用公网隧道
  $0 qemu-deploy -p 9090         # 部署QEMU版本到端口9090
  $0 test-all                    # 测试所有API
EOF
}

check_dependencies() {
    log_info "检查依赖..."

    # 检查Rust
    if ! command -v cargo &> /dev/null; then
        log_error "需要安装Rust: https://rustup.rs/"
        exit 1
    fi

    # 检查Docker (仅QEMU版本需要)
    if [[ "$1" == "qemu" ]] && ! command -v docker &> /dev/null; then
        log_error "QEMU版本需要Docker: https://docker.com/"
        exit 1
    fi

    log_success "依赖检查完成"
}

stop_services() {
    log_info "停止现有服务..."

    # 停止Cloudflare tunnel
    if [[ -f "$CLOUDFLARED_PID_FILE" ]]; then
        local pid=$(cat "$CLOUDFLARED_PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid"
            log_info "已停止Cloudflare tunnel (PID: $pid)"
        fi
        rm -f "$CLOUDFLARED_PID_FILE"
    fi

    # 停止KMS API服务
    pkill -f "kms-server" || true
    pkill -f "target.*kms-api" || true

    # 停止Docker容器
    docker stop kms-qemu-tee 2>/dev/null || true
    docker rm kms-qemu-tee 2>/dev/null || true

    log_success "服务已停止"
}

deploy_mock_version() {
    local port="${1:-8080}"
    local enable_tunnel="$2"

    log_info "部署Mock-TEE版本到端口 $port..."

    cd "$KMS_DIR/kms-api"

    # 构建并启动Mock版本
    log_info "构建Mock-TEE版本..."
    cargo build --release

    log_info "启动KMS API服务 (Mock-TEE)..."
    RUST_LOG=info cargo run --release &
    local api_pid=$!

    # 等待服务启动
    sleep 3

    # 健康检查
    if curl -s "http://localhost:$port/health" > /dev/null; then
        log_success "Mock-TEE版本部署成功！"
        log_info "API服务地址: http://localhost:$port"
        log_info "进程ID: $api_pid"

        if [[ "$enable_tunnel" == "true" ]]; then
            setup_cloudflare_tunnel "$port"
        fi
    else
        log_error "Mock-TEE版本部署失败"
        kill $api_pid 2>/dev/null || true
        exit 1
    fi
}

deploy_qemu_version() {
    local port="${1:-8080}"
    local enable_tunnel="$2"

    log_info "部署QEMU-TEE版本到端口 $port..."

    # 检查OP-TEE环境
    if [[ ! -d "$SCRIPT_DIR/third_party" ]]; then
        log_error "OP-TEE环境未初始化，请先运行: git submodule update --init --recursive"
        exit 1
    fi

    # 启动Docker OP-TEE环境
    log_info "启动Docker OP-TEE环境..."
    docker run -d \
        --name kms-qemu-tee \
        -p "$port:8080" \
        -v "$SCRIPT_DIR:/workspace" \
        -w "/workspace/kms" \
        teaclave/teaclave-trustzone-sdk-builder:latest \
        bash -c "cd kms-api && OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/out/export cargo run --release"

    # 等待服务启动
    sleep 10

    # 健康检查
    if curl -s "http://localhost:$port/health" > /dev/null; then
        log_success "QEMU-TEE版本部署成功！"
        log_info "API服务地址: http://localhost:$port"
        log_info "Docker容器: kms-qemu-tee"

        if [[ "$enable_tunnel" == "true" ]]; then
            setup_cloudflare_tunnel "$port"
        fi
    else
        log_error "QEMU-TEE版本部署失败"
        docker stop kms-qemu-tee 2>/dev/null || true
        exit 1
    fi
}

setup_cloudflare_tunnel() {
    local port="$1"

    log_info "设置Cloudflare Tunnel..."

    if ! command -v cloudflared &> /dev/null; then
        log_warn "cloudflared未安装，跳过公网隧道设置"
        return
    fi

    # 启动隧道
    cloudflared tunnel --url "http://localhost:$port" &
    local tunnel_pid=$!
    echo "$tunnel_pid" > "$CLOUDFLARED_PID_FILE"

    sleep 3
    log_success "Cloudflare Tunnel已启动 (PID: $tunnel_pid)"
    log_info "请查看上方输出获取公网URL"
}

test_all_apis() {
    local base_url="${1:-http://localhost:8080}"

    log_info "测试所有KMS API端点..."
    log_info "目标地址: $base_url"

    # 1. 健康检查
    log_info "1. 测试健康检查..."
    local health_response=$(curl -s "$base_url/health")
    echo "健康检查响应: $health_response"

    # 2. 创建密钥
    log_info "2. 测试创建密钥..."
    local create_response=$(curl -s -X POST "$base_url/" \
        -H "Content-Type: application/x-amz-json-1.1" \
        -H "X-Amz-Target: TrentService.CreateKey" \
        -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}')
    echo "创建密钥响应: $create_response"

    # 提取KeyId
    local key_id=$(echo "$create_response" | grep -o '"KeyId":"[^"]*"' | cut -d'"' -f4)
    if [[ -z "$key_id" ]]; then
        log_error "未能获取KeyId，创建密钥可能失败"
        return 1
    fi
    log_success "成功创建密钥: $key_id"

    # 3. 获取公钥
    log_info "3. 测试获取公钥..."
    local pubkey_response=$(curl -s -X POST "$base_url/" \
        -H "Content-Type: application/x-amz-json-1.1" \
        -H "X-Amz-Target: TrentService.GetPublicKey" \
        -d "{\"KeyId\":\"$key_id\"}")
    echo "获取公钥响应: $pubkey_response"

    # 4. 签名测试
    log_info "4. 测试消息签名..."
    local sign_response=$(curl -s -X POST "$base_url/" \
        -H "Content-Type: application/x-amz-json-1.1" \
        -H "X-Amz-Target: TrentService.Sign" \
        -d "{\"KeyId\":\"$key_id\",\"Message\":\"SGVsbG8gV29ybGQ=\",\"MessageType\":\"RAW\"}")
    echo "签名响应: $sign_response"

    # 5. 列出密钥
    log_info "5. 测试列出密钥..."
    local list_response=$(curl -s "$base_url/keys")
    echo "列出密钥响应: $list_response"

    # 6. 错误处理测试
    log_info "6. 测试错误处理..."
    local error_response=$(curl -s -X POST "$base_url/" \
        -H "Content-Type: application/x-amz-json-1.1" \
        -H "X-Amz-Target: TrentService.GetPublicKey" \
        -d '{"KeyId":"non-existent-key"}')
    echo "错误处理响应: $error_response"

    log_success "API测试完成！"
}

check_status() {
    log_info "检查部署状态..."

    # 检查进程
    local kms_processes=$(pgrep -f "kms-server\|kms-api" || true)
    if [[ -n "$kms_processes" ]]; then
        log_success "KMS进程运行中 (PIDs: $kms_processes)"
    else
        log_warn "未发现KMS进程"
    fi

    # 检查Docker
    local docker_status=$(docker ps --filter "name=kms-qemu-tee" --format "{{.Status}}" 2>/dev/null || true)
    if [[ -n "$docker_status" ]]; then
        log_success "Docker容器状态: $docker_status"
    else
        log_warn "Docker容器未运行"
    fi

    # 检查Cloudflare tunnel
    if [[ -f "$CLOUDFLARED_PID_FILE" ]]; then
        local tunnel_pid=$(cat "$CLOUDFLARED_PID_FILE")
        if kill -0 "$tunnel_pid" 2>/dev/null; then
            log_success "Cloudflare Tunnel运行中 (PID: $tunnel_pid)"
        else
            log_warn "Cloudflare Tunnel已停止"
            rm -f "$CLOUDFLARED_PID_FILE"
        fi
    else
        log_warn "Cloudflare Tunnel未启动"
    fi
}

# 解析命令行参数
PORT="8080"
ENABLE_TUNNEL="false"
COMMAND=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--port)
            PORT="$2"
            shift 2
            ;;
        -t|--tunnel)
            ENABLE_TUNNEL="true"
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        mock-deploy|qemu-deploy|test-all|status|stop|clean)
            COMMAND="$1"
            shift
            ;;
        *)
            log_error "未知参数: $1"
            show_help
            exit 1
            ;;
    esac
done

# 执行命令
case "$COMMAND" in
    "mock-deploy")
        check_dependencies "mock"
        stop_services
        deploy_mock_version "$PORT" "$ENABLE_TUNNEL"
        ;;
    "qemu-deploy")
        check_dependencies "qemu"
        stop_services
        deploy_qemu_version "$PORT" "$ENABLE_TUNNEL"
        ;;
    "test-all")
        test_all_apis
        ;;
    "status")
        check_status
        ;;
    "stop")
        stop_services
        ;;
    "clean")
        stop_services
        log_info "清理构建缓存..."
        cd "$KMS_DIR" && cargo clean
        log_success "清理完成"
        ;;
    *)
        log_error "请指定命令"
        show_help
        exit 1
        ;;
esac