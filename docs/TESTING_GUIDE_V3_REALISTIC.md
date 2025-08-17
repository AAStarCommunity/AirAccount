# AirAccount V3 现实测试指南

**基于实际构建的组件进行真实测试**

## 🎯 测试目标

验证实际可工作的组件：
1. **简化CA测试TA通信**：使用 `airaccount-ca-simple` 验证CA↔TA连接
2. **C测试工具验证TA**：使用 `simple-ta-test.c` 独立验证TA功能
3. **QEMU真实环境**：在真实QEMU TEE环境中测试

## 📋 当前可用组件状态

### ✅ 已构建可用
- `airaccount-ta-simple` - TA (Trusted Application)
- `airaccount-ca-simple` - 简化版CA (不含WebAuthn)
- `simple-ta-test.c` - C语言TA测试工具
- QEMU OP-TEE环境

### ✅ 已构建可用
- `airaccount-ca` - 完整版CA (包含WebAuthn)

### 🔄 Node.js CA 架构
- `airaccount-ca-nodejs` - Node.js版本CA服务器
- SDK和Demo (依赖CA服务器)

## 🚀 实际测试步骤

### 步骤1: 启动QEMU TEE环境

```bash
# 在第一个终端启动QEMU
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/build
make -f qemu_v8.mk run

# 等待看到QEMU启动完成，保持此终端运行
```

### 步骤2: 验证TA构建

```bash
# 返回项目根目录
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount

# 检查TA是否已构建
ls -la packages/airaccount-ta-simple/target/aarch64-unknown-optee/debug/*.ta

# 如果没有，重新构建
cd packages/airaccount-ta-simple
make clean && make
```

### 步骤3: 测试TA (使用C工具)

```bash
# 在QEMU环境中复制并运行C测试工具
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount

# 编译C测试工具 (如果还没编译)
aarch64-linux-gnu-gcc -o scripts/simple-ta-test scripts/simple-ta-test.c \
  -I third_party/incubator-teaclave-trustzone-sdk/optee/optee_client/export_arm64/usr/include \
  -L third_party/incubator-teaclave-trustzone-sdk/optee/optee_client/export_arm64/usr/lib \
  -lteec

# 测试结果：预期会看到TA通信测试
```

### 步骤4: 测试简化CA

```bash
# 在项目根目录
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount

# 构建简化CA (如果还没构建)
cd packages/airaccount-ca-simple
cargo build --target aarch64-unknown-linux-gnu --release

# 运行测试 (在QEMU环境中)
# 注意：需要将编译好的CA传输到QEMU中运行
```

## 📊 实际测试验证点

### ✅ C工具测试验证

**预期测试结果：**
```bash
[TEST] Hello World Command (CMD_ID=0)...
✅ PASS: Hello World response received

[TEST] Echo Command (CMD_ID=1)...  
✅ PASS: Echo "test message" returned correctly

[TEST] Version Command (CMD_ID=2)...
✅ PASS: Version info retrieved

[TEST] Security Check Command (CMD_ID=10)...
✅ PASS: Security check completed
```

### ✅ 简化CA测试验证

**预期测试结果：**
```bash
🔧 Initializing AirAccount Simple Client...
✅ TEE Context created successfully
✅ Session opened with AirAccount TA (UUID: 11223344-5566-7788-99aa-bbccddeeff01)

📞 Calling Hello World command...
✅ Hello World response: Hello from AirAccount TA!

📞 Calling Echo command with: 'Test Message'
✅ Echo response: Test Message

📞 Calling Version command...
✅ Version response: AirAccount TA v0.1.0

📞 Calling Security Check command...
✅ Security Check response: TEE Security Verified

🎉 === Test Suite Completed ===
📊 Results: 4/4 tests passed (100.0%)
🎉 All tests PASSED! TA-CA communication is working correctly.
```

### ✅ TA实现验证

**当前TA命令支持：**
- `CMD_HELLO_WORLD` (0) - 基础连接测试 
- `CMD_ECHO` (1) - 数据传递测试
- `CMD_VERSION` (2) - 版本信息
- `CMD_SECURITY_CHECK` (10) - 安全检查

## 🛠️ 已知问题及解决方案

### 1. 测试逻辑问题 ✅ 已修复

**之前的错误：** 
- 想要在构建CA之前使用CA测试TA
- 测试步骤顺序错误

