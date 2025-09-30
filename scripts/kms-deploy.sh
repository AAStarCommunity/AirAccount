#!/bin/bash
# KMS快速部署脚本 - 构建并同步到QEMU

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

# 检查容器是否运行
if ! docker ps | grep -q teaclave_dev_env; then
    echo -e "${RED}[ERROR]${NC} 容器未运行！请先执行: ./scripts/kms-dev-env.sh start"
    exit 1
fi

# 是否清理构建
CLEAN_BUILD=${1:-no}
if [ "$CLEAN_BUILD" = "clean" ]; then
    log_step "1/3 清理旧构建..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/kms && make clean"
else
    log_info "跳过清理（增量构建），如需完整构建请运行: $0 clean"
fi

# 构建KMS
log_step "2/3 构建KMS项目..."
docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/kms && make 2>&1 | grep -E 'Compiling|Finished|SIGN|error' || make"

if [ $? -ne 0 ]; then
    log_warn "构建失败，查看完整日志:"
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/kms && make"
    exit 1
fi

# 同步到QEMU共享目录
log_step "3/3 同步到QEMU共享目录..."
docker exec teaclave_dev_env bash -l -c "
    mkdir -p /opt/teaclave/shared && \
    cp /root/teaclave_sdk_src/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/teaclave/shared/kms && \
    cp /root/teaclave_sdk_src/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ && \
    ls -lh /opt/teaclave/shared/ | grep -E 'kms|\.ta$'
"

echo ""
log_info "✅ 部署完成！"
echo ""
echo -e "${BLUE}下一步：在QEMU Guest VM中运行${NC}"
echo "  1. 挂载共享目录: mount -t 9p -o trans=virtio host shared"
echo "  2. 复制TA: cp shared/*.ta /lib/optee_armtz/"
echo "  3. 运行KMS: cd shared && ./kms --help"
echo ""