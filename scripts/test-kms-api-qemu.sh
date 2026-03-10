#!/bin/bash
# KMS API Server 测试脚本 - 在QEMU Guest VM中运行

echo "🎯 KMS API Server 功能测试"
echo "================================"

# 检查API服务器是否运行
if ! ps | grep -v grep | grep -q kms-api-server; then
    echo "⚠️  API服务器未运行，正在启动..."
    ./kms-api-server > /tmp/kms-api-server.log 2>&1 &
    API_PID=$!
    sleep 3

    if ps | grep -v grep | grep -q kms-api-server; then
        echo "✅ API服务器已启动 (PID: $API_PID)"
    else
        echo "⚠️  正在检查API服务器状态..."
        sleep 2
        if curl -s http://localhost:3000/health > /dev/null 2>&1; then
            echo "✅ API服务器响应正常"
        else
            echo "❌ API服务器启动失败，查看日志:"
            cat /tmp/kms-api-server.log
            exit 1
        fi
    fi
else
    echo "✅ API服务器已运行"
fi

echo ""
echo "================================"
echo ""

# 1. 创建密钥
echo "1️⃣ 测试 CreateKey..."
CREATE_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "Description": "Test Key for QEMU",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }')

if command -v jq > /dev/null 2>&1; then
    echo "$CREATE_RESPONSE" | jq '.'
else
    echo "$CREATE_RESPONSE"
fi

KEY_ID=$(echo "$CREATE_RESPONSE" | jq -r '.KeyMetadata.KeyId // empty')

if [ -z "$KEY_ID" ]; then
    echo "❌ CreateKey 失败"
    exit 1
fi

echo "✅ 密钥创建成功: $KEY_ID"

# 2. 获取公钥
echo ""
echo "2️⃣ 测试 GetPublicKey..."
GET_PUB_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.GetPublicKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d "{
    \"KeyId\": \"$KEY_ID\"
  }")

echo "$GET_PUB_RESPONSE" | jq '.'

PUBLIC_KEY=$(echo "$GET_PUB_RESPONSE" | jq -r '.PublicKey // empty')

if [ -n "$PUBLIC_KEY" ]; then
    echo "✅ GetPublicKey 成功"
else
    echo "⚠️  GetPublicKey 返回空 (可能未实现)"
fi

# 3. 派生地址
echo ""
echo "3️⃣ 测试 DeriveAddress..."
DERIVE_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.DeriveAddress' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\"
  }")

echo "$DERIVE_RESPONSE" | jq '.'

ADDRESS=$(echo "$DERIVE_RESPONSE" | jq -r '.Address // empty')

if [ -n "$ADDRESS" ]; then
    echo "✅ DeriveAddress 成功: $ADDRESS"
else
    echo "❌ DeriveAddress 失败"
fi

# 4. 签名交易
echo ""
echo "4️⃣ 测试 Sign (签名以太坊交易)..."
SIGN_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\",
    \"Transaction\": {
      \"chainId\": 1,
      \"nonce\": 0,
      \"to\": \"0x742d35Cc6634C0532925a3b844Bc454e4438f44e\",
      \"value\": \"0x0de0b6b3a7640000\",
      \"gasPrice\": \"0x04a817c800\",
      \"gas\": 21000,
      \"data\": \"0x\"
    }
  }")

echo "$SIGN_RESPONSE" | jq '.'

SIGNATURE=$(echo "$SIGN_RESPONSE" | jq -r '.Signature // empty')

if [ -n "$SIGNATURE" ]; then
    echo "✅ Sign 成功"
    echo "   签名长度: ${#SIGNATURE} 字符"
else
    echo "❌ Sign 失败"
fi

# 5. 列出密钥
echo ""
echo "5️⃣ 测试 ListKeys..."
LIST_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.ListKeys' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{}')

echo "$LIST_RESPONSE" | jq '.'

KEY_COUNT=$(echo "$LIST_RESPONSE" | jq '.Keys | length')

if [ "$KEY_COUNT" -gt 0 ]; then
    echo "✅ ListKeys 成功，共 $KEY_COUNT 个密钥"
else
    echo "⚠️  ListKeys 返回空列表"
fi

# 6. 描述密钥
echo ""
echo "6️⃣ 测试 DescribeKey..."
DESCRIBE_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.DescribeKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d "{
    \"KeyId\": \"$KEY_ID\"
  }")

echo "$DESCRIBE_RESPONSE" | jq '.'

KEY_STATE=$(echo "$DESCRIBE_RESPONSE" | jq -r '.KeyMetadata.KeyState // empty')

if [ -n "$KEY_STATE" ]; then
    echo "✅ DescribeKey 成功，状态: $KEY_STATE"
else
    echo "❌ DescribeKey 失败"
fi

# 7. 删除密钥
echo ""
echo "7️⃣ 测试 ScheduleKeyDeletion..."
DELETE_RESPONSE=$(curl -s -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.ScheduleKeyDeletion' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"PendingWindowInDays\": 7
  }")

echo "$DELETE_RESPONSE" | jq '.'

DELETION_DATE=$(echo "$DELETE_RESPONSE" | jq -r '.DeletionDate // empty')

if [ -n "$DELETION_DATE" ]; then
    echo "✅ ScheduleKeyDeletion 成功"
else
    echo "❌ ScheduleKeyDeletion 失败"
fi

# 测试总结
echo ""
echo "================================"
echo "🎉 KMS API 测试完成！"
echo ""
echo "API服务器日志位置: /tmp/kms-api-server.log"
echo "查看日志: tail -f /tmp/kms-api-server.log"
echo ""
echo "停止API服务器: pkill -f kms-api-server"