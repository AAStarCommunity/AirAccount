# KMS 系统启动和使用指南

最后更新: 2025-10-01 17:41

## 🚀 快速启动

### Mac 重启或首次启动后：

```bash
# 1. 启动 Docker 容器
docker start teaclave_dev_env

# 2. 启动 Cloudflare 隧道（仅需运行一次）
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &

# 3. 启动 KMS 服务（选择下面的方式 A 或 B）
```

### 方式 A: 一键自动启动 ⭐ 推荐

```bash
./scripts/kms-auto-start.sh
```

**优点**：
- 一条命令完成所有启动
- 自动等待并验证服务
- 45 秒后自动测试 API

**查看日志**：
```bash
./scripts/kms-monitor.sh
```

### 方式 B: 手动三终端启动（用于调试）

**终端 1**（Secure World 日志 - TA）：
```bash
./scripts/terminal3-secure-log.sh
```

**终端 2**（Guest VM Shell - CA）：
```bash
./scripts/terminal2-guest-vm.sh
```

**终端 3**（QEMU + API Server）：
```bash
./scripts/terminal1-qemu.sh
```

**优点**：
- 实时查看所有日志
- 适合开发和调试

## 📡 访问地址

- **本地**: http://localhost:3000
- **公网**: https://kms.aastar.io
- **健康检查**: `curl http://localhost:3000/health`

## 🔧 常用命令

### 查看系统状态
```bash
./scripts/kms-startup-guide.sh
```

### 部署新代码
```bash
./scripts/kms-deploy.sh
```

### 重启 API Server
```bash
./scripts/kms-restart-api.sh
```

### 监控日志
```bash
./scripts/kms-monitor.sh
```
选择：
1. Secure World 日志 (TA)
2. Guest VM Shell (CA)
3. QEMU 日志
4. API Server 日志
5. Cloudflared 日志

## 📝 API 使用示例

所有 POST API 都需要 AWS KMS 兼容的 HTTP 头：

### 健康检查
```bash
curl http://localhost:3000/health
```

### 创建密钥
```bash
curl -X POST http://localhost:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "My test key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'
```

### 列出密钥
```bash
curl -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}'
```

### 查询密钥详情
```bash
curl -X POST http://localhost:3000/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d '{"KeyId": "your-key-id"}'
```

### 推导地址
```bash
curl -X POST http://localhost:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d '{
    "KeyId": "your-key-id",
    "AddressIndex": 0
  }'
```

### 签名
```bash
curl -X POST http://localhost:3000/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d '{
    "KeyId": "your-key-id",
    "Message": "SGVsbG8gV29ybGQ=",
    "MessageType": "RAW",
    "SigningAlgorithm": "ECDSA_SHA_256"
  }'
```

### 获取公钥
```bash
curl -X POST http://localhost:3000/GetPublicKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.GetPublicKey" \
  -d '{"KeyId": "your-key-id"}'
```

### 删除密钥
```bash
curl -X POST http://localhost:3000/DeleteKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ScheduleKeyDeletion" \
  -d '{
    "KeyId": "your-key-id",
    "PendingWindowInDays": 7
  }'
```

## 🔍 故障排查

### 1. API 返回 Connection refused
**原因**: QEMU 或 API Server 未运行
**解决**: 运行 `./scripts/kms-auto-start.sh`

### 2. Cloudflared 错误
**检查日志**: `tail -f /tmp/cloudflared.log`
**重启**:
```bash
pkill cloudflared
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &
```

### 3. 端口被占用
脚本会自动清理端口，如果仍有问题：
```bash
docker exec teaclave_dev_env pkill -f qemu-system-aarch64
docker exec teaclave_dev_env pkill -f socat
```

### 4. 查看 QEMU 日志
```bash
docker exec teaclave_dev_env cat /tmp/qemu.log
```

## 📚 相关文档

- **完整变更日志**: `docs/Changes.md`
- **KMS 详细说明**: `kms/README.md`
- **部署指南**: `docs/Deploy.md`

## ⚙️ 开发工作流

### 开发流程 A: 使用 auto-start（快速迭代）

```bash
# 1. 修改代码
vim kms/host/src/api_server.rs

# 2. 部署
./scripts/kms-deploy.sh

# 3. 重启服务（快速）
docker exec teaclave_dev_env pkill -f qemu-system-aarch64
./scripts/kms-auto-start.sh

# 4. 测试
curl http://localhost:3000/health

# 5. 查看日志（如需要）
./scripts/kms-monitor.sh
```

### 开发流程 B: 使用手动终端（实时监控）

```bash
# 1. 修改代码
vim kms/host/src/api_server.rs

# 2. 部署
./scripts/kms-deploy.sh

# 3. 清理并准备手动启动
./scripts/kms-cleanup.sh

# 4. 启动三个终端（可实时看日志）
# Terminal 1: ./scripts/terminal3-secure-log.sh
# Terminal 2: ./scripts/terminal2-guest-vm.sh
# Terminal 3: ./scripts/terminal1-qemu.sh

# 5. 测试（在第四个终端）
curl http://localhost:3000/health
```

