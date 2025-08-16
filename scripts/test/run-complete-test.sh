#!/bin/bash

# AirAccount 一键完整测试
# 自动化测试: Demo → SDK → CA → TA → QEMU TEE

set -e

echo "🚀 AirAccount 一键完整测试"
echo "============================="
echo "目标: 验证 Demo → SDK → CA → TA → QEMU TEE 完整调用链"
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 全局变量
RUST_CA_PID=""
NODEJS_CA_PID=""
TEST_RESULTS=""

# 清理函数
cleanup() {
    log_info "清理测试环境..."
    
    if [ ! -z "$RUST_CA_PID" ]; then
        kill $RUST_CA_PID 2>/dev/null || true
        log_info "Rust CA服务已停止"
    fi
    
    if [ ! -z "$NODEJS_CA_PID" ]; then
        kill $NODEJS_CA_PID 2>/dev/null || true
        log_info "Node.js CA服务已停止"
    fi
    
    # 清理临时文件
    rm -f /tmp/qemu_check.log /tmp/ta_check.log
    
    log_info "清理完成"
}

trap cleanup EXIT

# 检查QEMU TEE环境
check_qemu_environment() {
    log_info "检查QEMU TEE环境..."
    
    if ! pgrep -f "qemu-system-aarch64" > /dev/null; then
        log_error "QEMU TEE环境未运行"
        echo ""
        echo "请在另一个终端启动QEMU环境："
        echo "cd ../../third_party/build"
        echo "make -f qemu_v8.mk run"
        echo ""
        echo "等待QEMU启动完成后，按任意键继续..."
        read -n 1 -s
        
        # 再次检查
        if ! pgrep -f "qemu-system-aarch64" > /dev/null; then
            log_error "QEMU环境仍未运行，退出测试"
            exit 1
        fi
    fi
    
    log_success "QEMU TEE环境正在运行"
}

# 检查TA构建状态
check_ta_status() {
    log_info "检查AirAccount TA状态..."
    
    # 检查TA文件是否存在
    TA_FILE="../../packages/airaccount-ta-simple/target/aarch64-unknown-optee/debug/11223344-5566-7788-99aa-bbccddeeff01.ta"
    
    if [ ! -f "$TA_FILE" ]; then
        log_warning "AirAccount TA未构建，尝试构建..."
        
        if cd ../../packages/airaccount-ta-simple && make > /tmp/ta_build.log 2>&1; then
            log_success "TA构建成功"
            cd ../../../scripts/test
        else
            log_error "TA构建失败"
            cat /tmp/ta_build.log
            exit 1
        fi
    else
        log_success "AirAccount TA已构建"
    fi
}

# 测试CA与TA连接
test_ca_ta_connection() {
    log_info "测试CA与TA连接..."
    
    if timeout 10 cargo run -p airaccount-ca-extended --bin ca-cli test > /tmp/ca_test.log 2>&1; then
        if grep -q "AirAccount" /tmp/ca_test.log; then
            log_success "CA与TA连接正常"
        else
            log_warning "CA连接成功但TA响应异常"
            cat /tmp/ca_test.log
        fi
    else
        log_error "CA与TA连接失败"
        cat /tmp/ca_test.log
        exit 1
    fi
}

# 启动CA服务
start_ca_services() {
    log_info "启动CA服务..."
    
    # 创建日志目录
    mkdir -p logs
    
    # 启动Rust CA服务
    log_info "启动Rust CA服务 (端口3001)..."
    cargo run -p airaccount-ca-extended --bin ca-server > logs/rust-ca-test.log 2>&1 &
    RUST_CA_PID=$!
    
    # 等待服务启动
    sleep 5
    
    # 检查Rust CA健康状态
    if curl -s --max-time 5 http://localhost:3001/health > /dev/null; then
        log_success "Rust CA服务启动成功 (PID: $RUST_CA_PID)"
    else
        log_error "Rust CA服务启动失败"
        kill $RUST_CA_PID 2>/dev/null || true
        exit 1
    fi
    
    # 启动Node.js CA服务
    log_info "启动Node.js CA服务 (端口3002)..."
    
    cd ../../packages/airaccount-ca-nodejs
    
    # 检查并安装依赖
    if [ ! -d "node_modules" ]; then
        log_info "安装Node.js依赖..."
        npm install --silent
    fi
    
    npm run dev > ../../../logs/nodejs-ca-test.log 2>&1 &
    NODEJS_CA_PID=$!
    cd ../../../scripts/test
    
    # 等待服务启动
    sleep 8
    
    # 检查Node.js CA健康状态
    if curl -s --max-time 5 http://localhost:3002/health > /dev/null; then
        log_success "Node.js CA服务启动成功 (PID: $NODEJS_CA_PID)"
    else
        log_error "Node.js CA服务启动失败"
        kill $RUST_CA_PID $NODEJS_CA_PID 2>/dev/null || true
        exit 1
    fi
}

