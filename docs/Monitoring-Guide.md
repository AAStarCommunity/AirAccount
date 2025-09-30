# KMS API 监控指南

## 概述

完整的 KMS API 调用链监控系统，让你实时看到从公网请求到 OP-TEE Secure World 的完整执行过程。

## 监控架构

```
┌─────────────────────────────────────────────────────┐
│ Web UI / API Client                                 │
│ https://kms.aastar.io                               │
└──────────────┬──────────────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 4: Cloudflared Tunnel                      │
│ - 监控 HTTPS 请求进入                                │
│ - 显示隧道连接状态                                   │
│ - 记录响应时间                                       │
└──────────────┬──────────────────────────────────────┘
               │
               ↓ (localhost:3000)
┌─────────────────────────────────────────────────────┐
│ Terminal 2: KMS API Server (CA - Normal World)      │
│ - HTTP 请求解析                                      │
│ - API 端点路由                                       │
│ - CA → TA 调用                                       │
└──────────────┬──────────────────────────────────────┘
               │
               ↓ (TEEC API)
┌─────────────────────────────────────────────────────┐
│ Terminal 3: OP-TEE TA (Secure World)                │
│ - TA 命令处理                                        │
│ - 密钥管理操作                                       │
│ - 加密签名操作                                       │
└──────────────┬──────────────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 1: QEMU Guest VM                           │
│ - 系统日志                                           │
│ - 内核消息                                           │
│ - QEMU 运行状态                                      │
└─────────────────────────────────────────────────────┘
```

## 快速启动

### 方案 A: 一键启动（推荐）

使用 tmux 在单个窗口中启动所有监控：

```bash
./scripts/monitor-all-tmux.sh
```

这会自动创建一个 tmux 会话，分为 4 个面板：
- **左上**: QEMU 监控
- **右上**: CA (KMS API Server) 监控
- **左下**: TA (Secure World) 监控
- **右下**: Cloudflared Tunnel 监控

**tmux 快捷键**:
- `Ctrl+B`, `方向键` - 在面板间切换
- `Ctrl+B`, `[` - 进入滚动模式（`q` 退出）
- `Ctrl+B`, `d` - 断开会话（监控继续运行）
- `Ctrl+B`, `&` - 关闭整个会话

### 方案 B: 手动启动（4个终端窗口）

在 4 个独立的终端窗口中分别运行：

```bash
# Terminal 1 - QEMU 监控
./scripts/monitor-terminal1-qemu.sh

# Terminal 2 - CA 监控
./scripts/monitor-terminal2-ca.sh

# Terminal 3 - TA 监控
./scripts/monitor-terminal3-ta.sh

# Terminal 4 - Cloudflared 监控
./scripts/monitor-terminal4-cloudflared.sh
```

## 监控内容详解

### Terminal 1: QEMU Guest VM

**监控内容**:
- QEMU 进程状态
- Guest VM 启动日志
- 系统内核消息
- QEMU 运行时错误

**日志文件**: `/tmp/qemu.log` (在 Docker 内)

**示例输出**:
```
📊 QEMU 进程信息:
root   19299  2.1  6.7 3140588 544776 ?  Sl  10:54  0:18 qemu-system-aarch64 ...

📝 最近的 QEMU 日志:
[    0.000000] Booting Linux on physical CPU 0x0
[    0.000000] Linux version 5.15.0-optee ...
```

### Terminal 2: KMS API Server (CA)

**监控内容**:
- HTTP 请求接收
- API 端点匹配
- 请求体解析
- CA → TA 调用
- 响应返回

**日志文件**: `/tmp/kms.log` (在 QEMU Guest 内)

**示例输出**:
```
🚀 KMS API Server starting on http://0.0.0.0:3000
📚 Supported APIs:
   POST /CreateKey - Create new TEE wallet
   ...

[2025-09-30T11:30:45Z] POST /CreateKey
[2025-09-30T11:30:45Z] Calling TA: CreateWallet
[2025-09-30T11:30:46Z] TA returned: wallet_uuid=xxx
[2025-09-30T11:30:46Z] Response 200 OK
```

### Terminal 3: OP-TEE TA (Secure World)

