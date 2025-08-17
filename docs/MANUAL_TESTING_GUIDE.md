# AirAccount 手工测试完整指南

## 🎯 测试目标

验证 **QEMU → TA → CA(Node.js CA, Rust CA) → WebAuthn → Demo** 完整调用链，确保WebAuthn/Passkey正常工作。

### 📋 完整测试流程

```
QEMU OP-TEE 环境 (基础TEE环境)
    ↓
TA (Trusted Application) (安全世界应用)
    ↓
CA (Client Application) 分为三种类型:
    ├── Basic CA-TA (框架测试) → 基础通信验证
    ├── Simple CA-TA (功能测试) → 钱包和WebAuthn功能
    └── Real CA-TA (生产版本) → 完整优化版本 (待实现)
    
其中Simple CA分为两种实现:
    ├── Node.js CA (Web API服务) → WebAuthn服务 → Demo前端
    └── Rust CA (CLI工具) → WebAuthn功能测试
```

## 📦 CA-TA架构说明

基于eth_wallet通信标准，项目重新组织为三种类型：

### 1. Basic CA-TA（基础框架测试）
- **位置**: `packages/airaccount-basic/`
- **目的**: 验证最基本的CA-TA通信机制
- **功能**: Hello World, Echo, Version  
- **通信模式**: 严格遵循eth_wallet三参数标准
- **用途**: 框架和通信机制测试

### 2. Simple CA-TA（功能测试）
- **位置**: `packages/airaccount-ca/` 和 `packages/airaccount-ta-simple/`
- **目的**: 测试钱包和WebAuthn等业务功能
- **功能**: 钱包管理, 混合熵源, 安全验证, WebAuthn集成
- **通信模式**: 基于Basic的标准模式扩展
- **用途**: 完整功能测试

### 3. Real CA-TA（生产版本）
- **位置**: `packages/airaccount-real/` (待实现)
- **目的**: 未来的完整生产版本
- **功能**: 高性能优化, 完整安全机制
- **用途**: 生产环境部署

### 🔧 CA-TA通信标准

**关键修复**: 基于eth_wallet标准的三参数模式

```rust
// ✅ 正确的CA端参数设置
let p0 = ParamTmpRef::new_input(input_data);           // 输入数据
let p1 = ParamTmpRef::new_output(output_buffer);       // 输出缓冲区  
let p2 = ParamValue::new(0, 0, ParamType::ValueInout); // 输出长度值
let mut operation = Operation::new(0, p0, p1, p2, ParamNone);

// ✅ 对应的TA端处理
let mut p0 = unsafe { params.0.as_memref()? };
let mut p1 = unsafe { params.1.as_memref()? }; 
let mut p2 = unsafe { params.2.as_value()? };
// 设置输出数据和长度
p1.buffer()[..output_len].copy_from_slice(&output_data);
p2.set_a(output_len as u32);  // 关键：设置输出长度

// ✅ CA端读取结果
let output_len = operation.parameters().2.a() as usize;
let response = String::from_utf8_lossy(&output[..output_len]);
```

**错误模式** (之前的问题):
```rust
// ❌ 错误：缺少p2参数
let mut operation = Operation::new(0, p0, p1, ParamNone, ParamNone);
```

## 📋 测试前准备

### 环境检查清单

**🖥️ 基础环境层:**
- [ ] QEMU OP-TEE 4.7环境正常运行
- [ ] TEE设备(/dev/teepriv0)可访问
- [ ] OP-TEE内核模块已加载

**🔒 TA(安全世界)层:**
- [ ] AirAccount TA已构建并部署到QEMU
- [ ] TA文件在/lib/optee_armtz/可访问
- [ ] TA UUID (11223344-5566-7788-99aa-bbccddeeff01)正确

**⚙️ CA(普通世界)层:**
- [ ] Node.js CA服务可启动 (端口3002)
- [ ] Rust CA工具已构建并可执行
- [ ] CA能与TA正常通信

**🌐 WebAuthn层:**
- [ ] 浏览器支持WebAuthn (Chrome/Safari)
- [ ] Demo前端应用可启动 (端口5174)
- [ ] WebAuthn API端点响应正常

### 1. 启动QEMU TEE环境

```bash
# 终端1: 启动QEMU TEE环境
cd third_party/incubator-teaclave-trustzone-sdk/tests/
./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04

# 或者使用已验证的集成测试脚本
./test_airaccount_fixed.sh

# 等待看到QEMU完全启动的提示
# 保持此终端运行
```

