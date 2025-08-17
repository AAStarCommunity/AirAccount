# AirAccount 手工测试完整指南 (优化版)

**创建时间**: 2025-08-17 09:42:35 +07
**最后更新**: 2025-08-17 09:42:35 +07

## 🎯 测试目标

验证 **QEMU → TA → CA(Node.js CA, Rust CA) → WebAuthn → Demo** 完整调用链，确保所有组件按照预期正常工作，完成完整的用户加密账户生命周期管理。

## 🚀 优化的五步测试法

基于用户反馈优化，按照清晰的五步法进行系统性测试：

### 第一步：QEMU环境基础验证

**测试目标**: 确保QEMU OP-TEE环境正常启动和运行
**测试重点**: TEE基础环境稳定性验证

#### 步骤1.1: 启动QEMU OP-TEE环境

```bash
# 终端1: 启动QEMU TEE环境
cd third_party/incubator-teaclave-trustzone-sdk/tests/ && ./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04

# 等待看到QEMU完全启动的提示
# 保持此终端运行
```

#### 步骤1.2: 验证QEMU环境基础功能

```bash
# 检查QEMU进程是否运行
ps aux | grep qemu-system-aarch64

# 期望看到QEMU进程正在运行
#  ps aux | grep qemu-system-aarch64
nicolasshuaishuai 26403   0.4  2.6 413630576 438656 s003  S+    9:04AM   0:45.05 /opt/homebrew/bin/qemu-system-aarch64 -nodefaults -nographic -serial stdio -serial file:/tmp/serial.log -smp 2 -machine virt,secure=on,acpi=off,gic-version=3 -cpu cortex-a57 -d unimp -semihosting-config enable=on,target=native -m 1057 -bios bl1.bin -initrd rootfs.cpio.gz -append console=ttyAMA0,115200 keep_bootcon root=/dev/vda2 -kernel Image -fsdev local,id=fsdev0,path=/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/tests/aarch64-optee-4.7.0-qemuv8-ubuntu-24.04/../shared,security_model=none -device virtio-9p-device,fsdev=fsdev0,mount_tag=host -netdev user,id=vmnic -device virtio-net-device,netdev=vmnic
nicolasshuaishuai 38681   0.4  2.5 413613168 418480 s004  S+   10:50AM   0:12.64 /opt/homebrew/bin/qemu-system-aarch64 -nodefaults -nographic -serial stdio -serial file:/tmp/serial.log -smp 2 -machine virt,secure=on,acpi=off,gic-version=3 -cpu cortex-a57 -d unimp -semihosting-config enable=on,target=native -m 1057 -bios bl1.bin -initrd rootfs.cpio.gz -append console=ttyAMA0,115200 keep_bootcon root=/dev/vda2 -kernel Image -fsdev local,id=fsdev0,path=/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/tests/aarch64-optee-4.7.0-qemuv8-ubuntu-24.04/../shared,security_model=none -device virtio-9p-device,fsdev=fsdev0,mount_tag=host -netdev user,id=vmnic -device virtio-net-device,netdev=vmnic
nicolasshuaishuai 38703   0.0  0.0 410724096   1456 s002  S+   10:52AM   0:00.00 grep --color=auto --exclude-dir=.bzr --exclude-dir=CVS --exclude-dir=.git --exclude-dir=.hg --exclude-dir=.svn --exclude-dir=.idea --exclude-dir=.tox --exclude-dir=.venv --exclude-dir=venv qemu-system-aarch64

why we got two? is correct?

#
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
# Welcome to Buildroot, type root or test to login
buildroot login: root
# ls -la /dev/tee*
crw-rw----    1 root     teeclnt   247,   0 Aug 17 03:50 /dev/tee0
crw-rw----    1 root     tee       247,  16 Aug 17 03:50 /dev/teepriv0
# dmesg | grep -i optee
[    0.000000] OF: reserved mem: 0x000000000e100000..0x000000000effffff (15360 KiB) nomap non-reusable optee_core@e100000
[    0.000000] OF: reserved mem: 0x0000000042000000..0x00000000421fffff (2048 KiB) nomap non-reusable optee_shm@42000000
[    0.457852] optee: probing for conduit method.
[    0.458316] optee: revision 4.7 (112396a58cf0d5d7)
[    0.460448] optee: Asynchronous notifications enabled
[    0.460778] optee: dynamic shared memory is enabled
[    0.465996] optee: initialized driver
# ls -la /shared/
ls: /shared/: No such file or directory
# ls
# ls /
bin      init     linuxrc  opt      run      tmp
dev      lib      media    proc     sbin     usr
etc      lib64    mnt      root     sys      var
# why no shared? how to fix?

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
cd packages/airaccount-ta-simple && make clean && make

error:
cd packages/airaccount-ta-simple && make clean && make
     Removed 536 files, 157.5MiB total
   Compiling compiler_builtins v0.1.109
   Compiling core v0.0.0 (/Users/nicolasshuaishuai/.rustup/toolchains/nightly-2024-05-15-aarch64-apple-darwin/lib/rustlib/src/rust/library/core)
   Compiling proc-macro2 v1.0.95
   Compiling unicode-ident v1.0.18
   Compiling proc-macro2 v0.4.30
   Compiling zerofrom v0.1.5
   Compiling unicode-xid v0.1.0
   Compiling litemap v0.7.4
   Compiling prettyplease v0.2.36
   Compiling optee-utee-sys v0.5.0 (/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee-utee/optee-utee-sys)
   Compiling syn v0.15.44
   Compiling rustversion v1.0.21
   Compiling libc v0.2.174
   Compiling uuid v1.17.0
   Compiling heck v0.5.0
   Compiling quote v0.6.13
   Compiling quote v1.0.40
   Compiling syn v2.0.104
   Compiling optee-utee-macros v0.5.0 (/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee-utee/macros)
warning: unexpected `cfg` condition value: `optee`
  --> /Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee-utee/macros/src/lib.rs:21:11
   |
21 | #[cfg(not(target_os = "optee"))]
   |           ^^^^^^^^^^^^^^^^^^^
   |
   = note: expected values for `target_os` are: `aix`, `android`, `cuda`, `dragonfly`, `emscripten`, `espidf`, `freebsd`, `fuchsia`, `haiku`, `hermit`, `horizon`, `hurd`, `illumos`, `ios`, `l4re`, `linux`, `macos`, `netbsd`, `none`, `nto`, `openbsd`, `psp`, `redox`, `solaris`, `solid_asp3`, `teeos`, `tvos`, `uefi`, `unknown`, `visionos`, `vita`, `vxworks`, `wasi`, `watchos`, `windows` and 2 more
   = note: see <https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#rustc-check-cfg> for more information about checking conditional configuration
   = note: `#[warn(unexpected_cfgs)]` on by default

