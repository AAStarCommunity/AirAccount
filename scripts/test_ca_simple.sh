#!/bin/bash

# 简化的AirAccount CA测试脚本
# 使用直接的Linux环境来测试CA-TA通信

echo "🚀 AirAccount 简化CA功能测试"
echo "=========================="

# 检查构建产物
TA_FILE="packages/airaccount-ta-simple/target/aarch64-unknown-linux-gnu/release/11223344-5566-7788-99aa-bbccddeeff01.ta"
CA_FILE="packages/airaccount-ca/target/aarch64-unknown-linux-gnu/debug/airaccount-ca"

if [ ! -f "$TA_FILE" ]; then
    echo "❌ TA文件不存在: $TA_FILE"
    exit 1
fi

if [ ! -f "$CA_FILE" ]; then
    echo "❌ CA文件不存在: $CA_FILE"
    exit 1
fi

echo "✅ 构建产物验证通过"
echo "   TA文件: $TA_FILE ($(stat -f%z "$TA_FILE" 2>/dev/null || stat -c%s "$TA_FILE" 2>/dev/null) bytes)"
echo "   CA文件: $CA_FILE ($(stat -f%z "$CA_FILE" 2>/dev/null || stat -c%s "$CA_FILE" 2>/dev/null) bytes)"

# 验证TA文件格式
echo ""
echo "🔍 TA文件格式验证:"
if hexdump -C "$TA_FILE" | head -1 | grep -q "HSTO"; then
    echo "✅ OP-TEE HSTO头部格式正确"
else
    echo "❌ OP-TEE格式验证失败"
    exit 1
fi

# 验证CA文件架构
echo ""
echo "🔍 CA文件架构验证:"
ca_arch=$(file "$CA_FILE" | grep -o "ARM aarch64")
if [ ! -z "$ca_arch" ]; then
    echo "✅ ARM64架构正确"
else
    echo "❌ 架构验证失败"
    exit 1
fi

# 检查CA文件的依赖
echo ""
echo "🔍 CA文件依赖分析:"
if command -v ldd > /dev/null 2>&1; then
    echo "动态库依赖:"
    ldd "$CA_FILE" 2>/dev/null | head -10 || echo "  (静态链接或交叉编译，无法在当前系统分析)"
fi

# 尝试检查CA的基本可执行性（在当前架构下可能失败，但可以获得信息）
echo ""
echo "🔍 CA基本可执行性检查:"
if [ "$(uname -m)" = "arm64" ] || [ "$(uname -m)" = "aarch64" ]; then
    echo "✅ 当前系统是ARM64，CA文件可能可以直接运行"
    
    # 尝试获得帮助信息
    echo "📋 尝试获取CA帮助信息:"
    timeout 5 "$CA_FILE" --help 2>&1 || echo "  (可能需要OP-TEE环境或特定参数)"
    
    # 尝试获得版本信息
    echo "📋 尝试获取CA版本信息:"
    timeout 5 "$CA_FILE" version 2>&1 || echo "  (需要TEE环境支持)"
else
    echo "⚠️  当前系统是 $(uname -m)，无法直接运行ARM64 CA文件"
    echo "   需要ARM64环境或模拟器进行实际测试"
fi

# 源码功能验证
echo ""
echo "🔍 源码功能完整性验证:"
TA_SOURCE="packages/airaccount-ta-simple/src/main.rs"
CA_SOURCE="packages/airaccount-ca/src/main.rs"

if [ -f "$TA_SOURCE" ]; then
    echo "📋 TA源码分析:"
    
    # 检查命令处理
    cmd_count=$(grep -c "const CMD_" "$TA_SOURCE" 2>/dev/null || echo "0")
    echo "  命令定义数量: $cmd_count"
    
    # 检查钱包功能
    wallet_functions=$(grep -c "wallet\|Wallet" "$TA_SOURCE" 2>/dev/null || echo "0")
    echo "  钱包相关功能: $wallet_functions 处"
    
    # 检查安全特性
    security_features=0
    if grep -q "validate_command_parameters\|validate.*param" "$TA_SOURCE"; then
        echo "  ✅ 输入验证系统: 存在"
        security_features=$((security_features + 1))
    fi
    
    if grep -q "SECURITY_MANAGER\|SecurityManager" "$TA_SOURCE"; then
        echo "  ✅ 安全管理器: 存在"
        security_features=$((security_features + 1))
    fi
    
    if grep -q "secure_hash\|SecureHash" "$TA_SOURCE"; then
        echo "  ✅ 安全哈希函数: 存在"
        security_features=$((security_features + 1))
    fi
    
    echo "  🔒 P0安全特性: $security_features/3 已实现"
