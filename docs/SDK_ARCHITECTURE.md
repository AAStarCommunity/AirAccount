# AirAccount SDK 架构文档

## 概述

AirAccount SDK 是官方的 JavaScript/TypeScript SDK，用于 Web3 账户管理和 TEE 硬件安全。

## 架构设计

### 系统架构图

```
┌─────────────────┐
│   用户 DApp     │ (前端应用)
└─────────┬───────┘
          │ HTTP/WebSocket
┌─────────▼───────┐
│ AirAccount SDK  │ (packages/node-sdk)
└─────────┬───────┘
          │ REST API
┌─────────▼───────┐
│   CA Service    │ (双实现)
│ ├─ Rust CA      │ (packages/airaccount-ca-extended)
│ └─ Node.js CA   │ (packages/airaccount-ca-nodejs)
└─────────┬───────┘
          │ optee-teec
┌─────────▼───────┐
│      TA         │ (packages/airaccount-ta-simple)
└─────────┬───────┘
          │ TEE API
┌─────────▼───────┐
│   QEMU TEE      │ (third_party/build)
└─────────┬───────┘
          │ 硬件模拟
┌─────────▼───────┐
│   硬件安全      │ (ARM TrustZone)
└─────────────────┘
```

### 组件说明

#### 1. AirAccount SDK (`packages/node-sdk/`)
- **功能**: 为开发者提供简洁的 TypeScript API
- **特性**: 
  - WebAuthn/Passkey 集成
  - 自动重试和错误处理
  - 支持双 CA 架构
  - 跨平台兼容
- **主要文件**:
  - `src/index.ts` - 主要 SDK 类
  - `examples/` - 使用示例
  - `README.md` - SDK 文档

#### 2. CA Service 双实现

##### Rust CA (`packages/airaccount-ca-extended/`)
- **基础**: 扩展现有 airaccount-ca
- **特性**: 高性能，TEE 深度集成
- **端口**: 3001
- **优势**: 
  - 原生 TEE 支持
  - 更好的性能
  - 内存安全

##### Node.js CA (`packages/airaccount-ca-nodejs/`)
- **基础**: Simple WebAuthn 集成
- **特性**: 完整浏览器支持
- **端口**: 3002
- **优势**:
  - 完整 WebAuthn 支持
  - 易于开发和调试
  - Web 生态兼容

#### 3. TA (Trusted Application) (`packages/airaccount-ta-simple/`)
- **UUID**: `11223344-5566-7788-99aa-bbccddeeff01`
- **功能**: 
  - 钱包创建和管理
  - 私钥生成和存储
  - 交易签名
  - 地址派生

#### 4. 测试工具 (`packages/airaccount-sdk-test/`)
- **功能**: SDK-CA-TA-TEE 完整调用链测试
- **脚本**:
  - `test-ca-integration.js` - 集成测试
  - `demo-full-flow.js` - 完整流程演示

## 用户凭证架构

### 自主控制原则

根据用户反馈："未来这个 AirAccount节点，包括 TEE 也好，CA也好，TA也好，它们都有可能随时跑路的，但用户端呢，它要有能力存储自己的passkey和email，这是它持有wallet，account的凭证"

### 凭证分布

```
用户设备 (自主控制):
├── Passkey 凭证 (存储在设备安全硬件)
├── Email 地址 (用户记忆)
└── 钱包地址 (派生信息)

CA 服务 (临时服务):
├── WebAuthn Challenge (临时)
├── Session Token (会话)
└── 业务逻辑处理

TEE 硬件 (隔离存储):
├── 私钥 (永不导出)
├── 钱包种子 (硬件生成)
└── 签名功能
```

### 恢复机制

1. **用户保存信息**:
   - Email 地址
   - Passkey 凭证 (设备存储)
   - 钱包地址

2. **服务迁移场景**:
   - CA 服务可以更换
   - 用户凭证不依赖特定服务
   - TEE 私钥可重新绑定

## API 设计

### SDK 核心接口

```typescript
interface AirAccountSDK {
  // 初始化
  initialize(): Promise<void>
  
  // 认证
  registerWithWebAuthn(data: WebAuthnRegistrationData): Promise<RegistrationResult>
  authenticateWithWebAuthn(data: WebAuthnAuthenticationData): Promise<AuthResult>
  
  // 钱包管理
  createAccount(): Promise<WalletInfo>
  getBalance(walletId?: number): Promise<WalletInfo>
  listWallets(): Promise<WalletInfo[]>
  
  // 交易
  transfer(params: TransferParams, walletId?: number): Promise<TransferResult>
  
  // 会话管理
  getCurrentUser(): string | undefined
  isAuthenticated(): boolean
  logout(): void
}
```