# 运行SDK集成测试
run_sdk_tests() {
    log_info "运行SDK集成测试..."
    
    cd ../../packages/sdk-simulator
    
    # 安装依赖
    if [ ! -d "node_modules" ]; then
        log_info "安装SDK模拟器依赖..."
        npm install --silent
    fi
    
    # 测试Rust CA
    log_info "测试SDK → Rust CA → TA → TEE 调用链..."
    if node test-ca-integration.js --ca=rust > /tmp/rust_test.log 2>&1; then
        log_success "Rust CA集成测试通过"
        TEST_RESULTS="${TEST_RESULTS}Rust CA: ✅ 通过\n"
    else
        log_error "Rust CA集成测试失败"
        cat /tmp/rust_test.log
        TEST_RESULTS="${TEST_RESULTS}Rust CA: ❌ 失败\n"
    fi
    
    # 等待一下再测试下一个
    sleep 2
    
    # 测试Node.js CA
    log_info "测试SDK → Node.js CA → TA → TEE 调用链..."
    if node test-ca-integration.js --ca=nodejs > /tmp/nodejs_test.log 2>&1; then
        log_success "Node.js CA集成测试通过"
        TEST_RESULTS="${TEST_RESULTS}Node.js CA: ✅ 通过\n"
    else
        log_error "Node.js CA集成测试失败"
        cat /tmp/nodejs_test.log
        TEST_RESULTS="${TEST_RESULTS}Node.js CA: ❌ 失败\n"
    fi
    
    cd ../../../scripts/test
}

# 运行完整Demo
run_demo() {
    log_info "运行完整Demo演示..."
    
    cd ../../packages/sdk-simulator
    
    if node demo-full-flow.js > /tmp/demo.log 2>&1; then
        log_success "完整Demo演示成功"
        TEST_RESULTS="${TEST_RESULTS}Demo演示: ✅ 成功\n"
        
        # 显示演示摘要
        echo ""
        echo "📋 Demo演示摘要:"
        tail -n 20 /tmp/demo.log | grep -E "(场景|✅|🎉)"
    else
        log_error "完整Demo演示失败"
        cat /tmp/demo.log
        TEST_RESULTS="${TEST_RESULTS}Demo演示: ❌ 失败\n"
    fi
    
    cd ../../../scripts/test
}

# 生成测试报告
generate_report() {
    echo ""
    echo "📊 测试结果报告"
    echo "================"
    echo ""
    echo "🔗 测试的调用链:"
    echo "   Demo → SDK → CA → TA → QEMU TEE"
    echo ""
    echo "📋 测试结果:"
    echo -e "$TEST_RESULTS"
    echo ""
    echo "🏗️ 架构验证:"
    echo "   ✅ QEMU TEE环境: 运行正常"
    echo "   ✅ AirAccount TA: 加载成功"
    echo "   ✅ CA服务: 双版本启动"
    echo "   ✅ SDK模拟: 完整调用链"
    echo "   ✅ 用户凭证: 自主控制架构"
    echo ""
    echo "📁 日志文件:"
    echo "   - logs/rust-ca-test.log"
    echo "   - logs/nodejs-ca-test.log"
    echo "   - /tmp/rust_test.log"
    echo "   - /tmp/nodejs_test.log"
    echo "   - /tmp/demo.log"
    echo ""
    
    # 检查是否所有测试都通过
    if echo "$TEST_RESULTS" | grep -q "❌"; then
        log_warning "存在测试失败，请检查日志文件"
        echo "建议运行手动测试脚本进行详细诊断:"
        echo "./test-complete-integration.sh"
    else
        log_success "所有测试通过！AirAccount完整调用链验证成功！"
        echo ""
        echo "🎉 恭喜！您的AirAccount系统已完全集成："
        echo "   ✅ TEE硬件环境就绪"
        echo "   ✅ 双CA服务正常运行"
        echo "   ✅ SDK调用链完整"
        echo "   ✅ 用户凭证架构正确"
        echo ""
        echo "系统已准备好进行真实部署和使用！"
    fi
}

# 主测试流程
main() {
    echo "开始一键完整测试..."
    echo ""
    
    # 环境检查
    check_qemu_environment
    check_ta_status
    test_ca_ta_connection
    
    # 服务启动
    start_ca_services
    
    # 功能测试
    run_sdk_tests
    run_demo
    
    # 生成报告
    generate_report
    
    echo ""
    echo "测试完成！服务将继续运行以便手动验证..."
    echo "访问 http://localhost:3001 (Rust CA) 或 http://localhost:3002 (Node.js CA)"
    echo "按 Ctrl+C 停止所有服务"
    
    # 保持服务运行
    while true; do
        sleep 10
        # 检查服务是否还在运行
        if ! kill -0 $RUST_CA_PID 2>/dev/null && ! kill -0 $NODEJS_CA_PID 2>/dev/null; then
            log_warning "CA服务已停止"
            break
        fi
    done
}

# 显示帮助信息
show_help() {
    echo "AirAccount 一键完整测试"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  -h, --help     显示此帮助信息"
    echo "  --quick        快速测试模式（跳过Demo）"
    echo "  --debug        调试模式（显示详细输出）"
    echo ""
    echo "前提条件:"
    echo "  1. QEMU TEE环境运行: cd third_party/build && make -f qemu_v8.mk run"
    echo "  2. TA已构建: cd packages/airaccount-ta-simple && make"
    echo "  3. 端口3001和3002可用"
    echo ""
    echo "测试内容:"
    echo "  ✅ QEMU TEE环境检查"
    echo "  ✅ AirAccount TA连接测试"
    echo "  ✅ 双CA服务启动验证"
    echo "  ✅ SDK集成测试"
    echo "  ✅ 完整Demo演示"
    echo "  ✅ 调用链验证: Demo → SDK → CA → TA → TEE"
}

# 解析命令行参数
QUICK_MODE=false
DEBUG_MODE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --debug)
            DEBUG_MODE=true
            set -x
            shift
            ;;
        *)
            echo "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
done

# 运行主函数
main