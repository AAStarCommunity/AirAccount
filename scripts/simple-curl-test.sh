#!/bin/bash

# 简单的KMS API Curl测试脚本
# Simple KMS API curl test script

# 配置
BASE_URL="${1:-https://atom-become-ireland-travels.trycloudflare.com}"

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}=== KMS API 简单测试 ===${NC}"
echo "目标URL: $BASE_URL"
echo

# 1. 健康检查
echo -e "${BLUE}1. 健康检查${NC}"
curl -s "$BASE_URL/health" | jq . || curl -s "$BASE_URL/health"
echo -e "\n"

# 2. 创建密钥
echo -e "${BLUE}2. 创建密钥${NC}"
CREATE_RESPONSE=$(curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}')

echo "$CREATE_RESPONSE" | jq . 2>/dev/null || echo "$CREATE_RESPONSE"

# 提取密钥ID
KEY_ID=$(echo "$CREATE_RESPONSE" | grep -o '"KeyId":"[^"]*"' | cut -d'"' -f4)
echo -e "\n${GREEN}提取的密钥ID: $KEY_ID${NC}\n"

# 3. 获取公钥
if [[ -n "$KEY_ID" ]]; then
    echo -e "${BLUE}3. 获取公钥${NC}"
    curl -s -X POST "$BASE_URL/" \
      -H "Content-Type: application/json" \
      -H "X-Amz-Target: TrentService.GetPublicKey" \
      -d "{\"KeyId\":\"$KEY_ID\"}" | jq . 2>/dev/null || \
    curl -s -X POST "$BASE_URL/" \
      -H "Content-Type: application/json" \
      -H "X-Amz-Target: TrentService.GetPublicKey" \
      -d "{\"KeyId\":\"$KEY_ID\"}"
    echo -e "\n"
fi

# 4. 签名消息（修复版本）
if [[ -n "$KEY_ID" ]]; then
    echo -e "${BLUE}4. 签名消息${NC}"
    MESSAGE="Hello World"
    MESSAGE_B64=$(echo -n "$MESSAGE" | base64)

    curl -s -X POST "$BASE_URL/" \
      -H "Content-Type: application/json" \
      -H "X-Amz-Target: TrentService.Sign" \
      -d "{\"KeyId\":\"$KEY_ID\",\"Message\":\"$MESSAGE_B64\",\"MessageType\":\"RAW\",\"SigningAlgorithm\":\"ECDSA_SHA_256\"}" | \
      jq . 2>/dev/null || \
    curl -s -X POST "$BASE_URL/" \
      -H "Content-Type: application/json" \
      -H "X-Amz-Target: TrentService.Sign" \
      -d "{\"KeyId\":\"$KEY_ID\",\"Message\":\"$MESSAGE_B64\",\"MessageType\":\"RAW\",\"SigningAlgorithm\":\"ECDSA_SHA_256\"}"
    echo -e "\n"
fi

# 5. 列出密钥
echo -e "${BLUE}5. 列出密钥${NC}"
curl -s "$BASE_URL/keys" | jq . 2>/dev/null || curl -s "$BASE_URL/keys"
echo -e "\n"

# 6. 错误处理测试
echo -e "${BLUE}6. 错误处理测试（不存在的密钥）${NC}"
curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d '{"KeyId":"non-existent-key"}' | jq . 2>/dev/null || \
curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d '{"KeyId":"non-existent-key"}'
echo -e "\n"

echo -e "${GREEN}=== 测试完成 ===${NC}"