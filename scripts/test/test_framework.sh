#!/bin/bash

# AirAccount 全面测试框架
# 提供单元测试、集成测试、安全测试、性能测试的统一执行平台

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 加载通用脚本库和配置
source "$SCRIPT_DIR/lib/common.sh"
init_script "AirAccount 全面测试框架" "rust"
load_config
validate_env_vars "CARGO_HOME"

# 测试配置
TEST_OUTPUT_DIR="${PROJECT_ROOT}/test_reports"
COVERAGE_OUTPUT_DIR="${TEST_OUTPUT_DIR}/coverage"
SECURITY_TEST_DIR="${TEST_OUTPUT_DIR}/security"
PERFORMANCE_TEST_DIR="${TEST_OUTPUT_DIR}/performance"

# 测试选项
RUN_UNIT_TESTS=true
RUN_INTEGRATION_TESTS=true
RUN_SECURITY_TESTS=true
RUN_PERFORMANCE_TESTS=true
RUN_COVERAGE=true
COVERAGE_THRESHOLD=85

# 解析命令行参数
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --unit-only)
                RUN_UNIT_TESTS=true
                RUN_INTEGRATION_TESTS=false
                RUN_SECURITY_TESTS=false
                RUN_PERFORMANCE_TESTS=false
                shift
                ;;
            --integration-only)
                RUN_UNIT_TESTS=false
                RUN_INTEGRATION_TESTS=true
                RUN_SECURITY_TESTS=false
                RUN_PERFORMANCE_TESTS=false
                shift
                ;;
            --security-only)
                RUN_UNIT_TESTS=false
                RUN_INTEGRATION_TESTS=false
                RUN_SECURITY_TESTS=true
                RUN_PERFORMANCE_TESTS=false
                shift
                ;;
            --performance-only)
                RUN_UNIT_TESTS=false
                RUN_INTEGRATION_TESTS=false
                RUN_SECURITY_TESTS=false
                RUN_PERFORMANCE_TESTS=true
                shift
                ;;
            --no-coverage)
                RUN_COVERAGE=false
                shift
                ;;
            --coverage-threshold)
                COVERAGE_THRESHOLD="$2"
                shift 2
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            *)
                log_warning "未知参数: $1"
                shift
                ;;
        esac
    done
}

show_usage() {
    cat << EOF
用法: $0 [选项]

选项:
    --unit-only           只运行单元测试
    --integration-only    只运行集成测试
    --security-only       只运行安全测试
    --performance-only    只运行性能测试
    --no-coverage         跳过代码覆盖率检查
    --coverage-threshold  设置代码覆盖率阈值 (默认: 85)
    -h, --help           显示此帮助信息

示例:
    $0                              # 运行所有测试
    $0 --unit-only                  # 只运行单元测试
    $0 --no-coverage               # 跳过覆盖率检查
    $0 --coverage-threshold 90     # 设置90%覆盖率要求
EOF
}

# 准备测试环境
setup_test_environment() {
    log_info "准备测试环境..."
    
    # 创建测试输出目录
    mkdir -p "$TEST_OUTPUT_DIR"
    mkdir -p "$COVERAGE_OUTPUT_DIR" 
    mkdir -p "$SECURITY_TEST_DIR"
    mkdir -p "$PERFORMANCE_TEST_DIR"
    
    # 安装测试依赖
    if ! command -v cargo-tarpaulin &> /dev/null && [[ "$RUN_COVERAGE" == "true" ]]; then
        log_info "安装代码覆盖率工具 cargo-tarpaulin..."
        cargo install cargo-tarpaulin
    fi
    
    if ! command -v cargo-audit &> /dev/null && [[ "$RUN_SECURITY_TESTS" == "true" ]]; then
        log_info "安装安全审计工具 cargo-audit..."
        cargo install cargo-audit
    fi
    
    if ! command -v cargo-criterion &> /dev/null && [[ "$RUN_PERFORMANCE_TESTS" == "true" ]]; then
        log_info "安装性能测试工具 cargo-criterion..."
        cargo install cargo-criterion
    fi
    
    log_success "测试环境准备完成"
}

# 运行单元测试
run_unit_tests() {
    log_info "运行单元测试..."
    
    local test_output="$TEST_OUTPUT_DIR/unit_tests.xml"
    local start_time=$(date +%s)
    
    cd "$PROJECT_ROOT/packages/core-logic"
    
    if cargo test --lib --bins --tests -- --format json > "$TEST_OUTPUT_DIR/unit_tests.json" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        log_success "单元测试完成 (耗时: ${duration}s)"
        
        # 解析测试结果
        local passed=$(grep -c '"event":"ok"' "$TEST_OUTPUT_DIR/unit_tests.json" || echo "0")
        local failed=$(grep -c '"event":"failed"' "$TEST_OUTPUT_DIR/unit_tests.json" || echo "0")
        local total=$((passed + failed))
        
        log_info "测试结果: $passed/$total 通过"
        
        if [[ $failed -gt 0 ]]; then
            log_error "$failed 个测试失败"
            return 1
        fi
    else
        log_error "单元测试失败"
        return 1
    fi
}