error: failed to run custom build command for `optee-utee-sys v0.5.0 (/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee-utee/optee-utee-sys)`

Caused by:
  process didn't exit successfully: `/Volumes/UltraDisk/Dev2/aastar/AirAccount/packages/airaccount-ta-simple/target/release/build/optee-utee-sys-dbd6556fb421698f/build-script-build` (exit status: 101)
  --- stderr
  thread 'main' panicked at /Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee-utee/optee-utee-sys/build.rs:41:51:
  called `Result::unwrap()` on an `Err` value: NotPresent
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...
warning: `optee-utee-macros` (lib) generated 1 warning
make: *** [ta] Error 101


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

#### 步骤5.1: 加密钱包创建测试

```bash
# 通过Demo界面创建新钱包
# 在浏览器中执行:
# 1. 登录成功后点击"创建钱包"
# 2. 选择钱包类型 (以太坊/比特币等)
# 3. 验证钱包地址生成

# 或使用API直接测试
curl -X POST http://localhost:3002/api/wallet/create \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "test-user-001",
    "walletType": "ethereum",
    "passkey": "authenticated_session_token"
  }' | jq
```

#### 步骤5.2: 交易签名测试

```bash
# 模拟交易签名流程
curl -X POST http://localhost:3002/api/wallet/sign \
  -H "Content-Type: application/json" \
  -d '{
    "walletId": "wallet-12345",
    "transaction": {
      "to": "0x742d35Cc6634C0532925a3b8D",
      "value": "0.1",
      "gasLimit": "21000"
    },
    "passkey": "authenticated_session_token"
  }' | jq

# 期望返回签名结果和交易哈希
```

#### 步骤5.3: 账户管理功能测试

```bash
# 查看账户信息
curl -X GET http://localhost:3002/api/wallet/info \
  -H "Authorization: Bearer authenticated_token" | jq

# 查看交易历史
curl -X GET http://localhost:3002/api/wallet/transactions \
  -H "Authorization: Bearer authenticated_token" | jq

# 查看账户余额
curl -X GET http://localhost:3002/api/wallet/balance \
  -H "Authorization: Bearer authenticated_token" | jq
```

#### 步骤5.4: 账户备份和恢复测试

```bash
# 备份账户
# 在Demo界面中:
# 1. 点击"备份账户"
# 2. 验证Passkey
# 3. 导出加密备份文件

# 恢复账户
# 1. 点击"恢复账户"
# 2. 上传备份文件
# 3. 验证Passkey
# 4. 验证账户恢复成功
```

#### 步骤5.5: 账户清除测试

```bash
# 清除账户数据 (安全操作)
curl -X DELETE http://localhost:3002/api/wallet/clear \
  -H "Content-Type: application/json" \
  -d '{
    "userId": "test-user-001",
    "confirmPhrase": "DELETE_MY_ACCOUNT",
    "passkey": "authenticated_session_token"
  }' | jq

# 验证账户已清除
curl -X GET http://localhost:3002/api/wallet/info \
  -H "Authorization: Bearer authenticated_token"
# 期望返回404或账户不存在错误
```

