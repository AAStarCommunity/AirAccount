#!/bin/bash
# 完整发布KMS API到https://kms.aastar.io
# 步骤：重启Docker（端口映射）→ QEMU → KMS → cloudflared

set -e

RED='\033[0;31m'
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

echo "🚀 完整发布KMS API到 https://kms.aastar.io"
echo "========================================="

log_step "1/5 重启Docker容器（端口映射3000）"
docker stop teaclave_dev_env 2>/dev/null || true
docker rm teaclave_dev_env 2>/dev/null || true
./scripts/kms-dev-env.sh start
./scripts/kms-dev-env.sh sync

log_step "2/5 启动QEMU（端口转发）"
docker exec -d teaclave_dev_env bash -l -c "cd /opt/teaclave/images/x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory && ./qemu-system-aarch64 -nodefaults -nographic -serial tcp:localhost:54320,server,nowait -serial tcp:localhost:54321,server,nowait -smp 2 -s -machine virt,secure=on,acpi=off,gic-version=3 -cpu cortex-a57 -d unimp -semihosting-config enable=on,target=native -m 1057 -bios bl1.bin -initrd rootfs.cpio.gz -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' -kernel Image -fsdev local,id=fsdev0,path=/opt/teaclave/shared,security_model=none -device virtio-9p-device,fsdev=fsdev0,mount_tag=host -netdev user,id=vmnic,hostfwd=:127.0.0.1:54433-:4433,hostfwd=tcp:127.0.0.1:3000-:3000 -device virtio-net-device,netdev=vmnic > /tmp/qemu.log 2>&1"

log_info "等待QEMU启动..."
sleep 10

log_step "3/5 在QEMU中启动KMS API服务器"
docker exec teaclave_dev_env bash -l -c "
(
echo 'root'
sleep 2
echo ''
sleep 2
echo 'mkdir -p /root/shared'
sleep 1
echo 'mount -t 9p -o trans=virtio host /root/shared'
sleep 2
echo 'cp /root/shared/*.ta /lib/optee_armtz/'
sleep 1
echo 'cd /root/shared'
sleep 1
echo './kms-api-server > /tmp/kms.log 2>&1 &'
sleep 3
) | socat - TCP:localhost:54320
"

log_step "4/5 验证KMS API（Docker内部）"
docker exec teaclave_dev_env bash -l -c "curl -s http://127.0.0.1:3000/health" | jq .

log_step "5/5 启动cloudflared隧道"
log_info "隧道: https://kms.aastar.io -> Docker:3000 -> QEMU Guest:3000"
echo ""
echo "========================================"
echo "🎉 KMS API发布完成！"
echo ""
echo "📡 公网地址: https://kms.aastar.io"
echo "🔧 Docker内: http://127.0.0.1:3000（仅Docker内可访问）"
echo ""
echo "现在启动cloudflared隧道 (Ctrl+C停止):"
echo "========================================"
echo ""

cloudflared tunnel run kms-tunnel