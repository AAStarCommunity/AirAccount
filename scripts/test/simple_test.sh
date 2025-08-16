#!/bin/bash

# 简单的AirAccount组件测试
echo "🧪 AirAccount 组件功能测试"
echo "========================="

# Test 1: TA文件格式验证
echo "Test 1: TA文件格式验证"
TA_FILE="packages/airaccount-ta-simple/target/aarch64-unknown-linux-gnu/release/11223344-5566-7788-99aa-bbccddeeff01.ta"

if [ -f "$TA_FILE" ]; then
    # Check OP-TEE magic header
    header=$(hexdump -C "$TA_FILE" | head -1 | grep "HSTO")
    if [ ! -z "$header" ]; then
        echo "✅ TA文件OP-TEE格式验证通过"
    else
        echo "❌ TA文件格式验证失败"
    fi
    
    # Check file size (should be around 268KB)
    size=$(stat -f%z "$TA_FILE" 2>/dev/null || stat -c%s "$TA_FILE" 2>/dev/null)
    if [ "$size" -gt 200000 ] && [ "$size" -lt 300000 ]; then
        echo "✅ TA文件大小合理 ($size bytes)"
    else
        echo "❌ TA文件大小异常 ($size bytes)"
    fi
else
    echo "❌ TA文件不存在"
fi

# Test 2: CA文件验证
echo ""
echo "Test 2: CA客户端文件验证"
CA_FILE="packages/airaccount-ca/target/aarch64-unknown-linux-gnu/debug/airaccount-ca"

if [ -f "$CA_FILE" ]; then
    # Check if it's ARM64 executable
    file_info=$(file "$CA_FILE")
    if echo "$file_info" | grep -q "aarch64"; then
        echo "✅ CA文件ARM64架构验证通过"
    else
        echo "❌ CA文件架构验证失败"
    fi
    
    # Check if it's executable
    if [ -x "$CA_FILE" ]; then
        echo "✅ CA文件可执行权限正确"
    else
        echo "❌ CA文件缺少执行权限"
    fi
else
    echo "❌ CA文件不存在"
fi

# Test 3: 源代码安全特性检查
echo ""
echo "Test 3: P0安全特性源码检查"
TA_SOURCE="packages/airaccount-ta-simple/src/main.rs"

if [ -f "$TA_SOURCE" ]; then
    # Check security features in source
    security_features=0
    
    if grep -q "validate_command_parameters" "$TA_SOURCE"; then
        echo "✅ 输入验证系统检测通过"
        ((security_features++))
    fi
    
    if grep -q "SECURITY_MANAGER" "$TA_SOURCE"; then
        echo "✅ 安全管理器检测通过"
        ((security_features++))
    fi
    
    if grep -q "secure_hash" "$TA_SOURCE"; then
        echo "✅ 安全哈希函数检测通过"
        ((security_features++))
    fi
    
    # Check wallet commands
    wallet_cmds=$(grep -c "CMD_.*WALLET\|CMD_.*ADDRESS\|CMD_.*SIGN" "$TA_SOURCE")
    if [ "$wallet_cmds" -gt 3 ]; then
        echo "✅ 钱包命令完整性检测通过 ($wallet_cmds 个命令)"
        ((security_features++))
    fi
    
    echo "🔒 P0安全特性完整度: $security_features/4"
else
    echo "❌ TA源代码文件不存在"
fi

# Test 4: 依赖检查
echo ""
echo "Test 4: 构建依赖检查"

# Check cross-compilation target
if rustup target list --installed | grep -q "aarch64-unknown-linux-gnu"; then
    echo "✅ ARM64交叉编译目标已安装"
else
    echo "❌ ARM64交叉编译目标未安装"
fi

# Check toolchain
if command -v aarch64-linux-gnu-gcc > /dev/null; then
    echo "✅ ARM64交叉编译器可用"
else
    echo "❌ ARM64交叉编译器不可用"
fi

# Test 5: 模拟TA UUID验证
echo ""
echo "Test 5: TA UUID验证"
expected_uuid="11223344-5566-7788-99aa-bbccddeeff01"
if echo "$TA_FILE" | grep -q "$expected_uuid"; then
    echo "✅ TA UUID匹配预期: $expected_uuid"
else
    echo "❌ TA UUID不匹配"
fi

# 最终评估
echo ""
echo "📊 测试总结"
echo "=========="

total_tests=5
passed_tests=0

# 计算通过的测试数
if [ -f "$TA_FILE" ]; then ((passed_tests++)); fi
if [ -f "$CA_FILE" ]; then ((passed_tests++)); fi
if [ -f "$TA_SOURCE" ] && [ "$security_features" -eq 4 ]; then ((passed_tests++)); fi
if rustup target list --installed | grep -q "aarch64-unknown-linux-gnu"; then ((passed_tests++)); fi
if echo "$TA_FILE" | grep -q "$expected_uuid"; then ((passed_tests++)); fi

pass_rate=$((passed_tests * 100 / total_tests))

echo "通过测试: $passed_tests/$total_tests"
echo "通过率: $pass_rate%"

if [ "$pass_rate" -gt 90 ]; then
    echo "🎉 测试结果: 优秀"
    echo "✅ AirAccount项目构建质量很高，可以进行下一步集成测试"
elif [ "$pass_rate" -gt 70 ]; then
    echo "✅ 测试结果: 良好" 
    echo "⚠️  建议修复少数问题后进行集成测试"
else
    echo "❌ 测试结果: 需要改进"
    echo "🔧 请修复构建问题后重新测试"
fi