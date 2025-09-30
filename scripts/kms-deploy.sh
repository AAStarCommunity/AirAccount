#!/bin/bash
# KMS快速部署脚本 - 同步源码、构建并部署到QEMU

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

log_step() {
    echo -e "\n${BLUE}==>${NC} $1"
}

# 项目路径
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KMS_DEV_DIR="$PROJECT_ROOT/kms"
KMS_SDK_DIR="$PROJECT_ROOT/third_party/teaclave-trustzone-sdk/projects/web3/kms"

# 检查容器是否运行
if ! docker ps | grep -q teaclave_dev_env; then
    echo -e "${RED}[ERROR]${NC} 容器未运行！请先执行: ./scripts/kms-dev-env.sh start"
    exit 1
fi

# Step 0: 同步开发源码到SDK
log_step "0/4 同步开发源码到SDK..."
log_info "从 kms/ 同步到 third_party/teaclave-trustzone-sdk/projects/web3/kms/"
rsync -av --delete "$KMS_DEV_DIR/" "$KMS_SDK_DIR/"
log_info "✅ 源码同步完成"

# 检查STD模式依赖
log_info "检查STD模式依赖（rust/libc）..."
if ! docker exec teaclave_dev_env bash -l -c "[ -d /root/teaclave_sdk_src/rust/rust ] && [ -d /root/teaclave_sdk_src/rust/libc ]"; then
    log_warn "rust/libc依赖不存在，开始初始化（首次运行或SDK被重置后需要）..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src && ./setup_std_dependencies.sh"
    log_info "✅ STD依赖初始化完成"
else
    log_info "✅ STD依赖已存在"
fi

# 是否清理构建
CLEAN_BUILD=${1:-no}
if [ "$CLEAN_BUILD" = "clean" ]; then
    log_step "1/4 清理旧构建..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make clean"
else
    log_info "跳过清理（增量构建），如需完整构建请运行: $0 clean"
fi

# 构建KMS
log_step "2/4 构建KMS项目（Host + TA）..."
docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make 2>&1 | grep -E 'Compiling|Finished|SIGN|error' || make"

if [ $? -ne 0 ]; then
    log_warn "构建失败，查看完整日志:"
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make"
    exit 1
fi

# 同步到QEMU共享目录
log_step "3/4 部署到QEMU共享目录..."
docker exec teaclave_dev_env bash -l -c "
    mkdir -p /opt/teaclave/shared && \
    cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/teaclave/shared/kms && \
    cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/kms-api-server && \
    cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ && \
    ls -lh /opt/teaclave/shared/ | grep -E 'kms|\.ta$'
"

log_step "4/4 验证部署..."
docker exec teaclave_dev_env bash -l -c "ls -lh /opt/teaclave/shared/ | grep -E 'kms|\.ta$'"

echo ""
log_info "✅ 部署完成！"
echo ""
echo -e "${BLUE}开发流程说明：${NC}"
echo "  📝 日常开发: 编辑 kms/ 目录下的代码"
echo "  🚀 构建部署: ./scripts/kms-deploy.sh"
echo "  🧹 完整构建: ./scripts/kms-deploy.sh clean"
echo ""
echo -e "${BLUE}QEMU中运行：${NC}"
echo "  1. 挂载共享目录: mount -t 9p -o trans=virtio host shared"
echo "  2. 部署TA: cp shared/*.ta /lib/optee_armtz/"
echo "  3a. 运行CLI: cd shared && ./kms --help"
echo "  3b. 运行API服务器: cd shared && ./kms-api-server"
echo ""