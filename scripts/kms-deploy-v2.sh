#!/bin/bash
# KMS Deploy V2 - 增强版部署脚本
# 直接在 docker 容器内编译并自动部署到 QEMU Guest VM

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${CYAN}==>${NC} ${BLUE}$1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

# 项目路径
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KMS_DEV_DIR="$PROJECT_ROOT/kms"
KMS_SDK_DIR="$PROJECT_ROOT/third_party/teaclave-trustzone-sdk/projects/web3/kms"

# 显示帮助
show_help() {
    cat <<EOF
${CYAN}KMS Deploy V2 - 增强版部署脚本${NC}

用法: $0 [选项]

选项:
  -c, --clean       清理构建缓存后重新编译
  -n, --no-restart  编译后不重启 API Server
  -h, --help        显示此帮助信息

示例:
  $0                  # 增量编译并部署
  $0 --clean          # 完全重新编译
  $0 --no-restart     # 只编译不重启

工作流程:
  1. 同步源码到 SDK 目录
  2. 在 Docker 容器内编译 (使用所有 CPU 核心)
  3. 复制二进制文件到 QEMU 共享目录
  4. 自动重启 Guest VM 中的 API Server
EOF
}

# 解析命令行参数
CLEAN_BUILD=0
NO_RESTART=0

while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--clean)
            CLEAN_BUILD=1
            shift
            ;;
        -n|--no-restart)
            NO_RESTART=1
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            log_error "未知参数: $1"
            show_help
            exit 1
            ;;
    esac
done

# 检查容器是否运行
log_step "Step 1/6: 检查环境"
if ! docker ps | grep -q teaclave_dev_env; then
    log_error "Docker 容器未运行！"
    echo "请先启动容器:"
    echo "  docker start teaclave_dev_env"
    echo "或运行:"
    echo "  ./scripts/kms-auto-start.sh"
    exit 1
fi
log_success "Docker 容器运行中"

# Step 1: 同步源码到SDK
log_step "Step 2/6: 同步源码到 SDK"
log_info "从 kms/ → third_party/teaclave-trustzone-sdk/projects/web3/kms/"

rsync -av --delete \
    --exclude 'target/' \
    --exclude '*.ta' \
    --exclude '.cargo/' \
    "$KMS_DEV_DIR/" "$KMS_SDK_DIR/"

log_success "源码同步完成"

# 检查 STD 依赖
log_info "检查 Rust std 依赖..."
if ! docker exec teaclave_dev_env bash -l -c "[ -d /root/teaclave_sdk_src/rust/rust ] && [ -d /root/teaclave_sdk_src/rust/libc ]"; then
    log_warn "Rust std 依赖缺失，开始初始化..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src && ./setup_std_dependencies.sh"
    log_success "Rust std 依赖初始化完成"
else
    log_info "Rust std 依赖已就绪"
fi

# Step 2: 清理构建（可选）
if [ $CLEAN_BUILD -eq 1 ]; then
    log_step "Step 3/6: 清理构建缓存"
    docker exec teaclave_dev_env bash -l -c "
        cd /root/teaclave_sdk_src/projects/web3/kms && \
        make clean && \
        echo 'Build cache cleaned'
    "
    log_success "构建缓存已清理"
else
    log_step "Step 3/6: 跳过清理（增量构建）"
    log_info "使用 --clean 参数进行完全重新编译"
fi

# Step 3: 编译 KMS (Host + TA)
log_step "Step 4/6: 编译 KMS (Host CA + TA)"

# 获取 CPU 核心数
CPU_CORES=$(docker exec teaclave_dev_env nproc 2>/dev/null || echo "4")
log_info "使用 $CPU_CORES 个 CPU 核心编译"

log_info "开始编译..."
START_TIME=$(date +%s)

# 在容器内编译，显示关键输出
docker exec teaclave_dev_env bash -l -c "
    cd /root/teaclave_sdk_src/projects/web3/kms && \
    make -j$CPU_CORES 2>&1 | tee /tmp/kms_build.log | grep -E 'Compiling|Finished|SIGN.*\.ta|error:' --line-buffered || true
"

BUILD_EXIT_CODE=$?
END_TIME=$(date +%s)
BUILD_DURATION=$((END_TIME - START_TIME))

if [ $BUILD_EXIT_CODE -ne 0 ]; then
    log_error "编译失败！查看完整日志:"
    echo "  docker exec teaclave_dev_env cat /tmp/kms_build.log"
    exit 1
fi

log_success "编译完成 (耗时 ${BUILD_DURATION}s)"

# Step 4: 部署到 QEMU 共享目录
log_step "Step 5/6: 部署到 QEMU 共享目录"

