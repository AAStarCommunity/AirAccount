# 监控系统故障排查

*最后更新: 2025-09-30 20:15*

## 常见问题和解决方案

### 问题 1: Terminal 2 (CA) 和 Terminal 3 (TA) 没有日志显示

#### 症状
- Terminal 1 (QEMU) 和 Terminal 4 (Cloudflared) 显示正常
- Terminal 2 (CA) 显示"开始监控..."但没有日志输出
- Terminal 3 (TA) 显示"提示: OP-TEE 日志通常在..."但没有实际日志

#### 根本原因
**socat 命令在 tmux 环境中不稳定**

原始监控脚本使用 `socat` 连接到 QEMU 串口 (`tcp:localhost:54320`) 来读取 Guest VM 内部的日志:

```bash
# 原始方案 - 在 tmux 中不稳定
docker exec -it teaclave_dev_env bash -c "
(
echo 'tail -f /tmp/kms.log'
) | socat - TCP:localhost:54320
"
```

**问题**:
1. `socat` 在交互式 shell (`-it`) 中需要持续的终端连接
2. 在 tmux 的面板中，这种连接容易超时或阻塞
3. `socat` 发送命令后，如果 QEMU Guest 没有立即响应，会导致挂起

#### 解决方案: 使用替代监控脚本

我们创建了改进版的监控脚本，不依赖 socat:

##### 方案 A: 使用 V2 监控脚本（推荐）

```bash
./scripts/monitor-all-tmux-v2.sh
```

这个脚本使用：
- `monitor-terminal2-ca-alt.sh` - 从 Cloudflared 日志提取 API 调用
- `monitor-terminal3-ta-alt.sh` - 显示 TA 命令参考和状态信息

**优点**:
- ✅ 完全稳定，不使用 socat
- ✅ Terminal 2 能看到所有 API 端点调用
- ✅ Terminal 3 显示 TA 支持的命令和状态
- ✅ 通过 Terminal 2 和 4 的组合可以推断 TA 操作

##### 方案 B: 手动查看 QEMU 内的日志

如果你想看到真实的 CA 日志，可以直接连接到 QEMU Guest VM:

```bash
# 在单独的终端中连接到 QEMU
./scripts/connect-to-qemu-shell.sh

# 或者使用 socat
socat - TCP:localhost:54320

# 登录后查看日志
tail -f /tmp/kms.log
```

**注意**: 这种方式占用了 QEMU 串口，会阻止监控脚本工作。

##### 方案 C: 使用日志文件挂载

如果 QEMU Guest 能够挂载共享目录，可以将日志写入共享目录:

```bash
# 在 QEMU Guest 中
mount -t 9p -o trans=virtio host /root/shared
./kms-api-server > /root/shared/kms.log 2>&1 &

# 在 Mac/Docker 中监控
docker exec -it teaclave_dev_env tail -f /opt/teaclave/shared/kms.log
```

### 问题 2: 监控脚本启动时卡住

#### 症状
运行 `./scripts/monitor-all-tmux.sh` 后，某个面板卡在"开始监控..."不动

#### 原因
socat 尝试连接但超时

#### 解决
```bash
# 1. 杀掉卡住的 tmux 会话
tmux kill-session -t kms-monitor

# 2. 使用 V2 脚本
./scripts/monitor-all-tmux-v2.sh
```

### 问题 3: Terminal 4 (Cloudflared) 看不到 HTTP 请求

#### 症状
Terminal 4 只显示隧道连接信息，没有 GET/POST 请求记录

#### 原因
cloudflared 默认日志级别（info）不记录 HTTP 请求

#### 解决
```bash
# 重启 cloudflared 并启用 debug 日志
./scripts/start-cloudflared-debug.sh

# 然后启动监控
./scripts/monitor-all-tmux-v2.sh
```

**验证**:
```bash
# 应该看到类似的日志
2025-09-30T12:08:47Z DBG POST https://kms.aastar.io/CreateKey HTTP/1.1
2025-09-30T12:08:47Z DBG 200 OK
```

### 问题 4: Terminal 3 (TA) 日志为空是正常的吗？

#### 回答
**是的，这是正常现象。**

**原因**:
1. **OP-TEE TA 默认不输出详细日志**: Secure World 的日志需要在编译时启用
2. **dmesg 只显示框架级别日志**: 例如 "TA session opened", "TA invoke command" 等
3. **TA 内部操作是安全的**: 详细的加密操作不应该输出到系统日志

