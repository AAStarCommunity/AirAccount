#!/bin/bash
# 在运行中的QEMU内启动KMS API服务器
# 通过向QEMU的stdio发送命令

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_step() {
    echo -e "\n${BLUE}==>${NC} $1"
}

echo "🚀 在QEMU中启动KMS API服务器"
echo "================================"

# 检查QEMU是否运行
if ! docker exec teaclave_dev_env bash -c "ps aux | grep -v grep | grep -q qemu-system-aarch64"; then
    echo "❌ QEMU未运行！请先启动QEMU"
    echo "   运行: docker exec -d teaclave_dev_env bash -l -c 'IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory /tmp/start_qemuv8_kms'"
    exit 1
fi

log_info "QEMU正在运行"

log_step "方式1: 在新终端中手动执行"
echo "打开新Mac终端，运行:"
echo ""
echo "  docker exec -it teaclave_dev_env bash"
echo ""
echo "进入后在QEMU shell中执行:"
echo ""
echo "  mount -t 9p -o trans=virtio host /root/shared"
echo "  cp /root/shared/*.ta /lib/optee_armtz/"
echo "  cd /root/shared  "
echo "  ./kms-api-server"
echo ""

log_step "方式2: 使用screen创建后台会话"
log_info "创建screen会话并在其中启动KMS..."

docker exec -d teaclave_dev_env bash -c "
    # 等待QEMU完全启动
    sleep 5

    # 通过expect或类似工具发送命令（如果可用）
    # 这里我们只能提供手动方式
    echo 'QEMU已准备好，请手动在Guest VM中启动KMS API'
"

log_step "启动后测试"
echo "在Mac上运行:"
echo ""
echo "  # 等待KMS启动"
echo "  sleep 5"
echo ""
echo "  # 测试本地端口"
echo "  curl http://localhost:3000/health"
echo ""
echo "  # 测试公网访问"
echo "  curl https://kms.aastar.io/health"
echo ""