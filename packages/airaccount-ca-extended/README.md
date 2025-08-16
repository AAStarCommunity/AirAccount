# AirAccount CA Extended

基于现有 `airaccount-ca` 的扩展版本，添加了 WebAuthn 支持和 HTTP API 功能。

## 功能特性

### 核心功能（继承自 airaccount-ca）
- ✅ TEE 连接和通信
- ✅ 钱包创建和管理
- ✅ 地址派生
- ✅ 交易签名
- ✅ 安全测试

### 扩展功能
- 🆕 WebAuthn/Passkey 集成
- 🆕 HTTP API 服务器
- 🆕 账户与 Passkey 绑定
- 🆕 RESTful API 接口

## 架构设计

```
DApp/SDK → HTTP API → TEE Client → airaccount-ta-simple
```

### 兼容性
- 使用与 `airaccount-ta-simple` 相同的 UUID: `11223344-5566-7788-99aa-bbccddeeff01`
- 兼容现有 TA 命令（CMD_CREATE_WALLET, CMD_DERIVE_ADDRESS 等）
- 扩展新命令支持 WebAuthn（CMD_CREATE_ACCOUNT_WITH_PASSKEY 等）

## 快速开始

### 构建
```bash
cd packages/airaccount-ca-extended
cargo build
```

### CLI 模式
```bash
# 测试 TEE 连接
cargo run --bin ca-cli -- test

# 创建账户（需要 Passkey 数据）
cargo run --bin ca-cli -- create-wallet \
  --email "user@example.com" \
  --credential-id "abc123" \
  --public-key "base64_encoded_key"

# 查询钱包信息
cargo run --bin ca-cli -- get-wallet-info 1

# 列出所有钱包
cargo run --bin ca-cli -- list-wallets
```

### HTTP API 服务器
```bash
# 启动 API 服务器
cargo run --bin ca-server

# 服务器将在 http://0.0.0.0:3001 启动
```

## API 接口

### 健康检查
```bash
curl http://localhost:3001/health
```

### 创建账户
```bash
curl -X POST http://localhost:3001/api/account/create \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "passkey_credential_id": "credential_123",
    "passkey_public_key_base64": "base64_encoded_public_key"
  }'
```

### 查询余额
```bash
curl -X POST http://localhost:3001/api/account/balance \
  -H "Content-Type: application/json" \
  -d '{"wallet_id": 1}'
```

### 转账
```bash
curl -X POST http://localhost:3001/api/transaction/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_id": 1,
    "to_address": "0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A",
    "amount": "1000000000000000000"
  }'
```

### 列出钱包
```bash
curl http://localhost:3001/api/wallet/list
```

## 技术栈

### 核心依赖
- `optee-teec` - TEE 通信
- `axum` - HTTP 服务器框架
- `webauthn-rs` - WebAuthn 支持
- `sqlx` - 数据库（可选）

### 安全特性
- TEE 硬件安全
- WebAuthn/Passkey 生物识别
- 双重签名验证
- 安全存储

## 开发说明

### 扩展原则
1. **兼容性优先** - 与现有 TA 保持完全兼容
2. **渐进式增强** - 在现有功能基础上添加新特性
3. **模块化设计** - 清晰分离 TEE 客户端和 HTTP 服务
4. **安全第一** - 所有敏感操作都通过 TEE 执行

### 代码结构
```
src/
├── main.rs          # HTTP API 服务器
├── cli.rs           # CLI 工具
├── tee_client.rs    # TEE 客户端封装
└── lib.rs           # 公共库（可选）
```

## 测试

### 前提条件
确保 QEMU OP-TEE 环境正在运行，并且 `airaccount-ta-simple` 已加载。

### 集成测试
```bash
# 启动 QEMU 环境
cd third_party/build
make -f qemu_v8.mk run

# 在另一个终端测试
cd packages/airaccount-ca-extended
cargo test
```

## 与原版 airaccount-ca 的区别

| 功能 | airaccount-ca | airaccount-ca-extended |
|------|---------------|------------------------|
| TEE 通信 | ✅ | ✅ |
| CLI 工具 | ✅ | ✅ (增强) |
| HTTP API | ❌ | ✅ |
| WebAuthn | ❌ | ✅ |
| 数据库支持 | ❌ | ✅ (可选) |
| CORS 支持 | ❌ | ✅ |

## 后续计划

- [ ] 完善 WebAuthn 验证流程
- [ ] 添加数据库持久化
- [ ] 实现用户会话管理
- [ ] 添加更多安全检查
- [ ] 性能优化和监控