### 2. 验证TA构建状态

```bash
# 检查TA是否已构建
cd packages/airaccount-ta-simple
ls -la target/aarch64-unknown-optee/debug/*.ta

# 如果没有文件，执行构建
# 注意：必须在配置了OP-TEE工具链的环境下执行
make clean && make

# 或者使用现有的预编译文件（用于测试）
ls -la third_party/incubator-teaclave-trustzone-sdk/tests/shared/
```

### 3. 准备CA服务

```bash
cd packages/airaccount-ca-nodejs
npm install

# 检查依赖是否正确安装
npm list @simplewebauthn/server
```

## 🚀 分层测试方案

按照 **QEMU → TA → CA → WebAuthn → Demo** 流程进行逐层验证：

### 阶段0: QEMU环境与TA功能测试

这是**最关键的基础测试阶段** - 验证QEMU OP-TEE环境正常运行，TA(Trusted Application)正确加载并能响应CA(Client Application)调用。

**测试重点**: 确保TEE基础环境和安全世界应用正常工作

#### 步骤0.1: 在QEMU中直接测试TA通信

```bash
# 确保QEMU已启动且TA文件已复制
# 在QEMU console中执行以下命令:

# 1. 检查TA文件是否存在
ls -la /lib/optee_armtz/*.ta

# 2. 检查CA二进制文件
ls -la shared/airaccount-ca

# 3. 测试Hello World命令
./shared/airaccount-ca hello

# 期望输出:
# 🔧 Initializing AirAccount Client...
# ✅ TEE Context created successfully  
# ✅ Session opened with AirAccount TA (UUID: 11223344-5566-7788-99aa-bbccddeeff01)
# 📞 Calling Hello World command...
# ✅ Hello World response: Hello from AirAccount TA!

# 4. 测试Echo命令
./shared/airaccount-ca echo "Hello TEE Test"

# 期望输出:
# 📞 Calling Echo command with: 'Hello TEE Test'
# ✅ Echo response: Hello TEE Test

# 5. 测试完整测试套件
./shared/airaccount-ca test

# 期望输出:
# 🧪 === AirAccount TA-CA Communication Tests ===
# Test 1 - Hello World: ✅ PASS
# Test 2 - Echo Simple: ✅ PASS  
# Test 3 - Echo UTF-8: ✅ PASS
# Test 4 - Echo Empty: ✅ PASS
# Test 5 - Multiple Operations: ✅ PASS (5/5 operations)
# 📊 Results: 5/5 tests passed (100.0%)
# 🎉 All tests PASSED! TA-CA communication is working perfectly!

# 6. 测试安全状态验证
./shared/airaccount-ca security

# 期望输出:
# 📞 Verifying TEE security state...
# ✅ Security state verified: TEE_SECURE_READY

# 7. 测试混合熵源账户创建
./shared/airaccount-ca hybrid test@example.com

# 期望输出:
# 📞 Creating hybrid account for user: test@example.com
# ✅ Hybrid account created: ACCOUNT_ID_12345
```

#### 步骤0.2: 使用集成测试脚本验证

```bash
# 在主机上运行集成测试脚本(自动化版本)
cd third_party/incubator-teaclave-trustzone-sdk/tests/
./test_airaccount_fixed.sh

# 期望看到完整的测试输出包括:
# ✅ AirAccount TA-CA Integration Test PASSED!
# ✅ Core functionality verified in OP-TEE environment
# 📊 Test Results Summary: Successful checks: 5/5
```

#### 步骤0.3: TA测试验收标准

**必须通过的检查点:**
- [ ] TA文件正确安装到 `/lib/optee_armtz/`
- [ ] CA成功连接到TA (UUID: 11223344-5566-7788-99aa-bbccddeeff01)
- [ ] Hello World命令返回正确响应
- [ ] Echo命令能正确回显各种输入
- [ ] 完整测试套件5/5通过
- [ ] TEE安全状态验证通过
- [ ] 混合熵源账户创建功能正常

**如果TA测试失败，请检查:**
```bash
# 1. 检查TA文件权限
ls -la /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta

# 2. 检查OP-TEE日志
dmesg | grep -i optee

# 3. 检查TEE设备
ls -la /dev/tee*

# 4. 重新安装TA
cp shared/*.ta /lib/optee_armtz/
chmod 444 /lib/optee_armtz/*.ta
```

