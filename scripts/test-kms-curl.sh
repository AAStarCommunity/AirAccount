#!/bin/bash

# KMS API Curl测试脚本
# 完整测试所有KMS API端点功能

set -e

# 配置
BASE_URL="${1:-https://atom-become-ireland-travels.trycloudflare.com}"
TEMP_DIR="/tmp/kms-test-$$"
mkdir -p "$TEMP_DIR"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

run_test() {
    local test_name="$1"
    local curl_command="$2"
    local expected_check="$3"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    log_info "测试 $TOTAL_TESTS: $test_name"
    echo "命令: $curl_command"

    # 执行curl命令并保存结果
    local output_file="$TEMP_DIR/test_${TOTAL_TESTS}.json"
    local start_time=$(date +%s.%N)

    if eval "$curl_command" > "$output_file" 2>/dev/null; then
        local end_time=$(date +%s.%N)
        local duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0.0")

        # 检查响应内容
        local response=$(cat "$output_file")
        echo "响应: $response"

        # 验证预期结果
        if [[ -n "$expected_check" ]]; then
            if echo "$response" | grep -q "$expected_check"; then
                log_success "✅ 测试通过 (${duration}s)"
                PASSED_TESTS=$((PASSED_TESTS + 1))
                return 0
            else
                log_error "❌ 测试失败: 响应中未找到预期内容 '$expected_check'"
                FAILED_TESTS=$((FAILED_TESTS + 1))
                return 1
            fi
        else
            log_success "✅ 测试通过 (${duration}s)"
            PASSED_TESTS=$((PASSED_TESTS + 1))
            return 0
        fi
    else
        log_error "❌ 测试失败: curl命令执行失败"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

extract_key_id() {
    local response="$1"
    # 从响应中提取KeyId
    echo "$response" | grep -o '"KeyId":"[^"]*"' | cut -d'"' -f4 | head -1
}

# 开始测试
log_info "开始KMS API测试"
log_info "目标URL: $BASE_URL"
log_info "=========================================="

# 1. 健康检查
run_test "健康检查" \
    "curl -s '$BASE_URL/health'" \
    '"status":"healthy"'

# 2. 创建密钥
CREATE_KEY_RESPONSE=$(mktemp)
run_test "创建密钥" \
    "curl -s -X POST '$BASE_URL/' \
        -H 'Content-Type: application/json' \
        -H 'X-Amz-Target: TrentService.CreateKey' \
        -d '{\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\"}' \
        | tee '$CREATE_KEY_RESPONSE'" \
    '"KeyId"'

# 提取密钥ID用于后续测试
if [[ -f "$CREATE_KEY_RESPONSE" ]]; then
    KEY_ID=$(extract_key_id "$(cat "$CREATE_KEY_RESPONSE")")
    if [[ -n "$KEY_ID" ]]; then
        log_info "提取到密钥ID: $KEY_ID"
    else
        log_warn "未能提取密钥ID，后续测试可能失败"
        KEY_ID="test-key-id"
    fi
else
    log_warn "创建密钥响应文件不存在"
    KEY_ID="test-key-id"
fi

# 3. 获取公钥
if [[ -n "$KEY_ID" && "$KEY_ID" != "test-key-id" ]]; then
    run_test "获取公钥" \
        "curl -s -X POST '$BASE_URL/' \
            -H 'Content-Type: application/json' \
            -H 'X-Amz-Target: TrentService.GetPublicKey' \
            -d '{\"KeyId\":\"$KEY_ID\"}'" \
        '"PublicKey"'
else
    log_warn "跳过获取公钥测试（无有效密钥ID）"
fi

# 4. 签名消息
if [[ -n "$KEY_ID" && "$KEY_ID" != "test-key-id" ]]; then
    MESSAGE="Hello KMS World"
    MESSAGE_B64=$(echo -n "$MESSAGE" | base64)

    run_test "签名消息" \
        "curl -s -X POST '$BASE_URL/' \
            -H 'Content-Type: application/json' \
            -H 'X-Amz-Target: TrentService.Sign' \
            -d '{\"KeyId\":\"$KEY_ID\",\"Message\":\"$MESSAGE_B64\",\"MessageType\":\"RAW\"}'" \
        '"Signature"'
else
    log_warn "跳过签名测试（无有效密钥ID）"
fi

# 5. 列出密钥
run_test "列出密钥" \
    "curl -s '$BASE_URL/keys'" \
    '"keys"'

# 6. 错误处理测试 - 不存在的密钥
run_test "错误处理（不存在的密钥）" \
    "curl -s -X POST '$BASE_URL/' \
        -H 'Content-Type: application/json' \
        -H 'X-Amz-Target: TrentService.GetPublicKey' \
        -d '{\"KeyId\":\"non-existent-key-12345\"}'" \
    '"error"'

# 7. 错误处理测试 - 无效JSON
run_test "错误处理（无效JSON）" \
    "curl -s -X POST '$BASE_URL/' \
        -H 'Content-Type: application/json' \
        -H 'X-Amz-Target: TrentService.CreateKey' \
        -d 'invalid-json'" \
    '"error"'

# 8. 错误处理测试 - 缺少Header
run_test "错误处理（缺少X-Amz-Target）" \
    "curl -s -X POST '$BASE_URL/' \
        -H 'Content-Type: application/json' \
        -d '{\"KeyUsage\":\"SIGN_VERIFY\"}'" \
    '"error"'

# 9. 批量创建密钥测试
log_info "=========================================="
log_info "批量创建密钥性能测试（5个密钥）"

BULK_START_TIME=$(date +%s.%N)
BULK_SUCCESS=0

for i in {1..5}; do
    BULK_RESPONSE=$(mktemp)
    if curl -s -X POST "$BASE_URL/" \
        -H 'Content-Type: application/json' \
        -H 'X-Amz-Target: TrentService.CreateKey' \
        -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' \
        > "$BULK_RESPONSE" 2>/dev/null; then

        if grep -q '"KeyId"' "$BULK_RESPONSE"; then
            BULK_SUCCESS=$((BULK_SUCCESS + 1))
            BULK_KEY_ID=$(extract_key_id "$(cat "$BULK_RESPONSE")")
            log_success "批量密钥 $i: $BULK_KEY_ID"
        else
            log_error "批量密钥 $i: 响应无效"
        fi
    else
        log_error "批量密钥 $i: 请求失败"
    fi
    rm -f "$BULK_RESPONSE"
done

BULK_END_TIME=$(date +%s.%N)
BULK_DURATION=$(echo "$BULK_END_TIME - $BULK_START_TIME" | bc -l 2>/dev/null || echo "0.0")
BULK_AVG=$(echo "scale=3; $BULK_DURATION / 5" | bc -l 2>/dev/null || echo "0.0")

log_info "批量测试结果: $BULK_SUCCESS/5 成功，总耗时: ${BULK_DURATION}s，平均: ${BULK_AVG}s/个"

# 10. 性能基准测试
log_info "=========================================="
log_info "性能基准测试"

# 测试健康检查延迟
HEALTH_TIMES=()
for i in {1..3}; do
    START_TIME=$(date +%s.%N)
    curl -s "$BASE_URL/health" > /dev/null 2>&1
    END_TIME=$(date +%s.%N)
    DURATION=$(echo "$END_TIME - $START_TIME" | bc -l 2>/dev/null || echo "0.0")
    HEALTH_TIMES+=("$DURATION")
done

HEALTH_AVG=$(echo "scale=3; (${HEALTH_TIMES[0]} + ${HEALTH_TIMES[1]} + ${HEALTH_TIMES[2]}) / 3" | bc -l 2>/dev/null || echo "0.0")
log_info "健康检查平均延迟: ${HEALTH_AVG}s"

# 最终报告
log_info "=========================================="
log_info "测试报告总结"
log_info "=========================================="

SUCCESS_RATE=$(echo "scale=1; $PASSED_TESTS * 100 / $TOTAL_TESTS" | bc -l 2>/dev/null || echo "0.0")

if [[ $FAILED_TESTS -eq 0 ]]; then
    log_success "🎉 所有测试通过！ $PASSED_TESTS/$TOTAL_TESTS (${SUCCESS_RATE}%)"
else
    log_warn "⚠️  测试结果: $PASSED_TESTS/$TOTAL_TESTS 通过 (${SUCCESS_RATE}%)"
    log_error "失败测试数: $FAILED_TESTS"
fi

log_info "批量操作成功率: $BULK_SUCCESS/5"
log_info "平均响应时间: ${HEALTH_AVG}s"

# 生成详细报告文件
REPORT_FILE="kms-test-report-$(date +%Y%m%d-%H%M%S).txt"
cat > "$REPORT_FILE" << EOF
KMS API 测试报告
================

测试时间: $(date)
目标URL: $BASE_URL

基本测试结果:
- 总测试数: $TOTAL_TESTS
- 通过测试: $PASSED_TESTS
- 失败测试: $FAILED_TESTS
- 成功率: ${SUCCESS_RATE}%

性能测试结果:
- 批量创建成功率: $BULK_SUCCESS/5
- 平均健康检查延迟: ${HEALTH_AVG}s
- 批量操作平均时间: ${BULK_AVG}s/个

测试详细日志保存在: $TEMP_DIR/
EOF

log_info "详细报告已保存到: $REPORT_FILE"

# 清理临时文件
# rm -rf "$TEMP_DIR"
rm -f "$CREATE_KEY_RESPONSE"

# 退出码
if [[ $FAILED_TESTS -gt 0 ]]; then
    exit 1
else
    exit 0
fi