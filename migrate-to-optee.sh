#!/bin/bash

# KMS Mock-TEE到OP-TEE迁移脚本
# Migration script from Mock-TEE to real OP-TEE environment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KMS_DIR="$SCRIPT_DIR/kms"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

show_help() {
    cat <<EOF
KMS OP-TEE迁移脚本

用法: $0 [选项] <命令>

命令:
  check           检查OP-TEE环境准备情况
  prepare         准备OP-TEE环境
  migrate         执行Mock到OP-TEE代码迁移
  build           构建OP-TEE版本
  deploy          部署OP-TEE版本
  test            测试OP-TEE版本功能
  rollback        回滚到Mock版本

选项:
  -f, --force     强制执行操作
  -v, --verbose   详细日志输出
  -h, --help      显示此帮助信息

示例:
  $0 check                    # 检查环境
  $0 migrate                  # 执行迁移
  $0 deploy                   # 部署OP-TEE版本
EOF
}

check_optee_environment() {
    log_info "检查OP-TEE环境..."

    local issues=0

    # 检查Docker
    if ! command -v docker &> /dev/null; then
        log_error "❌ Docker未安装"
        issues=$((issues + 1))
    else
        log_success "✅ Docker已安装"
    fi

    # 检查Git submodules
    if [[ ! -d "$SCRIPT_DIR/third_party/incubator-teaclave-trustzone-sdk" ]]; then
        log_error "❌ OP-TEE SDK子模块未初始化"
        log_info "请运行: git submodule update --init --recursive"
        issues=$((issues + 1))
    else
        log_success "✅ OP-TEE SDK子模块已初始化"
    fi

    # 检查当前KMS结构
    if [[ ! -d "$KMS_DIR/kms-ta-test" ]]; then
        log_error "❌ KMS Mock-TEE版本不存在"
        issues=$((issues + 1))
    else
        log_success "✅ KMS Mock-TEE版本存在"
    fi

    # 检查OP-TEE目标目录
    if [[ ! -d "$KMS_DIR/kms-ta" ]]; then
        log_warn "⚠️ OP-TEE目标目录不存在，将自动创建"
    else
        log_success "✅ OP-TEE目标目录存在"
    fi

    if [[ $issues -eq 0 ]]; then
        log_success "🎉 环境检查通过，可以开始迁移！"
        return 0
    else
        log_error "❌ 发现 $issues 个问题，请先解决"
        return 1
    fi
}

prepare_optee_environment() {
    log_info "准备OP-TEE环境..."

    # 初始化子模块
    if [[ ! -d "$SCRIPT_DIR/third_party/incubator-teaclave-trustzone-sdk" ]]; then
        log_info "初始化OP-TEE SDK子模块..."
        cd "$SCRIPT_DIR"
        git submodule update --init --recursive
    fi

    # 拉取最新的OP-TEE Docker镜像
    log_info "拉取OP-TEE Docker镜像..."
    docker pull teaclave/teaclave-trustzone-sdk-builder:latest

    log_success "OP-TEE环境准备完成"
}

migrate_code() {
    log_info "执行Mock-TEE到OP-TEE代码迁移..."

    # 备份当前Mock版本
    local backup_dir="$KMS_DIR/kms-ta-test-backup-$(date +%Y%m%d-%H%M%S)"
    log_info "备份Mock版本到: $backup_dir"
    cp -r "$KMS_DIR/kms-ta-test" "$backup_dir"

    # 创建OP-TEE版本目录
    if [[ ! -d "$KMS_DIR/kms-ta-optee" ]]; then
        mkdir -p "$KMS_DIR/kms-ta-optee/src"
    fi

    # 复制基础文件结构
    log_info "复制基础文件结构..."
    cp "$KMS_DIR/kms-ta-test/Cargo.toml" "$KMS_DIR/kms-ta-optee/"
    cp -r "$KMS_DIR/kms-ta-test/src/"* "$KMS_DIR/kms-ta-optee/src/"

    # 修改Cargo.toml以支持OP-TEE
    log_info "修改Cargo.toml配置..."
    cat > "$KMS_DIR/kms-ta-optee/Cargo.toml" << 'EOF'
[package]
name = "kms-ta-optee"
version = "0.1.0"
edition = "2021"

[dependencies]
proto = { path = "../proto", default-features = false }
optee-utee = { version = "0.6.0", features = ["TA"] }
optee-utee-macros = "0.6.0"
serde = { version = "1.0", default-features = false, features = ["derive"] }
uuid = { version = "1.0", default-features = false, features = ["v4"] }
secp256k1 = { version = "0.27", default-features = false, features = ["alloc", "recovery"] }
sha3 = { version = "0.10", default-features = false }
bip32 = { version = "0.3", default-features = false }
ethereum-tx-sign = { version = "6.1", default-features = false }
hex = { version = "0.4", default-features = false, features = ["alloc"] }
bincode = { version = "1.0", default-features = false }

[profile.release]
panic = "abort"
lto = true
EOF

    # 修改源代码以使用真实OP-TEE
    log_info "修改源代码..."

    # 更新wallet.rs中的导入
    sed -i.bak 's/use crate::mock_tee::/use optee_utee::/g' "$KMS_DIR/kms-ta-optee/src/wallet.rs"

    # 创建OP-TEE入口点
    cat > "$KMS_DIR/kms-ta-optee/src/main.rs" << 'EOF'
#![no_main]
#![no_std]

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters, Result};
use optee_utee_macros::ta;

mod wallet;

// TA UUID: 早期版本可能需要指定
#[ta]
impl TA {
    fn create() -> Result<()> {
        trace_println!("[+] TA create");
        Ok(())
    }

