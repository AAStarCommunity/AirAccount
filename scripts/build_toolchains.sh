#!/bin/bash
# 构建交叉编译工具链脚本

set -e

echo "=== 构建交叉编译工具链 ==="

# 项目目录设置
PROJECT_ROOT=$(pwd)
SDK_DIR="$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk"

echo "1. 检查SDK目录..."
if [ ! -d "$SDK_DIR" ]; then
    echo "❌ Teaclave SDK目录不存在: $SDK_DIR"
    echo "请先运行 ./scripts/setup_teaclave_sdk.sh"
    exit 1
else
    echo "✅ SDK目录存在: $SDK_DIR"
fi

echo ""
echo "2. 检查构建环境..."

# 检查必要工具
required_tools=("make" "git" "curl" "python3")
for tool in "${required_tools[@]}"; do
    if command -v "$tool" &> /dev/null; then
        echo "✅ $tool: $(command -v $tool)"
    else
        echo "❌ $tool: 未找到"
        exit 1
    fi
done

# 检查Rust工具链
if command -v rustc &> /dev/null && command -v cargo &> /dev/null; then
    echo "✅ Rust工具链: $(rustc --version)"
else
    echo "❌ Rust工具链未安装，请先运行 ./scripts/setup_rust.sh"
    exit 1
fi

echo ""
echo "3. 切换到SDK目录..."
cd "$SDK_DIR"

echo "当前目录: $(pwd)"
echo "检查Makefile..."
if [ -f "Makefile" ]; then
    echo "✅ Makefile存在"
else
    echo "❌ Makefile不存在"
    exit 1
fi

echo ""
echo "4. 检查SDK构建方式..."

# 检查是否有Docker镜像方式
echo "🔍 检查新版SDK构建方式..."
if [ -f "docs/emulate-and-dev-in-docker.md" ]; then
    echo "✅ 发现新版SDK文档，使用Docker开发环境"
    echo ""
    echo "📋 新版Teaclave TrustZone SDK信息:"
    echo "   - 使用Docker镜像提供预构建环境"
    echo "   - 不再需要手动构建工具链" 
    echo "   - 支持QEMU emulation开发"
    echo ""
    echo "🐳 推荐的Docker开发流程:"
    echo "1. 拉取预构建镜像："
    echo "   docker pull teaclave/teaclave-trustzone-emulator-nostd-optee-4.5.0-expand-memory:latest"
    echo ""
    echo "2. 启动开发环境："
    echo "   docker run -it --rm --name teaclave_dev_env \\"
    echo "     -v \$(pwd):/root/teaclave_sdk_src \\"
    echo "     -w /root/teaclave_sdk_src \\"
    echo "     teaclave/teaclave-trustzone-emulator-nostd-optee-4.5.0-expand-memory:latest"
    echo ""
    echo "3. 在容器内构建示例："
    echo "   make -C examples/hello_world-rs/"
    
    # 检查Docker可用性
    if command -v docker &> /dev/null; then
        echo ""
        echo "✅ Docker已安装: $(docker --version)"
        echo "📦 正在拉取Teaclave开发环境镜像..."
        
        echo "构建开始时间: $(date)"
        if docker pull teaclave/teaclave-trustzone-emulator-nostd-optee-4.5.0-expand-memory:latest; then
            echo ""
            echo "🎉 Docker镜像拉取成功！"
            echo "构建结束时间: $(date)"
        else
            echo ""
            echo "⚠️  Docker镜像拉取失败，但可以稍后再试"
            echo "构建结束时间: $(date)"
        fi
    else
        echo ""
        echo "⚠️  Docker未安装，请安装Docker后使用推荐流程"
        echo "   macOS: brew install --cask docker"
        echo "   Linux: 参考 https://docs.docker.com/engine/install/"
    fi
else
    echo "❌ 未找到预期的文档文件"
    exit 1
fi

echo ""
echo "5. 验证SDK环境..."

# 检查示例项目
echo "检查可用示例..."
if [ -d "examples" ]; then
    echo "✅ 示例目录存在"
    example_count=$(find examples -name "Makefile" | wc -l)
    echo "   可用示例项目: $example_count 个"
    
    # 列出一些关键示例
    key_examples=("hello_world-rs" "signature_verification-rs" "secure_storage-rs" "acipher-rs")
    echo "   关键示例:"
    for example in "${key_examples[@]}"; do
        if [ -d "examples/$example" ]; then
            echo "   ✅ $example"
        else
            echo "   ❌ $example (不存在)"
        fi
    done
else
    echo "❌ 示例目录不存在"
fi

# 检查Rust目标文件
echo ""
echo "检查Rust目标配置..."
if [ -f "aarch64-unknown-optee.json" ]; then
    echo "✅ ARM64 TEE目标配置存在"
else
    echo "⚠️  ARM64 TEE目标配置不存在"
fi

if [ -f "arm-unknown-optee.json" ]; then
    echo "✅ ARM32 TEE目标配置存在"  
else
    echo "⚠️  ARM32 TEE目标配置不存在"
fi

echo ""
echo "=== 开发环境检查完成 ==="
echo "📍 SDK位置: $SDK_DIR"
echo "📍 开发方式: Docker容器化环境（推荐）"
echo ""
echo "📝 下一步建议:"
echo "1. 如果要使用Docker: 运行上述Docker命令启动开发环境"
echo "2. 如果要本地开发: 尝试构建hello_world示例测试环境"
echo "3. 继续执行 ./scripts/test_hello_world.sh 验证环境"

cd "$PROJECT_ROOT"