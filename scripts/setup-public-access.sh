#!/bin/bash

# KMS公网访问设置脚本
# Setup script for KMS public access via Cloudflare Tunnel

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

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

LOCAL_PORT="${1:-8080}"
TUNNEL_PID_FILE="/tmp/cloudflared-kms.pid"
TUNNEL_URL_FILE="/tmp/kms-public-url.txt"

show_help() {
    cat <<EOF
KMS公网访问设置脚本

用法: $0 [端口] [命令]

端口: 本地KMS服务端口 (默认: 8080)

命令:
  start       启动公网隧道
  stop        停止公网隧道
  status      检查隧道状态
  url         显示公网URL
  test        测试公网API

示例:
  $0 8080 start       # 启动8080端口的公网隧道
  $0 url              # 显示当前公网URL
  $0 test             # 测试公网API
EOF
}

check_local_service() {
    log_info "检查本地KMS服务 (端口:$LOCAL_PORT)..."

    if curl -s "http://localhost:$LOCAL_PORT/health" > /dev/null; then
        log_success "✅ 本地KMS服务正常运行"
        return 0
    else
        log_error "❌ 本地KMS服务未运行"
        log_info "请先启动本地服务:"
        log_info "  cd kms/kms-api && cargo run --release"
        return 1
    fi
}

start_tunnel() {
    if [[ -f "$TUNNEL_PID_FILE" ]]; then
        local pid=$(cat "$TUNNEL_PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            log_warn "隧道已在运行 (PID: $pid)"
            return 0
        fi
    fi

    log_info "启动Cloudflare隧道..."

    if ! command -v cloudflared &> /dev/null; then
        log_error "cloudflared未安装"
        log_info "安装方法:"
        log_info "  macOS: brew install cloudflare/cloudflare/cloudflared"
        log_info "  或下载: https://github.com/cloudflare/cloudflared/releases"
        return 1
    fi

    if ! check_local_service; then
        return 1
    fi

    # 启动隧道并捕获输出
    log_info "创建临时隧道到 localhost:$LOCAL_PORT..."

    # 使用临时文件捕获cloudflared输出
    local tunnel_log="/tmp/cloudflared-$$.log"

    cloudflared tunnel --url "http://localhost:$LOCAL_PORT" > "$tunnel_log" 2>&1 &
    local tunnel_pid=$!

    echo "$tunnel_pid" > "$TUNNEL_PID_FILE"

    # 等待隧道启动并获取URL
    log_info "等待隧道启动..."
    sleep 5

    # 从日志中提取URL
    local public_url=""
    for i in {1..10}; do
        if [[ -f "$tunnel_log" ]]; then
            public_url=$(grep -o 'https://.*\.trycloudflare\.com' "$tunnel_log" | head -1)
            if [[ -n "$public_url" ]]; then
                break
            fi
        fi
        sleep 1
    done

    if [[ -n "$public_url" ]]; then
        echo "$public_url" > "$TUNNEL_URL_FILE"
        log_success "✅ 隧道启动成功!"
        log_info "公网URL: $public_url"
        log_info "PID: $tunnel_pid"

        # 测试公网访问
        log_info "测试公网访问..."
        sleep 2
        if curl -s "$public_url/health" > /dev/null; then
            log_success "✅ 公网API可访问"
        else
            log_warn "⚠️ 公网API暂时不可访问，请稍后重试"
        fi
    else
        log_error "❌ 无法获取公网URL"
        kill "$tunnel_pid" 2>/dev/null || true
        rm -f "$TUNNEL_PID_FILE"
        return 1
    fi

    # 清理日志文件
    rm -f "$tunnel_log"
}

stop_tunnel() {
    if [[ -f "$TUNNEL_PID_FILE" ]]; then
        local pid=$(cat "$TUNNEL_PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid"
            log_success "✅ 隧道已停止 (PID: $pid)"
        else
            log_warn "隧道进程不存在"
        fi
        rm -f "$TUNNEL_PID_FILE"
        rm -f "$TUNNEL_URL_FILE"
    else
        log_warn "隧道未运行"
    fi
}

show_status() {
    log_info "KMS服务状态:"

    # 检查本地服务
    if curl -s "http://localhost:$LOCAL_PORT/health" > /dev/null; then
        log_success "✅ 本地服务: http://localhost:$LOCAL_PORT (运行中)"
    else
        log_error "❌ 本地服务: http://localhost:$LOCAL_PORT (未运行)"
    fi

    # 检查隧道
    if [[ -f "$TUNNEL_PID_FILE" ]]; then
        local pid=$(cat "$TUNNEL_PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            local public_url=$(cat "$TUNNEL_URL_FILE" 2>/dev/null || echo "未知")
            log_success "✅ 公网隧道: $public_url (运行中, PID: $pid)"

            # 测试公网访问
            if [[ "$public_url" != "未知" ]] && curl -s "$public_url/health" > /dev/null; then
                log_success "✅ 公网API可访问"
            else
                log_warn "⚠️ 公网API不可访问"
            fi
        else
            log_error "❌ 公网隧道: 未运行"
            rm -f "$TUNNEL_PID_FILE" "$TUNNEL_URL_FILE"
        fi
    else
        log_warn "⚠️ 公网隧道: 未启动"
    fi
}

show_url() {
    if [[ -f "$TUNNEL_URL_FILE" ]]; then
        local public_url=$(cat "$TUNNEL_URL_FILE")
        echo "$public_url"
    else
        log_error "公网隧道未运行，请先启动:"
        log_info "  $0 start"
        return 1
    fi
}

test_api() {
    log_info "测试KMS API..."

    # 测试本地API
    log_info "1. 测试本地API..."
    if curl -s "http://localhost:$LOCAL_PORT/health" > /dev/null; then
        log_success "✅ 本地API正常"
        curl -s "http://localhost:$LOCAL_PORT/health" | jq . 2>/dev/null || curl -s "http://localhost:$LOCAL_PORT/health"
    else
        log_error "❌ 本地API失败"
    fi

    echo

    # 测试公网API
    if [[ -f "$TUNNEL_URL_FILE" ]]; then
        local public_url=$(cat "$TUNNEL_URL_FILE")
        log_info "2. 测试公网API..."
        log_info "URL: $public_url"

        if curl -s "$public_url/health" > /dev/null; then
            log_success "✅ 公网API正常"
            curl -s "$public_url/health" | jq . 2>/dev/null || curl -s "$public_url/health"

            echo
            log_info "3. 测试创建密钥..."
            curl -s -X POST "$public_url/" \
                -H "Content-Type: application/json" \
                -H "X-Amz-Target: TrentService.CreateKey" \
                -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}' | \
                jq . 2>/dev/null || curl -s -X POST "$public_url/" \
                -H "Content-Type: application/json" \
                -H "X-Amz-Target: TrentService.CreateKey" \
                -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}'
        else
            log_error "❌ 公网API失败"
        fi
    else
        log_warn "⚠️ 公网隧道未运行"
    fi
}

# 解析参数
if [[ $# -eq 0 ]]; then
    show_help
    exit 0
fi

# 如果第一个参数是数字，认为是端口
if [[ "$1" =~ ^[0-9]+$ ]]; then
    LOCAL_PORT="$1"
    shift
fi

COMMAND="$1"

case "$COMMAND" in
    "start")
        start_tunnel
        ;;
    "stop")
        stop_tunnel
        ;;
    "status")
        show_status
        ;;
    "url")
        show_url
        ;;
    "test")
        test_api
        ;;
    *)
        show_help
        ;;
esac