### 关键命令

- **清理所有进程**: `./scripts/kms-cleanup.sh`
- **重启 API**: `./scripts/kms-restart-api.sh`
- **查看日志**: `./scripts/kms-monitor.sh`
- **查看状态**: `./scripts/kms-startup-guide.sh`

## 🎯 系统架构

### 完整网络通信层次图

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Internet 用户                               │
│                    (https://kms.aastar.io)                          │
└────────────────────────────┬────────────────────────────────────────┘
                             │ HTTPS
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    Cloudflare Edge Network                          │
│                   (全球 CDN + DDoS 防护)                             │
└────────────────────────────┬────────────────────────────────────────┘
                             │ Cloudflare Tunnel (加密隧道)
                             ↓
┌─────────────────────────────────────────────────────────────────────┐
│                      Mac 宿主机 (macOS)                              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  cloudflared 进程                                             │  │
│  │  - 监听 Cloudflare 隧道                                       │  │
│  │  - 转发到: 127.0.0.1:3000                                    │  │
│  └────────────────────┬─────────────────────────────────────────┘  │
│                       │ TCP (localhost)                             │
│                       ↓                                             │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Mac localhost:3000                                           │  │
│  │  (本地回环地址)                                               │  │
│  └────────────────────┬─────────────────────────────────────────┘  │
└───────────────────────┼─────────────────────────────────────────────┘
                        │ Docker 端口映射 (-p 3000:3000)
                        ↓
┌─────────────────────────────────────────────────────────────────────┐
│              Docker 容器 (teaclave_dev_env)                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Docker 内部网络                                              │  │
│  │  - 容器 IP: 172.17.0.x                                       │  │
│  │  - 监听: 0.0.0.0:3000 (所有接口)                             │  │
│  └────────────────────┬─────────────────────────────────────────┘  │
│                       │ QEMU 端口转发                               │
│                       │ (hostfwd=tcp:0.0.0.0:3000-:3000)           │
│                       ↓                                             │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  QEMU 虚拟机 (qemu-system-aarch64)                            │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │  Guest OS (Ubuntu 24.04 ARM64)                         │  │  │
│  │  │  ┌──────────────────────────────────────────────────┐  │  │  │
│  │  │  │  KMS API Server (Rust + Warp)                    │  │  │  │
│  │  │  │  - 监听: 0.0.0.0:3000                            │  │  │  │
│  │  │  │  - 进程: kms-api-server                          │  │  │  │
│  │  │  │  - 日志: /root/shared/kms-api.log                │  │  │  │
│  │  │  └────────────────┬───────────────────────────────┘  │  │  │
│  │  │                   │ TEE Client API (libteec.so)       │  │  │
│  │  │                   ↓                                   │  │  │
│  │  │  ┌──────────────────────────────────────────────────┐  │  │  │
│  │  │  │  OP-TEE Client (CA - Client Application)         │  │  │  │
│  │  │  │  - 正常世界 (Normal World)                       │  │  │  │
│  │  │  │  - 调用 TEEC_* API                               │  │  │  │
│  │  │  └────────────────┬───────────────────────────────┘  │  │  │
│  │  │                   │ OP-TEE 内核驱动 (/dev/tee0)      │  │  │  │
│  │  │                   ↓                                   │  │  │
│  │  │  ┌──────────────────────────────────────────────────┐  │  │  │
│  │  │  │  ARM TrustZone 安全监视器 (SMC 调用)             │  │  │  │
│  │  │  └────────────────┬───────────────────────────────┘  │  │  │
│  │  │                   │ 世界切换 (World Switch)           │  │  │  │
│  │  │                   ↓                                   │  │  │
│  │  │  ┌──────────────────────────────────────────────────┐  │  │  │
│  │  │  │  OP-TEE OS (Secure World)                        │  │  │  │
│  │  │  │  ┌────────────────────────────────────────────┐  │  │  │  │
│  │  │  │  │  KMS TA (Trusted Application)              │  │  │  │  │
│  │  │  │  │  - UUID: 4319f351-0b24-4097-b659-...       │  │  │  │  │
│  │  │  │  │  - 私钥生成和签名                          │  │  │  │  │
│  │  │  │  │  - BIP39 助记词管理                        │  │  │  │  │
│  │  │  │  └──────────────┬─────────────────────────────┘  │  │  │  │
│  │  │  │                 │ OP-TEE 内部 API                 │  │  │  │
│  │  │  │                 ↓                                 │  │  │  │
│  │  │  │  ┌────────────────────────────────────────────┐  │  │  │  │
│  │  │  │  │  Secure Storage (TEE 加密存储)             │  │  │  │  │
│  │  │  │  │  - 私钥存储 (硬件加密)                     │  │  │  │  │
│  │  │  │  │  - 助记词存储                              │  │  │  │  │
│  │  │  │  │  - 密钥元数据                              │  │  │  │  │
│  │  │  │  └────────────────────────────────────────────┘  │  │  │  │
│  │  │  └──────────────────────────────────────────────────┘  │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  辅助通道:                                                              │
│  - Serial Port 54320 (Guest VM Shell) → socat → Terminal 2             │
│  - Serial Port 54321 (Secure World Log) → socat → Terminal 3           │
│  - 9p virtio 共享目录: /opt/teaclave/shared ↔ /root/shared (QEMU)      │
└─────────────────────────────────────────────────────────────────────────┘
```

### 端口映射详情

| 层次 | 地址/端口 | 说明 |
|------|----------|------|
| 公网访问 | `https://kms.aastar.io` | Cloudflare 代理 |
| Cloudflare → Mac | `127.0.0.1:3000` | cloudflared 转发目标 |
| Mac → Docker | `localhost:3000 → 172.17.0.x:3000` | Docker 端口映射 `-p 3000:3000` |
| Docker → QEMU | `0.0.0.0:3000 → QEMU Guest:3000` | QEMU hostfwd 转发 |
| QEMU Guest | `0.0.0.0:3000` | kms-api-server 监听 |
| 调试端口 54320 | Docker → QEMU Serial0 | Guest VM Shell 输出 |
| 调试端口 54321 | Docker → QEMU Serial1 | Secure World TA 日志 |
| TLS 端口 54433 | Docker → QEMU Guest:4433 | OP-TEE 示例 TLS 服务 |

### 数据流向示例

**用户请求创建密钥的完整流程**:

```
1. 用户浏览器
   POST https://kms.aastar.io/CreateKey
   ↓
2. Cloudflare Edge (DDoS 防护 + SSL 终止)
   ↓
3. Cloudflare Tunnel → Mac cloudflared
   ↓
4. Mac localhost:3000
   ↓
5. Docker 容器端口映射
   ↓
6. QEMU hostfwd 端口转发
   ↓
7. QEMU Guest - KMS API Server (Warp 路由)
   ↓
8. OP-TEE Client API 调用
   TEEC_OpenSession() → TA UUID
   ↓
9. ARM TrustZone 世界切换 (SMC)
   Normal World → Secure World
   ↓
10. OP-TEE OS 调度 KMS TA
    ↓
11. KMS TA 执行:
    - 生成 BIP39 助记词
    - 派生私钥 (secp256k1)
    - 存储到 Secure Storage
    ↓
12. 返回路径相反
    KeyId + Metadata → API Server → 用户
```

### 安全边界

```
┌──────────────────────────────────────────────┐
│  不受信任区域 (Untrusted)                     │
│  - 公网请求                                   │
│  - Cloudflare 边缘节点                        │
│  - Mac 主机                                   │
│  - Docker 容器                                │
│  - QEMU Guest OS                              │
│  - KMS API Server (Normal World)              │
└────────────────┬─────────────────────────────┘
                 │ ARM TrustZone 硬件隔离
                 ↓
┌──────────────────────────────────────────────┐
│  受信任区域 (Trusted - Secure World)          │
│  - OP-TEE OS                                  │
│  - KMS TA (私钥操作)                          │
│  - Secure Storage (硬件加密存储)              │
│  - 私钥永不离开此区域                         │
└──────────────────────────────────────────────┘
```

### 关键组件说明

1. **Cloudflare Tunnel (cloudflared)**
   - 建立加密隧道到 Cloudflare Edge
   - 无需开放防火墙端口
   - 自动 SSL/TLS 终止
   - 配置文件: `~/.cloudflared/config.yml`

2. **Docker 容器 (teaclave_dev_env)**
   - 基于 Ubuntu 20.04
   - 包含 OP-TEE 开发环境
   - 挂载宿主机代码目录
   - 运行 QEMU ARM64 虚拟机

3. **QEMU 虚拟机**
   - 模拟 ARMv8 + TrustZone
   - 运行 OP-TEE OS
   - 9p virtio 共享目录与 Docker 交换文件
   - Serial 端口输出日志

4. **KMS API Server (Rust)**
   - Warp 异步 Web 框架
   - AWS KMS 兼容 API
   - 通过 libteec 调用 TA
   - 日志: `/root/shared/kms-api.log`

5. **OP-TEE TA (Trusted Application)**
   - 运行在 Secure World
   - Rust 编写 (`kms/ta/`)
   - 私钥生成、签名、密钥派生
   - 存储到 Secure Storage

6. **Secure Storage**
   - 基于 RPMB 或文件系统加密
   - 硬件密钥保护
   - 只能在 Secure World 访问
