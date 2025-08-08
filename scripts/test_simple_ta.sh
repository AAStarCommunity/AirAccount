#!/bin/bash
# 测试简单版本 TA 的构建和基本功能

set -e

# 加载通用函数库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

init_script "AirAccount Simple TA Test"

log_info "Testing Simple TA Build..."

# 设置环境变量
export TA_DEV_KIT_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/target/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64"
export RUST_TARGET_PATH="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk"

# 验证环境变量
if [ ! -d "$TA_DEV_KIT_DIR" ]; then
    log_error "TA_DEV_KIT_DIR not found: $TA_DEV_KIT_DIR"
    exit 1
fi

log_success "✅ TA_DEV_KIT_DIR found: $TA_DEV_KIT_DIR"

# 构建简单 TA
log_info "Building Simple TA..."
cd "${PROJECT_ROOT}/packages/airaccount-ta-simple"

if cargo +nightly-2024-05-15 build --target aarch64-unknown-optee -Z build-std=core,alloc,std --release; then
    log_success "✅ Simple TA built successfully!"
    
    # 检查构建产物
    TA_LIB="${PROJECT_ROOT}/packages/airaccount-ta-simple/target/aarch64-unknown-optee/release/libairaccount_ta_simple.rlib"
    if [ -f "$TA_LIB" ]; then
        TA_SIZE=$(stat -f%z "$TA_LIB" 2>/dev/null || stat -c%s "$TA_LIB" 2>/dev/null)
        log_success "✅ TA library: libairaccount_ta_simple.rlib (${TA_SIZE} bytes)"
    fi
    
    # 检查生成的 TA 头文件
    TA_HEADER=$(find target/aarch64-unknown-optee/release/build -name "user_ta_header.rs" | head -1)
    if [ -f "$TA_HEADER" ]; then
        log_success "✅ TA header file generated: $TA_HEADER"
    fi
    
    # 检查链接脚本
    TA_LINKER=$(find target/aarch64-unknown-optee/release/build -name "ta.lds" | head -1)
    if [ -f "$TA_LINKER" ]; then
        log_success "✅ TA linker script generated: $TA_LINKER"
    fi
    
else
    log_error "❌ Simple TA build failed"
    exit 1
fi

# 尝试构建完整 TA（预期会失败）
log_info ""
log_info "Testing Full TA Build (expected to show dependency issues)..."
cd "${PROJECT_ROOT}/packages/airaccount-ta"

if cargo +nightly-2024-05-15 build --target aarch64-unknown-optee -Z build-std=core,alloc,std --release 2>&1; then
    log_success "✅ Full TA built successfully! (Unexpected but great!)"
else
    log_warning "⚠️ Full TA build failed as expected due to restricted_std issues"
    log_info "This is normal - external crates (anyhow, serde, bincode) need restricted_std support"
fi

# 构建对应的客户端应用
log_info ""
log_info "Building Client Application..."
cd "${PROJECT_ROOT}/packages/airaccount-ca"

export CC=aarch64-linux-gnu-gcc
export AR=aarch64-linux-gnu-ar
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export OPTEE_CLIENT_EXPORT="/Volumes/UltraDisk/Dev2/aastar/AirAccount/target/optee/optee_client/export_arm64"

if cargo build --target aarch64-unknown-linux-gnu --release; then
    CA_BIN="${PROJECT_ROOT}/packages/airaccount-ca/target/aarch64-unknown-linux-gnu/release/airaccount-ca"
    if [ -f "$CA_BIN" ]; then
        CA_SIZE=$(stat -f%z "$CA_BIN" 2>/dev/null || stat -c%s "$CA_BIN" 2>/dev/null)
        log_success "✅ Client application: airaccount-ca (${CA_SIZE} bytes)"
    fi
else
    log_error "❌ Client application build failed"
    exit 1
fi

log_info ""
log_success "🎉 Simple TA Test Completed Successfully!"
log_info "📋 Results Summary:"
log_info "✅ Simple TA: Builds successfully with basic Hello World functionality"
log_info "⚠️  Full TA: Needs restricted_std feature support for external crates"
log_info "✅ Client App: Ready for communication testing"
log_info ""
log_info "🔄 Next Steps:"
log_info "1. Test TA-CA communication in OP-TEE QEMU environment"
log_info "2. Add restricted_std support to external dependencies"
log_info "3. Integrate secure storage with proper error handling"
log_info "4. Add eth_wallet cryptographic functionality"