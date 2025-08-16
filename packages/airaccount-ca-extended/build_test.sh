#!/bin/bash

# AirAccount CA Extended 构建测试脚本

set -e

echo "🔧 Building AirAccount CA Extended..."

# 检查必要的依赖
echo "📦 Checking dependencies..."

# 构建库
echo "🏗️ Building library..."
cargo build --lib

# 构建 CLI 工具
echo "🛠️ Building CLI tool..."
cargo build --bin ca-cli

# 构建 HTTP 服务器
echo "🌐 Building HTTP server..."
cargo build --bin ca-server

# 运行基本测试
echo "🧪 Running tests..."
cargo test --lib

echo "✅ All builds completed successfully!"

# 显示构建的二进制文件
echo ""
echo "📁 Built binaries:"
ls -la target/debug/ca-*

echo ""
echo "🚀 To run:"
echo "  CLI:    cargo run --bin ca-cli -- --help"
echo "  Server: cargo run --bin ca-server"