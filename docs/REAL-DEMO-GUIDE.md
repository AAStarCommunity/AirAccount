# AirAccount 真实Demo启动指南

## 🎯 最终架构

```
demo-real/ (React + 真实WebAuthn)
    ↓ HTTP API
ca-service-real/ (Express + @simplewebauthn/server + SQLite)
    ↓ 模拟TEE调用
Mock TEE (真实TA将在这里集成)
```

## 🚀 快速启动

### 1. 启动真实CA服务

```bash
cd ca-service-real
npm install
npm run dev

# 服务运行在 http://localhost:3002
# ✅ 支持真实WebAuthn/Passkey
# ✅ SQLite数据库存储
# ✅ 挑战-响应验证
```

### 2. 启动真实Demo

```bash
cd demo-real
npm install
npm run dev

# 前端运行在 http://localhost:5174
# ✅ 真实浏览器Passkey注册
# ✅ 真实生物识别验证
# ✅ 浏览器兼容性检查
```

### 3. 测试流程

1. **浏览器要求**: Chrome 67+、Firefox 60+、Safari 14+
2. **访问**: http://localhost:5174
3. **注册**: 输入邮箱 → 触发真实Passkey注册
4. **生物识别**: 使用指纹/面容ID完成注册
5. **查看结果**: 显示真实以太坊地址和账户信息

## 📁 目录结构

```
AirAccount/
├── airaccount-sdk-real/     # 最终SDK (真实HTTP客户端)
├── ca-service-real/         # 真实CA服务 (WebAuthn + SQLite)
├── demo-real/              # 真实Demo (React + 真实Passkey)
├── packages/core-logic/    # Rust核心逻辑
└── third_party/           # OP-TEE组件
```

## 🔑 真实功能

### CA服务 (ca-service-real/)
- ✅ 真实WebAuthn挑战生成
- ✅ Passkey注册验证
- ✅ Passkey认证验证  
- ✅ SQLite数据持久化
- ✅ 凭证计数器跟踪
- ✅ 挑战过期管理

### Demo应用 (demo-real/)
- ✅ 浏览器WebAuthn支持检查
- ✅ 真实Passkey注册流程
- ✅ 设备兼容性验证
- ✅ 错误处理和用户引导
- ✅ 本地账户状态管理

### SDK (airaccount-sdk-real/)
- ✅ 纯HTTP API客户端
- ✅ 正确的架构分离
- ✅ TypeScript类型安全
- ✅ 错误处理和重试
- ✅ 事件系统

## 🧪 与TA集成

当真实TA准备好时，只需要修改：

```typescript
// 在 ca-service-real/src/webauthn-service.ts
private async callTEECreateAccount(email: string, publicKey: Uint8Array) {
  // 替换为真实TA调用
  return await realTA.createAccount(email, publicKey)
}
```

## 🔧 环境变量

**ca-service-real/.env**
```bash
PORT=3002
RP_ID=localhost               # 生产环境: yourdomain.com
ORIGIN=http://localhost:3002  # 生产环境: https://yourdomain.com
```

## 🚨 注意事项

1. **HTTPS要求**: 生产环境必须使用HTTPS
2. **域名设置**: RP_ID必须与域名匹配
3. **浏览器兼容**: 检查WebAuthn支持
4. **数据库**: SQLite文件在ca-service-real/airaccount.db

## ✅ 验证成功

当您看到以下输出，说明一切正常：

**CA服务控制台:**
```
🚀 AirAccount CA Service (Real)
📡 Server running on http://localhost:3002
🔑 Real WebAuthn/Passkey features:
  ✓ Real browser Passkey registration
  ✓ Real biometric authentication
  ✓ SQLite database storage
```

**Demo控制台:**
```
🔑 Starting Passkey registration...
📋 获取到注册挑战: {challenge, userId}
✅ Passkey registration successful
✅ 账户创建成功
```

## 🔄 下一步

1. 集成真实OP-TEE TA
2. 添加转账功能（需Passkey认证）
3. 集成真实区块链
4. 部署到HTTPS域名