### CA API 端点

#### 通用端点
- `GET /health` - 健康检查
- `POST /api/webauthn/register/begin` - 开始注册
- `POST /api/webauthn/register/complete` - 完成注册
- `POST /api/webauthn/authenticate/begin` - 开始认证
- `POST /api/webauthn/authenticate/complete` - 完成认证

#### 钱包端点
- `POST /api/account/create` - 创建账户
- `POST /api/account/balance` - 查询余额
- `POST /api/transaction/transfer` - 执行转账
- `GET /api/wallet/list` - 列出钱包

### TA 命令接口

```rust
// TA 命令定义
const CMD_HELLO_WORLD: u32 = 0;
const CMD_CREATE_WALLET: u32 = 10;
const CMD_DERIVE_ADDRESS: u32 = 12;
const CMD_SIGN_TRANSACTION: u32 = 13;
const CMD_GET_WALLET_INFO: u32 = 14;
const CMD_LIST_WALLETS: u32 = 15;
```

## 安全设计

### 威胁模型

1. **CA 服务妥协**: 用户凭证不受影响
2. **网络攻击**: HTTPS + 会话令牌保护
3. **设备丢失**: Passkey 设备绑定保护
4. **私钥泄露**: TEE 硬件隔离防护

### 安全措施

#### 1. 分层安全
- **应用层**: HTTPS, CORS, 输入验证
- **会话层**: JWT Token, 会话超时
- **认证层**: WebAuthn, 生物识别
- **硬件层**: TEE, 安全存储

#### 2. 密钥管理
- **生成**: TEE 硬件随机数
- **存储**: TEE 安全存储
- **使用**: 仅在 TEE 内签名
- **导出**: 永不导出私钥

#### 3. 用户自主控制
- **Passkey**: 用户设备存储
- **恢复**: 基于 Email + Passkey
- **迁移**: 独立于服务提供商

## 部署架构

### 开发环境
```
QEMU TEE 环境
├── OP-TEE OS
├── AirAccount TA
└── CA Service (Rust/Node.js)
```

### 生产环境
```
硬件 TEE 环境
├── Raspberry Pi 5 + OP-TEE
├── AirAccount TA
├── CA Service (负载均衡)
└── HTTPS + 域名
```

### 高可用部署
```
多节点集群
├── CA Service × N (容器化)
├── 负载均衡器
├── TEE 硬件集群
└── 监控和日志
```

## 性能指标

### 基准性能 (MacBook Pro M1)

| 操作 | Rust CA | Node.js CA | TEE 操作时间 |
|------|---------|------------|-------------|
| SDK 初始化 | ~100ms | ~150ms | 包含 TEE 连接检查 |
| WebAuthn 注册 | ~50ms | ~80ms | Challenge 生成 |
| 账户创建 | ~200ms | ~250ms | 包含 TEE 钱包创建 |
| 余额查询 | ~30ms | ~50ms | TEE 地址派生 |
| 转账签名 | ~100ms | ~120ms | TEE 交易签名 |

### 吞吐量测试
- **并发用户**: 50 用户同时操作
- **请求成功率**: >99%
- **平均响应时间**: <500ms
- **TEE 操作延迟**: <200ms

## 兼容性

### 浏览器支持
- Chrome 88+ (WebAuthn 完整支持)
- Firefox 87+ 
- Safari 14+ (iOS/macOS)
- Edge 88+

### Node.js 支持
- Node.js 16.0+
- TypeScript 5.0+
- ESM 和 CommonJS

### TEE 平台
- ARM TrustZone (OP-TEE)
- Intel SGX (未来支持)
- RISC-V TEE (计划中)

## 错误处理

### 错误分类
1. **网络错误**: 连接失败, 超时
2. **认证错误**: WebAuthn 失败, 会话过期
3. **业务错误**: 余额不足, 地址无效
4. **硬件错误**: TEE 连接失败, TA 错误

### 错误恢复策略
1. **自动重试**: 网络临时故障
2. **用户引导**: 认证失败重新登录
3. **降级处理**: 服务不可用时的备选方案
4. **监控告警**: 系统级错误的实时通知

## 扩展性

### 水平扩展
- CA 服务无状态设计
- 负载均衡支持
- 数据库读写分离

### 功能扩展
- 多链支持 (Polygon, Arbitrum)
- DeFi 协议集成
- NFT 资产管理
- 跨链桥接

### 性能优化
- 连接池复用
- 缓存层设计
- CDN 加速
- 异步处理

---

*本文档版本: v1.0.0*  
*最后更新: 2025-01-15*  
*维护者: AirAccount Team*