**监控内容**:
- TA 会话打开/关闭
- TA 命令调用
- Secure World 操作
- OP-TEE 内核消息

**日志来源**: `dmesg | grep optee` (在 QEMU Guest 内)

**示例输出**:
```
[  100.123456] optee: loading out-of-tree module taints kernel.
[  100.234567] optee: OP-TEE found, version 3.20.0
[  200.345678] optee: supplicant opened
[  300.456789] optee_client: session opened with TA 4319f351-...
[  300.567890] optee_client: invoke command 0x1001
```

**注意**: OP-TEE TA 的详细日志需要在编译时启用 debug 模式。

### Terminal 4: Cloudflared Tunnel

**监控内容**:
- 隧道连接状态
- HTTPS 请求进入
- 请求转发到 localhost:3000
- 响应返回
- 错误和重连

**日志文件**: `/tmp/cloudflared.log` (在 Docker 内)

**示例输出**:
```
2025-09-30T11:30:45Z INF Registered tunnel connection connIndex=0
2025-09-30T11:30:46Z INF Request received method=POST path=/CreateKey
2025-09-30T11:30:46Z INF Proxying to origin url=http://localhost:3000/CreateKey
2025-09-30T11:30:47Z INF Response sent status=200 duration=1.2s
```

## 测试流程示例

### 1. 启动监控

```bash
./scripts/monitor-all-tmux.sh
```

### 2. 发送测试请求

在浏览器或新终端中：

**浏览器**: 访问 https://kms.aastar.io

**命令行**:
```bash
curl -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{
    "Description": "test-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'
```

### 3. 观察调用链

**Terminal 4 (Cloudflared)**:
```
✅ 看到 HTTPS 请求从公网进入
2025-09-30T11:30:46Z INF Request POST /CreateKey
```

**Terminal 2 (CA)**:
```
✅ KMS API Server 处理请求
POST /CreateKey received
Parsing request body...
Calling TA CreateWallet command...
```

**Terminal 3 (TA)**:
```
✅ TA 在 Secure World 执行
optee_client: invoke command 0x1001 (CreateWallet)
TA: Generating new mnemonic
TA: Wallet created: uuid=xxx
```

**Terminal 4 (Cloudflared)**:
```
✅ 响应返回到公网
2025-09-30T11:30:47Z INF Response 200 OK duration=1.2s
```

## 完整测试场景

### 场景 1: 创建钱包

```bash
# 发送请求
curl -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"Description":"my-wallet","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
```

**预期监控输出**:
1. Terminal 4: 请求进入
2. Terminal 2: POST /CreateKey → CA 调用 TA
3. Terminal 3: TA 生成助记词和密钥
4. Terminal 2: 返回 wallet UUID
5. Terminal 4: 200 OK 响应

### 场景 2: 派生地址

```bash
# 使用上一步的 KeyId
curl -X POST https://kms.aastar.io/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.DeriveAddress" \
  -d '{"KeyId":"<your-wallet-uuid>","DerivationPath":"m/44'"'"'/60'"'"'/0'"'"'/0/0"}'
```

**预期监控输出**:
1. Terminal 4: DeriveAddress 请求
2. Terminal 2: 解析派生路径
3. Terminal 3: TA 派生私钥 → 计算地址
4. Terminal 2: 返回以太坊地址
5. Terminal 4: 200 OK

### 场景 3: 签名交易

```bash
curl -X POST https://kms.aastar.io/Sign \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.Sign" \
  -d '{
    "KeyId":"<your-wallet-uuid>",
    "Message":"SGVsbG8gV29ybGQ=",
    "SigningAlgorithm":"ECDSA_SHA_256"
  }'
```

**预期监控输出**:
1. Terminal 4: Sign 请求进入
2. Terminal 2: Base64 解码消息
3. Terminal 3: TA 使用私钥签名
4. Terminal 2: 返回签名结果
5. Terminal 4: 200 OK

## 故障排查

### 问题 1: Terminal 2 没有日志输出

**原因**: KMS API Server 未启动或日志文件不存在