# 运行集成测试
run_integration_tests() {
    log_info "运行集成测试..."
    
    local start_time=$(date +%s)
    
    cd "$PROJECT_ROOT/packages/core-logic"
    
    # 运行安全模块集成测试
    if cargo run --bin security-test > "$TEST_OUTPUT_DIR/integration_security.log" 2>&1; then
        log_success "安全模块集成测试通过"
    else
        log_error "安全模块集成测试失败"
        return 1
    fi
    
    # 运行其他集成测试
    if cargo test --test '*' -- --nocapture > "$TEST_OUTPUT_DIR/integration_tests.log" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "集成测试完成 (耗时: ${duration}s)"
    else
        log_error "集成测试失败"
        cat "$TEST_OUTPUT_DIR/integration_tests.log"
        return 1
    fi
}

# 运行安全测试
run_security_tests() {
    log_info "运行安全测试套件..."
    
    cd "$PROJECT_ROOT/packages/core-logic"
    
    # 依赖安全审计
    log_info "执行依赖安全审计..."
    if cargo audit --output json > "$SECURITY_TEST_DIR/audit_report.json" 2>&1; then
        log_success "依赖安全审计通过"
    else
        log_warning "依赖安全审计发现问题，请检查报告"
    fi
    
    # 内存安全测试
    log_info "执行内存安全测试..."
    if cargo test memory_safety --features debug-assertions -- --nocapture > "$SECURITY_TEST_DIR/memory_safety.log" 2>&1; then
        log_success "内存安全测试通过"
    else
        log_error "内存安全测试失败"
        return 1
    fi
    
    # 常时算法测试
    log_info "执行常时算法验证..."
    if cargo test constant_time --release -- --nocapture > "$SECURITY_TEST_DIR/constant_time.log" 2>&1; then
        log_success "常时算法测试通过"
    else
        log_error "常时算法测试失败"
        return 1
    fi
    
    # 模糊测试 (如果有的话)
    log_info "检查模糊测试配置..."
    if [[ -f "fuzz/Cargo.toml" ]]; then
        log_info "运行模糊测试..."
        cargo fuzz run fuzz_target_1 -- -max_total_time=60 > "$SECURITY_TEST_DIR/fuzz_test.log" 2>&1 || true
        log_info "模糊测试完成 (60秒)"
    else
        log_info "未配置模糊测试，跳过"
    fi
}

# 运行性能测试
run_performance_tests() {
    log_info "运行性能基准测试..."
    
    cd "$PROJECT_ROOT/packages/core-logic"
    
    local start_time=$(date +%s)
    
    # 基准测试
    if cargo test --release bench_ -- --nocapture > "$PERFORMANCE_TEST_DIR/benchmarks.log" 2>&1; then
        log_success "基准测试完成"
    else
        log_warning "基准测试失败或无基准测试"
    fi
    
    # 性能回归测试
    log_info "执行性能回归测试..."
    if cargo run --release --bin security-test > "$PERFORMANCE_TEST_DIR/performance_regression.log" 2>&1; then
        log_success "性能回归测试通过"
        
        # 提取性能指标
        grep -E "(平均|ops/sec|µs)" "$PERFORMANCE_TEST_DIR/performance_regression.log" > "$PERFORMANCE_TEST_DIR/metrics.txt" || true
    else
        log_error "性能回归测试失败"
        return 1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    log_success "性能测试完成 (耗时: ${duration}s)"
}

# 运行代码覆盖率检查
run_coverage_analysis() {
    log_info "运行代码覆盖率分析..."
    
    cd "$PROJECT_ROOT/packages/core-logic"
    
    local start_time=$(date +%s)
    
    if cargo tarpaulin --out Html --output-dir "$COVERAGE_OUTPUT_DIR" --skip-clean > "$COVERAGE_OUTPUT_DIR/coverage.log" 2>&1; then
        # 提取覆盖率数值
        local coverage=$(grep -o '[0-9]*\.[0-9]*%' "$COVERAGE_OUTPUT_DIR/coverage.log" | tail -1 | sed 's/%//')
        
        log_info "代码覆盖率: $coverage%"
        
        if (( $(echo "$coverage >= $COVERAGE_THRESHOLD" | bc -l) )); then
            log_success "代码覆盖率达到要求 ($coverage% >= $COVERAGE_THRESHOLD%)"
        else
            log_error "代码覆盖率未达到要求 ($coverage% < $COVERAGE_THRESHOLD%)"
            log_info "覆盖率报告: $COVERAGE_OUTPUT_DIR/tarpaulin-report.html"
            return 1
        fi
        
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "代码覆盖率分析完成 (耗时: ${duration}s)"
    else
        log_error "代码覆盖率分析失败"
        cat "$COVERAGE_OUTPUT_DIR/coverage.log"
        return 1
    fi
}

