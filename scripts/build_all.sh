#!/bin/bash
# 完整构建脚本

set -e

# 加载环境配置
source "$(dirname "$0")/setup_optee_env.sh"

echo ""
echo "🚀 开始 AirAccount 完整构建..."
echo "======================================"

# 记录开始时间
START_TIME=$(date +%s)

# 1. 构建 Mock 版本
echo "1️⃣ 构建 Mock 版本..."
if [ -d "$PROJECT_ROOT/packages/mock-hello" ]; then
    cd "$PROJECT_ROOT/packages/mock-hello"
    echo "   构建 Mock Hello..."
    cargo build --release
    echo "   ✅ Mock Hello 构建完成"
    
    echo "   运行快速测试..."
    timeout 30 cargo run --bin mock-ca test || {
        echo "   ❌ Mock 测试失败"
        exit 1
    }
    echo "   ✅ Mock 测试通过"
else
    echo "   ⚠️  Mock Hello 项目不存在，跳过"
fi

# 2. 构建核心逻辑
echo ""
echo "2️⃣ 构建核心逻辑..."
if [ -d "$PROJECT_ROOT/packages/core-logic" ]; then
    cd "$PROJECT_ROOT/packages/core-logic"
    echo "   构建核心逻辑库..."
    cargo build --release
    echo "   ✅ 核心逻辑构建完成"
else
    echo "   ⚠️  核心逻辑项目不存在，跳过"
fi

# 3. 构建 OP-TEE 客户端应用
echo ""
echo "3️⃣ 构建 OP-TEE 客户端应用..."

# Hello World 客户端
if [ -d "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host" ]; then
    echo "   构建 Hello World 客户端..."
    cd "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host"
    timeout 120 cargo build --target aarch64-unknown-linux-gnu --release
    echo "   ✅ Hello World 客户端构建完成"
else
    echo "   ⚠️  Hello World 示例不存在，跳过"
fi

# eth_wallet 客户端
if [ -d "$TEACLAVE_SDK_DIR/projects/web3/eth_wallet/host" ]; then
    echo "   构建 eth_wallet 客户端..."
    cd "$TEACLAVE_SDK_DIR/projects/web3/eth_wallet/host"
    timeout 120 cargo build --target aarch64-unknown-linux-gnu --release
    echo "   ✅ eth_wallet 客户端构建完成"
else
    echo "   ⚠️  eth_wallet 示例不存在，跳过"
fi

# 4. 尝试构建 TA (可能失败)
echo ""
echo "4️⃣ 尝试构建 Trusted Applications..."

if [ -d "$TEACLAVE_SDK_DIR/examples/hello_world-rs/ta" ]; then
    echo "   尝试构建 Hello World TA..."
    cd "$TEACLAVE_SDK_DIR/examples/hello_world-rs/ta"
    
    # 设置 Rust 依赖
    if [ ! -d "$TEACLAVE_SDK_DIR/rust/libc" ]; then
        echo "   设置 Rust 依赖符号链接..."
        mkdir -p "$TEACLAVE_SDK_DIR/rust"
        LIBC_PATH=$(find ~/.cargo/registry/src/ -name "libc-0.2.*" -type d | head -1)
        if [ -n "$LIBC_PATH" ]; then
            ln -sf "$LIBC_PATH" "$TEACLAVE_SDK_DIR/rust/libc"
            echo "   ✅ libc 依赖链接完成"
        fi
    fi
    
    # 尝试构建 TA
    echo "   构建 TA (使用 build-std)..."
    if TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR" timeout 300 cargo +nightly-2024-05-15 build \
        --target "$TEACLAVE_SDK_DIR/aarch64-unknown-optee.json" \
        -Z build-std=core,alloc,std --release 2>/dev/null; then
        echo "   ✅ Hello World TA 构建成功！"
    else
        echo "   ⚠️  TA 构建失败 - 这是已知问题 (optee-utee-sys std 依赖)"
        echo "      客户端构建已完成，可以继续开发"
    fi
else
    echo "   ⚠️  Hello World TA 不存在，跳过"
fi

# 5. 工作区构建
echo ""
echo "5️⃣ 构建整个工作区..."
cd "$PROJECT_ROOT"
if [ -f "Cargo.toml" ]; then
    echo "   构建工作区..."
    cargo build --workspace --release
    echo "   ✅ 工作区构建完成"
fi

# 计算构建时间
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo "======================================"
echo "🎉 构建完成！"
echo "⏱️  总用时: ${DURATION} 秒"
echo ""
echo "📦 构建产物:"

# 列出主要构建产物
if [ -d "$PROJECT_ROOT/packages/mock-hello/target/release" ]; then
    echo "   Mock Hello CA: packages/mock-hello/target/release/mock-ca"
fi

if [ -f "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host/target/aarch64-unknown-linux-gnu/release/hello_world-rs" ]; then
    echo "   Hello World CA: third_party/.../hello_world-rs/host/target/aarch64-unknown-linux-gnu/release/hello_world-rs"
fi

if [ -f "$TEACLAVE_SDK_DIR/projects/web3/eth_wallet/host/target/aarch64-unknown-linux-gnu/release/eth_wallet-rs" ]; then
    echo "   eth_wallet CA: third_party/.../eth_wallet/host/target/aarch64-unknown-linux-gnu/release/eth_wallet-rs"
fi

echo ""
echo "💡 下一步:"
echo "   - 运行 ./scripts/test_all.sh 执行完整测试"
echo "   - 开始 AirAccount TEE 应用开发"
echo "   - 使用 Mock 版本进行快速原型开发"