#### 步骤5.6: 完整生命周期集成测试

使用自动化脚本测试完整流程:
```bash
# 运行完整生命周期测试
node scripts/test/test-full-lifecycle.js

# 期望看到:
# ✅ 用户注册成功
# ✅ Passkey创建成功
# ✅ 钱包创建成功
# ✅ 交易签名成功
# ✅ 账户管理功能正常
# ✅ 数据备份/恢复成功
# ✅ 账户清除成功
```

**第五步验收标准**:
- [ ] 加密钱包创建成功
- [ ] 交易签名功能正常
- [ ] 账户信息查询正常
- [ ] 交易历史记录正确
- [ ] 账户备份功能正常
- [ ] 账户恢复功能正常
- [ ] 账户清除功能安全
- [ ] 完整生命周期自动化测试通过

---

## 📊 优化后的测试验收标准

### ✅ 必须通过的检查点

按照五步法逐步验证：

**第一步: QEMU环境基础验证**
   - [ ] QEMU OP-TEE 4.7正常运行
   - [ ] TEE设备(/dev/teepriv0)可访问
   - [ ] OP-TEE内核模块已加载
   - [ ] 共享目录正确挂载

**第二步: TA构建部署与基础测试**
   - [ ] 第一步全部通过 (前置条件)
   - [ ] TA文件正确安装到/lib/optee_armtz/
   - [ ] Hello World命令返回正确响应
   - [ ] Echo命令能正确回显各种输入
   - [ ] 完整测试套件5/5通过

**第三步: CA构建与CA-TA通信测试**
   - [ ] 第二步全部通过 (前置条件)
   - [ ] Rust CA与TA通信正常
   - [ ] Node.js CA服务启动无错误
   - [ ] 健康检查返回healthy状态
   - [ ] TEE连接验证通过

**第四步: WebAuthn完整用户流程测试**
   - [ ] 第三步全部通过 (前置条件)
   - [ ] 模拟模式注册流程完整
   - [ ] 真实模式注册成功创建Passkey
   - [ ] 真实模式Passkey认证成功
   - [ ] 数据库正确记录用户信息
   - [ ] 第二次登录使用现有Passkey成功

**第五步: 端到端加密账户生命周期测试**
   - [ ] 第四步全部通过 (前置条件)
   - [ ] 加密钱包创建成功
   - [ ] 交易签名功能正常
   - [ ] 账户备份和恢复功能正常
   - [ ] 账户清除功能安全
   - [ ] 完整生命周期自动化测试通过

**完整调用链验证**
   - [ ] QEMU OP-TEE环境 ✅ 稳定运行
   - [ ] TA ✅ 响应CA调用
   - [ ] Node.js CA ✅ 提供WebAuthn API
   - [ ] Demo前端 ✅ 调用CA API成功
   - [ ] Rust CA ✅ CLI工具功能完整

## 🔧 测试问题排查和修复方案

### 1. QEMU TEE环境问题

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

### 2. TA部署问题

```bash
# 检查TA文件权限
ls -la /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta

# 检查OP-TEE日志
dmesg | grep -i optee

# 重新安装TA
cp shared/*.ta /lib/optee_armtz/
chmod 444 /lib/optee_armtz/*.ta
```

### 3. CA服务端口冲突

```bash
# 检查端口占用
lsof -i :3002

# 杀死占用进程
kill $(lsof -t -i:3002)
```

### 4. WebAuthn浏览器兼容性

```bash
# 测试WebAuthn可用性 (在浏览器控制台执行)
if (window.PublicKeyCredential) {
  console.log("✅ WebAuthn supported");
} else {
  console.log("❌ WebAuthn not supported");
}
```

## 📈 测试结果记录

### 测试环境信息

- **操作系统**: macOS/Linux
- **Node.js版本**: `node --version`
- **浏览器**: Chrome/Safari版本
- **QEMU状态**: 运行/停止
- **OP-TEE版本**: OP-TEE 4.7
- **TEE设备**: /dev/teepriv0 可用

### 性能基准

| 操作 | 期望时间 | 实际时间 | 状态 |
|------|----------|----------|------|
| QEMU环境启动 | <30s | ___ | ⏳ |
| TA-CA连接建立 | <2s | ___ | ⏳ |
| TA Hello World | <50ms | ___ | ⏳ |
| TA完整测试套件 | <5s | ___ | ⏳ |
| CA服务启动 | <5s | ___ | ⏳ |
| WebAuthn注册流程 | <10s | ___ | ⏳ |
| 钱包创建 | <3s | ___ | ⏳ |
| 交易签名 | <2s | ___ | ⏳ |

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
- 每次修改代码后都要重新运行完整的五步测试
- 每一步都必须在前一步全部通过后才能开始
- 保持QEMU环境运行期间进行所有测试
- 记录所有测试结果用于后续分析
- 在生产环境中确保设置正确的环境变量以启用真实WebAuthn验证
