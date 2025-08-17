#!/bin/bash
# 测试三种类型的CA-TA通信

set -e

echo "🧪 测试三种类型的CA-TA通信"
echo "=" * 50

# 测试目录
TEST_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/tests"
SHARED_DIR="$TEST_DIR/shared"

echo "📁 测试目录: $TEST_DIR"
echo "📁 共享目录: $SHARED_DIR"

# 检查文件存在
echo -e "\n🔍 检查现有文件:"
ls -la "$SHARED_DIR/" | grep -E "(ta|ca)"

# 检查是否有现成的工作示例
echo -e "\n🔍 检查eth_wallet示例:"
ETH_WALLET_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/projects/web3/eth_wallet"
if [ -d "$ETH_WALLET_DIR" ]; then
    echo "✅ eth_wallet目录存在"
    ls -la "$ETH_WALLET_DIR/host/" 2>/dev/null | head -5 || echo "无host目录"
    ls -la "$ETH_WALLET_DIR/ta/" 2>/dev/null | head -5 || echo "无ta目录"
else
    echo "❌ eth_wallet目录不存在"
fi

# 检查我们创建的Basic版本
echo -e "\n🔍 检查Basic CA-TA:"
BASIC_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-basic"
if [ -d "$BASIC_DIR" ]; then
    echo "✅ Basic目录存在"
    ls -la "$BASIC_DIR/ca/" 2>/dev/null | head -3 || echo "无ca目录"
    ls -la "$BASIC_DIR/ta/" 2>/dev/null | head -3 || echo "无ta目录"
else
    echo "❌ Basic目录不存在"
fi

# 检查Simple版本
echo -e "\n🔍 检查Simple CA-TA:"
if [ -f "/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-ca/src/main.rs" ]; then
    echo "✅ Simple CA代码存在"
    grep -n "ParamValue::new" "/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-ca/src/main.rs" | head -2
else
    echo "❌ Simple CA代码不存在"
fi

if [ -f "/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-ta-simple/src/main.rs" ]; then
    echo "✅ Simple TA代码存在"
    grep -n "p2.set_a" "/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-ta-simple/src/main.rs" | head -2
else
    echo "❌ Simple TA代码不存在"
fi

echo -e "\n🎯 测试计划:"
echo "1. 先用eth_wallet验证QEMU环境正常"
echo "2. 测试修复后的Simple CA-TA" 
echo "3. 如果工作正常，创建Basic版本"
echo "4. 然后进行完整的5阶段测试"

echo -e "\n✅ 文件检查完成"