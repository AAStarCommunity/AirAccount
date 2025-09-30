# KMS API 使用文档

> 创建时间: 2025-09-30

## 概述

KMS API Server 提供AWS KMS兼容的HTTP API接口，用于管理基于OP-TEE TrustZone的以太坊钱包。

**支持两种运行模式：**
- **Mock模式** (默认): 无需OP-TEE环境，用于开发和测试
- **Real TA模式**: 连接真实的OP-TEE Trusted Application，提供硬件级安全

## 快速开始

### 1. Mock模式运行 (无需OP-TEE)

```bash
# 编译并运行 (默认mock模式)
cargo build --release --bin kms-api-server
cargo run --release --bin kms-api-server

# 或直接运行
./target/release/kms-api-server
```

### 2. Real TA模式运行 (需要OP-TEE环境)

```bash
# 在QEMU OP-TEE环境中编译并运行
cargo build --release --bin kms-api-server --features ta_integration
cargo run --release --bin kms-api-server --features ta_integration
```

## API端点

服务器默认监听 `http://0.0.0.0:3000`

### 健康检查

```bash
curl http://localhost:3000/health
```

**响应示例:**
```json
{
  "status": "healthy",
  "service": "kms-api",
  "version": "0.1.0",
  "ta_mode": "mock",
  "endpoints": {
    "POST": ["/CreateKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/DeleteKey"],
    "GET": ["/health"]
  }
}
```

---

## API详细说明

所有POST请求需要包含以下HTTP头：
```
Content-Type: application/json
x-amz-target: TrentService.<ActionName>
```

### 1. CreateKey - 创建钱包

**端点:** `POST /CreateKey`

**请求头:**
```
x-amz-target: TrentService.CreateKey
Content-Type: application/json
```

**请求体:**
```json
{
  "Description": "My Ethereum Wallet",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1",
  "Origin": "AWS_KMS"
}
```

**响应:**
```json
{
  "KeyMetadata": {
    "KeyId": "550e8400-e29b-41d4-a716-446655440000",
    "Arn": "arn:aws:kms:region:account:key/550e8400-e29b-41d4-a716-446655440000",
    "CreationDate": "2025-09-30T12:00:00Z",
    "Enabled": true,
    "Description": "My Ethereum Wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  },
  "Mnemonic": "[MOCK_MNEMONIC]"
}
```

**curl示例:**
```bash
curl -X POST http://localhost:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "Test Wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'
```

---

### 2. DescribeKey - 查询钱包元数据

**端点:** `POST /DescribeKey`

**请求头:**
```
x-amz-target: TrentService.DescribeKey
Content-Type: application/json
```

**请求体:**
```json
{
  "KeyId": "550e8400-e29b-41d4-a716-446655440000"
}
```

**响应:**
```json
{
  "KeyMetadata": {
    "KeyId": "550e8400-e29b-41d4-a716-446655440000",
    "Arn": "arn:aws:kms:region:account:key/550e8400-e29b-41d4-a716-446655440000",
    "CreationDate": "2025-09-30T12:00:00Z",
    "Enabled": true,
    "Description": "Test Wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }
}
```

**curl示例:**
```bash
KEY_ID="<your-key-id>"
curl -X POST http://localhost:3000/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}"
```

---

### 3. ListKeys - 列出所有钱包

**端点:** `POST /ListKeys`

**请求头:**
```
x-amz-target: TrentService.ListKeys
Content-Type: application/json
```

**请求体:**
```json
{
  "Limit": 100,
  "Marker": null
}
```

**响应:**
```json
{
  "Keys": [
    {
      "KeyId": "550e8400-e29b-41d4-a716-446655440000",
      "KeyArn": "arn:aws:kms:region:account:key/550e8400-e29b-41d4-a716-446655440000"
    },
    {
      "KeyId": "660e8400-e29b-41d4-a716-446655440001",
      "KeyArn": "arn:aws:kms:region:account:key/660e8400-e29b-41d4-a716-446655440001"
    }
  ]
}
```

**curl示例:**
```bash
curl -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}'
```

---

### 4. DeriveAddress - 派生以太坊地址

**端点:** `POST /DeriveAddress`

**请求头:**
```
x-amz-target: TrentService.DeriveAddress
Content-Type: application/json
```

**请求体:**
```json
{
  "KeyId": "550e8400-e29b-41d4-a716-446655440000",
  "DerivationPath": "m/44'/60'/0'/0/0"
}
```

**响应:**
```json
{
  "Address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "PublicKey": "[MOCK_PUBKEY]"
}
```

**curl示例:**
```bash
KEY_ID="<your-key-id>"
curl -X POST http://localhost:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\"
  }"
```

---

### 5. Sign - 签名以太坊交易

**端点:** `POST /Sign`

**请求头:**
```
x-amz-target: TrentService.Sign
Content-Type: application/json
```

