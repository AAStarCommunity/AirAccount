#!/bin/bash
# Rust工具链配置脚本

set -e

echo "=== AirAccount Rust工具链配置 ==="

echo "1. 检查Rust安装状态..."

# 检查Rust是否已安装
if command -v rustc &> /dev/null && command -v cargo &> /dev/null; then
    echo "✅ Rust已安装:"
    echo "   rustc: $(rustc --version)"
    echo "   cargo: $(cargo --version)"
    echo "   rustup: $(rustup --version)"
else
    echo "⚠️  Rust未安装，正在安装..."
    
    # 安装Rust
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    
    # 重新加载环境
    source ~/.cargo/env
    
    echo "✅ Rust安装完成:"
    echo "   rustc: $(rustc --version)"
    echo "   cargo: $(cargo --version)"
fi

echo ""
echo "2. 配置TEE开发相关的目标平台..."

# 定义标准目标平台（可直接通过rustup安装）
standard_targets=(
    "aarch64-unknown-linux-gnu"
    "armv7-unknown-linux-gnueabihf"
)

# 添加标准目标平台
for target in "${standard_targets[@]}"; do
    if rustup target list --installed | grep -q "^$target$"; then
        echo "✅ $target: 已安装"
    else
        echo "📦 安装目标平台: $target"
        rustup target add "$target"
        echo "✅ $target: 安装完成"
    fi
done

# 检查TEE特定目标（需要Teaclave SDK支持）
echo ""
echo "🔍 检查TEE目标平台支持..."
if rustc --print=target-list | grep -q "aarch64-unknown-optee-trustzone"; then
    echo "✅ aarch64-unknown-optee-trustzone: 已由当前工具链支持"
    if ! rustup target list --installed | grep -q "^aarch64-unknown-optee-trustzone$"; then
        rustup target add aarch64-unknown-optee-trustzone
    fi
else
    echo "⚠️  aarch64-unknown-optee-trustzone: 需要Teaclave TrustZone SDK的自定义Rust工具链"
    echo "   此目标平台将在安装Teaclave SDK后可用"
fi

echo ""
echo "3. 安装必要的cargo工具..."

# 检查并安装cargo-make
if cargo install --list | grep -q "cargo-make"; then
    echo "✅ cargo-make: 已安装"
else
    echo "📦 安装cargo-make..."
    cargo install cargo-make
    echo "✅ cargo-make: 安装完成"
fi

echo ""
echo "4. 验证Rust配置..."

# 验证工具链
echo "已安装的工具链:"
rustup toolchain list

echo ""
echo "已安装的目标平台:"
rustup target list --installed | grep -E "(aarch64|armv7)"

echo ""
echo "Cargo工具:"
cargo --version
if command -v cargo-make &> /dev/null; then
    cargo-make --version
fi

echo ""
echo "=== Rust工具链配置完成 ==="
echo "📝 提示: 如果这是首次安装Rust，请运行 'source ~/.cargo/env' 或重新打开终端"