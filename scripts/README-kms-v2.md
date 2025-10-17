# KMS 自动启动脚本 v2 使用指南

## 概述

`kms-auto-start-v2.sh` 是新版本的 KMS 启动脚本，专为开发调试设计。与 v1 版本的主要区别：

- ✅ 启动 QEMU，但**不自动启动 API Server**
- ✅ 保持 Guest VM 串口（54320）可交互
- ✅ 自动管理 Cloudflare Tunnel
- ✅ 提供完整的交互式管理工具

## 快速开始

```bash
# 1. 启动 KMS 环境（不启动 API Server）
./scripts/kms-auto-start-v2.sh

# 2. 使用交互式工具
./scripts/kms-guest-interactive.sh
# 选择选项 3: 启动 API Server

# 3. 监控日志（可选）
./scripts/kms-qemu-terminal2-enhanced.sh  # 监控 CA
./scripts/kms-qemu-terminal3-v2.sh        # 监控 TA
```

## 版本对比

| 特性 | v1 (kms-auto-start.sh) | v2 (kms-auto-start-v2.sh) |
|------|----------------------|---------------------------|
| QEMU 启动 | ✅ | ✅ |
| API Server | ✅ 自动启动 | ❌ 手动启动 |
| 串口可用性 | ❌ 被占用 | ✅ 可交互 |
| Cloudflare Tunnel | 需手动 | ✅ 自动 |
| 监听器管理 | 外部脚本 | ✅ 内置 |
| 适用场景 | 生产部署 | 开发调试 |

## 详细使用说明

### 1. 启动 KMS 环境

```bash
./scripts/kms-auto-start-v2.sh
```

脚本会自动完成：
1. 清理旧的 QEMU 和监听器进程
2. 启动 Secure World 日志监听器（端口 54321）
3. 启动 Guest VM 串口监听器（端口 54320）
4. 启动 QEMU（带 3000 端口转发）
5. 启动 Cloudflare Tunnel

### 2. 管理 API Server

#### 方式 A：使用交互式工具（推荐）

```bash
./scripts/kms-guest-interactive.sh
```

菜单选项：
- **选项 1**: 查看 shared 目录文件
- **选项 2**: 检查 Guest VM 状态
- **选项 3**: 启动 API Server ⭐
- **选项 4**: 停止 API Server
- **选项 5**: 检查 API Server 状态
- **选项 6**: 列出钱包
- **选项 7**: 执行自定义命令
- **选项 8**: 部署新的 TA 二进制

#### 方式 B：命令行直接启动

```bash
# 启动 API Server
echo 'cd /root/shared && nohup ./kms_ca > api.log 2>&1 &' | \
  docker exec -i teaclave_dev_env socat - TCP:localhost:54320

# 等待启动（15 秒）
sleep 15

# 测试
curl http://localhost:3000/health
```

#### 方式 C：停止 API Server

```bash
echo 'pkill -f kms_ca' | \
  docker exec -i teaclave_dev_env socat - TCP:localhost:54320
```

### 3. 监控日志

#### CA (Client Application) 日志

```bash
./scripts/kms-qemu-terminal2-enhanced.sh
```

显示：
- API Server 健康状态
- HTTP 请求/响应
- CA → TA 调用链

#### TA (Trusted Application) 日志

```bash
./scripts/kms-qemu-terminal3-v2.sh
```

显示：
- Secure World 启动信息
- TA 加载和初始化
- TA 内部日志输出

⚠️ **注意**：使用 v2 启动脚本时，必须使用 `kms-qemu-terminal3-v2.sh`（而不是旧的 `kms-qemu-terminal3.sh`），以避免端口冲突。

### 4. 直接访问 Guest VM Shell（高级）

```bash
docker exec -it teaclave_dev_env socat STDIN TCP:localhost:54320
```

