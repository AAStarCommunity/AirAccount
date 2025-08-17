# AirAccount 手工测试完整指南

**最后更新**: 2025-08-17 09:42:35 +07

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

## 🚀 优化的五步测试方案

基于用户反馈优化，按照清晰的五步法进行系统性测试：

### 第一步：QEMU环境基础验证

**测试目标**: 确保QEMU OP-TEE环境正常启动和运行
**测试重点**: TEE基础环境稳定性验证

#### 步骤1.1: 启动QEMU OP-TEE环境

```bash
# 终端1: 启动QEMU TEE环境
cd third_party/incubator-teaclave-trustzone-sdk/tests/
./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04

# 等待看到QEMU完全启动的提示
# 保持此终端运行
```

#### 步骤1.2: 验证QEMU环境基础功能

```bash
# 检查QEMU进程是否运行
ps aux | grep qemu-system-aarch64

# 期望看到QEMU进程正在运行
```

#### 步骤1.3: 验证TEE设备可用性

在QEMU控制台中执行：
```bash
# 检查TEE设备
ls -la /dev/tee*

# 期望输出:
# crw------- 1 root root 254, 0 Aug 17 02:42 /dev/tee0
# crw------- 1 root root 254, 1 Aug 17 02:42 /dev/teepriv0

# 检查OP-TEE内核模块
dmesg | grep -i optee

# 期望看到OP-TEE初始化成功的日志
```

#### 步骤1.4: 验证共享目录挂载

```bash
# 在QEMU中检查共享目录
ls -la /shared/

# 期望看到:
# 11223344-5566-7788-99aa-bbccddeeff01.ta
# airaccount-ca (可执行文件)
```

**第一步验收标准**:
- [ ] QEMU进程正常运行
- [ ] TEE设备(/dev/teepriv0)可访问
- [ ] OP-TEE内核模块已加载
- [ ] 共享目录正确挂载

---

### 第二步：TA构建部署与基础测试

**测试目标**: 确保最新版本TA正确构建、部署和基础功能验证
**测试重点**: TA版本管理和基础通信测试

#### 步骤2.1: 备份和清理旧TA

```bash
# 在QEMU中备份现有TA
cp /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta /tmp/backup_ta_$(date +%Y%m%d_%H%M%S).ta

# 或删除旧TA确保使用最新版本
rm /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta
```

#### 步骤2.2: 构建最新版本TA

```bash
# 在主机上构建最新TA
cd packages/airaccount-ta-simple
make clean && make

# 检查构建结果
ls -la target/aarch64-unknown-optee/debug/*.ta

# 期望看到最新的TA文件
```

#### 步骤2.3: 部署并测试TA基础功能

```bash
# 在QEMU中安装新TA
cp /shared/11223344-5566-7788-99aa-bbccddeeff01.ta /lib/optee_armtz/
chmod 444 /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta

# 测试基础TA功能
./shared/airaccount-ca hello
./shared/airaccount-ca echo "TA Test"
./shared/airaccount-ca test

# 期望所有基础测试通过
```

**第二步验收标准**:
- [ ] 新TA文件成功构建
- [ ] TA正确安装到/lib/optee_armtz/
- [ ] Hello World命令响应正确
- [ ] Echo测试通过
- [ ] 完整测试套件通过(5/5)

---

### 第三步：CA构建与CA-TA通信测试

**测试目标**: 确保Rust CA和Node.js CA正确构建，并能与TA正常通信
**测试重点**: 双CA架构验证和基础通信功能

#### 步骤3.1: 构建Rust CA

```bash
# 构建Rust CA (如果尚未构建)
cd packages/airaccount-ca
cargo build --target aarch64-unknown-linux-gnu --release

# 检查构建结果
ls -la target/aarch64-unknown-linux-gnu/release/airaccount-ca
```

#### 步骤3.2: 测试Rust CA基础功能

```bash
# 在QEMU中测试Rust CA
./shared/airaccount-ca interactive

# 期望看到交互界面启动
# 测试基础命令: hello, echo, security
```

#### 步骤3.3: 构建和启动Node.js CA

```bash
# 构建Node.js CA
cd packages/airaccount-ca-nodejs
npm install

# 启动CA服务
npm run dev

# 期望输出:
# 🚀 AirAccount CA Service
# 📡 Server running on http://localhost:3002
# 🔑 WebAuthn features enabled
```

#### 步骤3.4: 测试Node.js CA基础功能

```bash
# 测试健康检查
curl http://localhost:3002/health

# 期望返回健康状态JSON
# 测试TEE连接验证
curl http://localhost:3002/api/webauthn/security/verify
```

**第三步验收标准**:
- [ ] Rust CA构建成功
- [ ] Rust CA与TA通信正常
- [ ] Node.js CA服务启动无错误
- [ ] Node.js CA健康检查通过
- [ ] 两种CA都能正常与TA通信

---

### 第四步：WebAuthn完整用户流程测试

**测试目标**: 验证完整的WebAuthn用户注册和认证流程
**测试重点**: 支持模拟和真实两种测试路径，完整用户生命周期

#### 步骤4.1: 配置测试模式

**环境变量配置**:
```bash
# 测试模式 (跳过实际WebAuthn验证)
export NODE_ENV=development
export WEBAUTHN_TEST_MODE=true

# 真实模式 (需要真实设备验证)
export NODE_ENV=production
export WEBAUTHN_TEST_MODE=false
```

#### 步骤4.2: 用户注册流程测试 (模拟模式)

```bash
# 测试注册选项生成
curl -X POST http://localhost:3002/api/webauthn/register/begin \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@airaccount.dev",
    "displayName": "Test User"
  }' | jq

# 期望返回包含challenge和options的JSON
```

#### 步骤4.3: 用户注册流程测试 (真实模式)

**浏览器测试步骤**:
1. 访问 http://localhost:5174/
2. 输入邮箱: test@airaccount.dev
3. 点击"注册Passkey" 
4. 完成生物识别验证 (Touch ID/Face ID/USB Key)
5. 验证注册成功响应

#### 步骤4.4: 用户登录流程测试

**模拟模式**:
```bash
# 测试认证选项生成
curl -X POST http://localhost:3002/api/webauthn/authenticate/begin \
  -H "Content-Type: application/json" \
  -d '{"email": "test@airaccount.dev"}' | jq
```

**真实模式**:
1. 在浏览器中点击"登录"
2. 输入已注册邮箱
3. 使用Passkey完成认证
4. 验证登录成功

#### 步骤4.5: 数据库操作验证

```bash
# 检查用户数据
sqlite3 packages/airaccount-ca-nodejs/airaccount.db "SELECT * FROM users;"

# 检查认证记录
sqlite3 packages/airaccount-ca-nodejs/airaccount.db "SELECT * FROM user_credentials;"

# 检查挑战记录
sqlite3 packages/airaccount-ca-nodejs/airaccount.db "SELECT * FROM challenges ORDER BY created_at DESC LIMIT 5;"
```

**第四步验收标准**:
- [ ] 模拟模式注册流程完整
- [ ] 真实模式注册成功创建Passkey
- [ ] 模拟模式认证流程正常
- [ ] 真实模式Passkey认证成功
- [ ] 数据库正确记录用户信息
- [ ] 第二次登录使用现有Passkey成功

---

### 第五步：端到端加密账户生命周期测试

**测试目标**: 验证完整的加密钱包生命周期管理
**测试重点**: 从用户交互到TA执行的完整加密货币功能

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