**请求体:**
```json
{
  "KeyId": "550e8400-e29b-41d4-a716-446655440000",
  "DerivationPath": "m/44'/60'/0'/0/0",
  "Transaction": {
    "chainId": 1,
    "nonce": 0,
    "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
    "value": "0x0",
    "gasPrice": "0x3b9aca00",
    "gas": 21000,
    "data": "0x"
  }
}
```

**响应:**
```json
{
  "Signature": "[MOCK_SIGNATURE]",
  "TransactionHash": "[MOCK_TX_HASH]"
}
```

**curl示例:**
```bash
KEY_ID="<your-key-id>"
curl -X POST http://localhost:3000/Sign \
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
  }"
```

---

### 6. DeleteKey - 删除钱包

**端点:** `POST /DeleteKey`

**请求头:**
```
x-amz-target: TrentService.DeleteKey
Content-Type: application/json
```

**请求体:**
```json
{
  "KeyId": "550e8400-e29b-41d4-a716-446655440000"
}
```

**响应:**
```json
{
  "DeletionDate": "2025-09-30T12:00:00Z"
}
```

**curl示例:**
```bash
KEY_ID="<your-key-id>"
curl -X POST http://localhost:3000/DeleteKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeleteKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}"
```

---

## 完整测试流程

```bash
#!/bin/bash

# 1. 健康检查
echo "=== Health Check ==="
curl http://localhost:3000/health | jq
echo ""

# 2. 创建钱包
echo "=== Create Wallet ==="
RESPONSE=$(curl -s -X POST http://localhost:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "Test Wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }')
echo $RESPONSE | jq

# 提取KeyId
KEY_ID=$(echo $RESPONSE | jq -r '.KeyMetadata.KeyId')
echo "KeyId: $KEY_ID"
echo ""

# 3. 列出所有钱包
echo "=== List Keys ==="
curl -s -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}' | jq
echo ""

# 4. 查询钱包详情
echo "=== Describe Key ==="
curl -s -X POST http://localhost:3000/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}" | jq
echo ""

# 5. 派生地址
echo "=== Derive Address ==="
curl -s -X POST http://localhost:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\"
  }" | jq
echo ""

# 6. 签名交易
echo "=== Sign Transaction ==="
curl -s -X POST http://localhost:3000/Sign \
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
  }" | jq
echo ""

# 7. 删除钱包
echo "=== Delete Key ==="
curl -s -X POST http://localhost:3000/DeleteKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeleteKey" \
  -d "{\"KeyId\": \"$KEY_ID\"}" | jq
echo ""
```

保存为 `test_kms_api.sh` 并执行：
```bash
chmod +x test_kms_api.sh
./test_kms_api.sh
```

---

## Mock模式 vs Real TA模式

| 特性 | Mock模式 | Real TA模式 |
|-----|---------|------------|
| 需要OP-TEE | ❌ | ✅ |
| 真实签名 | ❌ | ✅ |
| 硬件安全 | ❌ | ✅ |
| 开发测试 | ✅ | ✅ |
| 生产使用 | ❌ | ✅ |
| 编译命令 | `cargo build` | `cargo build --features ta_integration` |

---

## 部署到QEMU OP-TEE环境

1. **在宿主机编译（Real TA模式）:**
```bash
# 使用kms-deploy脚本
./scripts/kms-deploy.sh
```

2. **在QEMU Guest中运行API服务器:**
```bash
# 复制TA
cd shared
cp *.ta /lib/optee_armtz/

# 运行API服务器
./kms-api-server
```

3. **从宿主机测试API:**
```bash
# 假设QEMU网络已配置端口转发
curl http://localhost:3000/health
```

---

## 错误处理

所有错误响应格式：
```json
{
  "error": "Error message description"
}
```

**常见错误码:**
- `400 Bad Request`: 请求参数错误
- `404 Not Found`: 钱包不存在
- `500 Internal Server Error`: 服务器内部错误（TA调用失败等）

---

## 安全注意事项

1. **Mock模式仅用于开发**: 不要在生产环境使用mock模式
2. **HTTPS加密**: 生产环境应使用HTTPS
3. **认证授权**: 当前版本未实现认证，生产环境需要添加
4. **Rate Limiting**: 建议添加请求限流
5. **助记词安全**: 创建钱包返回的助记词应妥善保存

---

## 下一步计划

- [ ] 添加JWT认证
- [ ] 添加请求速率限制
- [ ] 支持批量操作
- [ ] 添加Prometheus指标
- [ ] 支持HTTPS/TLS
- [ ] 完善错误处理和日志

---

## 相关文档

- [KMS开发工作流程](./kms-workflow.md)
- [eth_wallet分析](./eth_wallet-analysis.md)
- [OP-TEE存储分析](./optee-storage-analysis.md)