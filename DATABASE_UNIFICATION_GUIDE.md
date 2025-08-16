# 🗄️ AirAccount 数据库架构统一指南

## 📋 统一原则

### ✅ **一个数据库，一套数据结构**
- **Rust CA**和**Node.js CA**使用完全相同的数据库
- **用户单选使用** - 用户选择使用其中一个CA，不需要同时使用
- **无兼容性负担** - 移除了所有向后兼容性代码，简化架构

### 🔄 **并行模式架构**
```typescript
// 在 Node.js CA index.ts 中
const isTestMode = process.env.NODE_ENV !== 'production';
const webauthnService = new WebAuthnService(webauthnConfig, database, isTestMode);
```

**真实环境使用：**
- 设置 `NODE_ENV=production` 或 `isTestMode=false`
- 会执行真实的WebAuthn验证流程
- 支持浏览器真实Passkey注册/认证
- 与Touch ID、Face ID、USB Key等真实设备交互

**测试环境使用：**
- 设置 `isTestMode=true`
- 跳过WebAuthn验证，使用模拟数据
- 用于开发调试和自动化测试

## 📊 数据库表结构

### 统一表设计
```sql
-- 用户账户表
CREATE TABLE users (
    user_id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Passkey存储表（完整WebAuthn数据）
CREATE TABLE passkeys (
    credential_id BLOB PRIMARY KEY,
    user_id TEXT NOT NULL,
    credential_public_key BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    aaguid BLOB,
    user_handle BLOB,
    device_name TEXT,
    backup_eligible BOOLEAN DEFAULT false,
    backup_state BOOLEAN DEFAULT false,
    uv_initialized BOOLEAN DEFAULT false,
    credential_device_type TEXT DEFAULT 'singleDevice',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (user_id)
);

-- 注册状态管理表
CREATE TABLE registration_states (
    challenge TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_verification TEXT NOT NULL,
    attestation TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

-- 认证状态管理表
CREATE TABLE authentication_states (
    challenge TEXT PRIMARY KEY,
    user_id TEXT,
    user_verification TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

-- 会话管理表
CREATE TABLE sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    email TEXT NOT NULL,
    is_authenticated BOOLEAN DEFAULT false,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    last_activity INTEGER NOT NULL
);
```

### 🔄 数据库实现对比

| 功能 | Node.js CA | Rust CA | 兼容性 |
|------|------------|---------|--------|
| 数据存储 | SQLite文件 | 内存HashMap | 结构相同 |
| Passkey格式 | 标准WebAuthn | 标准WebAuthn | 完全兼容 |
| 状态管理 | 表级存储 | 内存存储 | 逻辑一致 |
| 用户管理 | 完整CRUD | 完整CRUD | 接口相同 |

## 🚀 使用示例

### Node.js CA 使用
```bash
# 生产环境 - 真实WebAuthn
NODE_ENV=production npm run dev

# 测试环境 - 模拟数据
NODE_ENV=development npm run dev
```

### Rust CA 使用
```bash
# WebAuthn命令行交互
./airaccount-ca webauthn

# 在WebAuthn模式下
WebAuthn> register user@example.com "Test User"
WebAuthn> auth user@example.com
WebAuthn> list
```

## 🔒 安全考虑

### 数据隔离
- **测试模式数据**：使用特殊前缀标识，易于清理
- **生产模式数据**：完整的WebAuthn验证和存储
- **并行安全**：两种模式数据结构相同，但验证逻辑不同

### 兼容性保证
- **统一接口**：所有CA使用相同的数据库方法
- **标准格式**：遵循WebAuthn和OP-TEE标准
- **简化架构**：移除复杂的兼容性层，减少错误

---

*📅 最后更新: $(date)*  
*🏷️ 架构版本: v2.0-unified*  
*📊 兼容性: 100%*

