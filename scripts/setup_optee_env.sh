#!/bin/bash
# OP-TEE 环境变量配置脚本
# 用法: source scripts/setup_optee_env.sh

set -e

# 获取项目根目录
export PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# OP-TEE 相关路径
export OPTEE_DIR="${PROJECT_ROOT}/target/optee"
export TA_DEV_KIT_DIR="${OPTEE_DIR}/optee_os/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="${OPTEE_DIR}/optee_client/export_arm64"

# 交叉编译器配置
export CROSS_COMPILE32="armv7-unknown-linux-gnueabihf-"
export CROSS_COMPILE64="aarch64-unknown-linux-gnu-"
export CROSS_COMPILE_TA="aarch64-unknown-linux-gnu-"
export CROSS_COMPILE_HOST="aarch64-unknown-linux-gnu-"

# Rust 目标配置
export TARGET_TA="aarch64-unknown-optee"
export TARGET_HOST="aarch64-unknown-linux-gnu"
export STD="y"

# Teaclave SDK 路径
export TEACLAVE_SDK_DIR="${PROJECT_ROOT}/third_party/incubator-teaclave-trustzone-sdk"

# 显示配置信息
echo "🔧 OP-TEE 开发环境配置"
echo "======================================"
echo "PROJECT_ROOT: $PROJECT_ROOT"
echo "OPTEE_DIR: $OPTEE_DIR"
echo "TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
echo "OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"
echo "CROSS_COMPILE64: $CROSS_COMPILE64"
echo "CROSS_COMPILE32: $CROSS_COMPILE32"
echo ""

# 基本健康检查
echo "🔍 环境检查..."

# 检查交叉编译器
if command -v aarch64-unknown-linux-gnu-gcc >/dev/null 2>&1; then
    echo "✅ ARM64 交叉编译器: $(which aarch64-unknown-linux-gnu-gcc)"
else
    echo "❌ ARM64 交叉编译器未找到"
    echo "请运行: brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu"
fi

if command -v armv7-unknown-linux-gnueabihf-gcc >/dev/null 2>&1; then
    echo "✅ ARM32 交叉编译器: $(which armv7-unknown-linux-gnueabihf-gcc)"
else
    echo "❌ ARM32 交叉编译器未找到"
    echo "请运行: brew install messense/macos-cross-toolchains/armv7-unknown-linux-gnueabihf"
fi

# 检查 Rust 工具
if command -v xargo >/dev/null 2>&1; then
    echo "✅ xargo: $(which xargo)"
else
    echo "❌ xargo 未找到，请运行: cargo install xargo"
fi

# 检查 Python 依赖
if python3 -c "import elftools" 2>/dev/null; then
    echo "✅ pyelftools 已安装"
else
    echo "❌ pyelftools 未找到，请运行: pip3 install pyelftools"
fi

# 检查 Teaclave SDK
if [ -d "$TEACLAVE_SDK_DIR" ]; then
    echo "✅ Teaclave SDK: $TEACLAVE_SDK_DIR"
else
    echo "❌ Teaclave SDK 未找到"
    echo "请运行: git submodule update --init --recursive third_party/incubator-teaclave-trustzone-sdk"
fi

echo ""
echo "✅ 环境配置完成！"
echo "💡 提示: 现在可以运行 ./scripts/verify_optee_setup.sh 进行完整验证"