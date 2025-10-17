# 启用真实的 CA 和 TA 日志监控

*最后更新: 2025-09-30 20:30*

## 问题说明

默认情况下，KMS API Server 的日志在 QEMU Guest VM 内部（`/tmp/kms.log`），无法从 Docker/Mac 直接访问。

**解决方案**: 将日志输出到**共享目录** `/root/shared`，这样就可以在 Docker 中直接读取。

## 手动配置步骤

### Step 1: 连接到 QEMU Guest VM

在一个**单独的终端**中运行：

```bash
socat - TCP:localhost:54320
```

你会看到 QEMU 的串口输出。按 `Enter` 几次，应该会出现登录提示或 shell 提示符。

### Step 2: 登录（如果需要）

```bash
# 用户名
root

# 密码（通常无密码，直接按 Enter）
```

### Step 3: 停止现有的 KMS API Server

```bash
killall kms-api-server
```

### Step 4: 挂载共享目录（如果未挂载）

```bash
# 检查是否已挂载
mount | grep shared

# 如果没有，执行挂载
mkdir -p /root/shared
mount -t 9p -o trans=virtio host /root/shared
```

### Step 5: 启动 KMS API Server 并将日志输出到共享目录

```bash
cd /root/shared
./kms-api-server > /root/shared/kms-api.log 2>&1 &
```

### Step 6: 验证日志文件

```bash
# 在 QEMU Guest 中
ls -lh /root/shared/kms-api.log
tail /root/shared/kms-api.log
```

### Step 7: 退出 socat 连接

按 `Ctrl+C` 或 `Ctrl+D` 退出 socat

## 启动真实日志监控

现在你可以使用真实日志监控脚本：

```bash
# 启动完整的监控系统（真实日志版）
./scripts/monitor-all-tmux-direct.sh
```

这会启动 4 个面板：
- **Terminal 1**: QEMU 系统日志
- **Terminal 2**: **真实的 CA 日志**（从共享文件读取）
- **Terminal 3**: **真实的 TA 日志**（从 dmesg 读取）
- **Terminal 4**: Cloudflared 日志

## 验证

### 验证 CA 日志可见

```bash
# 在 Docker 中
docker exec teaclave_dev_env tail -f /opt/teaclave/shared/kms-api.log
```

你应该看到 Rust 的日志输出，例如：
```
[2025-09-30T12:30:45Z INFO] Starting KMS API Server on 0.0.0.0:3000
[2025-09-30T12:30:46Z INFO] POST /CreateKey received
[2025-09-30T12:30:46Z DEBUG] Calling TA: CreateWallet
...
```

### 测试 API 并观察日志

```bash
# 发送测试请求
curl -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"Description":"test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
```

在监控界面的 Terminal 2，你应该立即看到新的日志行。

## 关于 TA 日志

OP-TEE TA 的日志有两种：

### 1. 系统级日志（dmesg）

这是 OP-TEE 框架级别的日志，显示 TA 会话管理等信息：

```bash
# 在 QEMU Guest 中
dmesg | grep -i optee | tail -20
```

典型输出：
```
[  123.456] optee: loading out-of-tree module
[  123.567] optee: OP-TEE found, version 3.20.0
[  200.123] optee_client: session opened with TA 4319f351...
[  200.234] optee_client: invoke command 0x1001
```

### 2. TA 内部日志（需要启用 trace）

如果你想看到 TA 内部的详细日志（例如"正在生成助记词"），需要：

**a. 在 TA 代码中添加 trace**

编辑 `kms/ta/src/lib.rs`:

```rust
use optee_utee::trace_println;

fn create_wallet(...) -> Result<...> {
    trace_println!("[TA] Creating new wallet");

    // 生成助记词
    let mnemonic = generate_mnemonic()?;
    trace_println!("[TA] Generated mnemonic: {} words", mnemonic.word_count());

    // ...
    trace_println!("[TA] Wallet created: {}", uuid);
    Ok(...)
}
```

**b. 重新编译和部署**

```bash
./scripts/kms-deploy.sh
```

**c. trace 输出位置**

`trace_println!()` 的输出会出现在：
- **CA 的标准输出**（不是 dmesg）
- 如果 CA 将输出重定向到文件，trace 也会在那个文件中

## 监控脚本对比

| 脚本 | CA 日志来源 | TA 日志来源 | 需要手动配置 |
|------|-------------|-------------|--------------|
| `monitor-all-tmux.sh` | socat（不稳定） | socat（不稳定） | 否 |
| `monitor-all-tmux-v2.sh` | Cloudflared 推断 | 命令参考 | 否 |
| `monitor-all-tmux-direct.sh` | 共享文件（真实） | dmesg（真实） | **是** |

## 快速参考

### 完整工作流

```bash
# 1. 在单独终端中连接 QEMU 并配置日志
socat - TCP:localhost:54320
# (按上述步骤配置)

# 2. 退出 socat 后，启动监控
./scripts/monitor-all-tmux-direct.sh

# 3. 测试 API
open https://kms.aastar.io/test
```

### 单独监控 CA 日志

```bash
./scripts/monitor-terminal2-ca-direct.sh
```

### 单独监控 TA 日志

```bash
./scripts/monitor-terminal3-ta-direct.sh
```

## 故障排查

### Q: 共享目录中没有 kms-api.log 文件

**A**: KMS API Server 可能未将日志输出到共享目录。

**解决**:
1. 连接到 QEMU: `socat - TCP:localhost:54320`
2. 检查进程: `ps aux | grep kms-api-server`
3. 如果看到 `./kms-api-server` 但没有重定向，重启它：
   ```bash
   killall kms-api-server
   cd /root/shared && ./kms-api-server > /root/shared/kms-api.log 2>&1 &
   ```

### Q: CA 日志中看不到 trace_println! 输出

**A**: `trace_println!()` 是 TA 内部的宏，输出会出现在 CA 的标准输出（因为 TA 通过 CA 的进程启动）。

确保：
1. TA 已重新编译
2. CA 的日志包含标准输出和标准错误: `> log 2>&1`

### Q: dmesg 中没有 OP-TEE 日志

**A**: 可能是日志级别设置或 TA 还没有被调用。

**解决**:
1. 发送一个 API 请求触发 TA 调用
2. 立即查看 dmesg: `dmesg | grep -i optee | tail -20`

## 总结

- ✅ **推荐方案**: `monitor-all-tmux-direct.sh` - 真实的 CA 和 TA 日志
- ⚠️  **需要一次性手动配置**: 将 KMS API Server 的日志重定向到共享目录
- 🎯 **结果**: 可以看到真实的 Rust 日志和 OP-TEE 内核日志

---

*如有问题，参考 docs/Monitoring-Troubleshooting.md*