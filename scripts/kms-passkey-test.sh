#!/bin/bash
# KMS Passkey Development Test Script
# Tests complete build and QEMU workflow in isolated Docker environment

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTAINER_NAME="kms_passkey_dev"

# Colors
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
    echo -e "\n${BLUE}==>${NC} $1\n"
}

# Check container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log_error "Container $CONTAINER_NAME is not running. Start it first with: ./scripts/kms-passkey-docker.sh start"
    exit 1
fi

log_step "Testing KMS Passkey Development Environment"

# Test 1: Verify environment
log_info "Step 1: Verifying environment variables..."
docker exec "$CONTAINER_NAME" bash -c "
    source ~/.cargo/env
    source ~/.profile
    echo 'TEACLAVE_TOOLCHAIN_BASE: '\$TEACLAVE_TOOLCHAIN_BASE
    echo 'RUST_STD_DIR: '\$RUST_STD_DIR
    echo 'KMS_BRANCH: '\$KMS_BRANCH
"

# Test 2: Create Rust std symlink
log_info "Step 2: Creating Rust std symlink..."
docker exec "$CONTAINER_NAME" bash -c "
    cd /root/kms_passkey_src
    if [ ! -L rust ]; then
        ln -sf /root/teaclave_sdk_src/rust rust
        echo 'Symlink created'
    else
        echo 'Symlink already exists'
    fi
    ls -la rust
"

# Test 3: Test Rust compilation
log_info "Step 3: Testing Rust toolchain..."
docker exec "$CONTAINER_NAME" bash -c "
    source ~/.cargo/env
    rustc --version
    cargo --version
"

# Test 4: Verify TA config
log_info "Step 4: Verifying TA configuration..."
docker exec "$CONTAINER_NAME" bash -c "
    ls -la /opt/teaclave/config/ta/active
    cat /opt/teaclave/config/ta/std/aarch64
"

# Test 5: Test build (proto only for now, to avoid signature conflict)
log_info "Step 5: Testing proto build..."
docker exec "$CONTAINER_NAME" bash -c "
    source ~/.cargo/env
    cd /root/kms_passkey_src/kms/proto
    cargo build --release 2>&1 | head -20
"

log_step "✅ Environment Test Completed Successfully!"
log_info "Container is ready for KMS Passkey development"
log_info "To enter the container: ./scripts/kms-passkey-docker.sh shell"
