#!/bin/bash
# Teaclave TrustZone SDK 克隆和初始化脚本

set -e

echo "=== Teaclave TrustZone SDK 设置 ==="

# 项目根目录
PROJECT_ROOT=$(pwd)
THIRD_PARTY_DIR="$PROJECT_ROOT/third_party"
SDK_DIR="$THIRD_PARTY_DIR/incubator-teaclave-trustzone-sdk"

echo "1. 检查third_party目录..."
if [ ! -d "$THIRD_PARTY_DIR" ]; then
    echo "📁 创建third_party目录..."
    mkdir -p "$THIRD_PARTY_DIR"
else
    echo "✅ third_party目录已存在"
fi

echo ""
echo "2. 检查Teaclave TrustZone SDK..."

if [ -d "$SDK_DIR" ]; then
    echo "✅ Teaclave SDK目录已存在: $SDK_DIR"
    
    # 检查是否为git仓库
    if [ -d "$SDK_DIR/.git" ]; then
        echo "🔍 检查SDK状态..."
        cd "$SDK_DIR"
        
        # 检查远程URL
        current_remote=$(git remote get-url origin 2>/dev/null || echo "")
        expected_remote="https://github.com/apache/incubator-teaclave-trustzone-sdk.git"
        
        if [ "$current_remote" = "$expected_remote" ]; then
            echo "✅ 远程仓库正确: $current_remote"
            
            # 获取最新更新
            echo "📦 更新SDK仓库..."
            git fetch origin
            echo "✅ SDK仓库已更新"
        else
            echo "⚠️  远程仓库不匹配，预期: $expected_remote"
            echo "   实际: $current_remote"
        fi
        
        cd "$PROJECT_ROOT"
    else
        echo "⚠️  SDK目录存在但不是git仓库"
    fi
else
    echo "📦 克隆Teaclave TrustZone SDK..."
    
    # 克隆SDK到third_party目录
    cd "$THIRD_PARTY_DIR"
    git clone --recursive https://github.com/apache/incubator-teaclave-trustzone-sdk.git
    
    echo "✅ SDK克隆完成"
    cd "$PROJECT_ROOT"
fi

echo ""
echo "3. 初始化子模块..."

cd "$SDK_DIR"

# 检查并初始化子模块
echo "🔍 检查子模块状态..."
git submodule status

echo "📦 更新子模块..."
git submodule update --init --recursive

echo "✅ 子模块初始化完成"

echo ""
echo "4. 验证SDK结构..."

# 检查关键目录是否存在
key_dirs=(
    "examples"
    "optee-teec"
    "optee-utee" 
    "optee-qemuv8"
)

cd "$SDK_DIR"
for dir in "${key_dirs[@]}"; do
    if [ -d "$dir" ]; then
        echo "✅ $dir: 存在"
    else
        echo "⚠️  $dir: 不存在"
    fi
done

# 检查示例项目
echo ""
echo "5. 检查eth_wallet示例..."
ETH_WALLET_DIR="$SDK_DIR/examples/ethereum-wallet"
if [ -d "$ETH_WALLET_DIR" ]; then
    echo "✅ eth_wallet示例存在: $ETH_WALLET_DIR"
    
    # 列出示例项目结构
    echo "📁 eth_wallet项目结构:"
    ls -la "$ETH_WALLET_DIR"
else
    echo "⚠️  eth_wallet示例未找到"
    echo "🔍 查找类似的以太坊相关示例..."
    find "$SDK_DIR/examples" -name "*eth*" -o -name "*wallet*" -o -name "*ethereum*" 2>/dev/null || echo "   未找到相关示例"
fi

cd "$PROJECT_ROOT"

echo ""
echo "6. 检查构建依赖..."

# 检查Makefile
MAKEFILE="$SDK_DIR/Makefile"
if [ -f "$MAKEFILE" ]; then
    echo "✅ 主Makefile存在"
    
    # 检查主要构建目标
    echo "🎯 可用的构建目标:"
    grep "^[a-zA-Z0-9][^$#]*:.*$" "$MAKEFILE" | head -10
else
    echo "⚠️  主Makefile未找到"
fi

echo ""
echo "=== Teaclave TrustZone SDK 设置完成 ==="
echo "📍 SDK位置: $SDK_DIR"
echo "📝 提示: 可以运行 'cd $SDK_DIR && make help' 查看可用命令"