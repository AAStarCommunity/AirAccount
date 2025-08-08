# OP-TEE 开发环境搭建指南

本文档提供完整的 OP-TEE 开发环境搭建步骤，适用于 macOS 系统，支持 AirAccount 项目的 TEE 应用开发。

## 前置条件

### 系统要求
- macOS (测试环境: macOS 14.2+)
- Xcode Command Line Tools
- Homebrew
- Git

### 安装基础工具

```bash
# 安装 Xcode Command Line Tools
xcode-select --install

# 安装 Homebrew 依赖包
brew install automake coreutils curl gmp gnutls libtool libusb make wget

# 安装 Python 依赖
pip3 install pyelftools
```

## 第一步：克隆和初始化仓库

```bash
# 克隆主仓库
cd /path/to/your/projects
git clone https://github.com/your-org/AirAccount.git
cd AirAccount

# 初始化 Teaclave TrustZone SDK 子模块
git submodule update --init --recursive third_party/incubator-teaclave-trustzone-sdk
```

## 第二步：安装交叉编译工具链

```bash
# 添加 messense 交叉编译工具链 tap
brew tap messense/homebrew-macos-cross-toolchains

# 安装 ARM64 和 ARM32 交叉编译器
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
brew install messense/macos-cross-toolchains/armv7-unknown-linux-gnueabihf

# 验证安装
which aarch64-unknown-linux-gnu-gcc
which armv7-unknown-linux-gnueabihf-gcc
```

## 第三步：构建 OP-TEE 库

### 设置环境变量

创建环境配置脚本 `scripts/setup_optee_env.sh`：

```bash
#!/bin/bash
# OP-TEE 环境变量配置

export PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export OPTEE_DIR="${PROJECT_ROOT}/target/optee"
export TA_DEV_KIT_DIR="${OPTEE_DIR}/optee_os/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="${OPTEE_DIR}/optee_client/export_arm64"

export CROSS_COMPILE32="armv7-unknown-linux-gnueabihf-"
export CROSS_COMPILE64="aarch64-unknown-linux-gnu-"
export CROSS_COMPILE_TA="aarch64-unknown-linux-gnu-"
export CROSS_COMPILE_HOST="aarch64-unknown-linux-gnu-"

export TARGET_TA="aarch64-unknown-optee"
export TARGET_HOST="aarch64-unknown-linux-gnu"
export STD="y"

echo "✅ OP-TEE 环境变量已设置"
echo "OPTEE_DIR: $OPTEE_DIR"
echo "TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
echo "OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"
```

### 构建 OP-TEE 组件

```bash
# 加载环境变量
source scripts/setup_optee_env.sh

# 构建 OP-TEE OS 和 Client
cd third_party/incubator-teaclave-trustzone-sdk
./build_optee_libraries.sh "$OPTEE_DIR"

# 修复 macOS 特定问题：手动复制库文件
cp -r "$OPTEE_DIR/optee_client/out/libteec/"* "$OPTEE_CLIENT_EXPORT/usr/lib/"
```

## 第四步：安装 Rust 工具链

```bash
# 安装 xargo (用于 TA 构建)
cargo install xargo

# 添加 Rust 源码组件
rustup component add rust-src --toolchain nightly-2024-05-15-aarch64-apple-darwin

# 创建目标规范符号链接
mkdir -p ~/.rustup/toolchains/nightly-2024-05-15-aarch64-apple-darwin/lib/rustlib/aarch64-unknown-optee
ln -sf "$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk/aarch64-unknown-optee.json" \
       ~/.rustup/toolchains/nightly-2024-05-15-aarch64-apple-darwin/lib/rustlib/aarch64-unknown-optee/target.json
```

## 第五步：解决 Rust 依赖问题

### 创建必要的符号链接

```bash
cd third_party/incubator-teaclave-trustzone-sdk

# 创建 rust 目录并链接 libc
mkdir -p rust
ln -sf ~/.cargo/registry/src/index.crates.io-*/libc-0.2.* rust/libc
```

## 测试构建

### 1. 测试客户端应用构建

```bash
# 加载环境
source scripts/setup_optee_env.sh

# 测试 hello_world 客户端
cd third_party/incubator-teaclave-trustzone-sdk/examples/hello_world-rs/host
cargo build --target aarch64-unknown-linux-gnu --release

# 测试 eth_wallet 客户端  
cd ../../../projects/web3/eth_wallet/host
cargo build --target aarch64-unknown-linux-gnu --release
```

### 2. 测试 TA 构建 (高级)

```bash
# 进入 TA 目录
cd third_party/incubator-teaclave-trustzone-sdk/examples/hello_world-rs/ta

# 使用 build-std 构建 TA
TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR" \
cargo +nightly-2024-05-15 build \
--target "$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk/aarch64-unknown-optee.json" \
-Z build-std=core,alloc,std --release
```

## 验证脚本

创建验证脚本 `scripts/verify_optee_setup.sh`：