log_info "复制二进制文件..."
docker exec teaclave_dev_env bash -l -c "
    set -e
    mkdir -p /opt/teaclave/shared/ta
    mkdir -p /opt/teaclave/shared/plugin

    # 复制 Host 二进制文件
    cp -v /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/teaclave/shared/kms
    cp -v /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/kms-api-server
    cp -v /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/export_key /opt/teaclave/shared/export_key

    # 复制测试页面
    if [ -f /root/teaclave_sdk_src/projects/web3/kms/host/kms-test-page.html ]; then
        cp -v /root/teaclave_sdk_src/projects/web3/kms/host/kms-test-page.html /opt/teaclave/shared/kms-test-page.html
    fi

    # 复制 TA 文件
    cp -v /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ta/

    # 显示部署文件
    echo ''
    echo '=== Deployed Files ==='
    ls -lh /opt/teaclave/shared/ | grep -E 'kms|export_key|\.html$'
    echo ''
    echo '=== TA Files ==='
    ls -lh /opt/teaclave/shared/ta/*.ta
"

log_success "二进制文件已部署"

# Step 5: 重启 API Server (可选)
if [ $NO_RESTART -eq 1 ]; then
    log_step "Step 6/6: 跳过 API Server 重启"
    log_info "使用 --no-restart 参数跳过重启"
else
    log_step "Step 6/6: 重启 Guest VM 中的 API Server"

    # 检查 QEMU 是否在运行
    if ! docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
        log_warn "QEMU 未运行，跳过 API Server 重启"
        echo "启动 QEMU 后，可以手动启动 API Server:"
        echo "  cd /root/shared && ./kms-api-server &"
    else
        log_info "QEMU 正在运行，尝试重启 API Server..."

        # 创建重启脚本
        docker exec teaclave_dev_env bash -c "cat > /opt/teaclave/shared/.restart_api.sh << 'RESTART_SCRIPT'
#!/bin/sh
echo '[$(date)] Restarting KMS API Server...'

# 停止旧进程
pkill -9 kms-api-server 2>/dev/null || true
sleep 1

# 启动新版本
cd /root/shared || exit 1
nohup ./kms-api-server > kms-api.log 2>&1 &
API_PID=\$!

sleep 2

# 验证启动
if ps aux | grep -q \"[k]ms-api-server\"; then
    echo \"[SUCCESS] API Server started (PID: \$API_PID)\"
    tail -5 kms-api.log
else
    echo \"[ERROR] Failed to start API Server\"
    tail -20 kms-api.log
    exit 1
fi
RESTART_SCRIPT
chmod +x /opt/teaclave/shared/.restart_api.sh"

        # 尝试通过 socat 执行重启
        log_info "连接到 Guest VM (socat TCP:54320)..."
        timeout 10 docker exec teaclave_dev_env bash -c "
            echo 'sh /root/shared/.restart_api.sh' | socat - TCP:localhost:54320
        " 2>/dev/null && {
            log_success "API Server 重启成功"

            # 验证服务
            sleep 3
            if curl -s http://localhost:3000/health > /dev/null 2>&1; then
                log_success "API Server 健康检查通过"
            else
                log_warn "API Server 可能还在启动中，请等待几秒后访问"
            fi
        } || {
            log_warn "无法通过 socat 连接到 Guest VM (端口 54320)"
            echo "可能原因:"
            echo "  1. Guest VM 还在启动中"
            echo "  2. kms-qemu-terminal2.sh 未运行"
            echo ""
            echo "手动重启方法:"
            echo "  echo 'sh /root/shared/.restart_api.sh' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320"
        }
    fi
fi

# 完成总结
echo ""
echo -e "${CYAN}========================================${NC}"
echo -e "${GREEN}🎉 部署完成！${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""
echo -e "${BLUE}📊 部署摘要:${NC}"
echo "  ✅ 源码已同步到 SDK"
echo "  ✅ 编译完成 (耗时 ${BUILD_DURATION}s)"
echo "  ✅ 二进制文件已部署到 /opt/teaclave/shared"
if [ $NO_RESTART -eq 0 ]; then
    if docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
        echo "  🔄 API Server 已重启"
    else
        echo "  ⏸️  QEMU 未运行 (API Server 将在 QEMU 启动时自动启动)"
    fi
fi
echo ""
echo -e "${BLUE}🔗 访问地址:${NC}"
echo "  本地: http://localhost:3000"
echo "  测试页面: http://localhost:3000/test"
echo "  健康检查: http://localhost:3000/health"
if [ -f "$HOME/.cloudflared/config.yml" ]; then
    echo "  公网: https://kms.aastar.io"
fi
echo ""
echo -e "${BLUE}📝 开发流程:${NC}"
echo "  1. 修改代码: 编辑 kms/ 目录"
echo "  2. 部署: ./scripts/kms-deploy-v2.sh"
echo "  3. 测试: curl http://localhost:3000/health"
echo ""
echo -e "${BLUE}🔧 其他命令:${NC}"
echo "  查看日志: docker exec teaclave_dev_env cat /opt/teaclave/shared/kms-api.log"
echo "  重启服务: echo 'sh /root/shared/.restart_api.sh' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320"
echo "  进入 Guest VM: ./scripts/kms-guest-shell.sh"
echo ""
