# AirAccount 用户故事集

> 基于 TEE 的去中心化 Web3 账户系统用户场景与开发指南

## 🎯 项目概览

AirAccount 是基于 TEE (Trusted Execution Environment) 的跨平台 Web3 账户系统，使用 WebAuthn/Passkey 技术提供硬件级安全保障。用户的私钥存储在设备的安全区域中，通过生物识别验证进行交易签名。

### 核心架构特点
- **用户控制**: 私钥和凭证完全由用户设备管理
- **双重签名**: 客户端(用户控制) + 服务端(TEE控制) 的渐进式去中心化
- **硬件安全**: 基于 ARM TrustZone 的 OP-TEE 实现
- **跨平台**: 支持浏览器、移动设备、硬件密钥

---

## 👥 普通用户故事

### 🔐 故事1: 首次注册 Web3 账户

**角色**: 新用户 Alice  
**目标**: 使用生物识别创建安全的 Web3 账户  

#### 用户旅程

1. **访问应用**
   ```
   Alice 打开浏览器访问 http://localhost:5175
   看到简洁的 AirAccount 界面
   ```

2. **输入身份信息**
   ```
   Alice 输入邮箱: alice@example.com
   系统提示她的邮箱将用作账户恢复标识
   ```

3. **设备兼容性检查**
   ```
   系统自动检测:
   ✅ WebAuthn支持: 是
   ✅ 平台认证器: 可用 (Touch ID)
   ✅ 用户验证: 支持
   显示绿色安全图标
   ```

4. **生物识别注册**
   ```
   Alice 点击"注册生物识别"按钮
   macOS 弹出 Touch ID 验证窗口
   Alice 用指纹完成验证
   ```

5. **账户创建成功**
   ```
   系统显示:
   ✅ 钱包ID: 920
   ✅ 以太坊地址: 0x000000000000000000000000000c1ceb19782a2c
   ✅ 凭证ID: dGVzdF9jcmVkZW50aWFsX2lk
   
   重要恢复信息:
   📧 Email: alice@example.com
   🔑 Credential ID: dGVzdF9jcmVkZW50aWFsX2lk
   💰 Wallet ID: 920
   📍 Ethereum Address: 0x000000000000000000000000000c1ceb19782a2c
   ```

#### 技术实现
- **前端**: React + Vite + SimpleWebAuthn Browser
- **后端**: Node.js + Express + SimpleWebAuthn Server
- **TEE**: QEMU OP-TEE 4.7 混合熵源生成
- **存储**: 用户设备 + 临时服务端验证

#### 价值体现
- ✅ **无密码**: 不需要记住复杂密码
- ✅ **硬件安全**: 私钥存储在设备安全区域
- ✅ **快速便捷**: 30秒完成整个注册流程
- ✅ **恢复保障**: 提供多重恢复信息

---

### 🔓 故事2: 日常登录与交易

**角色**: 现有用户 Bob  
**目标**: 快速登录并发起加密货币转账  

#### 用户旅程

1. **快速登录**
   ```
   Bob 访问应用，输入邮箱: bob@example.com
   点击"生物识别登录"
   Face ID 扫描完成，1秒内登录成功
   ```

2. **查看钱包状态**
   ```
   显示个人钱包信息:
   💰 ETH 余额: 2.5 ETH
   📍 地址: 0x1234...5678
   🔐 TEE安全状态: ✅ 已验证
   📱 注册设备: 2 个
   ```

3. **发起转账**
   ```
   Bob 填写转账信息:
   📤 收款地址: 0xabcd...efgh
   💎 金额: 0.1 ETH
   ⛽ Gas费: 0.002 ETH
   ```

4. **TEE签名确认**
   ```
   系统提示: "TEE正在生成安全签名..."
   Touch ID 二次验证
   交易签名完成: 0x789abc...def123
   ```

5. **交易完成**
   ```
   ✅ 交易哈希: 0x789abc...def123
   ⏱️ 预计确认时间: 30秒
   💰 剩余余额: 2.398 ETH
   ```