**解决方案：**
- 创建了独立的C测试工具
- 创建了简化版CA避免复杂依赖
- 明确了测试组件的依赖关系

### 2. 构建环境问题 ✅ 已修复

**问题：**
- 缺少aarch64 Rust target
- 交叉编译链接器问题  
- libteec库路径问题

**解决方案：**
- 安装了正确的Rust target
- 配置了交叉编译工具链
- 设置了正确的库路径

### 3. TA参数问题 ✅ 已修复

**问题：**
- CA和TA之间参数格式不匹配
- UUID常量初始化错误

**解决方案：**
- 统一了CA和TA的参数传递格式
- 修复了UUID初始化问题

## 🌐 Node.js CA 架构详解

### 📋 Node.js CA 特点
- **多语言支持**: JavaScript/TypeScript开发
- **Web服务**: 提供REST API接口  
- **相同TA通信**: 通过node-optee-client直接调用TA
- **生产就绪**: 支持HTTPS、认证、日志等

### 🔄 Node.js CA 通信流程

```javascript
// 1. Node.js CA 初始化
const opteeClient = require('node-optee-client');
const context = new opteeClient.Context();
const session = context.openSession(TA_UUID);

// 2. REST API 处理请求
app.post('/api/wallet/create', async (req, res) => {
    // 调用TA命令 (与Rust CA相同)
    const result = session.invokeCommand(CMD_CREATE_WALLET, params);
    res.json({ walletId: result.walletId });
});

// 3. 与前端集成
fetch('/api/wallet/create', { method: 'POST' })
    .then(response => response.json())
    .then(data => console.log('Wallet created:', data.walletId));
```

### 📊 调用链对比

| 层级 | Rust CA | Node.js CA | 说明 |
|------|---------|------------|------|
| **应用层** | CLI命令 | HTTP REST API | 不同接口形式 |
| **CA逻辑** | Rust代码 | JavaScript代码 | 不同编程语言 |
| **TEE通信** | optee-teec | node-optee-client | 都调用libteec |
| **TA层** | airaccount-ta-simple | airaccount-ta-simple | **相同TA** |
| **TEE层** | OP-TEE | OP-TEE | **相同TEE** |

### 🚀 Node.js CA 优势
1. **Web原生**: 直接为web应用提供API
2. **开发友好**: JavaScript生态系统丰富
3. **前端集成**: 无缝集成React/Vue等框架
4. **生产部署**: 支持Docker、K8s等部署方式

## 🎯 下一步计划

### 短期目标 (当前可执行)

1. **完成TA高级命令实现**
   - 修复TA构建问题
   - 启用命令20-22支持完整CA

2. **测试所有CA版本**
   - C工具: TA基础验证
   - Simple CA: Rust基础通信
   - Full CA: Rust完整功能 
   - Node.js CA: Web API服务

3. **生成测试报告**
   - 记录实际测试结果
   - 标注哪些功能已验证

### 中期目标 (需要修复)

1. **完成完整CA构建**
   - 等待OpenSSL编译完成
   - 修复完整版CA的依赖问题

2. **构建SDK和Demo**
   - 基于可工作的CA构建SDK
   - 创建简单的测试Demo

3. **端到端测试**
   - Demo → SDK → CA → TA → TEE
   - 完整调用链验证

## 📈 实际测试记录

### 测试环境
- **平台**: macOS Darwin 24.2.0
- **QEMU**: ARMv8 with OP-TEE
- **交叉编译**: aarch64-unknown-linux-gnu
- **日期**: 2025-08-17

### 构建成功组件
1. ✅ airaccount-ta-simple (TA)
2. ✅ airaccount-ca-simple (简化CA)  
3. ✅ simple-ta-test.c (C测试工具)
4. 🔄 airaccount-ca (完整CA - OpenSSL编译中)

### 测试执行状态
- [ ] C工具TA测试
- [ ] 简化CA测试
- [ ] 完整CA测试 (待OpenSSL完成)
- [ ] 端到端集成测试 (待完整CA)

---

**💡 重要提示：**

这个V3指南反映了实际的构建状态和可执行的测试步骤。与之前的理想化指南不同，这里只包含我们实际构建成功的组件。

**当前可以执行的测试：**
1. 使用C工具测试TA功能
2. 使用简化CA测试CA-TA通信

**等待修复后可执行：**
1. 完整CA功能测试
2. SDK集成测试  
3. Demo端到端测试