#!/bin/bash

# AirAccount OP-TEE环境设置脚本
# 创建时间: 2025-08-17 11:30:00 +07

echo "🔧 设置AirAccount OP-TEE环境变量"
echo "======================================"

# 设置基础路径
BASE_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk"

# 设置环境变量
export OPTEE_DIR="$BASE_DIR/optee"
export OPTEE_OS_DIR="$OPTEE_DIR/optee_os"
export OPTEE_CLIENT_DIR="$OPTEE_DIR/optee_client"
export TA_DEV_KIT_DIR="$OPTEE_OS_DIR/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="$OPTEE_CLIENT_DIR/export_arm64"

# 设置交叉编译工具链
export CROSS_COMPILE64="aarch64-linux-gnu-"
export TARGET_TA="aarch64-unknown-optee"
export TARGET_HOST="aarch64-unknown-linux-gnu"
export CROSS_COMPILE_TA="$CROSS_COMPILE64"
export CROSS_COMPILE_HOST="$CROSS_COMPILE64"

echo "✅ 环境变量已设置:"
echo "   OPTEE_DIR: $OPTEE_DIR"
echo "   TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
echo "   OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"

# 验证关键路径
echo ""
echo "🔍 验证路径存在性:"

if [ -d "$TA_DEV_KIT_DIR" ]; then
    echo "✅ TA_DEV_KIT_DIR 存在"
    echo "   库文件数量: $(ls -1 "$TA_DEV_KIT_DIR/lib/" 2>/dev/null | wc -l)"
else
    echo "❌ TA_DEV_KIT_DIR 不存在: $TA_DEV_KIT_DIR"
    NEED_BUILD=true
fi

if [ -d "$OPTEE_CLIENT_EXPORT" ]; then
    echo "✅ OPTEE_CLIENT_EXPORT 存在"
    echo "   库文件数量: $(ls -1 "$OPTEE_CLIENT_EXPORT/lib/" 2>/dev/null | wc -l)"
else
    echo "❌ OPTEE_CLIENT_EXPORT 不存在: $OPTEE_CLIENT_EXPORT"
    NEED_BUILD=true
fi

# 如果需要构建，提供指导
if [ "$NEED_BUILD" = "true" ]; then
    echo ""
    echo "⚠️ 需要构建OP-TEE库"
    echo "🔧 请运行以下命令:"
    echo "   cd $BASE_DIR"
    echo "   ./build_optee_libraries.sh optee/"
    echo ""
    echo "📝 或者使用预编译的库文件 (如果可用)"
else
    echo ""
    echo "🎉 OP-TEE环境配置完成!"
    echo ""
    echo "📋 现在可以构建TA:"
    echo "   cd packages/airaccount-ta-simple"
    echo "   make clean && make"
fi

# 保存环境变量到文件，供后续使用
ENV_FILE="$HOME/.airaccount_env"
cat > "$ENV_FILE" << EOF
# AirAccount OP-TEE环境变量
export OPTEE_DIR="$OPTEE_DIR"
export OPTEE_OS_DIR="$OPTEE_OS_DIR"
export OPTEE_CLIENT_DIR="$OPTEE_CLIENT_DIR"
export TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR"
export OPTEE_CLIENT_EXPORT="$OPTEE_CLIENT_EXPORT"
export CROSS_COMPILE64="$CROSS_COMPILE64"
export TARGET_TA="$TARGET_TA"
export TARGET_HOST="$TARGET_HOST"
export CROSS_COMPILE_TA="$CROSS_COMPILE_TA"
export CROSS_COMPILE_HOST="$CROSS_COMPILE_HOST"
EOF

echo ""
echo "💾 环境变量已保存到: $ENV_FILE"
echo "🔄 要在新终端中使用，请运行: source $ENV_FILE"