#!/bin/bash

# 简单TA测试工具编译脚本

set -e

echo "🔧 Building Simple TA Test Tool..."

# 检查环境变量
if [ -z "$OPTEE_CLIENT_EXPORT" ]; then
    echo "❌ OPTEE_CLIENT_EXPORT not set. Please run setup-env.sh first."
    exit 1
fi

# 编译设置
CC="aarch64-linux-gnu-gcc"
CFLAGS="-Wall -I$OPTEE_CLIENT_EXPORT/usr/include"
LDFLAGS="-L$OPTEE_CLIENT_EXPORT/usr/lib -lteec"

# 编译
echo "📝 Compiling simple-ta-test.c..."
$CC $CFLAGS scripts/simple-ta-test.c $LDFLAGS -o scripts/simple-ta-test

# 复制到共享目录
cp scripts/simple-ta-test /Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/tests/shared/

echo "✅ Simple TA test tool compiled and copied to shared directory"
echo "📝 Usage in QEMU: /shared/simple-ta-test"