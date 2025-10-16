#!/bin/bash
# KMS Clean Deploy - 完全重新编译部署脚本
# 包含 Docker 容器重启选项，确保干净的编译环境

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

# 显示帮助
show_help() {
    cat <<EOF
${CYAN}KMS Clean Deploy - 完全重新编译部署${NC}

用法: $0 [选项]

选项:
  -r, --restart-docker  重启 Docker 容器（推荐）
  -f, --force           强制清理所有缓存
  -h, --help            显示此帮助信息

示例:
  $0                    # 完全重新编译
  $0 --restart-docker   # 重启容器后编译（最可靠）
  $0 --force            # 强制清理所有缓存

EOF
}

# 解析命令行参数
RESTART_DOCKER=0
FORCE_CLEAN=0

while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--restart-docker)
            RESTART_DOCKER=1
            shift
            ;;
        -f|--force)
            FORCE_CLEAN=1
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

echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}🔧 KMS Clean Deploy${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

# Step 1: 重启 Docker 容器（可选）
if [ $RESTART_DOCKER -eq 1 ]; then
    log_step "Step 1/7: 重启 Docker 容器"

    log_info "停止容器..."
    docker stop teaclave_dev_env 2>/dev/null || true

    log_info "启动容器..."
    docker start teaclave_dev_env

    log_info "等待容器就绪..."
    sleep 5

    log_success "Docker 容器已重启"
else
    log_step "Step 1/7: 检查 Docker 容器"

    if ! docker ps | grep -q teaclave_dev_env; then
        log_error "Docker 容器未运行！"
        echo "请先启动容器或使用 --restart-docker 选项"
        exit 1
    fi

    log_success "Docker 容器运行中"
fi

# Step 2: 同步源码
log_step "Step 2/7: 同步源码到 SDK"

log_info "从 kms/ → third_party/teaclave-trustzone-sdk/projects/web3/kms/"

# 确保目标目录存在
KMS_SDK_DIR="$PROJECT_ROOT/third_party/teaclave-trustzone-sdk/projects/web3/kms"
mkdir -p "$KMS_SDK_DIR"

# 使用 rsync 同步
rsync -av --delete \
    --exclude 'target/' \
    --exclude '*.ta' \
    --exclude '.cargo/' \
    "$KMS_DEV_DIR/" "$KMS_SDK_DIR/"

log_success "源码同步完成"

# Step 3: 清理构建缓存
log_step "Step 3/7: 清理构建缓存"

if [ $FORCE_CLEAN -eq 1 ]; then
    log_info "强制清理所有缓存..."
    docker exec teaclave_dev_env bash -l -c "
        cd /root/teaclave_sdk_src/projects/web3/kms && \
        rm -rf host/target/ ta/target/ && \
        echo 'All caches cleared'
    "
else
    log_info "清理增量编译缓存..."
    docker exec teaclave_dev_env bash -l -c "
        cd /root/teaclave_sdk_src/projects/web3/kms && \
        rm -rf host/target/aarch64-unknown-linux-gnu/release/incremental/ \
               host/target/aarch64-unknown-linux-gnu/release/build/ \
               host/target/aarch64-unknown-linux-gnu/release/deps/ && \
        echo 'Incremental cache cleared'
    "
fi

log_success "缓存已清理"

# Step 4: 编译 Host (kms-api-server)
log_step "Step 4/7: 编译 Host (kms-api-server)"

CPU_CORES=$(docker exec teaclave_dev_env nproc 2>/dev/null || echo "8")
log_info "使用 $CPU_CORES 个 CPU 核心编译"

START_TIME=$(date +%s)

docker exec teaclave_dev_env bash -l -c "
    cd /root/teaclave_sdk_src/projects/web3/kms/host && \
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc && \
    export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc && \
    export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ && \
    cargo build --release --target aarch64-unknown-linux-gnu --bin kms-api-server 2>&1 | \
    grep -E 'Compiling|Finished|error' || true
"

BUILD_EXIT_CODE=$?
END_TIME=$(date +%s)
BUILD_DURATION=$((END_TIME - START_TIME))

if [ $BUILD_EXIT_CODE -ne 0 ]; then
    log_error "Host 编译失败！"
    exit 1
fi

log_success "Host 编译完成 (耗时 ${BUILD_DURATION}s)"

# Step 5: 编译 TA (可选，通常不需要重新编译)
log_step "Step 5/7: 跳过 TA 编译"
log_info "TA 未修改，使用现有版本"

# Step 6: 部署二进制文件
log_step "Step 6/7: 部署到 QEMU 共享目录"

log_info "复制二进制文件..."
docker exec teaclave_dev_env bash -l -c "
    set -e
    mkdir -p /opt/teaclave/shared/ta
    mkdir -p /opt/teaclave/shared/kms-wallets-backup

    # 复制 Host 二进制文件
    cp -v /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/kms-api-server

    # 复制测试页面
    if [ -f /root/teaclave_sdk_src/projects/web3/kms/host/kms-test-page.html ]; then
        cp -v /root/teaclave_sdk_src/projects/web3/kms/host/kms-test-page.html /opt/teaclave/shared/kms-test-page.html
    fi

    # 显示文件信息
    echo ''
    echo '=== Deployed Files ==='
    ls -lh /opt/teaclave/shared/ | grep -E 'kms-api-server|\.html$'

    echo ''
    echo '=== Backup Directory ==='
    ls -lah /opt/teaclave/shared/kms-wallets-backup/ || echo '(empty)'
"

log_success "二进制文件已部署"

# Step 7: 重启 API Server
log_step "Step 7/7: 重启 Guest VM 中的 API Server"

# 检查 QEMU 是否在运行
if ! docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
    log_warn "QEMU 未运行，跳过 API Server 重启"
    echo ""
    echo "启动 QEMU 后，API Server 会自动启动"
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
        echo ""
        echo "手动重启方法:"
        echo "  echo 'sh /root/shared/.restart_api.sh' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320"
    }
fi

# 完成总结
echo ""
echo -e "${CYAN}========================================${NC}"
echo -e "${GREEN}🎉 部署完成！${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""
echo -e "${BLUE}📊 部署摘要:${NC}"
echo "  ✅ 源码已同步到 SDK"
echo "  ✅ 缓存已清理"
echo "  ✅ 编译完成 (耗时 ${BUILD_DURATION}s)"
echo "  ✅ 二进制文件已部署到 /opt/teaclave/shared"
echo "  ✅ API Server 已重启"
echo ""
echo -e "${BLUE}🔗 访问地址:${NC}"
echo "  本地: http://localhost:3000"
echo "  测试页面: http://localhost:3000/test"
echo "  健康检查: http://localhost:3000/health"
echo "  公网: https://kms.aastar.io"
echo ""
echo -e "${BLUE}💾 备份目录:${NC}"
echo "  Guest VM: /root/shared/kms-wallets-backup/"
echo "  本地映射: docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/kms-wallets-backup/"
echo ""