**解决**:
```bash
# 检查进程
docker exec teaclave_dev_env bash -c "(echo 'ps aux | grep kms-api-server'; sleep 1) | socat - TCP:localhost:54320"

# 重启服务
docker exec teaclave_dev_env bash -c "
(
echo 'killall kms-api-server'
sleep 1
echo 'cd /root/shared && ./kms-api-server > /tmp/kms.log 2>&1 &'
sleep 2
) | socat - TCP:localhost:54320
"
```

### 问题 2: Terminal 3 TA 日志为空

**原因**: OP-TEE 日志级别太低

**说明**: TA 内部日志默认不输出到 dmesg。要查看 TA 详细日志：
1. 在 TA 代码中使用 `trace_println!()` 宏
2. 重新编译 TA 并部署
3. 日志会出现在 CA 的输出中

### 问题 3: Terminal 4 显示连接错误

**原因**: cloudflared 未运行或无法连接到 localhost:3000

**解决**:
```bash
# 检查 cloudflared
docker exec teaclave_dev_env ps aux | grep cloudflared

# 重启 cloudflared
docker exec -d teaclave_dev_env bash -c \
  "pkill cloudflared; \
   cloudflared tunnel --config /root/.cloudflared/config-docker.yml run kms-tunnel \
   > /tmp/cloudflared.log 2>&1"
```

### 问题 4: socat 连接失败

**原因**: QEMU 串口未配置 TCP 或端口占用

**解决**:
```bash
# 检查 QEMU 启动参数
docker exec teaclave_dev_env ps aux | grep qemu | grep 54320

# 应该看到: -serial tcp:localhost:54320,server,nowait
```

## 高级技巧

### 1. 过滤特定 API 的日志

```bash
# 只监控 CreateKey 请求
docker exec -it teaclave_dev_env bash -c "tail -f /tmp/kms.log | grep CreateKey"

# 只监控错误
docker exec -it teaclave_dev_env bash -c "tail -f /tmp/kms.log | grep -i error"
```

### 2. 添加时间戳

确保 KMS API Server 日志包含时间戳（已在 `api_server.rs` 中通过 `env_logger` 实现）。

### 3. 保存监控日志

```bash
# 保存 CA 日志
docker exec teaclave_dev_env bash -c "cat /tmp/kms.log" > kms-ca-$(date +%Y%m%d-%H%M%S).log

# 保存 cloudflared 日志
docker exec teaclave_dev_env bash -c "cat /tmp/cloudflared.log" > cloudflared-$(date +%Y%m%d-%H%M%S).log
```

### 4. 性能分析

在 Terminal 2 的日志中添加性能计时：
```rust
// 在 api_server.rs 中
let start = std::time::Instant::now();
// ... 执行操作
let duration = start.elapsed();
log::info!("Operation completed in {:?}", duration);
```

## 监控脚本文件

| 脚本 | 功能 | 监控内容 |
|------|------|----------|
| `monitor-terminal1-qemu.sh` | QEMU 监控 | Guest VM 系统日志 |
| `monitor-terminal2-ca.sh` | CA 监控 | HTTP API 和 TA 调用 |
| `monitor-terminal3-ta.sh` | TA 监控 | Secure World 日志 |
| `monitor-terminal4-cloudflared.sh` | 隧道监控 | 公网流量 |
| `monitor-all-tmux.sh` | 一键启动 | 自动启动所有监控 |
| `start-monitoring.sh` | 使用说明 | 显示完整指南 |

## 日志级别

### CA (KMS API Server)

使用环境变量控制日志级别：
```bash
# 在 QEMU Guest 中
export RUST_LOG=debug
./kms-api-server
```

级别：
- `error` - 仅错误
- `warn` - 警告和错误
- `info` - 信息、警告、错误（默认）
- `debug` - 调试信息
- `trace` - 详细跟踪

### TA

TA 日志需要在编译时配置。修改 `ta/Cargo.toml`:
```toml
[features]
default = ["trace"]  # 启用 trace 日志
```

## 最佳实践

1. **开发时**: 使用 `monitor-all-tmux.sh` 启动完整监控
2. **调试时**: 使用单独的终端脚本关注特定层
3. **生产环境**: 将日志输出到文件并使用日志聚合工具
4. **性能测试**: 关注 Terminal 2 的响应时间
5. **安全审计**: 定期检查 Terminal 3 的 TA 调用日志

---

*最后更新: 2025-09-30*