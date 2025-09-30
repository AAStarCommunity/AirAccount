#!/bin/bash
# 发布KMS API到 kms.aastar.io
# 完整流程：重启QEMU + 启动KMS服务器 + 启动cloudflared隧道

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

echo "🚀 发布KMS API到 https://kms.aastar.io"
echo "========================================"

log_step "1/4 重启QEMU（添加端口转发）"
./scripts/start-qemu-with-kms-port.sh

log_step "2/4 等待QEMU完全启动..."
sleep 10

log_step "3/4 在QEMU中启动KMS API服务器"
log_info "发送命令到QEMU Guest VM..."

# 通过串口发送命令到QEMU
docker exec teaclave_dev_env bash -l -c "
    echo 'mount -t 9p -o trans=virtio host /root/shared' | nc localhost 54320 &
    sleep 2
    echo 'cp /root/shared/*.ta /lib/optee_armtz/' | nc localhost 54320 &
    sleep 2
    echo 'cd /root/shared' | nc localhost 54320 &
    sleep 1
    echo 'killall kms-api-server 2>/dev/null || true' | nc localhost 54320 &
    sleep 1
    echo './kms-api-server > /tmp/kms-api-server.log 2>&1 &' | nc localhost 54320 &
    sleep 3
"

log_info "KMS API服务器启动命令已发送"

log_step "4/4 启动cloudflared隧道"
log_info "隧道配置: kms.aastar.io -> localhost:3000"

# 检查cloudflared配置
if [ ! -f ~/.cloudflared/config.yml ]; then
    log_warn "cloudflared配置不存在！"
    echo "请先配置cloudflared隧道"
    exit 1
fi

log_info "启动cloudflared..."
echo ""
echo "========================================"
echo "🎉 KMS API发布完成！"
echo ""
echo "📡 访问地址: https://kms.aastar.io"
echo "🔧 本地地址: http://localhost:3000"
echo ""
echo "测试命令:"
echo "  curl https://kms.aastar.io/health"
echo ""
echo "现在启动cloudflared隧道 (Ctrl+C停止):"
echo "========================================"
echo ""

cloudflared tunnel run kms-tunnel