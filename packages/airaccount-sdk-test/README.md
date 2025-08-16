# AirAccount SDK 模拟器

完整的SDK-CA-TA-TEE集成测试工具，用于验证AirAccount在QEMU TEE环境中的完整调用链。

## 测试目标

验证以下完整调用链：
```
Demo → SDK → CA → TA → QEMU TEE 硬件
```

## 快速开始

### 1. 安装依赖

```bash
cd packages/sdk-simulator
npm install
```

### 2. 启动QEMU TEE环境

```bash
# 在项目根目录
cd third_party/build
make -f qemu_v8.mk run
```

### 3. 启动CA服务

```bash
# 启动Rust CA (端口3001)
cargo run -p airaccount-ca-extended --bin ca-server

# 启动Node.js CA (端口3002) - 新终端
cd packages/airaccount-ca-nodejs
npm run dev
```

### 4. 运行测试

```bash
# 测试Rust CA
npm run test-rust

# 测试Node.js CA  
npm run test-nodejs

# 测试双CA
npm run test-both

# 运行完整Demo
npm run demo
```

## 测试脚本说明

### 1. CA集成测试 (`test-ca-integration.js`)

**测试流程:**
1. SDK初始化 - 检查CA和TEE连接
2. WebAuthn注册 - 生成挑战和凭证
3. 账户创建 - 在TEE中创建私钥
4. 余额查询 - 验证钱包功能
5. 转账操作 - 测试签名功能
6. 钱包列表 - 验证管理功能

**命令:**
```bash
# 测试指定CA
node test-ca-integration.js --ca=rust
node test-ca-integration.js --ca=nodejs
node test-ca-integration.js --ca=both
```

### 2. 完整流程演示 (`demo-full-flow.js`)

**演示场景:**
- 用户注册和WebAuthn设置
- 生物识别认证模拟
- 钱包创建和资产操作
- 多用户和多CA支持
- 恢复信息展示

**运行:**
```bash
node demo-full-flow.js
```

## 测试验证点

### ✅ TEE硬件集成
- QEMU OP-TEE环境运行
- AirAccount TA加载和响应
- 硬件随机数生成
- 安全存储功能

### ✅ CA服务功能
- **Rust CA**: 基于airaccount-ca扩展
- **Node.js CA**: Simple WebAuthn集成
- HTTP API完整性
- WebAuthn challenge-response

### ✅ SDK模拟
- 真实API调用
- 错误处理
- 会话管理
- 用户体验模拟

### ✅ 安全架构
- 用户凭证客户端管理
- Passkey设备存储
- TEE私钥隔离
- 恢复信息自主控制

## 测试输出示例

```bash
🧪 开始RUST CA完整集成测试
==================================================
📱 [SDK-RUST] 初始化SDK...
✅ [SDK-RUST] TEE连接正常
✅ [SDK-RUST] SDK初始化成功
📱 [SDK-RUST] 开始WebAuthn注册: test-rust@airaccount.dev
📱 [SDK-RUST] WebAuthn挑战生成成功
📱 [SDK-RUST] 创建钱包账户...
✅ [SDK-RUST] 账户创建成功 - 钱包ID: 1
✅ [SDK-RUST] 以太坊地址: 0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A
📱 [SDK-RUST] 查询余额 - 钱包ID: 1
✅ [SDK-RUST] 余额查询成功: 1.0 ETH
📱 [SDK-RUST] 执行转账 - 金额: 0.1 ETH
✅ [SDK-RUST] 转账成功 - 交易哈希: 0x1234...
📱 [SDK-RUST] 列出所有钱包...
✅ [SDK-RUST] 钱包列表获取成功 - 总数: 1

✅ RUST CA完整集成测试成功！
🔗 验证调用链: SDK → RUST CA → TA → QEMU TEE
```

## 架构验证

### 调用链验证
```
1. Demo应用      → 模拟前端用户操作
2. SDK模拟器     → HTTP API调用
3. CA服务        → Rust/Node.js双实现
4. TA通信        → optee-teec库调用
5. QEMU TEE      → 真实硬件环境模拟
```

### 安全验证
```
1. 用户凭证     → 客户端自主管理
2. Passkey存储  → 设备安全硬件
3. 私钥隔离     → TEE硬件环境
4. 节点故障     → 用户可迁移恢复
```

## 故障排除

### 常见问题

**1. TEE连接失败**
```bash
❌ TEE连接异常
```
**解决:** 确保QEMU TEE环境正在运行
```bash
cd third_party/build && make -f qemu_v8.mk run
```

**2. CA服务无响应**
```bash
❌ 初始化失败: fetch failed
```
**解决:** 确保CA服务已启动
```bash
# Rust CA
cargo run -p airaccount-ca-extended --bin ca-server

# Node.js CA  
cd packages/airaccount-ca-nodejs && npm run dev
```

**3. TA未加载**
```bash
❌ TA connection test failed
```
**解决:** 构建和部署TA
```bash
cd packages/airaccount-ta-simple && make
```

### 调试模式

**查看详细日志:**
```bash
# CA服务日志
tail -f logs/rust-ca-server.log
tail -f logs/nodejs-ca-server.log

# TEE调试
cd third_party/build && make -f qemu_v8.mk run-debug
```

**手动API测试:**
```bash
# 测试CA健康状态
curl http://localhost:3001/health | jq
curl http://localhost:3002/health | jq

# 测试WebAuthn端点
curl -X POST http://localhost:3001/api/webauthn/register/begin \
  -H "Content-Type: application/json" \
  -d '{"user_id":"test","user_name":"test@example.com","user_display_name":"Test User","rp_name":"AirAccount","rp_id":"localhost"}'
```

## 性能基准

**测试环境:** MacBook Pro M1, 16GB RAM, QEMU ARM64

| 操作 | Rust CA | Node.js CA | 说明 |
|------|---------|------------|------|
| 初始化 | ~100ms | ~150ms | 包含TEE连接检查 |
| WebAuthn注册 | ~50ms | ~80ms | Challenge生成 |
| 账户创建 | ~200ms | ~250ms | 包含TEE钱包创建 |
| 余额查询 | ~30ms | ~50ms | TEE地址派生 |
| 转账签名 | ~100ms | ~120ms | TEE交易签名 |

## 贡献

欢迎提交问题报告和改进建议！

## 许可证

MIT License