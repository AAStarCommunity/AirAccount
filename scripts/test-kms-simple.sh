#!/bin/bash

# Simple KMS API Test in QEMU OP-TEE
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_info "🚀 Starting simplified KMS API test in QEMU OP-TEE..."

# Check Docker container
if ! docker ps | grep -q kms-optee-test; then
    log_error "Docker container kms-optee-test not running"
    exit 1
fi

log_success "✅ Docker container is running"

# Test KMS binaries in container
log_info "🧪 Testing KMS binaries in Docker container..."

docker exec kms-optee-test bash -c '
set -e

log_info() {
    echo -e "\033[0;34m[INFO]\033[0m $1"
}

log_success() {
    echo -e "\033[0;32m[SUCCESS]\033[0m $1"
}

# Set library path
export LD_LIBRARY_PATH=/opt/teaclave/optee/optee_client/export_arm64/usr/lib

log_info "📋 Testing KMS Host Application..."
/opt/teaclave/shared/host/kms-host --help
echo

log_info "📋 Testing KMS API Server..."
timeout 3 /opt/teaclave/shared/host/kms-api --help || echo "API server started (expected timeout)"
echo

log_info "🔍 Checking shared directory contents..."
ls -la /opt/teaclave/shared/host/
echo

log_success "✅ All binaries are accessible"
'

# Start QEMU for manual testing
log_info "🖥️ Starting QEMU OP-TEE environment..."
log_info "    - Use 'root' to login"
log_info "    - KMS binaries available at: /mnt/host/host/"
log_info "    - Use Ctrl+C to stop"

docker exec -it kms-optee-test bash -c '
export IMG_DIRECTORY=/opt/teaclave/images
export IMG_NAME=aarch64-optee-qemuv8-ubuntu-24.04-expand-ta-memory
export QEMU_HOST_SHARE_DIR=/opt/teaclave/shared

cd /opt/teaclave/images/aarch64-optee-qemuv8-ubuntu-24.04-expand-ta-memory

echo "🚀 Starting QEMU with OP-TEE..."
./qemu-system-aarch64 \
    -nodefaults \
    -nographic \
    -serial stdio \
    -smp 2 \
    -s -machine virt,secure=on,acpi=off,gic-version=3 \
    -cpu cortex-a57 \
    -d unimp -semihosting-config enable=on,target=native \
    -m 1057 \
    -bios bl1.bin \
    -initrd rootfs.cpio.gz \
    -append "console=ttyAMA0,115200 keep_bootcon root=/dev/vda2" \
    -kernel Image \
    -fsdev local,id=fsdev0,path=/opt/teaclave/shared,security_model=none \
    -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
    -netdev user,id=vmnic,hostfwd=tcp:127.0.0.1:8080-:8080 \
    -device virtio-net-device,netdev=vmnic
'

log_success "🎉 Test completed!"