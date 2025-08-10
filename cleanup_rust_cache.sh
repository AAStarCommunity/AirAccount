#!/bin/bash

# Licensed to AirAccount under the Apache License, Version 2.0
# Rust缓存清理和构建目录配置脚本

set -e

echo "🧹 AirAccount Rust缓存清理和构建目录配置"
echo "========================================="

# 设置构建目标目录到当前开发路径
export CARGO_TARGET_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/target"
mkdir -p "$CARGO_TARGET_DIR"

echo "📍 设置Cargo目标目录: $CARGO_TARGET_DIR"

# 创建.cargo/config.toml配置文件
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[build]
target-dir = "/Volumes/UltraDisk/Dev2/aastar/AirAccount/target"

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

[env]
# TEE环境变量
OPTEE_CLIENT_EXPORT = "/tmp/mock_tee"
PKG_CONFIG_ALLOW_CROSS = "1"
EOF

echo "📝 已创建Cargo配置文件"

# 检查cargo-cache是否已安装
if ! command -v cargo-cache &> /dev/null; then
    echo "❌ cargo-cache未安装，正在安装..."
    cargo install cargo-cache
else
    echo "✅ cargo-cache已安装"
fi

echo "📊 列出当前Rust缓存状态:"
cargo cache

echo ""
echo "🗂️  详细缓存信息:"
cargo cache --info

echo ""
echo "🧼 清理过时的注册表缓存 (保留最新版本):"
cargo cache --autoclean

echo ""
echo "🗑️  清理孤立的源码缓存:"
cargo cache --gc

# 只保留最近30天的构建缓存
echo ""
echo "🕒 清理30天前的构建缓存:"
cargo cache --autoclean-expensive

# 显示清理后的状态
echo ""
echo "📈 清理后的缓存状态:"
cargo cache

echo ""
echo "💾 磁盘空间使用情况:"
df -h /

echo ""
echo "🎯 构建目录设置:"
echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
ls -la "$CARGO_TARGET_DIR" 2>/dev/null || echo "目录为空或不存在"

echo ""
echo "✅ Rust缓存清理和配置完成!"
echo "📌 后续所有cargo构建都会使用: $CARGO_TARGET_DIR"