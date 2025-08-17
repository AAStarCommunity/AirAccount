#!/bin/bash

# 为aarch64交叉编译构建OpenSSL
set -e

OPENSSL_VERSION="3.0.8"
BUILD_DIR="/tmp/openssl_aarch64_build"
INSTALL_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/openssl_aarch64"

echo "🔧 Building OpenSSL ${OPENSSL_VERSION} for aarch64..."

# 创建构建目录
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# 下载OpenSSL源码（如果不存在）
if [ ! -f "openssl-${OPENSSL_VERSION}.tar.gz" ]; then
    echo "📥 Downloading OpenSSL source..."
    rm -f "openssl-${OPENSSL_VERSION}.tar.gz"  # 删除可能的错误文件
    curl -L -o "openssl-${OPENSSL_VERSION}.tar.gz" "https://github.com/openssl/openssl/archive/openssl-${OPENSSL_VERSION}.tar.gz"
fi

# 解压
if [ ! -d "openssl-openssl-${OPENSSL_VERSION}" ]; then
    echo "📦 Extracting OpenSSL source..."
    tar -xzf "openssl-${OPENSSL_VERSION}.tar.gz"
fi

cd "openssl-openssl-${OPENSSL_VERSION}"

# 设置交叉编译环境
export CC=aarch64-linux-gnu-gcc
export CXX=aarch64-linux-gnu-g++
export AR=aarch64-linux-gnu-ar
export STRIP=aarch64-linux-gnu-strip
export RANLIB=aarch64-linux-gnu-ranlib

# 配置OpenSSL
echo "⚙️ Configuring OpenSSL for aarch64..."
./Configure linux-aarch64 \
    --prefix="$INSTALL_DIR" \
    --openssldir="$INSTALL_DIR/ssl" \
    no-shared \
    no-async \
    -fPIC

# 编译
echo "🔨 Building OpenSSL..."
make -j$(sysctl -n hw.ncpu)

# 安装
echo "📦 Installing OpenSSL to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
make install

echo "✅ OpenSSL for aarch64 built successfully!"
echo "📍 Installed to: $INSTALL_DIR"
echo "🔍 Library files:"
ls -la "$INSTALL_DIR/lib/"