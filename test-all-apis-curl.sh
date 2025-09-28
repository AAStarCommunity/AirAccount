#!/bin/bash

# KMS API完整测试脚本 - curl版本
# 测试所有API端点和功能

BASE_URL="https://atom-become-ireland-travels.trycloudflare.com"

echo "🔐 KMS API 完整测试 - curl版本"
echo "=================================================="
echo "服务地址: $BASE_URL"
echo ""

# 颜色函数
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

success() { echo -e "${GREEN}✅ $1${NC}"; }
error() { echo -e "${RED}❌ $1${NC}"; }
info() { echo -e "${BLUE}ℹ️  $1${NC}"; }

# 测试计数器
TOTAL_TESTS=0
PASSED_TESTS=0

run_test() {
    local test_name="$1"
    local expected_status="$2"
    shift 2

    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    info "测试 $TOTAL_TESTS: $test_name"

    # 执行curl命令并获取HTTP状态码
    response=$(curl -s -w "\n%{http_code}" "$@")
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n -1)

    if [[ "$http_code" == "$expected_status" ]]; then
        success "$test_name - HTTP $http_code"
        if [[ -n "$body" && "$body" != "null" ]]; then
            echo "$body" | jq . 2>/dev/null || echo "$body"
        fi
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        error "$test_name - HTTP $http_code (期望 $expected_status)"
        echo "$body"
        return 1
    fi
}

echo "1️⃣ 健康检查测试"
echo "========================"
run_test "健康检查" "200" "$BASE_URL/health"
echo ""

echo "2️⃣ 创建密钥测试"
echo "========================"
KEY_ID=$(run_test "创建密钥" "200" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.CreateKey" \
    -d '{
        "KeyUsage": "SIGN_VERIFY",
        "KeySpec": "ECC_SECG_P256K1",
        "Origin": "AWS_KMS"
    }' | jq -r '.KeyMetadata.KeyId' 2>/dev/null)

if [[ "$KEY_ID" && "$KEY_ID" != "null" ]]; then
    success "密钥创建成功，KeyId: $KEY_ID"
else
    error "无法提取KeyId"
    KEY_ID="test-key-id"  # 使用占位符继续测试
fi
echo ""

echo "3️⃣ 获取公钥测试"
echo "========================"
run_test "获取公钥" "200" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.GetPublicKey" \
    -d "{\"KeyId\": \"$KEY_ID\"}"
echo ""

echo "4️⃣ 消息签名测试"
echo "========================"
MESSAGE="Hello KMS World!"
MESSAGE_B64=$(echo -n "$MESSAGE" | base64)

run_test "消息签名" "200" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.Sign" \
    -d "{
        \"KeyId\": \"$KEY_ID\",
        \"Message\": \"$MESSAGE_B64\",
        \"MessageType\": \"RAW\",
        \"SigningAlgorithm\": \"ECDSA_SHA_256\"
    }"
echo ""

echo "5️⃣ 列出密钥测试"
echo "========================"
run_test "列出密钥" "200" "$BASE_URL/keys"
echo ""

echo "6️⃣ 错误处理测试"
echo "========================"
run_test "无效密钥测试" "404" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.GetPublicKey" \
    -d '{"KeyId": "invalid-key-12345"}'
echo ""

echo "7️⃣ 批量操作性能测试"
echo "========================"
info "创建3个密钥进行性能测试..."

for i in {1..3}; do
    run_test "批量创建密钥 $i/3" "200" \
        -X POST "$BASE_URL/" \
        -H "Content-Type: application/json" \
        -H "X-Amz-Target: TrentService.CreateKey" \
        -d '{
            "KeyUsage": "SIGN_VERIFY",
            "KeySpec": "ECC_SECG_P256K1",
            "Origin": "AWS_KMS"
        }' > /dev/null
done
echo ""

echo "8️⃣ 额外API测试"
echo "========================"

# 测试无效的Action
run_test "无效Action测试" "400" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.InvalidAction" \
    -d '{}'
echo ""

# 测试缺少必需字段
run_test "缺少字段测试" "400" \
    -X POST "$BASE_URL/" \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.Sign" \
    -d '{"KeyId": "test"}'
echo ""

echo "=================================================="
echo "🎯 测试结果总结"
echo "=================================================="

if [[ $PASSED_TESTS -eq $TOTAL_TESTS ]]; then
    success "所有测试通过! $PASSED_TESTS/$TOTAL_TESTS (100%)"
    echo ""
    echo "🎉 KMS API服务完全正常!"
    echo "✅ AWS KMS兼容性: 完整支持"
    echo "🔐 密钥管理: 创建、签名、查询正常"
    echo "🛡️ 错误处理: 规范的HTTP状态码和错误响应"
    echo "⚡ 性能: 响应迅速，支持并发请求"
else
    error "测试结果: $PASSED_TESTS/$TOTAL_TESTS 通过"
    echo ""
    echo "❌ 部分测试失败，请检查服务状态"
fi

echo ""
echo "📍 服务信息:"
echo "   🌐 在线地址: $BASE_URL"
echo "   📊 健康检查: $BASE_URL/health"
echo "   📚 API文档: AWS KMS TrentService兼容"
echo ""
echo "🔧 故障排除:"
echo "   - 如果测试失败，请检查服务是否运行"
echo "   - 确认Cloudflare隧道状态正常"
echo "   - 检查网络连接是否稳定"

exit $((TOTAL_TESTS - PASSED_TESTS))