#### 技术细节
- **认证流程**: WebAuthn Challenge → Touch ID → 服务端验证
- **签名过程**: TEE 混合熵源 → 客户端+服务端双重签名
- **交易广播**: 直接发送到以太坊网络

---

### 🔧 故事3: 设备丢失恢复

**角色**: 用户 Carol  
**场景**: 手机丢失，需要在新设备上恢复账户  

#### 恢复流程

1. **准备恢复信息**
   ```
   Carol 在新设备上找到之前保存的恢复信息:
   📧 Email: carol@example.com
   🔑 Credential ID: oldDevice123...
   💰 Wallet ID: 456
   📍 Ethereum Address: 0x9876...5432
   ```

2. **启动恢复流程**
   ```
   在新设备上访问 AirAccount
   选择"恢复现有账户"
   输入邮箱和钱包ID
   ```

3. **新设备认证**
   ```
   系统生成新的 WebAuthn Challenge
   Carol 在新设备上完成 Face ID 注册
   获得新的 Credential ID: newDevice789...
   ```

4. **账户迁移完成**
   ```
   ✅ 新设备已添加到账户
   ✅ 原以太坊地址保持不变
   ✅ 资产完全恢复
   📱 设备列表: 旧设备(已失效) + 新设备(活跃)
   ```

#### 安全保障
- **多设备支持**: 单个账户可绑定多个设备
- **设备撤销**: 可远程禁用丢失设备的访问权限
- **地址不变**: 以太坊地址保持恒定，资产安全

---

## 👨‍💻 开发者故事

### 🛠️ 故事1: 集成 WebAuthn API

**角色**: 前端开发者 David  
**目标**: 在 DApp 中集成 AirAccount WebAuthn 功能  

#### 开发流程

1. **环境搭建**
   ```bash
   # 克隆项目
   git clone https://github.com/AAStarCommunity/AirAccount
   cd AirAccount
   
   # 初始化子模块
   git submodule update --init --recursive
   
   # 安装依赖
   cd packages/airaccount-ca-nodejs
   npm install
   ```

2. **启动开发环境**
   ```bash
   # 终端1: 启动 TEE 环境
   cd third_party/incubator-teaclave-trustzone-sdk/tests/
   ./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04
   
   # 终端2: 启动后端服务
   cd packages/airaccount-ca-nodejs
   npm run dev
   
   # 终端3: 启动前端 Demo
   cd demo-real
   npm install && npm run dev
   ```

3. **API 集成代码**
   ```typescript
   // 注册用户 Passkey
   const registerResponse = await fetch('/api/webauthn/register/begin', {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({
       email: 'user@example.com',
       displayName: 'User Name'
     })
   });
   
   const { options, sessionId } = await registerResponse.json();
   
   // 使用浏览器 WebAuthn API
   import { startRegistration } from '@simplewebauthn/browser';
   const credential = await startRegistration(options);
   
   // 完成注册
   const finishResponse = await fetch('/api/webauthn/register/finish', {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({
       email: 'user@example.com',
       registrationResponse: credential,
       challenge: options.challenge
     })
   });
   ```

4. **测试验证**
   ```bash
   # 运行自动化测试
   node scripts/test/test-webauthn-complete-flow.js
   
   # 健康检查
   curl http://localhost:3002/health
   
   # API 功能测试
   curl -X POST http://localhost:3002/api/webauthn/stats
   ```

#### 集成要点
- **环境变量**: 设置 `WEBAUTHN_TEST_MODE=true` 用于开发测试
- **HTTPS 要求**: 生产环境必须使用 HTTPS
- **域名配置**: 正确设置 RP ID 和 Origin
- **错误处理**: 实现用户友好的错误提示

---

### 🔍 故事2: TEE 开发与调试

**角色**: 安全开发者 Eve  
**目标**: 开发自定义 TEE 应用并集成 AirAccount  

#### 开发流程

1. **TEE 环境配置**
   ```bash
   # 安装 OP-TEE 工具链
   cd third_party/build
   make -j$(nproc) toolchains
   
   # 构建 QEMU 环境
   make -j$(nproc) -f qemu_v8.mk all
   
   # 测试环境
   make -f qemu_v8.mk run
   ```

