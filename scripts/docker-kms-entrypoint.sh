#!/bin/bash

# KMS Docker Entrypoint Script
# Sets up OP-TEE environment and provides KMS development tools

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[KMS-OPTEE]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Environment setup
export TEACLAVE_DIR=/opt/teaclave
export KMS_DIR=/opt/kms
export OPTEE_DIR=${TEACLAVE_DIR}/optee
export TA_DEV_KIT_DIR=${OPTEE_DIR}/optee_os/out/arm-plat-vexpress/export-ta_arm64
export OPTEE_CLIENT_EXPORT=${OPTEE_DIR}/optee_client/export_arm64
export CROSS_COMPILE=aarch64-linux-gnu-
export STD=y

# Add cargo to PATH
export PATH="/root/.cargo/bin:${PATH}"

show_banner() {
    echo -e "${BLUE}"
    cat << 'EOF'
╭─────────────────────────────────────────────╮
│           KMS OP-TEE Development            │
│          Trusted Execution Environment     │
├─────────────────────────────────────────────┤
│ • OP-TEE OS 4.7.0 (QEMU vexpress)         │
│ • Teaclave TrustZone SDK                   │
│ • KMS Trusted Application                  │
│ • ARM64 Cross-compilation                  │
╰─────────────────────────────────────────────╯
EOF
    echo -e "${NC}"
}

check_environment() {
    log_info "Checking OP-TEE environment..."

    # Check TA dev kit
    if [ -d "${TA_DEV_KIT_DIR}" ]; then
        log_success "TA dev kit available: ${TA_DEV_KIT_DIR}"
    else
        log_error "TA dev kit not found: ${TA_DEV_KIT_DIR}"
        return 1
    fi

    # Check client export
    if [ -d "${OPTEE_CLIENT_EXPORT}" ]; then
        log_success "Client export available: ${OPTEE_CLIENT_EXPORT}"
    else
        log_error "Client export not found: ${OPTEE_CLIENT_EXPORT}"
        return 1
    fi

    # Check cross compiler
    if command -v ${CROSS_COMPILE}gcc &> /dev/null; then
        log_success "Cross compiler available: ${CROSS_COMPILE}gcc"
    else
        log_error "Cross compiler not found: ${CROSS_COMPILE}gcc"
        return 1
    fi

    # Check Rust targets
    if rustup target list --installed | grep -q "aarch64-unknown-linux-gnu"; then
        log_success "Rust target available: aarch64-unknown-linux-gnu"
    else
        log_warn "Installing Rust target: aarch64-unknown-linux-gnu"
        rustup target add aarch64-unknown-linux-gnu
    fi

    if rustup target list --installed | grep -q "aarch64-unknown-optee"; then
        log_success "Rust target available: aarch64-unknown-optee"
    else
        log_warn "Installing Rust target: aarch64-unknown-optee"
        rustup target add aarch64-unknown-optee
    fi
}

build_kms_ta() {
    log_info "Building KMS Trusted Application..."

    # Create KMS TA based on eth_wallet structure
    if [ ! -d "${KMS_DIR}/kms-ta-optee" ]; then
        log_info "Creating KMS TA from eth_wallet template..."
        cp -r ${TEACLAVE_DIR}/projects/web3/eth_wallet ${KMS_DIR}/kms-ta-optee

        # Update UUID for KMS
        echo "bc50d971-d4c9-42c4-82cb-343fb7f37896" > ${KMS_DIR}/kms-ta-optee/uuid.txt
        log_success "KMS TA template created"
    fi

    cd ${KMS_DIR}/kms-ta-optee

    # Build host application
    log_info "Building KMS host application..."
    cd host
    cargo build --target aarch64-unknown-linux-gnu
    if [ $? -eq 0 ]; then
        log_success "KMS host application built successfully"
    else
        log_error "Failed to build KMS host application"
        return 1
    fi

    # Build TA
    log_info "Building KMS Trusted Application..."
    cd ../ta
    cargo build --target aarch64-unknown-optee
    if [ $? -eq 0 ]; then
        log_success "KMS Trusted Application built successfully"
    else
        log_error "Failed to build KMS Trusted Application"
        return 1
    fi

    cd ${KMS_DIR}
}

run_qemu_optee() {
    log_info "Preparing QEMU OP-TEE environment..."

    local QEMU_DIR=${OPTEE_DIR}/qemu_env
    mkdir -p ${QEMU_DIR}

    # Copy OP-TEE binaries
    cp ${OPTEE_DIR}/optee_os/out/arm-plat-vexpress/core/tee.bin ${QEMU_DIR}/

    # Copy KMS TA
    if [ -f "${KMS_DIR}/kms-ta-optee/ta/target/aarch64-unknown-optee/debug/ta" ]; then
        cp ${KMS_DIR}/kms-ta-optee/ta/target/aarch64-unknown-optee/debug/ta ${QEMU_DIR}/bc50d971-d4c9-42c4-82cb-343fb7f37896.ta
        log_success "KMS TA copied to QEMU environment"
    fi

    log_info "QEMU OP-TEE environment ready"
    log_info "TA UUID: bc50d971-d4c9-42c4-82cb-343fb7f37896"
    log_info "Host binary: ${KMS_DIR}/kms-ta-optee/host/target/aarch64-unknown-linux-gnu/debug/eth_wallet-rs"
}

show_help() {
    cat << EOF
KMS OP-TEE Development Environment

Commands:
  check          Check environment setup
  build          Build KMS TA and host application
  qemu           Prepare QEMU OP-TEE environment
  test           Run KMS TA tests
  shell          Start interactive bash shell
  help           Show this help message

Environment Variables:
  TA_DEV_KIT_DIR=${TA_DEV_KIT_DIR}
  OPTEE_CLIENT_EXPORT=${OPTEE_CLIENT_EXPORT}
  CROSS_COMPILE=${CROSS_COMPILE}

Example Usage:
  docker run -it kms-optee:latest check
  docker run -it kms-optee:latest build
  docker run -it kms-optee:latest qemu
EOF
}

# Main script logic
case "${1:-shell}" in
    "check")
        show_banner
        check_environment
        ;;
    "build")
        show_banner
        check_environment && build_kms_ta
        ;;
    "qemu")
        show_banner
        check_environment && run_qemu_optee
        ;;
    "test")
        show_banner
        check_environment && build_kms_ta && run_qemu_optee
        log_info "Running KMS TA tests..."
        # Add test commands here
        ;;
    "shell"|"bash")
        show_banner
        check_environment
        log_info "Starting interactive shell..."
        exec bash
        ;;
    "help"|"--help"|"-h")
        show_help
        ;;
    *)
        show_banner
        log_info "Unknown command: $1"
        show_help
        exit 1
        ;;
esac