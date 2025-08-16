# AirAccount CA API 规范

## 概述

AirAccount CA (Client Application) 提供HTTP API接口，作为DApp和TEE TA之间的桥梁。

## 架构层次

```
DApp → SDK → CA API → TEE TA → Hardware TEE
```

## API版本

当前版本: `v1`

基础URL: `https://api.airaccount.io/v1` (生产)
开发URL: `http://localhost:3002/api/v1` (开发)

## 认证

所有需要认证的端点都需要有效的Passkey验证或JWT token。

## 端点规范

### 1. WebAuthn/Passkey管理

#### 开始Passkey注册
```http
POST /api/v1/passkey/register/begin
Content-Type: application/json

{
  "email": "user@example.com"
}
```

**响应:**
```json
{
  "success": true,
  "challenge": "Y2hhbGxlbmdlX3N0cmluZw",
  "userId": "uuid-string",
  "options": {
    "rp": { "name": "AirAccount", "id": "airaccount.io" },
    "user": { "id": "...", "name": "...", "displayName": "..." },
    "challenge": "...",
    "pubKeyCredParams": [...]
  }
}
```

#### 完成Passkey注册
```http
POST /api/v1/passkey/register/complete
Content-Type: application/json

{
  "email": "user@example.com",
  "userId": "uuid-string", 
  "registrationData": {
    "id": "credential-id",
    "response": {
      "clientDataJSON": "...",
      "attestationObject": "..."
    }
  }
}
```

**响应:**
```json
{
  "success": true,
  "data": {
    "account_id": "account-uuid",
    "ethereum_address": "0x742d35Cc6634C0532925a3b8D84c5e1fC30a5d",
    "security_level": "hybrid_entropy",
    "tee_device_id": "tee-device-uuid",
    "created_at": 1640995200
  }
}
```

#### 开始Passkey认证
```http
POST /api/v1/passkey/authenticate/begin
Content-Type: application/json

{
  "accountId": "account-uuid"
}
```

#### 完成Passkey认证
```http
POST /api/v1/passkey/authenticate/complete
Content-Type: application/json

{
  "accountId": "account-uuid",
  "authenticationData": {
    "id": "credential-id",
    "response": {
      "clientDataJSON": "...",
      "authenticatorData": "...",
      "signature": "..."
    }
  }
}
```

### 2. 账户管理

#### 获取账户信息
```http
GET /api/v1/accounts/{accountId}
```

**响应:**
```json
{
  "success": true,
  "data": {
    "account_id": "account-uuid",
    "ethereum_address": "0x742d35Cc6634C0532925a3b8D84c5e1fC30a5d",
    "balance": "1.234567",
    "transaction_count": 42,
    "created_at": 1640995200,
    "social_accounts": [...],
    "passkeys": [...],
    "security_config": {...}
  }
}
```

### 3. 交易管理

#### 执行转账意图
```http
POST /api/v1/accounts/{accountId}/intents
Content-Type: application/json

{
  "action": "transfer",
  "to": "0x742d35Cc6634C0532925a3b8D84c5e1fC30a5d", 
  "amount": "0.1",
  "passkeyAuth": {
    "id": "credential-id",
    "response": {...}
  }
}
```

**响应:**
```json
{
  "success": true,
  "data": {
    "transaction_hash": "0xabcdef...",
    "gas_sponsored": true,
    "tee_signature": {
      "node_id": "tee-node-uuid",
      "signature": "0x123abc..."
    }
  }
}
```

#### 获取交易历史
```http
GET /api/v1/accounts/{accountId}/transactions?limit=50&offset=0
```

**响应:**
```json
{
  "success": true,
  "data": {
    "transactions": [
      {
        "hash": "0xabcdef...",
        "timestamp": 1640995200,
        "action": "transfer",
        "status": "confirmed",
        "amount": "0.1",
        "to": "0x742d35Cc6634C0532925a3b8D84c5e1fC30a5d",
        "from": "0x123abc..."
      }
    ],
    "total": 100
  }
}
```

### 4. 系统管理

#### 健康检查
```http
GET /health
```

**响应:**
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": 1640995200,
    "service": "airaccount-ca",
    "features": {
      "webauthn": true,
      "passkey": true,
      "tee_connected": true
    },
    "tee_nodes": [
      {
        "node_id": "tee-node-1",
        "status": "online",
        "last_heartbeat": 1640995200
      }
    ]
  }
}
```

## 错误响应

所有错误响应都遵循统一格式：

```json
{
  "success": false,
  "error": "Human readable error message",
  "error_code": "ERROR_CODE",
  "timestamp": 1640995200,
  "request_id": "req_uuid"
}
```

### 错误代码

- `INVALID_REQUEST` - 请求参数无效
- `ACCOUNT_NOT_FOUND` - 账户不存在
- `PASSKEY_REGISTRATION_FAILED` - Passkey注册失败
- `PASSKEY_AUTHENTICATION_FAILED` - Passkey认证失败
- `INSUFFICIENT_BALANCE` - 余额不足
- `TEE_ERROR` - TEE操作失败
- `NETWORK_ERROR` - 网络错误

## 实现要求

### Node.js版本 (ca-service-real/)
- Express.js框架
- @simplewebauthn/server库
- SQLite/PostgreSQL数据库
- HTTP客户端友好

### Rust版本 (ca-service-rust/)
- Axum/Actix Web框架
- webauthn-rs库
- 直接与TEE TA集成
- 高性能和安全性

### 共同要求
- HTTPS支持
- CORS配置
- 请求速率限制
- 日志记录
- 监控指标
- 优雅关闭