⚠️ **重要**: 只有TA测试全部通过后，才能进行后续的CA服务和WebAuthn测试！

---

### 阶段1: Node.js CA服务测试

**测试重点**: 验证Node.js CA与QEMU中的TA正常通信，提供WebAuthn HTTP API服务

#### 步骤1.1: 启动CA服务

```bash
# 终端2: 启动Node.js CA
cd packages/airaccount-ca-nodejs
npm run dev

# 期望输出:
# 🚀 AirAccount CA Service
# 📡 Server running on http://localhost:3002
# 🔑 WebAuthn features enabled
```

#### 步骤1.2: 健康检查

```bash
# 终端3: 测试基础连接
curl http://localhost:3002/health

# 期望输出:
# {
#   "status": "healthy",
#   "teeConnection": true/false,
#   "database": true/false,
#   "timestamp": "..."
# }
```

#### 步骤1.3: TEE连接验证

```bash
# 如果 healthcheck 显示 TEE连接异常，检查QEMU状态
curl http://localhost:3002/api/webauthn/security/verify

# 期望输出:
# {
#   "securityState": {
#     "verified": true,
#     "details": {...}
#   }
# }

# 如果TEE连接失败，检查QEMU进程
ps aux | grep qemu
```

### 阶段2: WebAuthn API功能测试

**测试重点**: 验证Node.js CA提供的WebAuthn HTTP API端点功能正常

#### 步骤2.1: 注册流程开始

```bash
# 测试注册选项生成
curl -X POST http://localhost:3002/api/webauthn/register/begin \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "displayName": "Test User"
  }' | jq

# 期望输出:
# {
#   "options": {
#     "challenge": "...",
#     "rp": {"name": "AirAccount", "id": "localhost"},
#     "user": {...}
#   },
#   "sessionId": "...",
#   "notice": {...}
# }
```

#### 步骤2.2: 认证流程开始

```bash
# 测试认证选项生成
curl -X POST http://localhost:3002/api/webauthn/authenticate/begin \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com"
  }' | jq

# 期望输出:
# {
#   "options": {
#     "challenge": "...",
#     "allowCredentials": [...]
#   },
#   "notice": {...}
# }
```

### 阶段3: WebAuthn完整流程自动化测试

**测试重点**: 使用自动化脚本验证完整的WebAuthn注册→认证流程

#### 步骤3.1: 运行完整WebAuthn流程测试

```bash
# 使用现有的测试脚本
node scripts/test/test-webauthn-complete-flow.js

# 如果失败，查看详细错误信息
```

#### 步骤3.2: 运行SDK集成测试

```bash
cd packages/airaccount-sdk-test
npm install
node test-ca-integration.js

# 期望看到完整的调用链测试
```

### 阶段4: 浏览器Demo完整流程测试

**测试重点**: 在真实浏览器环境中测试完整的WebAuthn/Passkey用户体验

#### 步骤4.1: 启动真实Demo

```bash
# 终端4: 启动React Demo
cd demo-real
npm install
npm run dev

# 前端运行在 http://localhost:5174
```

#### 步骤4.2: 浏览器WebAuthn测试

1. 打开 Chrome/Safari: http://localhost:5174
2. 输入邮箱地址
3. 点击"注册Passkey"
4. 完成生物识别验证
5. 查看账户创建结果

### 阶段5: Rust CA CLI工具测试

**🎉 更新**: Rust CA现在**完全支持WebAuthn功能**！

**测试重点**: 验证Rust CA作为CLI工具的TA通信和WebAuthn功能

#### 步骤5.1: Rust CA基础TEE通信测试

```bash
# 终端5: 测试Rust CA基础功能
cd packages/airaccount-ca

# 构建Rust CA (如果尚未构建)
cargo build --target aarch64-unknown-linux-gnu --release

# 运行基础测试套件
./target/aarch64-unknown-linux-gnu/release/airaccount-ca test

# 测试传统交互模式 (TEE通信)
./target/aarch64-unknown-linux-gnu/release/airaccount-ca interactive
```

#### 步骤5.2: Rust CA WebAuthn功能测试

