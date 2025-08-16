# AirAccount WebAuthn + Account Abstraction Demo

基于[passkey-demo](https://github.com/oceans404/passkey-demo)和[all-about-abstract-account](https://github.com/mingder78/all-about-abstract-account)最佳实践的综合演示。

## 🌟 特性展示

### 🔐 WebAuthn Passkey集成
- **无密码认证**: 使用设备生物识别（Touch ID、Face ID、Windows Hello）
- **客户端控制凭证**: 用户的Passkey存储在本地设备，服务器不保存私钥
- **跨设备同步**: 支持iCloud Keychain、Google Password Manager等同步
- **抗钓鱼攻击**: 内置域名绑定和挑战-响应机制

### ⚡ ERC-4337账户抽象
- **智能合约钱包**: 每个用户获得一个可编程的智能合约账户
- **Gasless交易**: 通过Paymaster赞助交易，用户无需持有ETH
- **批量执行**: 单次交易执行多个操作，节省gas费用
- **社交恢复**: 支持Guardian-based账户恢复机制

### 🔒 TEE硬件安全
- **混合熵源**: 结合工厂种子和TEE随机数生成更强的密钥
- **硬件隔离**: 私钥操作在TEE环境中执行，永不暴露到用户态
- **安全验证**: 实时验证TEE环境的完整性和安全状态

## 🚀 快速开始

### 1. 启动后端服务

```bash
# 启动Node.js CA服务
cd packages/airaccount-ca-nodejs
npm run build
npm start

# 或使用简化测试服务器
node test-basic-server.js
```

### 2. 访问演示页面

```bash
# 方法1: 使用Python简单服务器
cd packages/node-sdk/examples/webauthn-aa-demo
python3 -m http.server 3001

# 方法2: 使用Node.js http-server
npx http-server -p 3001 -c-1

# 然后访问 http://localhost:3001
```

### 3. 体验完整流程

1. **浏览器支持检查** - 验证WebAuthn功能可用性
2. **用户注册** - 创建Passkey并生成智能合约账户
3. **用户认证** - 使用生物识别登录（支持无密码模式）
4. **账户管理** - 查看余额、部署状态、执行交易
5. **安全验证** - 检查TEE环境和混合熵源状态

## 📋 API端点

### WebAuthn认证
```bash
# 开始注册
POST /api/webauthn/register/begin
{
  "email": "user@example.com",
  "displayName": "User Name"
}

# 完成注册
POST /api/webauthn/register/finish
{
  "email": "user@example.com",
  "registrationResponse": {...},
  "challenge": "..."
}

# 开始认证
POST /api/webauthn/authenticate/begin
{
  "email": "user@example.com"  // 可选，支持无密码模式
}

# 完成认证
POST /api/webauthn/authenticate/finish
{
  "email": "user@example.com",
  "authenticationResponse": {...},
  "challenge": "..."
}
```

### 账户抽象
```bash
# 创建抽象账户
POST /api/aa/create-account
{
  "sessionId": "...",
  "email": "user@example.com",
  "initialDeposit": "0.01",
  "recoveryGuardians": ["0x..."]
}

# 执行交易
POST /api/aa/execute-transaction
{
  "sessionId": "...",
  "to": "0x...",
  "value": "1000000000000000000",
  "usePaymaster": true
}

# 批量执行
POST /api/aa/execute-batch
{
  "sessionId": "...",
  "transactions": [
    {"to": "0x...", "value": "100"},
    {"to": "0x...", "data": "0x..."}
  ],
  "usePaymaster": false
}
```

### TEE安全
```bash
# 验证安全状态
GET /api/webauthn/security/verify

# 获取统计信息
GET /api/webauthn/stats
```

## 🏗️ 架构设计

### 客户端控制凭证模式
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Browser       │    │   Node.js CA    │    │   TEE (Rust)    │
│                 │    │                 │    │                 │
│ • Passkey Store │◄──►│ • Challenge API │◄──►│ • Private Keys  │
│ • WebAuthn API  │    │ • Session Mgmt  │    │ • Hybrid Entropy│
│ • User Control  │    │ • No Secrets    │    │ • Secure Ops    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### ERC-4337集成流程
```
1. WebAuthn认证 → 2. 智能账户创建 → 3. UserOperation构建 → 4. TEE签名 → 5. Bundler提交
```

## 🔧 开发指南

### 添加新的认证器支持
```javascript
// 检查特定认证器类型
const isRoamingAvailable = await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();

// 配置认证器偏好
const options = {
  authenticatorSelection: {
    authenticatorAttachment: 'platform', // 或 'cross-platform'
    userVerification: 'required',        // 或 'preferred', 'discouraged'
    residentKey: 'preferred'             // 或 'required', 'discouraged'
  }
};
```

### 集成自定义Paymaster
```typescript
// 实现Paymaster策略
interface PaymasterPolicy {
  canSponsor(userOp: UserOperation): boolean;
  getPaymasterData(userOp: UserOperation): Promise<string>;
}

class CustomPaymaster implements PaymasterPolicy {
  canSponsor(userOp: UserOperation): boolean {
    // 自定义赞助逻辑
    return true;
  }
  
  async getPaymasterData(userOp: UserOperation): Promise<string> {
    // 返回Paymaster签名数据
    return '0x...';
  }
}
```

### 扩展TEE功能
```rust
// 在TA中添加新的安全操作
#[no_mangle]
pub extern "C" fn TA_InvokeCommandEntryPoint(
    _sess_ctx: *mut c_void,
    cmd_id: u32,
    param_types: u32,
    params: *mut TEE_Param,
) -> TEE_Result {
    match cmd_id {
        CMD_CUSTOM_OPERATION => handle_custom_operation(param_types, params),
        _ => TEE_ERROR_BAD_PARAMETERS,
    }
}
```

## 🔒 安全考虑

### WebAuthn安全
- ✅ **防重放攻击**: 每次认证使用唯一challenge
- ✅ **防钓鱼攻击**: Passkey绑定到特定域名
- ✅ **用户验证**: 要求生物识别或PIN验证
- ✅ **凭证隔离**: 不同网站的凭证完全隔离

### 账户抽象安全
- ✅ **多重签名**: 支持Guardian多签恢复机制
- ✅ **Gas限制**: 防止无限gas消耗攻击
- ✅ **操作验证**: 关键操作需要额外确认
- ✅ **升级安全**: 账户逻辑升级需要时间锁

### TEE安全
- ✅ **内存保护**: 敏感数据仅存在于TEE内存
- ✅ **完整性验证**: 定期验证TEE环境完整性
- ✅ **密钥隔离**: 私钥永不离开硬件安全边界
- ✅ **熵源验证**: 验证随机数生成器质量

## 📚 参考资料

- [WebAuthn Guide](https://webauthn.guide/) - WebAuthn标准详解
- [ERC-4337](https://eips.ethereum.org/EIPS/eip-4337) - 账户抽象标准
- [SimpleWebAuthn](https://simplewebauthn.dev/) - WebAuthn库文档
- [OP-TEE](https://optee.readthedocs.io/) - 开源TEE实现
- [passkey-demo](https://github.com/oceans404/passkey-demo) - WebAuthn最佳实践
- [all-about-abstract-account](https://github.com/mingder78/all-about-abstract-account) - 账户抽象参考

## 🤝 贡献

欢迎提交Issue和Pull Request来改进这个演示！

## 📄 许可证

MIT License - 详见LICENSE文件