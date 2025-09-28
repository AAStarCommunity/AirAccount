# KMS API 快速curl测试命令

## 基础服务地址
```bash
BASE_URL="https://atom-become-ireland-travels.trycloudflare.com"
```

## 1. 健康检查
```bash
curl -s "$BASE_URL/health" | jq
```

## 2. 创建密钥
```bash
curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }' | jq

# 保存KeyId用于后续测试
KEY_ID="your-key-id-from-response"
```

## 3. 获取公钥
```bash
curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}" | jq
```

## 4. 消息签名
```bash
# 准备要签名的消息
MESSAGE="Hello KMS World!"
MESSAGE_B64=$(echo -n "$MESSAGE" | base64)

curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"Message\": \"$MESSAGE_B64\",
    \"MessageType\": \"RAW\",
    \"SigningAlgorithm\": \"ECDSA_SHA_256\"
  }" | jq
```

## 5. 列出所有密钥
```bash
curl -s "$BASE_URL/keys" | jq
```

## 6. 错误处理测试

### 无效密钥测试
```bash
curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d '{"KeyId": "invalid-key-12345"}' | jq
```

### 无效Action测试
```bash
curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.InvalidAction" \
  -d '{}' | jq
```

### 缺少字段测试
```bash
curl -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d '{"KeyId": "test"}' | jq
```

## 7. 完整测试工作流

```bash
#!/bin/bash
BASE_URL="https://atom-become-ireland-travels.trycloudflare.com"

echo "🔐 KMS API 快速测试"
echo "=================="

# 1. 健康检查
echo "1. 健康检查..."
curl -s "$BASE_URL/health" | jq

# 2. 创建密钥
echo -e "\n2. 创建密钥..."
RESPONSE=$(curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }')

echo "$RESPONSE" | jq
KEY_ID=$(echo "$RESPONSE" | jq -r '.KeyMetadata.KeyId')
echo "KeyId: $KEY_ID"

# 3. 获取公钥
echo -e "\n3. 获取公钥..."
curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}" | jq

# 4. 消息签名
echo -e "\n4. 消息签名..."
MESSAGE="Hello KMS World!"
MESSAGE_B64=$(echo -n "$MESSAGE" | base64)

curl -s -X POST "$BASE_URL/" \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"Message\": \"$MESSAGE_B64\",
    \"MessageType\": \"RAW\",
    \"SigningAlgorithm\": \"ECDSA_SHA_256\"
  }" | jq

# 5. 列出密钥
echo -e "\n5. 列出密钥..."
curl -s "$BASE_URL/keys" | jq

echo -e "\n✅ 测试完成!"
```

## 测试结果验证

### 成功响应示例

**健康检查:**
```json
{
  "service": "KMS API",
  "status": "healthy",
  "timestamp": "2025-09-28T03:34:18.692764012+00:00",
  "version": "0.1.0"
}
```

**创建密钥:**
```json
{
  "KeyMetadata": {
    "KeyId": "25f7fc63-7060-4913-ad37-5a76a44bb6b5",
    "Arn": "arn:aws:kms:us-west-2:123456789012:key/25f7fc63-7060-4913-ad37-5a76a44bb6b5",
    "CreationDate": "2025-09-28T03:23:49.539Z",
    "Enabled": true,
    "Description": "KMS key for digital signatures",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }
}
```

### 错误响应示例

**无效密钥:**
```json
{
  "__type": "NotFoundException",
  "message": "Key not found"
}
```

**缺少字段:**
```json
{
  "__type": "ValidationException",
  "message": "Invalid request: missing field `SigningAlgorithm`"
}
```