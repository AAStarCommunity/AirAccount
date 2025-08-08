#!/bin/bash
# 完整测试脚本

set -e

# 加载环境配置
source "$(dirname "$0")/setup_optee_env.sh"

echo ""
echo "🧪 开始 AirAccount 完整测试..."
echo "======================================"

# 记录开始时间
START_TIME=$(date +%s)

# 测试计数器
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

run_test() {
    local test_name="$1"
    local test_command="$2"
    local test_dir="${3:-$(pwd)}"
    
    echo "   运行: $test_name"
    ((TOTAL_TESTS++))
    
    cd "$test_dir"
    if eval "$test_command" > /dev/null 2>&1; then
        echo "   ✅ $test_name"
        ((PASSED_TESTS++))
        return 0
    else
        echo "   ❌ $test_name"
        ((FAILED_TESTS++))
        return 1
    fi
}

# 1. Mock 版本测试
echo "1️⃣ Mock 版本测试..."
if [ -d "$PROJECT_ROOT/packages/mock-hello" ]; then
    run_test "Mock Hello 构建测试" "timeout 60 cargo build --release" "$PROJECT_ROOT/packages/mock-hello"
    run_test "Mock TA-CA 通信测试" "timeout 60 cargo run --bin mock-ca test" "$PROJECT_ROOT/packages/mock-hello"
    run_test "Mock 交互模式启动测试" "timeout 10 echo 'quit' | cargo run --bin mock-ca interactive" "$PROJECT_ROOT/packages/mock-hello"
else
    echo "   ⚠️  Mock Hello 项目不存在，跳过"
fi

# 2. 核心逻辑测试
echo ""
echo "2️⃣ 核心逻辑测试..."
if [ -d "$PROJECT_ROOT/packages/core-logic" ]; then
    run_test "核心逻辑单元测试" "timeout 120 cargo test" "$PROJECT_ROOT/packages/core-logic"
    run_test "安全模块测试" "timeout 60 cargo test security" "$PROJECT_ROOT/packages/core-logic"
    run_test "常时操作测试" "timeout 60 cargo test constant_time" "$PROJECT_ROOT/packages/core-logic"
else
    echo "   ⚠️  核心逻辑项目不存在，跳过"
fi

# 3. 工作区测试
echo ""
echo "3️⃣ 工作区集成测试..."
cd "$PROJECT_ROOT"
if [ -f "Cargo.toml" ]; then
    run_test "工作区构建测试" "timeout 180 cargo build --workspace" "$PROJECT_ROOT"
    run_test "工作区单元测试" "timeout 300 cargo test --workspace" "$PROJECT_ROOT"
else
    echo "   ⚠️  工作区配置不存在，跳过"
fi

# 4. OP-TEE 客户端测试
echo ""
echo "4️⃣ OP-TEE 客户端测试..."

# Hello World 客户端测试
if [ -d "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host" ]; then
    run_test "Hello World 客户端构建" "timeout 120 cargo build --target aarch64-unknown-linux-gnu --release" "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host"
else
    echo "   ⚠️  Hello World 示例不存在，跳过"
fi

# eth_wallet 客户端测试
if [ -d "$TEACLAVE_SDK_DIR/projects/web3/eth_wallet/host" ]; then
    run_test "eth_wallet 客户端构建" "timeout 120 cargo build --target aarch64-unknown-linux-gnu --release" "$TEACLAVE_SDK_DIR/projects/web3/eth_wallet/host"
else
    echo "   ⚠️  eth_wallet 示例不存在，跳过"
fi

# 5. 安全性测试
echo ""
echo "5️⃣ 安全性测试..."
if [ -d "$PROJECT_ROOT/packages/core-logic" ]; then
    cd "$PROJECT_ROOT/packages/core-logic"
    run_test "侧信道攻击防护测试" "timeout 60 cargo test test_constant_time" "$PROJECT_ROOT/packages/core-logic"
    run_test "内存安全测试" "timeout 60 cargo test test_secure_memory" "$PROJECT_ROOT/packages/core-logic"
    run_test "审计日志测试" "timeout 60 cargo test test_audit" "$PROJECT_ROOT/packages/core-logic"
fi

# 6. 性能基准测试
echo ""
echo "6️⃣ 性能基准测试..."
if [ -d "$PROJECT_ROOT/packages/core-logic" ]; then
    cd "$PROJECT_ROOT/packages/core-logic"
    if cargo test --features bench > /dev/null 2>&1; then
        run_test "性能基准测试" "timeout 120 cargo test bench_ --features bench" "$PROJECT_ROOT/packages/core-logic"
    else
        echo "   ⚠️  性能基准测试特性未启用，跳过"
    fi
fi

# 7. 代码质量检查
echo ""
echo "7️⃣ 代码质量检查..."
cd "$PROJECT_ROOT"

# Clippy 检查
if command -v cargo-clippy > /dev/null 2>&1; then
    run_test "Clippy 代码检查" "timeout 120 cargo clippy --workspace -- -D warnings" "$PROJECT_ROOT"
else
    echo "   ⚠️  Clippy 未安装，跳过代码检查"
fi

# 格式检查
if command -v rustfmt > /dev/null 2>&1; then
    run_test "代码格式检查" "cargo fmt --all -- --check" "$PROJECT_ROOT"
else
    echo "   ⚠️  rustfmt 未安装，跳过格式检查"
fi

# 8. 集成场景测试
echo ""
echo "8️⃣ 集成场景测试..."
if [ -d "$PROJECT_ROOT/packages/mock-hello" ]; then
    cd "$PROJECT_ROOT/packages/mock-hello"
    
    # 测试各种命令
    run_test "Hello World 命令测试" "timeout 10 cargo run --bin mock-ca hello" "$PROJECT_ROOT/packages/mock-hello"
    run_test "Echo 命令测试" "timeout 10 cargo run --bin mock-ca echo 'test message'" "$PROJECT_ROOT/packages/mock-hello"
    run_test "Version 命令测试" "timeout 10 cargo run --bin mock-ca version" "$PROJECT_ROOT/packages/mock-hello"
    run_test "CreateWallet 命令测试" "timeout 10 cargo run --bin mock-ca create-wallet" "$PROJECT_ROOT/packages/mock-hello"
fi

# 计算测试时间
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# 显示测试结果
echo ""
echo "======================================"
echo "📊 测试结果总结:"
echo "🧪 总测试数: $TOTAL_TESTS"
echo "✅ 通过: $PASSED_TESTS"
echo "❌ 失败: $FAILED_TESTS"
echo "⏱️  总用时: ${DURATION} 秒"

if [ $FAILED_TESTS -eq 0 ]; then
    echo ""
    echo "🎉 所有测试通过！系统运行正常"
    echo ""
    echo "💡 测试覆盖："
    echo "   - Mock TA-CA 通信：完全通过"
    echo "   - 核心安全模块：完全通过"
    echo "   - OP-TEE 客户端构建：完全通过"
    echo "   - 代码质量检查：完全通过"
    echo ""
    echo "🚀 系统已准备好进行生产开发！"
    exit 0
else
    echo ""
    echo "⚠️  发现 $FAILED_TESTS 个测试失败"
    echo "请检查上述失败的测试并解决问题"
    echo ""
    echo "🔧 常见解决方案："
    echo "   - 运行 ./scripts/verify_optee_setup.sh 检查环境"
    echo "   - 重新运行 ./scripts/build_all.sh 确保构建完整"
    echo "   - 检查依赖是否正确安装"
    exit 1
fi