**如何看到 TA 操作？**

通过**推断**的方式：
1. **Terminal 2** 显示哪个 API 被调用 (例如 `POST /CreateKey`)
2. **Terminal 4** 显示请求成功 (`200 OK`)
3. **结论**: TA 成功执行了 `CMD_CREATE_WALLET` 命令

**完整的调用链示例**:
```
[Terminal 4] 12:08:47 POST /CreateKey
[Terminal 2] └─ 正在调用 TA: 创建新钱包 (CMD_CREATE_WALLET)
[Terminal 3] (TA 在 Secure World 中执行...)
[Terminal 2] ✅ 响应: 200 OK (size: 512 bytes)
[Terminal 4] 200 OK
```

### 问题 5: 如何启用 TA 详细日志？

#### 方法 1: 在 TA 代码中使用 trace

编辑 `kms/ta/src/lib.rs`:

```rust
use optee_utee::trace_println;

fn create_wallet(...) -> Result<...> {
    trace_println!("TA: Creating new wallet");
    // ...
    trace_println!("TA: Wallet created: {}", uuid);
    Ok(...)
}
```

重新编译并部署：
```bash
./scripts/kms-deploy.sh
```

**注意**: trace 输出会出现在 **CA 的输出**中，不是 dmesg。

#### 方法 2: 在 OP-TEE 编译时启用 debug

这需要重新编译整个 OP-TEE 系统，比较复杂，不推荐用于开发调试。

## 推荐的监控工作流

### 日常开发监控

```bash
# 1. 确保 cloudflared 启用 debug 日志
./scripts/start-cloudflared-debug.sh

# 2. 启动 V2 监控（稳定版）
./scripts/monitor-all-tmux-v2.sh

# 3. 在浏览器测试
# 访问 https://kms.aastar.io/test

# 4. 观察监控
# - Terminal 1: QEMU 系统状态
# - Terminal 2: API 调用和 TA 操作描述
# - Terminal 3: TA 命令参考
# - Terminal 4: 完整的 HTTP 请求/响应
```

### 深度调试时

如果需要看到真实的 CA 日志：

```bash
# 在单独的终端连接到 QEMU Guest
socat - TCP:localhost:54320

# 登录 (如果需要)
# 用户名: root (通常无密码)

# 查看实时日志
tail -f /tmp/kms.log

# 查看 OP-TEE 内核日志
dmesg | grep -i "optee\|tee" | tail -30
```

### 监控脚本对比

| 脚本 | Terminal 2 (CA) | Terminal 3 (TA) | 稳定性 | 推荐 |
|------|-----------------|-----------------|--------|------|
| `monitor-all-tmux.sh` | 使用 socat | 使用 socat | ❌ 不稳定 | ❌ |
| `monitor-all-tmux-v2.sh` | 从 Cloudflared 提取 | 显示命令参考 | ✅ 稳定 | ✅ |
| 手动 socat | 真实日志 | 真实日志 | ⚠️  单独终端可用 | ⏱️ 调试时 |

## 技术细节

### 为什么 socat 在 tmux 中不稳定？

1. **tmux 面板不是真正的 tty**: tmux 创建的是伪终端（pty）
2. **socat 需要持续的双向通信**: 在 pty 中，I/O 缓冲可能导致阻塞
3. **QEMU 串口的 TCP 模式**: `server,nowait` 意味着只接受一个连接，多个 socat 进程会冲突

### 替代方案的权衡

**从 Cloudflared 日志提取 API 调用**:
- ✅ 优点: 稳定、实时、包含所有 HTTP 信息
- ❌ 缺点: 看不到 CA 内部的 Rust 日志（例如 `log::info!` 输出）

**显示 TA 命令参考而不是日志**:
- ✅ 优点: 稳定、提供有用的参考信息
- ❌ 缺点: 不是实时的 TA 日志

**总体而言**: 对于日常开发和测试，V2 方案已经足够。深度调试时可以手动使用 socat。

## 总结

- ✅ **推荐使用**: `./scripts/monitor-all-tmux-v2.sh`
- ✅ **启动前运行**: `./scripts/start-cloudflared-debug.sh`
- ⚠️  **Terminal 2/3 的原始脚本**: 在 tmux 中不稳定
- 💡 **TA 日志为空是正常的**: 通过 API 调用推断 TA 操作

---

*如有其他问题，请查看 docs/Monitoring-Guide.md 和 docs/Monitoring-Setup.md*