```bash
#!/bin/bash
set -e

source "$(dirname "$0")/setup_optee_env.sh"

echo "🔍 验证 OP-TEE 开发环境..."

# 检查交叉编译器
echo "检查交叉编译器..."
aarch64-unknown-linux-gnu-gcc --version > /dev/null || {
    echo "❌ ARM64 交叉编译器未找到"
    exit 1
}
armv7-unknown-linux-gnueabihf-gcc --version > /dev/null || {
    echo "❌ ARM32 交叉编译器未找到"
    exit 1
}

# 检查 OP-TEE 库
echo "检查 OP-TEE 库..."
test -f "$OPTEE_DIR/optee_os/out/arm-plat-vexpress/core/tee.elf" || {
    echo "❌ OP-TEE OS 未构建"
    exit 1
}
test -f "$OPTEE_CLIENT_EXPORT/usr/lib/libteec.so" || {
    echo "❌ OP-TEE Client 库未找到"
    exit 1
}

# 检查 Rust 工具链
echo "检查 Rust 工具链..."
command -v xargo > /dev/null || {
    echo "❌ xargo 未安装"
    exit 1
}

echo "✅ 所有检查通过！OP-TEE 开发环境就绪"
```

## 构建和测试脚本

### 完整构建脚本 `scripts/build_all.sh`

```bash
#!/bin/bash
set -e

source "$(dirname "$0")/setup_optee_env.sh"

echo "🚀 开始完整构建..."

# 构建 Mock 版本 (快速验证)
echo "构建 Mock 版本..."
cd "$PROJECT_ROOT/packages/mock-hello"
cargo build --release
cargo run --bin mock-ca test

# 构建客户端应用
echo "构建 OP-TEE 客户端应用..."
cd "$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk/examples/hello_world-rs/host"
cargo build --target aarch64-unknown-linux-gnu --release

# 可选：尝试构建 TA
echo "尝试构建 TA (可能失败)..."
cd ../ta
TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR" \
cargo +nightly-2024-05-15 build \
--target "$PROJECT_ROOT/third_party/incubator-teaclave-trustzone-sdk/aarch64-unknown-optee.json" \
-Z build-std=core,alloc,std --release || {
    echo "⚠️ TA 构建失败 - 这是已知问题，需要进一步解决 optee-utee-sys 兼容性"
}

echo "✅ 构建完成"
```

### 测试脚本 `scripts/test_all.sh`

```bash
#!/bin/bash
set -e

source "$(dirname "$0")/setup_optee_env.sh"

echo "🧪 运行所有测试..."

# 运行 Mock 测试
echo "运行 Mock TA-CA 通信测试..."
cd "$PROJECT_ROOT/packages/mock-hello"
cargo run --bin mock-ca test

# 运行核心逻辑测试
echo "运行核心逻辑测试..."
cd "$PROJECT_ROOT/packages/core-logic"
cargo test

# 运行集成测试
echo "运行集成测试..."
cd "$PROJECT_ROOT"
cargo test --workspace

echo "✅ 所有测试通过"
```

## 常见问题和解决方案

### 1. 交叉编译器未找到
```bash
# 检查 PATH
echo $PATH | grep -o '/opt/homebrew/bin'

# 重新安装工具链
brew uninstall messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
```

### 2. OP-TEE 构建失败
```bash
# 清理并重新构建
rm -rf target/optee
source scripts/setup_optee_env.sh
cd third_party/incubator-teaclave-trustzone-sdk
./build_optee_libraries.sh "$OPTEE_DIR"
```

### 3. TA 构建中的 std 依赖问题
这是 Teaclave SDK 的已知限制。当前的解决方案：
- 使用 Mock 版本进行开发和测试
- 等待 Teaclave SDK 上游修复
- 或者修改 optee-utee-sys 以支持 no_std

### 4. macOS 兼容性问题
```bash
# 如果遇到 GNU 特定命令问题
brew install gnu-sed
export PATH="/opt/homebrew/opt/gnu-sed/libexec/gnubin:$PATH"
```

## 开发工作流

### 日常开发流程

1. **启动开发环境**
   ```bash
   cd /path/to/AirAccount
   source scripts/setup_optee_env.sh
   ```

2. **快速验证**
   ```bash
   ./scripts/verify_optee_setup.sh
   ```

3. **开发和测试**
   ```bash
   # 在 Mock 环境中快速迭代
   cd packages/mock-hello
   cargo run --bin mock-ca interactive
   
   # 运行完整测试
   ./scripts/test_all.sh
   ```

4. **构建发布版本**
   ```bash
   ./scripts/build_all.sh
   ```

### 持续集成

对于 CI/CD 环境，创建 `.github/workflows/optee-build.yml`：

```yaml
name: OP-TEE Build Test

on: [push, pull_request]

jobs:
  build:
    runs-on: macos-latest
    
    steps:
    - uses: actions/checkout@v3
      with:
        submodules: recursive
        
    - name: Install dependencies
      run: |
        brew install automake coreutils curl gmp gnutls libtool libusb make wget
        pip3 install pyelftools
        
    - name: Install cross-compilers
      run: |
        brew tap messense/homebrew-macos-cross-toolchains
        brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
        brew install messense/macos-cross-toolchains/armv7-unknown-linux-gnueabihf
        
    - name: Setup OP-TEE environment
      run: |
        chmod +x scripts/setup_optee_env.sh
        source scripts/setup_optee_env.sh
        
    - name: Verify setup
      run: ./scripts/verify_optee_setup.sh
      
    - name: Run tests
      run: ./scripts/test_all.sh
```

## 总结

通过以上步骤，你将获得一个完全功能的 OP-TEE 开发环境，支持：

- ✅ 客户端应用开发和测试
- ✅ Mock TA-CA 通信开发
- ✅ 真实 OP-TEE 环境集成 (客户端)
- ⚠️ TA 开发 (需要解决 optee-utee-sys 问题)

环境搭建完成后，可以开始进行 AirAccount 的 TEE 应用开发工作。