# KMS API 快速Curl命令集合

*创建时间: 2025-09-27 17:55*

## 🚀 一行命令快速测试

### 基础测试命令

```bash
# 1. 健康检查
curl -s https://atom-become-ireland-travels.trycloudflare.com/health | jq

# 2. 创建密钥
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' | jq

# 3. 列出所有密钥
curl -s https://atom-become-ireland-travels.trycloudflare.com/keys | jq

# 4. 获取公钥（替换YOUR_KEY_ID）
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d '{"KeyId":"YOUR_KEY_ID"}' | jq

# 5. 签名消息（替换YOUR_KEY_ID）
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d '{"KeyId":"YOUR_KEY_ID","Message":"SGVsbG8gV29ybGQ=","MessageType":"RAW","SigningAlgorithm":"ECDSA_SHA_256"}' | jq
```

### 错误处理测试

```bash
# 测试不存在的密钥
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d '{"KeyId":"non-existent-key"}' | jq

# 测试无效JSON
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d 'invalid-json'

# 测试缺少Header
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -d '{"KeyUsage":"SIGN_VERIFY"}'
```

## 🎯 完整测试流程（复制粘贴即用）

```bash
# 步骤1: 健康检查
echo "=== 1. 健康检查 ==="
curl -s https://atom-become-ireland-travels.trycloudflare.com/health | jq
echo

# 步骤2: 创建密钥并提取ID
echo "=== 2. 创建密钥 ==="
RESPONSE=$(curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}')
echo "$RESPONSE" | jq
KEY_ID=$(echo "$RESPONSE" | jq -r '.KeyMetadata.KeyId')
echo "提取的密钥ID: $KEY_ID"
echo

# 步骤3: 获取公钥
echo "=== 3. 获取公钥 ==="
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -d "{\"KeyId\":\"$KEY_ID\"}" | jq
echo

# 步骤4: 签名消息
echo "=== 4. 签名消息 ==="
curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d "{\"KeyId\":\"$KEY_ID\",\"Message\":\"SGVsbG8gV29ybGQ=\",\"MessageType\":\"RAW\",\"SigningAlgorithm\":\"ECDSA_SHA_256\"}" | jq
echo

# 步骤5: 列出密钥
echo "=== 5. 列出密钥 ==="
curl -s https://atom-become-ireland-travels.trycloudflare.com/keys | jq '.Keys | length'
echo "总密钥数量: $(curl -s https://atom-become-ireland-travels.trycloudflare.com/keys | jq '.Keys | length')"
```

## 🔧 本地测试（如果你本地部署了KMS）

```bash
# 替换URL为本地地址
LOCAL_URL="http://localhost:8080"

# 健康检查
curl -s $LOCAL_URL/health | jq

# 创建密钥
curl -s -X POST $LOCAL_URL/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}' | jq
```

## 📊 性能测试

```bash
# 测试响应时间
time curl -s https://atom-become-ireland-travels.trycloudflare.com/health

# 批量创建测试
for i in {1..5}; do
  echo "创建密钥 $i..."
  curl -s -X POST https://atom-become-ireland-travels.trycloudflare.com/ \
    -H "Content-Type: application/json" \
    -H "X-Amz-Target: TrentService.CreateKey" \
    -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}' | jq '.KeyMetadata.KeyId'
done
```

## 🛠️ 便捷脚本使用

```bash
# 运行简单测试
./simple-curl-test.sh

# 运行完整测试
./test-kms-curl.sh

# 指定不同URL测试
./simple-curl-test.sh http://localhost:8080
```

## 📝 示例响应

### 成功创建密钥响应
```json
{
  "KeyMetadata": {
    "Arn": "arn:aws:kms:us-west-2:123456789012:key/f0def8ac-eb1c-493b-a920-4b202ae71e8d",
    "CreationDate": "2025-09-27T11:21:02.058056012Z",
    "Description": "KMS generated key",
    "Enabled": true,
    "KeyId": "f0def8ac-eb1c-493b-a920-4b202ae71e8d",
    "KeySpec": "ECC_SECG_P256K1",
    "KeyUsage": "SIGN_VERIFY",
    "Origin": "AWS_KMS"
  }
}
```

### 成功签名响应
```json
{
  "KeyId": "f0def8ac-eb1c-493b-a920-4b202ae71e8d",
  "Signature": "4jNklcZdqe0nx5XytEhH57/1wqb47NKg+DvMoCxrM/woLal+BHqSfiLSviaYQZYe/vsqTCqYYps0bGryIBnO8Q==",
  "SigningAlgorithm": "ECDSA_SHA_256"
}
```

---

**这些命令可以直接复制粘贴使用，用来快速验证KMS API的所有功能！**