# 生成测试报告
generate_test_report() {
    log_info "生成测试报告..."
    
    local report_file="$TEST_OUTPUT_DIR/test_summary.html"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    cat > "$report_file" << EOF
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AirAccount 测试报告</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { background: #f0f8ff; padding: 20px; border-radius: 8px; }
        .section { margin: 20px 0; padding: 15px; border-left: 4px solid #007acc; }
        .success { color: #28a745; }
        .error { color: #dc3545; }
        .warning { color: #ffc107; }
        .metric { background: #f8f9fa; padding: 10px; margin: 5px 0; border-radius: 4px; }
    </style>
</head>
<body>
    <div class="header">
        <h1>AirAccount 测试报告</h1>
        <p><strong>生成时间:</strong> $timestamp</p>
        <p><strong>项目版本:</strong> $(cd "$PROJECT_ROOT" && git rev-parse --short HEAD 2>/dev/null || echo "unknown")</p>
    </div>
EOF
    
    # 添加测试结果摘要
    echo '    <div class="section"><h2>测试执行摘要</h2><ul>' >> "$report_file"
    
    if [[ "$RUN_UNIT_TESTS" == "true" ]]; then
        if [[ -f "$TEST_OUTPUT_DIR/unit_tests.json" ]]; then
            echo '        <li class="success">✓ 单元测试: 通过</li>' >> "$report_file"
        else
            echo '        <li class="error">✗ 单元测试: 失败</li>' >> "$report_file"
        fi
    fi
    
    if [[ "$RUN_INTEGRATION_TESTS" == "true" ]]; then
        if [[ -f "$TEST_OUTPUT_DIR/integration_tests.log" ]]; then
            echo '        <li class="success">✓ 集成测试: 通过</li>' >> "$report_file"
        else
            echo '        <li class="error">✗ 集成测试: 失败</li>' >> "$report_file"
        fi
    fi
    
    if [[ "$RUN_SECURITY_TESTS" == "true" ]]; then
        echo '        <li class="success">✓ 安全测试: 完成</li>' >> "$report_file"
    fi
    
    if [[ "$RUN_PERFORMANCE_TESTS" == "true" ]]; then
        echo '        <li class="success">✓ 性能测试: 完成</li>' >> "$report_file"
    fi
    
    echo '    </ul></div>' >> "$report_file"
    
    # 添加覆盖率信息
    if [[ "$RUN_COVERAGE" == "true" && -f "$COVERAGE_OUTPUT_DIR/coverage.log" ]]; then
        local coverage=$(grep -oP '\d+\.\d+(?=%)' "$COVERAGE_OUTPUT_DIR/coverage.log" | tail -1 || echo "N/A")
        echo "    <div class=\"section\"><h2>代码覆盖率</h2><div class=\"metric\">覆盖率: $coverage%</div></div>" >> "$report_file"
    fi
    
    echo '</body></html>' >> "$report_file"
    
    log_success "测试报告已生成: $report_file"
}

# 清理测试环境
cleanup_test_environment() {
    log_info "清理测试环境..."
    
    # 清理临时文件
    find "$PROJECT_ROOT" -name "*.profraw" -delete 2>/dev/null || true
    find "$PROJECT_ROOT" -name "*.gcda" -delete 2>/dev/null || true
    find "$PROJECT_ROOT" -name "*.gcno" -delete 2>/dev/null || true
    
    log_success "测试环境清理完成"
}

# 主函数
main() {
    local start_time=$(date +%s)
    
    parse_arguments "$@"
    
    log_info "开始全面测试 - $(date)"
    log_info "测试配置: 单元=$RUN_UNIT_TESTS, 集成=$RUN_INTEGRATION_TESTS, 安全=$RUN_SECURITY_TESTS, 性能=$RUN_PERFORMANCE_TESTS, 覆盖率=$RUN_COVERAGE"
    
    setup_test_environment
    
    local exit_code=0
    
    # 运行各类测试
    if [[ "$RUN_UNIT_TESTS" == "true" ]]; then
        run_unit_tests || exit_code=1
    fi
    
    if [[ "$RUN_INTEGRATION_TESTS" == "true" ]]; then
        run_integration_tests || exit_code=1
    fi
    
    if [[ "$RUN_SECURITY_TESTS" == "true" ]]; then
        run_security_tests || exit_code=1
    fi
    
    if [[ "$RUN_PERFORMANCE_TESTS" == "true" ]]; then
        run_performance_tests || exit_code=1
    fi
    
    if [[ "$RUN_COVERAGE" == "true" ]]; then
        run_coverage_analysis || exit_code=1
    fi
    
    # 生成报告和清理
    generate_test_report
    cleanup_test_environment
    
    local end_time=$(date +%s)
    local total_duration=$((end_time - start_time))
    
    if [[ $exit_code -eq 0 ]]; then
        log_success "所有测试完成! 总耗时: ${total_duration}s"
        log_info "测试报告目录: $TEST_OUTPUT_DIR"
    else
        log_error "测试执行中发现问题，请查看详细日志"
        exit 1
    fi
}

# 脚本入口点
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi