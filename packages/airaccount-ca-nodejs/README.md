# AirAccount CA - Node.js Implementation

基于 Simple WebAuthn 的现代化 AirAccount CA 服务实现。

## 核心架构原则

⚠️ **重要：用户凭证自主控制**

- **节点可能跑路**：AirAccount 节点、TEE、CA、TA 都可能不可用
- **用户凭证自管**：Passkey 和 email 凭证存储在用户设备中
- **恢复能力**：用户必须保留自己的恢复信息以访问钱包
- **临时服务**：CA 节点只提供临时 challenge 验证和会话管理

## 技术栈

- **Node.js + TypeScript**: 现代化开发体验
- **Simple WebAuthn**: 完整的 WebAuthn/Passkey 支持
- **Express**: Web 服务框架
- **SQLite**: 轻量级临时数据存储
- **Zod**: 请求验证
- **Winston**: 结构化日志

## 快速开始

### 1. 安装依赖

```bash
cd packages/airaccount-ca-nodejs
npm install
```

### 2. 环境配置

```bash
cp .env.example .env
# 编辑 .env 文件配置参数
```

### 3. 启动开发服务器

```bash
npm run dev
```

服务器将在 http://localhost:3002 启动

### 4. 生产构建

```bash
npm run build
npm start
```

## API 端点

### 健康检查
- `GET /health` - 服务状态检查

### WebAuthn 认证
- `POST /api/webauthn/register/begin` - 开始注册 Passkey
- `POST /api/webauthn/register/finish` - 完成注册
- `POST /api/webauthn/authenticate/begin` - 开始认证
- `POST /api/webauthn/authenticate/finish` - 完成认证
- `GET /api/webauthn/stats` - 统计信息

### 会话管理
- `POST /api/auth/verify` - 验证会话
- `POST /api/auth/logout` - 登出

### 钱包操作
- `POST /api/wallet/create` - 创建钱包
- `POST /api/wallet/balance` - 查询余额
- `POST /api/wallet/transfer` - 转账
- `GET /api/wallet/list` - 列出钱包

## 使用示例

### 1. WebAuthn 注册流程

```javascript
// 1. 开始注册
const registerResponse = await fetch('/api/webauthn/register/begin', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    email: 'user@example.com',
    displayName: 'User Name'
  })
});

const { sessionId, options } = await registerResponse.json();

// 2. 浏览器 WebAuthn API
const credential = await navigator.credentials.create({
  publicKey: options
});

// 3. 完成注册
const finishResponse = await fetch('/api/webauthn/register/finish', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    email: 'user@example.com',
    registrationResponse: credential,
    challenge: options.challenge
  })
});

const { walletResult, userInstructions } = await finishResponse.json();

// 4. 用户保存恢复信息
localStorage.setItem('airAccountRecovery', JSON.stringify(userInstructions.recoveryInfo));
```

### 2. WebAuthn 认证流程

```javascript
// 1. 开始认证
const authResponse = await fetch('/api/webauthn/authenticate/begin', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    email: 'user@example.com'
  })
});

const { options } = await authResponse.json();

// 2. 浏览器 WebAuthn API
const assertion = await navigator.credentials.get({
  publicKey: options
});

// 3. 完成认证
const finishResponse = await fetch('/api/webauthn/authenticate/finish', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    email: 'user@example.com',
    authenticationResponse: assertion,
    challenge: options.challenge
  })
});

const { sessionId } = await finishResponse.json();
```

## 架构设计

### 数据流

```
浏览器 WebAuthn → Simple WebAuthn → CA HTTP API → TEE TA → 硬件安全
     ↓                 ↓                ↓          ↓          ↓
用户Passkey       challenge验证     临时会话    密钥管理    硬件随机数
```

### 用户凭证管理

```
客户端存储:
├── Passkey 凭证 (浏览器安全存储)
├── Email 地址
├── 恢复信息 (credentialId, walletId, ethereumAddress)
└── 私钥 (存储在 TEE 硬件中，通过 Passkey 访问)

CA 节点临时存储:
├── 会话信息 (短期)
├── Challenge 记录 (防重放)
└── 统计信息 (非关键)
```

### 安全特性

1. **用户凭证自主控制**
   - Passkey 存储在用户设备的安全硬件中
   - 私钥由 TEE 硬件管理，用户通过 Passkey 授权访问
   - 用户保留完整的恢复信息

2. **无密码认证**
   - 支持生物识别 (Face ID, Touch ID, Windows Hello)
   - 支持硬件安全密钥 (YubiKey, 等)
   - 防钓鱼和中间人攻击

3. **节点故障恢复**
   - 用户可在其他兼容节点恢复钱包访问
   - 恢复信息包含所有必要的凭证标识
   - TEE 私钥与用户 Passkey 绑定，节点无法控制

## 开发

### 项目结构

```
src/
├── index.ts           # 主应用入口
├── services/          # 核心服务
│   ├── webauthn.ts    # Simple WebAuthn 集成
│   ├── tee-client.ts  # TEE 客户端通信
│   └── database.ts    # 临时数据存储
└── routes/            # API 路由
    ├── webauthn.ts    # WebAuthn 端点
    ├── auth.ts        # 认证管理
    ├── wallet.ts      # 钱包操作
    └── health.ts      # 健康检查
```

### 开发命令

```bash
npm run dev      # 开发模式 (自动重启)
npm run build    # 编译 TypeScript
npm run start    # 启动生产服务器
npm run test     # 运行测试
npm run lint     # 代码检查
npm run format   # 代码格式化
```

## 部署

### Docker 部署

```bash
# 构建镜像
docker build -t airaccount-ca-nodejs .

# 运行容器
docker run -d \
  --name airaccount-ca \
  -p 3002:3002 \
  -e NODE_ENV=production \
  airaccount-ca-nodejs
```

### 生产环境注意事项

1. **HTTPS 必需**: WebAuthn 需要安全上下文
2. **域名配置**: 正确设置 RP_ID 和 WEBAUTHN_ORIGIN
3. **日志监控**: 配置适当的日志级别和监控
4. **数据备份**: 虽然只存储临时数据，但建议定期备份会话状态

## 与 Rust 版本的比较

| 特性 | Node.js 版本 | Rust 版本 |
|------|-------------|-----------|
| WebAuthn | Simple WebAuthn (完整) | 简化实现 |
| 性能 | 中等 | 高性能 |
| 开发体验 | 现代化 | 系统级 |
| 部署 | 容易 | 编译复杂 |
| 生态 | 丰富 | 专业 |

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request。

## 安全披露

如发现安全问题，请发邮件至 security@airaccount.dev