#!/bin/bash

KMS_URL="https://kms.aastar.io"

echo "🎯 测试 $KMS_URL KMS API 隧道"
echo "📅 $(date)"
echo ""

# 测试 1: CreateKey
echo "🔑 测试 1: CreateKey"
KEY_ID=$(curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -H "Content-Type: application/json" \
  -d '{"Description":"Tunnel Test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' | jq -r '.KeyMetadata.KeyId')

if [ "$KEY_ID" != "null" ] && [ "$KEY_ID" != "" ]; then
    echo "✅ 创建密钥成功: $KEY_ID"
else
    echo "❌ 创建密钥失败"
    exit 1
fi

echo ""
echo "📝 测试 2: DescribeKey"
curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.DescribeKey" \
  -H "Content-Type: application/json" \
  -d "{\"KeyId\":\"$KEY_ID\"}" | jq .

echo ""
echo "📋 测试 3: ListKeys"
curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.ListKeys" \
  -H "Content-Type: application/json" \
  -d '{}' | jq .

echo ""
echo "✍️ 测试 4: Sign"
curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.Sign" \
  -H "Content-Type: application/json" \
  -d "{\"KeyId\":\"$KEY_ID\",\"Message\":\"SGVsbG8gV29ybGQ=\",\"SigningAlgorithm\":\"ECDSA_SHA_256\"}" | jq .

echo ""
echo "🔍 测试 5: GetPublicKey"
curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -H "Content-Type: application/json" \
  -d "{\"KeyId\":\"$KEY_ID\"}" | jq .

echo ""
echo "🗑️ 测试 6: ScheduleKeyDeletion"
curl -s "$KMS_URL/" \
  -H "X-Amz-Target: TrentService.ScheduleKeyDeletion" \
  -H "Content-Type: application/json" \
  -d "{\"KeyId\":\"$KEY_ID\",\"PendingWindowInDays\":7}" | jq .

echo ""
echo "🎉 KMS API 隧道测试完成！"
echo "🌐 隧道状态: ✅ 正常运行"
echo "📍 公网访问地址: $KMS_URL"
echo "🔗 本地服务地址: http://localhost:3000"