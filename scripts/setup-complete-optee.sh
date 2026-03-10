#!/bin/bash

# 完整OP-TEE环境设置脚本
# 支持eth_wallet host和ta的真实构建

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# 配置环境
export OPTEE_DIR=/tmp/optee-env
export BUILD_DIR=${OPTEE_DIR}/build
export TOOLCHAINS_DIR=${OPTEE_DIR}/toolchains
export QEMU_DIR=${OPTEE_DIR}/qemu

# 检查依赖
check_dependencies() {
    log_step "检查系统依赖..."

    local deps=(git make python3 python3-pip curl wget)
    local missing=()

    for dep in "${deps[@]}"; do
        if ! command -v $dep >/dev/null 2>&1; then
            missing+=($dep)
        fi
    done

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "缺少依赖: ${missing[*]}"
        log_info "请安装: brew install ${missing[*]}"
        exit 1
    fi

    log_info "✅ 系统依赖检查通过"
}

# 创建工作目录
setup_directories() {
    log_step "创建工作目录..."

    mkdir -p $OPTEE_DIR
    mkdir -p $BUILD_DIR
    mkdir -p $TOOLCHAINS_DIR
    mkdir -p $QEMU_DIR

    log_info "✅ 工作目录创建完成"
}

# 克隆OP-TEE仓库
clone_optee_repos() {
    log_step "克隆OP-TEE仓库..."

    cd $OPTEE_DIR

    # OP-TEE Build (包含所有submodules)
    if [ ! -d "build" ]; then
        log_info "克隆OP-TEE Build系统..."
        git clone --depth 1 https://github.com/OP-TEE/build.git
    fi

    cd build

    # 初始化repo (OP-TEE使用repo管理多个git仓库)
    if [ ! -f ".repo/repo/main.py" ]; then
        log_info "初始化repo环境..."
        repo init -u https://github.com/OP-TEE/manifest.git -m qemu_v8.xml
        repo sync -j$(nproc)
    fi

    log_info "✅ OP-TEE仓库克隆完成"
}

# 构建工具链
build_toolchains() {
    log_step "构建交叉编译工具链..."

    cd $OPTEE_DIR/build

    log_info "开始构建工具链 (这可能需要30-60分钟)..."
    make -j$(nproc) toolchains

    log_info "✅ 工具链构建完成"
}

# 构建OP-TEE OS
build_optee_os() {
    log_step "构建OP-TEE OS..."

    cd $OPTEE_DIR/build

    log_info "构建OP-TEE OS和Client..."
    make -j$(nproc) -f qemu_v8.mk all

    # 验证构建输出
    local ta_dev_kit="optee_os/out/arm-plat-vexpress/export-ta_arm64"
    local client_export="optee_client/out/export_arm64"

    if [ -d "$ta_dev_kit" ] && [ -d "$client_export" ]; then
        log_info "✅ OP-TEE构建成功"
        log_info "  - TA开发套件: $ta_dev_kit"
        log_info "  - Client库: $client_export"
    else
        log_error "❌ OP-TEE构建失败"
        exit 1
    fi
}

# 设置环境变量
setup_environment() {
    log_step "配置环境变量..."

    local build_dir="$OPTEE_DIR/build"

    # 导出关键环境变量
    export TA_DEV_KIT_DIR="$build_dir/optee_os/out/arm-plat-vexpress/export-ta_arm64"
    export OPTEE_CLIENT_EXPORT="$build_dir/optee_client/out/export_arm64"
    export CROSS_COMPILE="$build_dir/toolchains/aarch64/bin/aarch64-linux-gnu-"
    export TEEC_EXPORT="$OPTEE_CLIENT_EXPORT"

    # 创建环境配置文件
    cat > $OPTEE_DIR/optee-env.sh << 'EOF'
#!/bin/bash
# OP-TEE环境配置文件

export OPTEE_DIR=/tmp/optee-env
export BUILD_DIR=${OPTEE_DIR}/build

export TA_DEV_KIT_DIR="$BUILD_DIR/optee_os/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="$BUILD_DIR/optee_client/out/export_arm64"
export CROSS_COMPILE="$BUILD_DIR/toolchains/aarch64/bin/aarch64-linux-gnu-"
export TEEC_EXPORT="$OPTEE_CLIENT_EXPORT"

# 添加到PATH
export PATH="$BUILD_DIR/toolchains/aarch64/bin:$PATH"

echo "🔧 OP-TEE环境已加载"
echo "   - TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
echo "   - OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"
echo "   - CROSS_COMPILE: $CROSS_COMPILE"
EOF

    chmod +x $OPTEE_DIR/optee-env.sh

    log_info "✅ 环境配置完成"
    log_info "使用 'source /tmp/optee-env/optee-env.sh' 加载环境"
}

# 测试构建环境
test_build_environment() {
    log_step "测试构建环境..."

    # 加载环境
    source $OPTEE_DIR/optee-env.sh

    # 测试交叉编译器
    if ${CROSS_COMPILE}gcc --version >/dev/null 2>&1; then
        log_info "✅ 交叉编译器工作正常"
    else
        log_error "❌ 交叉编译器不可用"
        exit 1
    fi

    # 测试TA开发套件
    if [ -f "$TA_DEV_KIT_DIR/scripts/sign_encrypt.py" ]; then
        log_info "✅ TA开发套件可用"
    else
        log_error "❌ TA开发套件不完整"
        exit 1
    fi

    # 测试Client库
    if [ -f "$OPTEE_CLIENT_EXPORT/lib/libteec.a" ]; then
        log_info "✅ OP-TEE Client库可用"
    else
        log_error "❌ OP-TEE Client库不可用"
        exit 1
    fi
}

# 构建eth_wallet
build_eth_wallet() {
    log_step "构建eth_wallet应用..."

    # 加载环境
    source $OPTEE_DIR/optee-env.sh

    cd $(pwd)/kms

    log_info "构建TA..."
    cd ta
    make clean
    make

    if [ -f "target/aarch64-unknown-optee/release/*.ta" ]; then
        log_info "✅ TA构建成功"
    else
        log_error "❌ TA构建失败"
        return 1
    fi

    log_info "构建Host应用..."
    cd ../host
    cargo clean
    cargo build --target aarch64-unknown-linux-gnu --release

    if [ -f "target/aarch64-unknown-linux-gnu/release/eth_wallet-rs" ]; then
        log_info "✅ Host应用构建成功"
    else
        log_error "❌ Host应用构建失败"
        return 1
    fi

    log_info "✅ eth_wallet构建完成!"
}

# 启动QEMU测试环境
start_qemu() {
    log_step "启动QEMU测试环境..."

    cd $OPTEE_DIR/build

    log_info "启动QEMU (使用Ctrl+A X退出)..."
    make -f qemu_v8.mk run
}

# 主函数
main() {
    echo "🚀 开始设置完整的OP-TEE环境"
    echo "================================"

    check_dependencies
    setup_directories
    clone_optee_repos
    build_toolchains
    build_optee_os
    setup_environment
    test_build_environment

    log_info "✅ OP-TEE环境设置完成!"
    echo ""
    echo "📋 下一步操作:"
    echo "1. 加载环境: source /tmp/optee-env/optee-env.sh"
    echo "2. 构建eth_wallet: ./scripts/setup-complete-optee.sh build"
    echo "3. 启动QEMU测试: ./scripts/setup-complete-optee.sh qemu"
    echo ""

    if [ "$1" = "build" ]; then
        build_eth_wallet
    elif [ "$1" = "qemu" ]; then
        start_qemu
    fi
}

# 执行主函数
main "$@"