然后可以直接输入命令：
```bash
# 在 Guest VM 中执行
cd /root/shared
ls -la
./kms_ca > api.log 2>&1 &
ps aux | grep kms
```

按 `Ctrl+C` 退出。

## 常见任务

### 部署新的 TA 二进制

```bash
# 方式 1: 使用交互式工具
./scripts/kms-guest-interactive.sh
# 选择选项 8

# 方式 2: 命令行
docker exec teaclave_dev_env bash -c "
    cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta \
       /opt/teaclave/shared/ta/
"
```

### 导出私钥

```bash
# 使用交互式工具
./scripts/kms-guest-interactive.sh
# 选择选项 7，输入：
# cd /root/shared && ./export_key <wallet-id> "m/44'/60'/0'/0/0"
```

### 检查系统状态

```bash
# QEMU 状态
docker exec teaclave_dev_env pgrep -f qemu-system-aarch64

# 端口转发
docker exec teaclave_dev_env ps aux | grep qemu | grep hostfwd

# API Server 状态
curl http://localhost:3000/health

# Cloudflare Tunnel
ps aux | grep cloudflared | grep -v grep

# 公网访问（如 API Server 已启动）
curl https://kms.aastar.io/health
```

## 故障排查

### 问题：API Server 无响应

```bash
# 1. 检查进程
echo 'ps aux | grep kms_ca' | \
  docker exec -i teaclave_dev_env socat - TCP:localhost:54320

# 2. 检查日志
docker exec teaclave_dev_env cat /opt/teaclave/shared/api.log

# 3. 重启 API Server
echo 'pkill -f kms_ca' | \
  docker exec -i teaclave_dev_env socat - TCP:localhost:54320
# 然后重新启动
```

### 问题：端口 54321 冲突

```bash
# 错误信息: Address already in use
# 原因: 使用了旧的 terminal3 脚本

# 解决方案：使用 v2 版本
./scripts/kms-qemu-terminal3-v2.sh  # 正确
# 而不是
./scripts/kms-qemu-terminal3.sh     # 错误（会冲突）
```

### 问题：Guest VM 串口无响应

```bash
# 检查监听器
docker exec teaclave_dev_env pgrep -f "socat.*54320"

# 检查 QEMU
docker exec teaclave_dev_env pgrep -f qemu-system

# 重启整个环境
./scripts/kms-auto-start-v2.sh
```

## 脚本文件说明

| 文件 | 用途 | 版本 |
|------|------|------|
| `kms-auto-start-v2.sh` | 主启动脚本 | v2 |
| `kms-guest-interactive.sh` | 交互式管理工具 | v2 |
| `kms-qemu-terminal2-enhanced.sh` | CA 日志监控 | 通用 |
| `kms-qemu-terminal3-v2.sh` | TA 日志监控 | v2 |
| `kms-guest-exec.sh` | 单命令执行 | 通用 |
| `kms-guest-shell.sh` | 简单 shell | 通用 |
| `kms-auto-start.sh` | 旧版启动脚本 | v1 |
| `kms-qemu-terminal3.sh` | 旧版 TA 监控 | v1 |

## 与 v1 的兼容性

两个版本可以共存，但不要同时运行。选择适合您场景的版本：

**使用 v1 (生产部署)**：
```bash
./scripts/kms-auto-start.sh
```
- 自动启动所有服务
- 无需手动干预
- 适合快速部署

**使用 v2 (开发调试)**：
```bash
./scripts/kms-auto-start-v2.sh
```
- 保持串口可交互
- 灵活的 API Server 管理
- 适合开发和测试

## 更新日志

### 2025-10-16
- 创建 `kms-auto-start-v2.sh`
- 创建 `kms-guest-interactive.sh`
- 创建 `kms-qemu-terminal3-v2.sh`
- 修复 `lsof` 依赖问题
- 添加 Cloudflare Tunnel 自动管理
- 文档完善

---

**维护者**: Claude Code
**最后更新**: 2025-10-16