```bash
# 🔑 启动WebAuthn模式
./target/aarch64-unknown-linux-gnu/release/airaccount-ca webauthn

# 在WebAuthn模式下测试以下命令:

# 1. 注册新用户 (生成Passkey)
WebAuthn> register test@example.com "Test User"

# 2. 列出已注册用户
WebAuthn> list

# 3. 查看用户信息
WebAuthn> info test@example.com

# 4. 启动认证流程
WebAuthn> auth test@example.com

# 5. 退出WebAuthn模式
WebAuthn> quit
```

#### 步骤5.3: Rust CA完整WebAuthn流程测试

**真实WebAuthn注册→认证流程**:

```bash
# 第1步: 启动Rust CA WebAuthn服务
./target/aarch64-unknown-linux-gnu/release/airaccount-ca webauthn

# 第2步: 注册新用户passkey
WebAuthn> register user@airaccount.com "AirAccount User"
# 输出: Registration challenge created
# 包含完整的WebAuthn challenge JSON

# 第3步: 验证用户已注册
WebAuthn> list
# 输出: Registered users: user@airaccount.com

# 第4步: 查看用户详细信息
WebAuthn> info user@airaccount.com
# 输出: User info包含credential数量

# 第5步: 启动认证流程
WebAuthn> auth user@airaccount.com
# 输出: Authentication challenge created
# 包含完整的WebAuthn authentication JSON
```

#### 步骤5.4: CA架构对比 (更新)

| 功能 | Node.js CA | Rust CA | 状态 |
|------|------------|---------|------|
| WebAuthn注册 | ✅ 完整支持 | ✅ 完整支持 | 两者都支持 |
| Passkey认证 | ✅ 完整支持 | ✅ 完整支持 | 两者都支持 |
| Challenge生成 | ✅ SimpleWebAuthn | ✅ webauthn-rs | 不同库实现 |
| TEE通信 | ✅ 间接调用 | ✅ 直接调用 | 都支持TA通信 |
| 钱包功能 | ✅ 高级接口 | ✅ 底层接口 | 不同抽象层级 |
| HTTP API | ✅ REST服务 | ❌ CLI工具 | 不同交互方式 |
| 数据存储 | ✅ SQLite | ✅ 内存存储 | 不同持久化方案 |

**新架构说明**: 
- **Node.js CA**: Web服务形式的WebAuthn API，提供HTTP端点
- **Rust CA**: 命令行形式的WebAuthn工具，提供CLI交互
- **WebAuthn兼容**: 两者都生成标准的WebAuthn challenge和response格式
- **用途互补**: Node.js CA用于Web集成，Rust CA用于开发测试和CLI场景

## 🔧 问题修复方案

### 修复Challenge过期问题

基于分析，需要修复挑战验证逻辑：

#### 检查数据库挑战管理

```bash
# 检查SQLite数据库状态
cd packages/airaccount-ca-nodejs
sqlite3 airaccount.db ".tables"
sqlite3 airaccount.db "SELECT * FROM challenges ORDER BY created_at DESC LIMIT 5;"
```

#### 查看日志详情

```bash
# 启用调试模式
DEBUG=airaccount:* npm run dev

# 查看WebAuthn服务日志
tail -f logs/webauthn-service.log
```

### 常见问题排查

#### 1. QEMU TEE环境问题

```bash
# 检查QEMU进程
ps aux | grep qemu

# 重启QEMU TEE环境
cd third_party/incubator-teaclave-trustzone-sdk/tests/
# 关闭现有QEMU进程
pkill -f qemu-system-aarch64

# 重新启动
./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04
```

#### 2. CA服务端口冲突

```bash
# 检查端口占用
lsof -i :3002

# 杀死占用进程
kill $(lsof -t -i:3002)
```

#### 3. WebAuthn浏览器兼容性

```bash
# 测试WebAuthn可用性 (在浏览器控制台执行)
if (window.PublicKeyCredential) {
  console.log("✅ WebAuthn supported");
} else {
  console.log("❌ WebAuthn not supported");
}
```

## 📊 测试验收标准

### ✅ 必须通过的检查点

按照 **QEMU → TA → CA → WebAuthn → Demo** 流程验证：

**阶段0: QEMU环境与TA基础测试**
   - [ ] QEMU OP-TEE 4.7正常运行
   - [ ] TA文件正确安装到/lib/optee_armtz/
   - [ ] Rust CA与TA通信正常 (hello, echo, test命令)
   - [ ] 混合熵源功能验证通过
   - [ ] TEE安全状态验证正常

