#!/bin/bash
# KMS API 测试脚本
# 测试所有6个API端点

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

API_URL="${API_URL:-http://localhost:3000}"

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_test() {
    echo -e "\n${BLUE}==>${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1"
}

# 检查jq是否安装
if ! command -v jq &> /dev/null; then
    log_error "jq is not installed. Please install it first:"
    echo "  macOS: brew install jq"
    echo "  Linux: apt-get install jq"
    exit 1
fi

# 检查服务是否运行
log_test "Checking if KMS API server is running..."
if ! curl -s "$API_URL/health" > /dev/null 2>&1; then
    log_error "KMS API server is not running at $API_URL"
    echo "Please start the server first:"
    echo "  cargo run --release --bin kms-api-server"
    exit 1
fi
log_success "Server is running"

# 1. 健康检查
log_test "1/7 Health Check"
RESPONSE=$(curl -s "$API_URL/health")
echo "$RESPONSE" | jq .
TA_MODE=$(echo "$RESPONSE" | jq -r '.ta_mode')
log_success "Health check passed (TA mode: $TA_MODE)"

# 2. 创建钱包
log_test "2/7 Create Wallet"
RESPONSE=$(curl -s -X POST "$API_URL/CreateKey" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "Test Wallet from Script",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }')

echo "$RESPONSE" | jq .

# 提取KeyId
KEY_ID=$(echo "$RESPONSE" | jq -r '.KeyMetadata.KeyId')
if [ -z "$KEY_ID" ] || [ "$KEY_ID" == "null" ]; then
    log_error "Failed to create wallet"
    exit 1
fi
log_success "Wallet created with KeyId: $KEY_ID"

# 3. 查询钱包详情
log_test "3/7 Describe Key"
RESPONSE=$(curl -s -X POST "$API_URL/DescribeKey" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}")

echo "$RESPONSE" | jq .
DESCRIPTION=$(echo "$RESPONSE" | jq -r '.KeyMetadata.Description')
if [ "$DESCRIPTION" != "Test Wallet from Script" ]; then
    log_error "Describe key failed - description mismatch"
    exit 1
fi
log_success "Describe key passed"

# 4. 列出所有钱包
log_test "4/7 List Keys"
RESPONSE=$(curl -s -X POST "$API_URL/ListKeys" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}')

echo "$RESPONSE" | jq .
KEY_COUNT=$(echo "$RESPONSE" | jq '.Keys | length')
log_success "Found $KEY_COUNT key(s)"

# 5. 派生地址 (m/44'/60'/0'/0/0 - 第一个以太坊地址)
log_test "5/7 Derive Address"
RESPONSE=$(curl -s -X POST "$API_URL/DeriveAddress" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\"
  }")

echo "$RESPONSE" | jq .
ADDRESS=$(echo "$RESPONSE" | jq -r '.Address')
if [ -z "$ADDRESS" ] || [ "$ADDRESS" == "null" ]; then
    log_error "Failed to derive address"
    exit 1
fi
log_success "Address derived: $ADDRESS"

# 6. 签名交易
log_test "6/7 Sign Transaction"
RESPONSE=$(curl -s -X POST "$API_URL/Sign" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\",
    \"Transaction\": {
      \"chainId\": 1,
      \"nonce\": 0,
      \"to\": \"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb\",
      \"value\": \"0x0\",
      \"gasPrice\": \"0x3b9aca00\",
      \"gas\": 21000,
      \"data\": \"0x\"
    }
  }")

echo "$RESPONSE" | jq .
SIGNATURE=$(echo "$RESPONSE" | jq -r '.Signature')
if [ -z "$SIGNATURE" ] || [ "$SIGNATURE" == "null" ]; then
    log_error "Failed to sign transaction"
    exit 1
fi
log_success "Transaction signed: ${SIGNATURE:0:20}..."

# 7. 删除钱包
log_test "7/7 Delete Key"
RESPONSE=$(curl -s -X POST "$API_URL/DeleteKey" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeleteKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}")

echo "$RESPONSE" | jq .
DELETION_DATE=$(echo "$RESPONSE" | jq -r '.DeletionDate')
if [ -z "$DELETION_DATE" ] || [ "$DELETION_DATE" == "null" ]; then
    log_error "Failed to delete key"
    exit 1
fi
log_success "Key deleted at: $DELETION_DATE"

# 验证删除
log_test "Verifying deletion..."
RESPONSE=$(curl -s -X POST "$API_URL/DescribeKey" \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}" 2>&1)

if echo "$RESPONSE" | grep -q "error"; then
    log_success "Confirmed - Key no longer exists"
else
    log_error "Key still exists after deletion!"
    exit 1
fi

echo ""
log_info "========================================="
log_info "All tests passed! ✅"
log_info "========================================="
echo ""
echo "Test Summary:"
echo "  ✓ Health Check"
echo "  ✓ Create Wallet"
echo "  ✓ Describe Key"
echo "  ✓ List Keys"
echo "  ✓ Derive Address"
echo "  ✓ Sign Transaction"
echo "  ✓ Delete Key"
echo ""