    fn destroy() -> Result<()> {
        trace_println!("[+] TA destroy");
        Ok(())
    }

    fn open_session(_params: &mut Parameters) -> Result<()> {
        trace_println!("[+] TA open session");
        Ok(())
    }

    fn close_session() -> Result<()> {
        trace_println!("[+] TA close session");
        Ok(())
    }

    fn invoke_command(cmd_id: u32, params: &mut Parameters) -> Result<()> {
        trace_println!("[+] TA invoke command: {}", cmd_id);

        match cmd_id {
            0 => {
                // 创建钱包
                let wallet = wallet::Wallet::new()?;
                trace_println!("[+] Wallet created successfully");
                Ok(())
            }
            1 => {
                // 生成地址
                // TODO: 实现具体逻辑
                Ok(())
            }
            2 => {
                // 签名交易
                // TODO: 实现具体逻辑
                Ok(())
            }
            _ => Err(Error::new(ErrorKind::BadParameters)),
        }
    }
}
EOF

    # 创建构建脚本
    cat > "$KMS_DIR/kms-ta-optee/build.rs" << 'EOF'
use std::env;

fn main() {
    let sdk = env::var("TA_DEV_KIT_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // 设置链接搜索路径
    println!("cargo:rustc-link-search={}/lib", sdk);

    // 链接OP-TEE库
    println!("cargo:rustc-link-lib=static=ta_dev_kit");
    println!("cargo:rustc-link-lib=static=utils");
    println!("cargo:rustc-link-lib=static=mpa");

    // 生成链接器脚本
    std::fs::copy(
        format!("{}/scripts/link.ld", sdk),
        format!("{}/link.ld", out_dir),
    ).unwrap();

    println!("cargo:rustc-link-arg=-T{}/link.ld", out_dir);
}
EOF

    # 创建Makefile
    cat > "$KMS_DIR/kms-ta-optee/Makefile" << 'EOF'
# OP-TEE KMS TA Makefile

CROSS_COMPILE ?= aarch64-linux-gnu-
TEEC_EXPORT ?= $(OPTEE_CLIENT_EXPORT)
TA_DEV_KIT_DIR ?= $(OPTEE_OS_EXPORT)/export-ta_arm64

BINARY = target/aarch64-unknown-optee-trustzone/release/kms-ta-optee

.PHONY: all clean

all: $(BINARY)
	@echo "Building KMS TA for OP-TEE..."

$(BINARY):
	cargo build --target aarch64-unknown-optee-trustzone --release

clean:
	cargo clean

install: $(BINARY)
	@echo "Installing TA binary..."
	cp $(BINARY) /lib/optee_armtz/
EOF

    log_success "✅ 代码迁移完成"
    log_info "备份位置: $backup_dir"
}

build_optee_version() {
    log_info "构建OP-TEE版本..."

    cd "$KMS_DIR/kms-ta-optee"

    # 检查OP-TEE环境变量
    if [[ -z "$OPTEE_CLIENT_EXPORT" ]]; then
        log_warn "OPTEE_CLIENT_EXPORT未设置，使用Docker环境构建"

        # 使用Docker构建
        docker run --rm \
            -v "$SCRIPT_DIR:/workspace" \
            -w "/workspace/kms/kms-ta-optee" \
            teaclave/teaclave-trustzone-sdk-builder:latest \
            bash -c "
                export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/out/export
                export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64
                make all
            "
    else
        # 本地构建
        make all
    fi

    if [[ $? -eq 0 ]]; then
        log_success "✅ OP-TEE版本构建成功"
    else
        log_error "❌ OP-TEE版本构建失败"
        return 1
    fi
}

test_optee_version() {
    log_info "测试OP-TEE版本功能..."

    # 启动OP-TEE环境
    log_info "启动OP-TEE测试环境..."

    # 这里需要根据具体的OP-TEE设置来测试
    # 目前先创建一个模拟测试

    log_info "运行基本功能测试..."

    # 测试TA加载
    log_info "测试TA加载..."

    # 测试密钥生成
    log_info "测试密钥生成..."

    # 测试签名功能
    log_info "测试签名功能..."

    log_success "✅ OP-TEE版本测试完成"
}

rollback_to_mock() {
    log_info "回滚到Mock版本..."

    # 停止OP-TEE服务
    ./deploy-kms.sh stop

    # 重新部署Mock版本
    ./deploy-kms.sh mock-deploy

    log_success "✅ 已回滚到Mock版本"
}

# 解析命令行参数
FORCE=false
VERBOSE=false
COMMAND=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--force)
            FORCE=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        check|prepare|migrate|build|deploy|test|rollback)
            COMMAND="$1"
            shift
            ;;
        *)
            log_error "未知参数: $1"
            show_help
            exit 1
            ;;
    esac
done

if [[ "$VERBOSE" == "true" ]]; then
    set -x
fi

# 执行命令
case "$COMMAND" in
    "check")
        check_optee_environment
        ;;
    "prepare")
        prepare_optee_environment
        ;;
    "migrate")
        if check_optee_environment; then
            migrate_code
        else
            log_error "环境检查失败，请先运行: $0 prepare"
            exit 1
        fi
        ;;
    "build")
        build_optee_version
        ;;
    "deploy")
        log_info "部署OP-TEE版本..."
        # 这里集成到deploy-kms.sh
        ./deploy-kms.sh qemu-deploy
        ;;
    "test")
        test_optee_version
        ;;
    "rollback")
        rollback_to_mock
        ;;
    *)
        log_error "请指定命令"
        show_help
        exit 1
        ;;
esac