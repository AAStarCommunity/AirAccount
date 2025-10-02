#!/bin/bash
# KMS Passkey 部署脚本 - 在独立 Docker 环境中构建和部署

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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
    echo -e "\n${BLUE}==>${NC} $1"
}

# 容器配置
CONTAINER_NAME="kms_passkey_dev"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# 检查容器是否运行
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log_error "容器未运行！请先执行: ./scripts/kms-passkey-docker.sh start"
    exit 1
fi

log_step "KMS Passkey 部署流程"
log_info "容器: $CONTAINER_NAME"
log_info "端口: 54330(VM) / 54331(Log) / 3001(API)"

# Step 0: 创建 rust 符号链接（如果不存在）
log_step "0/4 检查环境设置..."
docker exec "$CONTAINER_NAME" bash -l -c "
    cd /root/kms_passkey_src
    if [ ! -L rust ]; then
        ln -sf /root/teaclave_sdk_src/rust rust
        echo 'Created rust symlink'
    else
        echo 'Rust symlink exists'
    fi
"

# 是否清理构建
CLEAN_BUILD=${1:-no}
if [ "$CLEAN_BUILD" = "clean" ]; then
    log_step "1/4 清理旧构建..."
    docker exec "$CONTAINER_NAME" bash -l -c "
        cd /root/kms_passkey_src/kms && make clean
    "
else
    log_info "跳过清理（增量构建），如需完整构建请运行: $0 clean"
fi

# 构建 KMS
log_step "2/4 构建 KMS 项目（Host + TA）..."
log_warn "注意: 当前 TA 编译可能因 signature 版本冲突失败，这是已知问题"

docker exec "$CONTAINER_NAME" bash -l -c "
    source ~/.cargo/env
    source ~/.profile
    export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
    export OPTEE_OS_DIR=/opt/teaclave/optee/optee_os
    export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64
    cd /root/kms_passkey_src/kms
    make 2>&1 | grep -E 'Compiling|Finished|SIGN|error' || make
" || {
    log_warn "构建过程中出现错误，查看详细日志:"
    docker exec "$CONTAINER_NAME" bash -l -c "
        source ~/.cargo/env
        source ~/.profile
        export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
        export OPTEE_OS_DIR=/opt/teaclave/optee_os
        export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64
        cd /root/kms_passkey_src/kms
        make
    "
}

# 同步到 QEMU 共享目录
log_step "3/4 部署到共享目录..."
docker exec "$CONTAINER_NAME" bash -l -c "
    mkdir -p /opt/kms_passkey/shared
    
    # 复制 Host 二进制
    if [ -f /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/kms ]; then
        cp /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/kms_passkey/shared/
        echo '✅ kms binary deployed'
    fi
    
    if [ -f /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server ]; then
        cp /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/kms_passkey/shared/
        echo '✅ kms-api-server binary deployed'
    fi
    
    if [ -f /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/export_key ]; then
        cp /root/kms_passkey_src/kms/host/target/aarch64-unknown-linux-gnu/release/export_key /opt/kms_passkey/shared/
        echo '✅ export_key binary deployed'
    fi
    
    # 复制 TA（如果存在）
    if ls /root/kms_passkey_src/kms/ta/target/aarch64-unknown-optee/release/*.ta 2>/dev/null; then
        cp /root/kms_passkey_src/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/kms_passkey/shared/
        echo '✅ TA binary deployed'
    else
        echo '⚠️  No TA binary (expected due to signature conflict)'
    fi
    
    # 复制测试页面
    if [ -f /root/kms_passkey_src/kms/host/kms-test-page.html ]; then
        cp /root/kms_passkey_src/kms/host/kms-test-page.html /opt/kms_passkey/shared/
        echo '✅ Test page deployed'
    fi
    
    echo ''
    echo 'Deployed files:'
    ls -lh /opt/kms_passkey/shared/
"

log_step "4/4 部署完成"
log_info "✅ Host 组件已部署到 /opt/kms_passkey/shared"
log_warn "⚠️  TA 编译被阻塞，需要先解决 signature 依赖冲突"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 下一步:"
echo "   1. 解决 signature 版本冲突"
echo "   2. 启动 QEMU: ./scripts/kms-passkey-qemu.sh start"
echo "   3. 测试 API: curl http://localhost:3001/health"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