2. **Trusted Application 开发**
   ```rust
   // packages/airaccount-ta-simple/src/main.rs
   use optee_utee::{
       ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session,
       trace_println, ErrorKind, Parameters, Result,
   };
   
   #[ta_create]
   fn create() -> Result<()> {
       trace_println!("AirAccount TA: 创建会话");
       Ok(())
   }
   
   #[ta_invoke_command]
   fn invoke_command(cmd_id: u32, params: Parameters) -> Result<()> {
       match cmd_id {
           0 => hello_world(params),
           1 => create_wallet(params),
           2 => sign_transaction(params),
           _ => Err(ErrorKind::BadParameters.into()),
       }
   }
   ```

3. **Client Application 开发**
   ```rust
   // packages/airaccount-ca/src/main.rs
   use optee_teec::{Context, Operation, Session, Uuid};
   
   fn main() -> Result<()> {
       let mut ctx = Context::new()?;
       let uuid = Uuid::parse_str("11223344-5566-7788-99aa-bbccddeeff01")?;
       
       let mut session = ctx.open_session(uuid)?;
       
       // 调用 TEE 创建钱包
       let mut operation = Operation::new(0, None, None);
       session.invoke_command(1, &mut operation)?;
       
       Ok(())
   }
   ```

4. **集成测试**
   ```bash
   # 构建 TA
   cd packages/airaccount-ta-simple
   make clean && make
   
   # 构建 CA
   cd packages/airaccount-ca
   cargo build --target aarch64-unknown-linux-gnu
   
   # 集成测试
   ./scripts/test/test_airaccount_integration.sh
   ```

#### 关键技术点
- **混合熵源**: 结合 TEE 硬件随机数和外部熵源
- **安全存储**: 使用 OP-TEE 安全对象存储私钥
- **签名算法**: 支持 ECDSA secp256k1 以太坊标准
- **内存保护**: TEE 环境提供内存隔离和防护

---

### 📱 故事3: 移动端适配

**角色**: 移动开发者 Frank  
**目标**: 将 AirAccount 集成到 iOS/Android 应用  

#### 技术方案

1. **Tauri 跨平台方案**
   ```toml
   # Cargo.toml
   [dependencies]
   tauri = { version = "1.0", features = ["api-all"] }
   tokio = { version = "1.0", features = ["full"] }
   serde = { version = "1.0", features = ["derive"] }
   
   [target.'cfg(target_os = "ios")'.dependencies]
   security-framework = "2.0"
   
   [target.'cfg(target_os = "android")'.dependencies]
   jni = "0.19"
   ```

2. **WebAuthn 移动适配**
   ```javascript
   // src-tauri/webauthn-mobile.js
   import { invoke } from '@tauri-apps/api/tauri';
   
   // iOS Touch ID / Face ID
   async function authenticateIOS() {
     return await invoke('ios_biometric_auth', {
       reason: 'AirAccount 需要验证您的身份'
     });
   }
   
   // Android Fingerprint / Face unlock
   async function authenticateAndroid() {
     return await invoke('android_biometric_auth', {
       title: 'AirAccount 身份验证',
       subtitle: '请使用生物识别验证'
     });
   }
   ```

3. **原生集成代码**
   ```rust
   // src-tauri/src/mobile.rs
   #[cfg(target_os = "ios")]
   #[tauri::command]
   async fn ios_biometric_auth(reason: String) -> Result<bool, String> {
       use security_framework::os::macos::keychain::SecAccessControl;
       // 实现 iOS Touch ID / Face ID 集成
       Ok(true)
   }
   
   #[cfg(target_os = "android")]
   #[tauri::command]
   async fn android_biometric_auth(title: String) -> Result<bool, String> {
       // 实现 Android 生物识别集成
       Ok(true)
   }
   ```

4. **部署配置**
   ```bash
   # iOS 构建
   cargo tauri ios build
   
   # Android 构建  
   cargo tauri android build
   
   # 统一部署
   cargo tauri build --target universal-apple-darwin
   ```