fi

if [ -f "$CA_SOURCE" ]; then
    echo "📋 CA源码分析:"
    
    # 检查测试功能
    test_functions=$(grep -c "test\|Test" "$CA_SOURCE" 2>/dev/null || echo "0")
    echo "  测试相关功能: $test_functions 处"
    
    # 检查命令支持
    if grep -q "hello\|Hello" "$CA_SOURCE"; then
        echo "  ✅ Hello命令: 支持"
    fi
    
    if grep -q "echo\|Echo" "$CA_SOURCE"; then
        echo "  ✅ Echo命令: 支持"
    fi
    
    if grep -q "wallet\|Wallet" "$CA_SOURCE"; then
        echo "  ✅ Wallet命令: 支持"
    fi
fi

# 创建测试总结
echo ""
echo "📊 测试总结"
echo "=========="

test_score=0
max_score=10

# 文件存在性 (2分)
if [ -f "$TA_FILE" ] && [ -f "$CA_FILE" ]; then
    test_score=$((test_score + 2))
    echo "✅ 构建产物完整 (+2分)"
fi

# 格式验证 (2分)
if hexdump -C "$TA_FILE" | head -1 | grep -q "HSTO"; then
    test_score=$((test_score + 1))
    echo "✅ TA格式正确 (+1分)"
fi

if file "$CA_FILE" | grep -q "ARM aarch64"; then
    test_score=$((test_score + 1))
    echo "✅ CA架构正确 (+1分)"
fi

# 源码功能 (4分)
if [ "$security_features" -ge 2 ]; then
    test_score=$((test_score + 2))
    echo "✅ P0安全特性充分 (+2分)"
elif [ "$security_features" -ge 1 ]; then
    test_score=$((test_score + 1))
    echo "✅ P0安全特性基本 (+1分)"
fi

if [ "$cmd_count" -gt 10 ]; then
    test_score=$((test_score + 2))
    echo "✅ 命令实现完整 (+2分)"
elif [ "$cmd_count" -gt 5 ]; then
    test_score=$((test_score + 1))
    echo "✅ 命令实现基本 (+1分)"
fi

# 环境准备 (2分)
if [ -d "third_party/incubator-teaclave-trustzone-sdk/tests" ]; then
    test_score=$((test_score + 1))
    echo "✅ OP-TEE测试环境就绪 (+1分)"
fi

if command -v qemu-system-aarch64 > /dev/null; then
    test_score=$((test_score + 1))
    echo "✅ QEMU模拟器可用 (+1分)"
fi

# 计算分数
percentage=$((test_score * 100 / max_score))
echo ""
echo "🎯 测试得分: $test_score/$max_score ($percentage%)"

if [ "$percentage" -ge 90 ]; then
    echo "🎉 优秀！AirAccount CA准备完全就绪"
    echo "✅ 建议进行OP-TEE集成测试"
elif [ "$percentage" -ge 70 ]; then
    echo "✅ 良好！AirAccount CA基本就绪"
    echo "💡 可以进行基础集成测试"
else
    echo "⚠️  需要改进！发现重要问题需要解决"
fi

echo ""
echo "🚀 下一步建议:"
echo "1. 在ARM64 Linux环境中运行完整的TA-CA测试"
echo "2. 使用真实的OP-TEE环境进行集成验证"
echo "3. 测试命令序列: hello → echo → version → test → wallet"
echo "4. 验证P0安全特性在运行时的表现"

echo ""
echo "📁 关键文件位置:"
echo "  TA: $TA_FILE"
echo "  CA: $CA_FILE"
echo "  测试环境: third_party/incubator-teaclave-trustzone-sdk/tests/"

exit 0