**阶段1: Node.js CA服务测试**
   - [ ] 阶段0全部通过 (前置条件)
   - [ ] Node.js CA服务启动无错误
   - [ ] 健康检查返回healthy状态
   - [ ] TEE连接验证通过

**阶段2: WebAuthn API功能测试**
   - [ ] 注册选项生成成功
   - [ ] 认证选项生成成功
   - [ ] 挑战验证逻辑正常

**阶段3: WebAuthn完整流程测试**
   - [ ] 自动化测试脚本通过
   - [ ] SDK集成测试通过

**阶段4: 浏览器Demo测试**
   - [ ] React Demo应用启动成功
   - [ ] 真实浏览器Passkey注册成功
   - [ ] 账户创建返回正确数据

**阶段5: Rust CA CLI工具测试**
   - [ ] Rust CA基础功能测试通过
   - [ ] WebAuthn CLI模式功能正常

**完整调用链验证**
   - [ ] QEMU OP-TEE环境 ✅ 稳定运行
   - [ ] TA ✅ 响应CA调用
   - [ ] Node.js CA ✅ 提供WebAuthn API
   - [ ] Demo前端 ✅ 调用CA API成功
   - [ ] Rust CA ✅ CLI工具功能完整

## 📈 测试结果记录

### 测试环境信息

- **操作系统**: macOS/Linux
- **Node.js版本**: `node --version` (Node.js v23.9.0 验证通过)
- **浏览器**: Chrome/Safari版本
- **QEMU状态**: 运行/停止
- **OP-TEE版本**: OP-TEE 4.7 (112396a58cf0d5d7)
- **TEE设备**: /dev/teepriv0 可用

### 性能基准

| 操作 | 期望时间 | 实际时间 | 状态 |
|------|----------|----------|------|
| TA-CA连接建立 | <2s | ___ | ⏳ |
| TA Hello World | <50ms | ___ | ⏳ |
| TA Echo测试 | <100ms | ___ | ⏳ |
| TA完整测试套件 | <5s | ___ | ⏳ |
| CA服务启动 | <5s | ___ | ⏳ |
| 健康检查 | <100ms | ___ | ⏳ |
| 注册选项生成 | <200ms | ___ | ⏳ |
| 账户创建 | <500ms | ___ | ⏳ |

### 错误日志收集

```bash
# 收集所有相关日志
mkdir -p test-logs/$(date +%Y%m%d-%H%M%S)
cd test-logs/$(date +%Y%m%d-%H%M%S)

# 复制CA服务日志
cp ../../packages/airaccount-ca-nodejs/logs/* ./

# 保存测试输出
node ../../scripts/test/test-webauthn-complete-flow.js > webauthn-test.log 2>&1

# 保存系统状态
ps aux | grep -E "(qemu|node)" > process-status.log
lsof -i :3001,3002 > port-status.log
```

## 🎯 下一步行动

### 测试通过后

1. **Commit + Tag**: 标记这个测试通过的版本
2. **部署准备**: 准备生产环境配置
3. **性能优化**: 基于测试结果优化响应时间

#### 已验证的系统状态 (更新: 2025-08-16)
- ✅ CA服务器: http://localhost:3002 运行中
- ✅ Demo应用: http://localhost:5174 运行中
- ✅ QEMU OP-TEE 4.7: 正常初始化
- ✅ WebAuthn API: 15个端点全部可用
- ✅ TEE设备: /dev/teepriv0 正常

### 测试失败后

1. **详细诊断**: 使用上述排查步骤
2. **修复代码**: 针对具体问题修复
3. **回归测试**: 确保修复不影响其他功能

## 🔧 测试模式配置说明

### Node.js CA 测试模式切换

```typescript
// 在 index.ts 中
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

### 测试模式说明

- **并行模式**: 测试模式和真实模式可以并行运行，通过 `isTestMode` 参数控制
- **统一数据库**: 两种模式使用相同的数据库结构，无需兼容性转换
- **灵活切换**: 可以在运行时通过环境变量切换测试/生产模式

---

🔔 **重要提醒**:
- 每次修改代码后都要重新运行完整测试
- 保持QEMU环境运行期间进行所有测试
- 记录所有测试结果用于后续分析
- 在生产环境中确保设置正确的环境变量以启用真实WebAuthn验证
