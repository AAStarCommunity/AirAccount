#!/bin/bash
# 启动QEMU并添加KMS API端口转发 (3000)
# 这个脚本会停止当前QEMU，然后用新配置重启

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

log_step "准备重启QEMU（添加KMS API端口转发）"

# 检查Docker容器是否运行
if ! docker ps | grep -q teaclave_dev_env; then
    log_error "Docker容器未运行！请先执行: ./scripts/kms-dev-env.sh start"
    exit 1
fi

log_info "停止当前QEMU进程..."
docker exec teaclave_dev_env bash -l -c "pkill -f qemu-system-aarch64 || true"
sleep 2

log_info "使用挂载的SDK脚本启动QEMU（已配置3000端口转发）..."
docker exec -d teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src && LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"

log_info "等待QEMU启动..."
sleep 5

log_step "QEMU已启动，端口转发配置："
echo "  📡 Guest :4433  -> Host 127.0.0.1:54433 (HTTPS)"
echo "  🔑 Guest :3000  -> Host 127.0.0.1:3000 (KMS API)"
echo "  🖥️  Serial :54320 (Guest VM Shell)"
echo "  🖥️  Serial :54321 (Secure Console)"

log_step "下一步："
echo "  1. 启动Guest VM Shell: ./scripts/terminal2-guest-vm.sh"
echo "  2. 在Guest VM中启动KMS API服务器"
echo "  3. 在Mac上启动cloudflared隧道"
echo "  4. 访问 https://kms.aastar.io"

log_info "✅ QEMU重启完成！"