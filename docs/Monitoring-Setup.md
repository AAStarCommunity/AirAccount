# 监控系统设置说明

## 重要：启用 Cloudflared Debug 日志

### 问题说明

默认情况下，cloudflared 的日志级别较低，**不会记录 HTTP 请求详情**。你会看到：
- ✅ 隧道连接状态（Registered tunnel connection）
- ✅ QUIC 协议信息
- ❌ **没有 HTTP GET/POST 请求记录**
- ❌ **没有响应状态码（200 OK, 404等）**

这会导致 Terminal 4 的监控脚本**看不到任何 API 请求**，即使 API 实际上在正常工作。

### 解决方案

在启动监控之前，需要先用 debug 模式重启 cloudflared：

```bash
# 1. 重启 cloudflared（启用 debug 日志）
./scripts/start-cloudflared-debug.sh

# 2. 然后启动监控系统
./scripts/monitor-all-tmux.sh
```

### 日志对比

**❌ 默认日志级别（info）- 看不到请求：**
```
2025-09-30T11:07:55Z INF Registered tunnel connection connIndex=0 ...
2025-09-30T11:07:56Z INF Registered tunnel connection connIndex=1 ...
(看不到任何 HTTP 请求)
```

**✅ Debug 日志级别 - 可以看到完整请求：**
```
2025-09-30T12:01:17Z DBG GET https://kms.aastar.io/health HTTP/1.1 connIndex=0 ...
2025-09-30T12:01:17Z DBG 200 OK connIndex=0 content-length=194 ...
2025-09-30T12:01:20Z DBG POST https://kms.aastar.io/CreateKey HTTP/1.1 connIndex=1 ...
2025-09-30T12:01:21Z DBG 200 OK connIndex=1 content-length=512 ...
```

## 完整的监控启动流程

### 快速启动（推荐）

```bash
# 一键启动（会自动配置 cloudflared）
./scripts/start-cloudflared-debug.sh && ./scripts/monitor-all-tmux.sh
```

### 分步启动

```bash
# Step 1: 确保 cloudflared 以 debug 模式运行
./scripts/start-cloudflared-debug.sh

# Step 2: 启动 tmux 四分屏监控
./scripts/monitor-all-tmux.sh

# 或者在 4 个独立终端中启动：
./scripts/monitor-terminal1-qemu.sh
./scripts/monitor-terminal2-ca.sh
./scripts/monitor-terminal3-ta.sh
./scripts/monitor-terminal4-cloudflared.sh
```

## 验证监控是否正常工作

### 测试步骤

1. **启动监控系统**（如上所述）

2. **发送测试请求**：
   ```bash
   curl https://kms.aastar.io/health
   ```

3. **检查 Terminal 4（Cloudflared）**：
   - ✅ 应该看到：`DBG GET https://kms.aastar.io/health`
   - ✅ 应该看到：`DBG 200 OK`

4. **在 Web UI 测试**：
   - 访问 https://kms.aastar.io/test
   - 点击任意 API 测试按钮
   - Terminal 4 应该立即显示对应的 POST 请求

### 如果看不到请求记录

```bash
# 检查 cloudflared 是否以 debug 模式运行
docker exec teaclave_dev_env ps aux | grep cloudflared | grep debug

# 如果没有 "--loglevel debug"，重新启动：
./scripts/start-cloudflared-debug.sh

# 手动查看日志验证
docker exec teaclave_dev_env tail -20 /tmp/cloudflared.log | grep -E "GET|POST|DBG"
```

## 监控界面说明

### Terminal 1: QEMU Guest VM
- **监控内容**: 系统启动日志、内核消息
- **日志文件**: `/tmp/qemu.log` (Docker 内)
- **更新频率**: 实时

### Terminal 2: KMS API Server (CA)
- **监控内容**: HTTP 请求处理、TA 调用
- **日志文件**: `/tmp/kms.log` (QEMU Guest 内)
- **更新频率**: 实时
- **注意**: 通过 socat 连接到 QEMU 串口

### Terminal 3: OP-TEE TA (Secure World)
- **监控内容**: TA 命令执行、Secure World 操作
- **日志来源**: `dmesg | grep optee`
- **更新频率**: 每 3 秒刷新
- **注意**: 需要 TA 启用 trace 才能看到详细日志

### Terminal 4: Cloudflared Tunnel ⚠️ 需要 Debug 模式
- **监控内容**: 公网 HTTPS 请求、隧道状态
- **日志文件**: `/tmp/cloudflared.log` (Docker 内)
- **更新频率**: 实时
- **⚠️ 重要**: 必须以 `--loglevel debug` 启动才能看到请求

## 常见问题

### Q1: Terminal 4 看不到任何请求

**原因**: cloudflared 没有以 debug 模式运行

**解决**:
```bash
./scripts/start-cloudflared-debug.sh
```

### Q2: Terminal 2 (CA) 没有日志输出

**原因**: KMS API Server 未启动或日志文件不存在

**解决**:
```bash
# 在 QEMU Guest 中重启服务
docker exec teaclave_dev_env bash -c "
(
echo 'killall kms-api-server'
sleep 1
echo 'cd /root/shared && ./kms-api-server > /tmp/kms.log 2>&1 &'
sleep 2
) | socat - TCP:localhost:54320
"
```

### Q3: Terminal 3 (TA) 日志为空

**原因**: OP-TEE 默认不输出详细日志到 dmesg

**说明**:
- TA 内部日志需要在编译时启用
- 大部分 TA 操作可以通过 Terminal 2 的 CA 日志看到
- dmesg 主要显示 OP-TEE 框架级别的日志

### Q4: 监控界面看起来很混乱

**提示**: 使用 tmux 滚动模式查看历史日志

```bash
# 在 tmux 中：
1. 按 Ctrl+B, 然后按 [（进入滚动模式）
2. 使用方向键或 Page Up/Down 滚动
3. 按 q 退出滚动模式
```

## 性能说明

- **监控开销**: 极低（主要是 tail -f）
- **日志文件大小**: cloudflared debug 日志会快速增长
- **建议**: 生产环境使用 info 级别，开发调试使用 debug 级别

## 持久化配置

如果希望 cloudflared 始终以 debug 模式启动，修改配置文件：

```bash
# 编辑 Docker 内的配置
docker exec teaclave_dev_env nano /root/.cloudflared/config-docker.yml

# 添加或修改：
loglevel: debug
```

然后重启 cloudflared：
```bash
docker exec teaclave_dev_env pkill cloudflared
docker exec -d teaclave_dev_env bash -c \
  "cloudflared tunnel --config /root/.cloudflared/config-docker.yml run kms-tunnel > /tmp/cloudflared.log 2>&1"
```

---

*最后更新: 2025-09-30 19:05*