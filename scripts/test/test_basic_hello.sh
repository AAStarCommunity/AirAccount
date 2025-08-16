#!/bin/bash

# Test script for basic hello world TA-CA communication
# Following the pattern from eth_wallet testing

set -e

# Load common functions
source "$(dirname "$0")/lib/common.sh"

init_script "AirAccount Basic Hello World Test"

log_info "开始基础 Hello World 测试"

# Build the basic components
log_info "构建 TA 和 CA 组件..."
cd packages/basic-hello
make clean
make all

if [ $? -eq 0 ]; then
    log_success "构建成功"
else
    handle_error "构建失败" 1
fi

# Check if we're in QEMU environment for testing
if [ "$OPTEE_ENV" = "qemu" ]; then
    log_info "在 QEMU 环境中运行测试..."
    make test
    
    if [ $? -eq 0 ]; then
        log_success "基础测试通过"
    else
        handle_error "测试失败" 2
    fi
else
    log_warning "未检测到 QEMU 环境，跳过运行测试"
    log_info "如需运行测试，请在 QEMU 环境中执行"
fi

log_success "基础 Hello World 架构验证完成"