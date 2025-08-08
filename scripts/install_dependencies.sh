#!/bin/bash
# OP-TEE 开发环境依赖安装脚本
# 适用于 macOS 系统

set -e

echo "🔧 安装 OP-TEE 开发环境依赖..."
echo "======================================"

# 检查操作系统
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "❌ 此脚本仅适用于 macOS 系统"
    exit 1
fi

# 检查 Homebrew
if ! command -v brew > /dev/null 2>&1; then
    echo "❌ 未检测到 Homebrew，请先安装 Homebrew"
    echo "   访问: https://brew.sh/"
    exit 1
fi

echo "✅ Homebrew 已安装"

# 安装 Xcode Command Line Tools
echo ""
echo "1️⃣ 检查 Xcode Command Line Tools..."
if ! xcode-select -p > /dev/null 2>&1; then
    echo "安装 Xcode Command Line Tools..."
    xcode-select --install
    echo "✅ 请完成 Xcode Command Line Tools 安装后重新运行此脚本"
    exit 0
else
    echo "✅ Xcode Command Line Tools 已安装"
fi

# 安装基础开发工具
echo ""
echo "2️⃣ 安装基础开发工具..."
echo "更新 Homebrew..."
brew update

echo "安装必需的软件包..."
PACKAGES=(
    automake
    coreutils
    curl
    gmp
    gnutls
    libtool
    libusb
    make
    wget
    git
)

for package in "${PACKAGES[@]}"; do
    if brew list "$package" > /dev/null 2>&1; then
        echo "✅ $package 已安装"
    else
        echo "安装 $package..."
        brew install "$package"
        echo "✅ $package 安装完成"
    fi
done

# 安装交叉编译工具链
echo ""
echo "3️⃣ 安装交叉编译工具链..."
echo "添加 messense tap..."
brew tap messense/homebrew-macos-cross-toolchains

CROSS_COMPILERS=(
    "messense/macos-cross-toolchains/aarch64-unknown-linux-gnu"
    "messense/macos-cross-toolchains/armv7-unknown-linux-gnueabihf"
)

for compiler in "${CROSS_COMPILERS[@]}"; do
    compiler_name=$(basename "$compiler")
    if brew list "$compiler_name" > /dev/null 2>&1; then
        echo "✅ $compiler_name 已安装"
    else
        echo "安装 $compiler_name..."
        brew install "$compiler"
        echo "✅ $compiler_name 安装完成"
    fi
done

# 验证交叉编译器安装
echo "验证交叉编译器..."
if command -v aarch64-unknown-linux-gnu-gcc > /dev/null 2>&1; then
    echo "✅ ARM64 交叉编译器: $(which aarch64-unknown-linux-gnu-gcc)"
else
    echo "❌ ARM64 交叉编译器安装失败"
    exit 1
fi

if command -v armv7-unknown-linux-gnueabihf-gcc > /dev/null 2>&1; then
    echo "✅ ARM32 交叉编译器: $(which armv7-unknown-linux-gnueabihf-gcc)"
else
    echo "❌ ARM32 交叉编译器安装失败"
    exit 1
fi

# 安装 Python 依赖
echo ""
echo "4️⃣ 安装 Python 依赖..."
if command -v pip3 > /dev/null 2>&1; then
    echo "安装 pyelftools..."
    pip3 install pyelftools
    
    # 验证安装
    if python3 -c "import elftools" 2>/dev/null; then
        echo "✅ pyelftools 安装成功"
    else
        echo "❌ pyelftools 安装失败"
        exit 1
    fi
else
    echo "❌ pip3 未找到，请安装 Python 3"
    exit 1
fi

# 安装 Rust 工具
echo ""
echo "5️⃣ 安装 Rust 工具..."

# 检查 Rust
if ! command -v rustup > /dev/null 2>&1; then
    echo "❌ Rust 未安装，请先安装 Rust"
    echo "   访问: https://rustup.rs/"
    exit 1
fi

echo "✅ Rust 已安装"

# 安装 xargo
if ! command -v xargo > /dev/null 2>&1; then
    echo "安装 xargo..."
    cargo install xargo
    echo "✅ xargo 安装完成"
else
    echo "✅ xargo 已安装"
fi

# 安装 Rust 源码组件
echo "添加 Rust 源码组件..."
rustup component add rust-src --toolchain nightly-2024-05-15-aarch64-apple-darwin || {
    echo "安装指定工具链..."
    rustup toolchain install nightly-2024-05-15-aarch64-apple-darwin
    rustup component add rust-src --toolchain nightly-2024-05-15-aarch64-apple-darwin
}
echo "✅ Rust 源码组件已添加"

# 安装可选的代码质量工具
echo ""
echo "6️⃣ 安装代码质量工具..."

# Clippy
if ! rustup component list | grep -q "clippy.*installed"; then
    echo "安装 clippy..."
    rustup component add clippy
    echo "✅ clippy 安装完成"
else
    echo "✅ clippy 已安装"
fi

# rustfmt
if ! rustup component list | grep -q "rustfmt.*installed"; then
    echo "安装 rustfmt..."
    rustup component add rustfmt
    echo "✅ rustfmt 安装完成"
else
    echo "✅ rustfmt 已安装"
fi

echo ""
echo "======================================"
echo "🎉 所有依赖安装完成！"
echo ""
echo "📋 安装总结:"
echo "✅ Xcode Command Line Tools"
echo "✅ Homebrew 基础包 (automake, coreutils, curl, gmp, gnutls, libtool, libusb, make, wget)"
echo "✅ ARM64/ARM32 交叉编译器"
echo "✅ Python elftools 模块"
echo "✅ Rust 工具链 (xargo, rust-src, clippy, rustfmt)"
echo ""
echo "🚀 下一步:"
echo "1. 克隆项目仓库: git clone <your-repo>"
echo "2. 初始化子模块: git submodule update --init --recursive"
echo "3. 运行环境验证: ./scripts/verify_optee_setup.sh"
echo "4. 构建项目: ./scripts/build_all.sh"
echo ""
echo "💡 如需帮助，请参考: docs/OP-TEE-Development-Setup.md"