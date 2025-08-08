#!/bin/bash
# OP-TEE 环境验证脚本

set -e

# 加载环境配置
source "$(dirname "$0")/setup_optee_env.sh"

echo ""
echo "🔍 开始 OP-TEE 环境完整验证..."
echo "======================================"

# 计数器
PASS_COUNT=0
FAIL_COUNT=0

check_result() {
    if [ $? -eq 0 ]; then
        echo "✅ $1"
        ((PASS_COUNT++))
    else
        echo "❌ $1"
        ((FAIL_COUNT++))
        return 1
    fi
}

# 1. 检查交叉编译器
echo "1️⃣ 检查交叉编译器..."
aarch64-unknown-linux-gnu-gcc --version > /dev/null 2>&1
check_result "ARM64 交叉编译器可用"

armv7-unknown-linux-gnueabihf-gcc --version > /dev/null 2>&1
check_result "ARM32 交叉编译器可用"

# 2. 检查 Rust 工具链
echo ""
echo "2️⃣ 检查 Rust 工具链..."
command -v xargo > /dev/null 2>&1
check_result "xargo 工具可用"

rustup component list --toolchain nightly-2024-05-15-aarch64-apple-darwin | grep -q "rust-src.*installed"
check_result "Rust 源码组件已安装"

# 3. 检查 Python 依赖
echo ""
echo "3️⃣ 检查 Python 依赖..."
python3 -c "import elftools" 2>/dev/null
check_result "pyelftools 模块可用"

# 4. 检查 OP-TEE 构建产物
echo ""
echo "4️⃣ 检查 OP-TEE 构建产物..."

if [ -f "$OPTEE_DIR/optee_os/out/arm-plat-vexpress/core/tee.elf" ]; then
    check_result "OP-TEE OS 已构建"
else
    echo "❌ OP-TEE OS 未构建"
    echo "   请运行: cd third_party/incubator-teaclave-trustzone-sdk && ./build_optee_libraries.sh \$OPTEE_DIR"
    ((FAIL_COUNT++))
fi

if [ -f "$OPTEE_CLIENT_EXPORT/usr/lib/libteec.so" ]; then
    check_result "OP-TEE Client 库可用"
else
    echo "❌ OP-TEE Client 库未找到"
    echo "   请检查库文件是否正确复制到 export 目录"
    ((FAIL_COUNT++))
fi

if [ -d "$TA_DEV_KIT_DIR" ]; then
    check_result "TA 开发套件目录存在"
else
    echo "❌ TA 开发套件目录不存在"
    ((FAIL_COUNT++))
fi

# 5. 检查目标规范文件
echo ""
echo "5️⃣ 检查 Rust 目标规范..."
RUST_TARGET_SPEC="$HOME/.rustup/toolchains/nightly-2024-05-15-aarch64-apple-darwin/lib/rustlib/aarch64-unknown-optee/target.json"
if [ -f "$RUST_TARGET_SPEC" ]; then
    check_result "Rust 目标规范文件已配置"
else
    echo "⚠️  Rust 目标规范文件未找到"
    echo "   请运行以下命令创建符号链接:"
    echo "   mkdir -p ~/.rustup/toolchains/nightly-2024-05-15-aarch64-apple-darwin/lib/rustlib/aarch64-unknown-optee"
    echo "   ln -sf \$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk/aarch64-unknown-optee.json \$RUST_TARGET_SPEC"
    ((FAIL_COUNT++))
fi

# 6. 测试客户端构建
echo ""
echo "6️⃣ 测试客户端构建..."

if [ -d "$PROJECT_ROOT/packages/mock-hello" ]; then
    cd "$PROJECT_ROOT/packages/mock-hello"
    if cargo build --release > /dev/null 2>&1; then
        check_result "Mock Hello 客户端构建成功"
    else
        echo "❌ Mock Hello 客户端构建失败"
        ((FAIL_COUNT++))
    fi
else
    echo "⚠️  Mock Hello 项目不存在，跳过测试"
fi

# 7. 测试 OP-TEE 客户端构建
if [ -d "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host" ]; then
    cd "$TEACLAVE_SDK_DIR/examples/hello_world-rs/host"
    if timeout 60 cargo build --target aarch64-unknown-linux-gnu --release > /dev/null 2>&1; then
        check_result "OP-TEE Hello World 客户端构建成功"
    else
        echo "❌ OP-TEE Hello World 客户端构建失败"
        ((FAIL_COUNT++))
    fi
else
    echo "⚠️  Hello World 示例不存在，跳过测试"
fi

# 8. 运行 Mock 测试
echo ""
echo "7️⃣ 运行功能测试..."
if [ -d "$PROJECT_ROOT/packages/mock-hello" ]; then
    cd "$PROJECT_ROOT/packages/mock-hello"
    if timeout 30 cargo run --bin mock-ca test > /dev/null 2>&1; then
        check_result "Mock TA-CA 通信测试通过"
    else
        echo "❌ Mock TA-CA 通信测试失败"
        ((FAIL_COUNT++))
    fi
fi

# 显示总结
echo ""
echo "======================================"
echo "📊 验证结果总结:"
echo "✅ 通过: $PASS_COUNT 项"
echo "❌ 失败: $FAIL_COUNT 项"

if [ $FAIL_COUNT -eq 0 ]; then
    echo ""
    echo "🎉 所有检查通过！OP-TEE 开发环境完全就绪"
    echo "💡 你现在可以:"
    echo "   - 运行 ./scripts/build_all.sh 进行完整构建"
    echo "   - 运行 ./scripts/test_all.sh 执行所有测试"
    echo "   - 开始 AirAccount TA-CA 开发"
    exit 0
else
    echo ""
    echo "⚠️  存在 $FAIL_COUNT 个问题需要解决"
    echo "请根据上述提示解决问题后重新运行验证"
    exit 1
fi