#### 移动端特性
- **原生生物识别**: 集成 Touch ID, Face ID, 指纹识别
- **安全存储**: 使用设备 Keychain/Keystore
- **离线签名**: 支持无网络环境下的交易签名
- **推送通知**: 交易确认和安全提醒

---

## 🔧 开发环境快速启动

### 🚀 一键启动脚本

```bash
#!/bin/bash
# quick-start.sh

echo "🚀 启动 AirAccount 开发环境..."

# 检查依赖
command -v node >/dev/null 2>&1 || { echo "需要安装 Node.js"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "需要安装 Rust"; exit 1; }

# 启动 TEE 环境 (后台)
cd third_party/incubator-teaclave-trustzone-sdk/tests/
./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04 &
sleep 10

# 启动后端服务 (后台)
cd ../../packages/airaccount-ca-nodejs
npm install
npm run dev &

# 启动前端 Demo
cd ../demo-real  
npm install
npm run dev

echo "✅ 环境启动完成!"
echo "📱 前端: http://localhost:5175"
echo "🔧 后端: http://localhost:3002"
echo "💊 健康检查: curl http://localhost:3002/health"
```

### 📋 测试检查清单

```bash
# 🔍 系统检查
□ curl http://localhost:3002/health
□ curl http://localhost:3002/api/webauthn/stats
□ node scripts/test/test-webauthn-complete-flow.js

# 🌐 浏览器测试
□ 访问 http://localhost:5175
□ 完成 WebAuthn 注册
□ 验证账户创建

# 🛠️ 开发工具
□ 查看 TEE 日志: tail -f qemu-console.log
□ 查看 CA 日志: tail -f packages/airaccount-ca-nodejs/logs/
□ 数据库检查: sqlite3 packages/airaccount-ca-nodejs/airaccount.db
```

---

## 📖 最佳实践指南

### 🔐 安全最佳实践

1. **私钥管理**
   - ✅ 私钥永远不离开 TEE 环境
   - ✅ 使用硬件随机数生成器
   - ✅ 实施密钥轮换策略

2. **WebAuthn 配置**
   - ✅ 设置正确的 RP ID 和 Origin
   - ✅ 启用用户验证 (UV)
   - ✅ 使用 Resident Key

3. **错误处理**
   - ✅ 不泄露敏感错误信息
   - ✅ 实施重试限制
   - ✅ 记录安全事件

### 🚀 性能优化

1. **响应时间目标**
   - 健康检查: < 100ms
   - 注册流程: < 500ms  
   - 认证流程: < 300ms
   - 交易签名: < 1s

2. **资源优化**
   - 复用 TEE 会话
   - 缓存 WebAuthn Challenge
   - 异步处理长任务

### 🌍 部署考虑

1. **生产环境**
   - 使用真实的 TEE 硬件 (树莓派5)
   - 配置 HTTPS 和域名
   - 设置监控和日志

2. **扩展性**
   - 水平扩展 CA 服务
   - 负载均衡和故障转移
   - 数据库分片策略

---

## 📞 支持与社区

### 🆘 问题排查

- **文档**: [完整测试指南](MANUAL_TESTING_GUIDE.md)
- **示例**: [Demo 应用代码](../demo-real/)
- **测试**: [自动化测试脚本](../scripts/test/)

### 🤝 贡献指南

```bash
# 1. Fork 项目
git clone https://github.com/your-username/AirAccount
cd AirAccount

# 2. 创建功能分支
git checkout -b feature/your-feature

# 3. 提交更改
git commit -m "feat: 添加新功能"

# 4. 推送和创建 PR
git push origin feature/your-feature
```

### 📧 联系方式

- **GitHub Issues**: [项目问题追踪](https://github.com/AAStarCommunity/AirAccount/issues)
- **技术讨论**: [社区论坛](#)
- **安全问题**: [安全报告邮箱](#)

---

*AirAccount - 让 Web3 账户管理变得简单而安全* 🚀