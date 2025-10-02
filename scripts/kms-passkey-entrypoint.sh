#!/bin/bash
# Entrypoint for KMS Passkey Docker Container
# Sets up STD mode environment for Teaclave TrustZone SDK

set -e

echo "🔧 KMS Passkey Docker Environment Initialization..."

# Source Cargo environment
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Source profile for RUST_STD_DIR
if [ -f "$HOME/.profile" ]; then
    source "$HOME/.profile"
fi

# Verify environment variables
echo "📋 Environment:"
echo "   TEACLAVE_TOOLCHAIN_BASE: ${TEACLAVE_TOOLCHAIN_BASE}"
echo "   RUST_STD_DIR: ${RUST_STD_DIR}"
echo "   KMS_BRANCH: ${KMS_BRANCH}"

# Create symbolic links for SDK dependencies in project
if [ -d "/root/teaclave_sdk_src" ]; then
    echo "📦 Creating SDK dependency symlinks..."
    cd /root/kms_passkey_src

    [ ! -L rust ] && ln -sf /root/teaclave_sdk_src/rust rust
    [ ! -L optee-teec ] && ln -sf /root/teaclave_sdk_src/optee-teec optee-teec
    [ ! -L optee-utee ] && ln -sf /root/teaclave_sdk_src/optee-utee optee-utee
    [ ! -L optee-utee-sys ] && ln -sf /root/teaclave_sdk_src/optee-utee-sys optee-utee-sys
    [ ! -L optee-utee-build ] && ln -sf /root/teaclave_sdk_src/optee-utee-build optee-utee-build
    [ ! -L crates ] && ln -sf /root/teaclave_sdk_src/crates crates

    echo "✅ SDK dependencies linked"
fi

# Verify STD mode configuration
echo "🔍 Verifying STD mode configuration..."
if [ -d "${TEACLAVE_TOOLCHAIN_BASE}/config/ta" ]; then
    ACTIVE_CONFIG=$(readlink ${TEACLAVE_TOOLCHAIN_BASE}/config/ta/active 2>/dev/null || echo "not set")
    echo "   Active TA Config: ${ACTIVE_CONFIG}"
fi

# Set OP-TEE build environment variables
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
export OPTEE_OS_DIR=/opt/teaclave/optee/optee_os
export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64

# Set RUST_TARGET_PATH for TA compilation
export RUST_TARGET_PATH=/root/kms_passkey_src/kms/ta

echo "✅ KMS Passkey Environment Ready"
echo "   Mode: STD (aarch64)"
echo "   Shared Dir: /opt/kms_passkey/shared"
echo "   OP-TEE Client: $OPTEE_CLIENT_EXPORT"
echo "   RUST_TARGET_PATH: $RUST_TARGET_PATH"
echo ""

# Execute the main command
exec "$@"
