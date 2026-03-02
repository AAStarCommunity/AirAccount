# KMS (Key Management Service) - OP-TEE 企业级密钥管理

基于 OP-TEE TrustZone 的 AWS KMS 兼容密钥管理服务，提供安全的钱包管理和交易签名。

## 🌐 在线演示

- **Web 测试界面**: https://kms.aastar.io/test
- **健康检查**: https://kms.aastar.io/health
- **API 根路径**: https://kms.aastar.io/

**实时监控系统**: 完整的四层调用链可视化（Web UI → Cloudflared → CA → TA → QEMU）

## 📁 项目架构

```
AirAccount/
├── kms/                                    # 📝 开发源码（在这里开发）
│   ├── host/                              # CA (Client Application)
│   │   ├── src/
│   │   │   ├── main.rs                    # CLI工具
│   │   │   ├── api_server.rs              # HTTP API服务器（AWS KMS兼容）
│   │   │   ├── ta_client.rs               # TA通信客户端
│   │   │   ├── cli.rs                     # 命令行接口
│   │   │   ├── tests.rs                   # 测试模块
│   │   │   └── lib.rs                     # 共享库
│   │   └── Cargo.toml                     # 双二进制配置
│   ├── ta/                                # TA (Trusted Application)
│   ├── proto/                             # 协议定义（Host-TA共享）
│   └── uuid.txt                           # TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd
│
├── third_party/teaclave-trustzone-sdk/    # 🔧 SDK (git submodule)
│   ├── projects/web3/kms/                 # 构建目标（脚本自动同步）
│   ├── rust/                              # STD模式依赖（自动初始化）
│   └── optee-teec/                        # OP-TEE客户端库
│
├── scripts/
│   ├── kms-deploy.sh                      # 🚀 一键部署
│   ├── kms-dev-env.sh                     # 开发环境管理
│   ├── monitor-all-tmux-direct.sh         # 真实日志监控（推荐）
│   ├── monitor-all-tmux-v2.sh             # 稳定版监控（无需配置）
│   └── start-cloudflared-debug.sh         # 启用 debug 日志
│
└── docs/
    ├── KMS-Development-Guide.md           # 完整开发指南
    ├── Monitoring-Guide.md                # 监控系统使用指南
    ├── Monitoring-Setup.md                # 监控配置说明
    ├── Monitoring-Troubleshooting.md      # 故障排查指南
    ├── Enable-Real-Logging.md             # 真实日志配置
    └── kms-test-page.html                 # Web UI 测试页面
```

## 🚀 快速开始

### 首次启动或 Mac 重启后

```bash
# 1. 启动 Docker 容器
docker start teaclave_dev_env

# 2. 启动 Cloudflare 隧道（仅需运行一次）
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &

# 3. 一键启动 KMS 服务（推荐）
./scripts/kms-auto-start.sh
```

### 开发流程

**方式 A: 快速迭代（推荐日常开发）**
```bash
# 1. 修改代码
vim kms/host/src/api_server.rs

# 2. 部署新代码
./scripts/kms-deploy.sh

# 3. 快速重启
docker exec teaclave_dev_env pkill -f qemu-system-aarch64
./scripts/kms-auto-start.sh

# 4. 测试
curl http://localhost:3000/health

# 5. 查看日志（如需要）
./scripts/kms-monitor.sh
```

**方式 B: 调试模式（实时查看日志）**
```bash
# 1. 修改代码
vim kms/host/src/api_server.rs

# 2. 部署新代码
./scripts/kms-deploy.sh

# 3. 清理并准备手动启动
./scripts/kms-cleanup.sh

# 4. 启动三个终端
# Terminal 1: ./scripts/terminal3-secure-log.sh  (TA 日志)
# Terminal 2: ./scripts/terminal2-guest-vm.sh    (CA 日志)
# Terminal 3: ./scripts/terminal1-qemu.sh        (QEMU)

# 5. 测试（在第四个终端）
curl http://localhost:3000/health
```

### 常用命令

```bash
# 查看系统状态
./scripts/kms-startup-guide.sh

# 清理所有进程
./scripts/kms-cleanup.sh

# 监控日志
./scripts/kms-monitor.sh

# 重启 API Server
./scripts/kms-restart-api.sh
```

### 测试 API

**浏览器测试**:
```bash
open https://kms.aastar.io/test
```

**命令行测试**:
```bash
# 创建密钥
curl -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{
    "Description": "test-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'

# 健康检查
curl https://kms.aastar.io/health | jq .
```

## 🔄 工作流程

```
📝 开发 (kms/)
  ↓ rsync同步
🔧 构建 (SDK/projects/web3/kms/)
  ↓ Docker编译
📦 部署 (/opt/teaclave/shared/)
  ↓ QEMU运行
🖥️ 测试 (Guest VM)
  ↓ Cloudflare Tunnel
🌐 公网访问 (https://kms.aastar.io)
```

### 关键理解
1. **Docker挂载**: SDK目录挂载到容器，本地修改实时可见
2. **STD依赖**: rust/libc在`.gitignore`中，需要自动初始化
3. **自动同步**: 脚本rsync代码，Docker挂载实时同步
4. **公网访问**: Cloudflare Tunnel 提供 24/7 HTTPS 访问

## 📊 API 功能

### 支持的 AWS KMS 兼容 API

| API 端点 | 功能 | 状态 |
|---------|------|------|
| `POST /CreateKey` | 创建新密钥（钱包） | ✅ |
| `POST /DescribeKey` | 查询密钥元数据 | ✅ |
| `POST /ListKeys` | 列出所有密钥 | ✅ |
| `POST /GetPublicKey` | 获取公钥 | ✅ |
| `POST /DeriveAddress` | 派生以太坊地址 | ✅ |
| `POST /Sign` | 签名消息 | ✅ |
| `POST /SignTransaction` | 签名交易 | ✅ |
| `POST /DeleteKey` | 删除密钥 | ✅ |

### TA 支持的命令

- `CMD_CREATE_WALLET (0x1001)`: 创建新钱包
- `CMD_IMPORT_WALLET (0x1002)`: 导入现有钱包
- `CMD_GET_WALLET_INFO (0x1003)`: 获取钱包信息
- `CMD_DELETE_WALLET (0x1004)`: 删除钱包
- `CMD_LIST_WALLETS (0x1005)`: 列出所有钱包
- `CMD_DERIVE_KEY (0x2001)`: 派生子密钥
- `CMD_GET_PUBLIC_KEY (0x2002)`: 获取公钥
- `CMD_SIGN_MESSAGE (0x3001)`: 签名消息
- `CMD_SIGN_TRANSACTION (0x3002)`: 签名交易

## 🧪 测试示例

### CLI 测试（QEMU Guest VM 中）

```bash
# 1. 创建钱包
./kms create-wallet
# 输出: Wallet ID: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx

# 2. 派生地址
./kms derive-address <wallet-id> "m/44'/60'/0'/0/0"
# 输出: Address: 0x...

# 3. 签名交易
./kms sign-transaction <wallet-id> "m/44'/60'/0'/0/0" \
  --chain-id 1 \
  --nonce 0 \
  --to 0x742d35Cc6634C0532925a3b844Bc454e4438f44e \
  --value 1000000000000000000 \
  --gas-price 20000000000 \
  --gas 21000

# 4. 删除钱包
./kms remove-wallet <wallet-id>
```

### API 测试（公网访问）

```bash
# 1. 创建密钥
curl -X POST https://kms.aastar.io/CreateKey \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.CreateKey' \
  -d '{
    "Description": "Test Key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'

# 2. 获取公钥
curl -X POST https://kms.aastar.io/GetPublicKey \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.GetPublicKey' \
  -d '{
    "KeyId": "<key-id-from-step-1>"
  }'

# 3. 派生地址
curl -X POST https://kms.aastar.io/DeriveAddress \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.DeriveAddress' \
  -d '{
    "KeyId": "<key-id>",
    "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0"
  }'

# 4. 签名交易
curl -X POST https://kms.aastar.io/Sign \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.Sign' \
  -d '{
    "KeyId": "<key-id>",
    "Message": "SGVsbG8gV29ybGQ=",
    "SigningAlgorithm": "ECDSA_SHA_256"
  }'

# 5. 列出所有密钥
curl -X POST https://kms.aastar.io/ListKeys \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.ListKeys' \
  -d '{}'

# 6. 删除密钥
curl -X POST https://kms.aastar.io/DeleteKey \
  -H 'Content-Type: application/json' \
  -H 'X-Amz-Target: TrentService.DeleteKey' \
  -d '{
    "KeyId": "<key-id>"
  }'
```

## 📺 监控系统

### 四层监控架构

```
┌─────────────────────────────────────────────────────┐
│ Web UI: https://kms.aastar.io/test                 │
│ 用户在浏览器中测试 API                                │
└──────────────┬──────────────────────────────────────┘
               │ HTTPS Request
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 4: Cloudflared Tunnel                      │
│ - 监控公网 HTTPS 请求进入                             │
│ - 隧道连接状态                                        │
│ - 请求/响应日志                                       │
└──────────────┬──────────────────────────────────────┘
               │ HTTP (localhost:3000)
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 2: KMS API Server (CA - Normal World)      │
│ - HTTP 请求解析                                      │
│ - API 端点路由                                       │
│ - CA → TA 调用                                       │
│ - 响应返回                                           │
└──────────────┬──────────────────────────────────────┘
               │ TEEC API
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 3: OP-TEE TA (Secure World)                │
│ - TA 会话管理                                        │
│ - 密钥管理操作                                       │
│ - 加密签名执行                                       │
│ - Secure World 日志                                 │
└──────────────┬──────────────────────────────────────┘
               │ Hardware TEE
               ↓
┌─────────────────────────────────────────────────────┐
│ Terminal 1: QEMU Guest VM                           │
│ - 系统启动日志                                       │
│ - 内核消息                                           │
│ - QEMU 运行状态                                      │
└─────────────────────────────────────────────────────┘
```

### 监控脚本对比

| 脚本 | CA 日志 | TA 日志 | 稳定性 | 需要配置 |
|------|---------|---------|--------|----------|
| `monitor-all-tmux.sh` | socat（不稳定） | socat（不稳定） | ❌ | 否 |
| `monitor-all-tmux-v2.sh` | Cloudflared 推断 | 命令参考 | ✅ | 否 |
| `monitor-all-tmux-direct.sh` | 共享文件（真实） | dmesg（真实） | ✅ | 是（一次性） |

**推荐**:
- 日常开发: `monitor-all-tmux-v2.sh`（稳定，无需配置）
- 深度调试: `monitor-all-tmux-direct.sh`（真实日志）

## 🐛 常见问题

### Q: Terminal 2/3 监控没有日志显示？

**A**: 这是正常的。有三种解决方案：

1. **使用 V2 监控**（推荐，无需配置）:
   ```bash
   ./scripts/start-cloudflared-debug.sh
   ./scripts/monitor-all-tmux-v2.sh
   ```

2. **使用真实日志监控**（需要一次性配置）:
   - 参考 [Enable-Real-Logging.md](../docs/Enable-Real-Logging.md)

3. **手动查看日志**:
   ```bash
   socat - TCP:localhost:54320
   # 在 QEMU 中: tail -f /tmp/kms.log
   ```

详细说明: [Monitoring-Troubleshooting.md](../docs/Monitoring-Troubleshooting.md)

### Q: 为何 setup_std_dependencies.sh 之前执行过，这次又需要？

**A**: rust/目录在`.gitignore`中，git reset时被删除。脚本现已自动检测并初始化。

### Q: 为何在 kms/ 开发，但 Docker 内编译的是 SDK 内的代码？

**A**:
1. rsync同步: `kms/` → `third_party/.../projects/web3/kms/`
2. Docker挂载: `third_party/...` → `/root/teaclave_sdk_src/`（实时同步）
3. 所以修改 kms/ 后，SDK内立即可见，Docker内也立即可见

### Q: CA 如何加载？

**A**: CA 是普通 Linux 程序，复制到共享目录即可运行。TA 需复制到 `/lib/optee_armtz/`

### Q: 如何启用 TA 详细日志？

**A**: 在 TA 代码中添加 `trace_println!()`:

```rust
use optee_utee::trace_println;

fn create_wallet(...) -> Result<...> {
    trace_println!("[TA] Creating new wallet");
    // ...
    trace_println!("[TA] Wallet created: {}", uuid);
    Ok(...)
}
```

然后重新编译: `./scripts/kms-deploy.sh`

## 📚 文档索引

### API 文档
- [KMS API Reference (DK2)](../docs/KMS-API-Reference.md) - 完整 REST API 参考、请求/响应结构、使用流程

### 开发指南
- [完整开发指南](../docs/KMS-Development-Guide.md) - Docker、QEMU、部署完整流程
- [DK2 开发部署](DK2-dev.md) - STM32MP157F-DK2 硬件开发指南
- [Changes.md](../docs/Changes.md) - 项目变更历史和技术要点

### 监控系统
- [监控系统使用指南](../docs/Monitoring-Guide.md) - 完整的监控架构和使用方法
- [监控配置说明](../docs/Monitoring-Setup.md) - Cloudflared debug 日志配置
- [故障排查指南](../docs/Monitoring-Troubleshooting.md) - 常见问题和解决方案
- [真实日志配置](../docs/Enable-Real-Logging.md) - 如何查看真实的 CA/TA 日志

### 部署和运维
- [Deploy.md](../docs/Deploy.md) - 部署指南（英文）
- [Deploy_zh.md](../docs/Deploy_zh.md) - 部署指南（中文）

### 架构设计
- [Plan.md](../docs/Plan.md) - 技术方案和架构设计
- [Solution.md](../docs/Solution.md) - 解决方案概述

## 🏗️ 架构流程图

<details>
<summary>点击展开完整流程图</summary>

### 代码同步与编译流程

```
┌─────────────────────────────────────────────────────────────┐
│                    Host macOS 文件系统                       │
│                                                              │
│  /Volumes/.../AirAccount/                                   │
│  ├── kms/host/src/main.rs          📝 你在这里开发          │
│  │         ↓                                                │
│  │    [rsync -av --delete]          🔄 脚本单向同步         │
│  │         ↓                                                │
│  └── third_party/teaclave-trustzone-sdk/                   │
│      └── projects/web3/kms/                                 │
│          └── host/src/main.rs       📄 同步后的文件         │
│                    ↓                                         │
│               [Docker -v 挂载]      🔗 实时双向同步         │
│                    ↓                                         │
└────────────────────┼────────────────────────────────────────┘
                     ↓
┌────────────────────┼────────────────────────────────────────┐
│         Docker 容器内文件系统 (linux/amd64)                  │
│                    ↓                                         │
│  /root/teaclave_sdk_src/                                    │
│  ├── projects/web3/kms/                                     │
│  │   └── host/src/main.rs          🐳 编译这个文件          │
│  │            ↓                                              │
│  │       [make]                     🔨 交叉编译             │
│  │            ↓                                              │
│  │   └── target/aarch64-unknown-linux-gnu/release/          │
│  │       ├── kms                    📦 CLI工具 (707K)       │
│  │       └── kms-api-server         📦 API服务 (2.8M)       │
│  │            ↓                                              │
│  │   └── ta/target/aarch64-unknown-optee/release/           │
│  │       └── 4319f351-*.ta          📦 TA应用 (595K)        │
│  │            ↓                                              │
│  │       [cp到共享目录]              📤 部署                 │
│  │            ↓                                              │
│  └── /opt/teaclave/shared/                                  │
│      ├── kms                        ✅ 部署成功              │
│      ├── kms-api-server             ✅ 部署成功              │
│      └── *.ta                       ✅ 部署成功              │
│                    ↓                                         │
│          [QEMU启动]                 🖥️ ARM64虚拟机          │
│                    ↓                                         │
└────────────────────┼────────────────────────────────────────┘
                     ↓
┌────────────────────┼────────────────────────────────────────┐
│         QEMU Guest VM (aarch64 Linux + OP-TEE)              │
│                    ↓                                         │
│  # mount -t 9p -o trans=virtio host shared                  │
│  # cp shared/*.ta /lib/optee_armtz/                         │
│  # cd shared && ./kms-api-server > kms-api.log 2>&1 &       │
│                                                              │
│  ┌──────────────────┬──────────────────────────┐            │
│  │  Normal World    │    Secure World          │            │
│  │  (Linux)         │    (OP-TEE)              │            │
│  ├──────────────────┼──────────────────────────┤            │
│  │  # ./kms         │  TA: 4319f351-*.ta       │            │
│  │    create-wallet │    └─ wallet.rs          │            │
│  │       ↓          │         ↓                 │            │
│  │    [TEEC API] ───┼────→ [TA命令]            │            │
│  │       ↓          │         ↓                 │            │
│  │    [Response] ←──┼──── [返回数据]           │            │
│  │       ↓          │                           │            │
│  │  wallet_id       │                           │            │
│  │                  │                           │            │
│  │  # ./kms-api-server (HTTP :3000)             │            │
│  │       ↓          │                           │            │
│  │  Docker :3000 ← Port Forward                 │            │
│  └──────────────────┴──────────────────────────┘            │
│                    ↓                                         │
└────────────────────┼────────────────────────────────────────┘
                     ↓
┌────────────────────┼────────────────────────────────────────┐
│              Docker Container (cloudflared)                  │
│                    ↓                                         │
│  cloudflared tunnel --loglevel debug                        │
│       ↓                                                      │
│  localhost:3000 → Cloudflare Edge                           │
└────────────────────┼────────────────────────────────────────┘
                     ↓
               🌐 Internet
                     ↓
         https://kms.aastar.io
```

</details>

## 🎯 项目特性

- ✅ **AWS KMS 兼容**: 完全兼容 AWS KMS API 格式
- ✅ **TEE 安全**: 密钥在 OP-TEE Secure World 中生成和存储
- ✅ **HD 钱包**: 支持 BIP32/BIP39/BIP44 标准
- ✅ **多链支持**: 以太坊、Polygon、Arbitrum 等 EVM 链
- ✅ **公网访问**: Cloudflare Tunnel 提供 HTTPS 访问
- ✅ **Web UI**: 美观的交互式测试界面
- ✅ **完整监控**: 四层调用链实时可视化
- ✅ **开发友好**: 完整的文档和工具链

## 🔐 安全特性

- **密钥隔离**: 私钥永不离开 Secure World
- **硬件保护**: 利用 ARM TrustZone 硬件隔离
- **安全存储**: TA 使用 OP-TEE 的安全存储
- **安全通信**: HTTPS + Cloudflare 边缘加密

## 📈 性能指标

- **部署时间**: ~30 秒（QEMU 启动 + KMS 部署）
- **API 响应**: 200-300ms（端到端，含公网延迟）
- **并发支持**: ✅（Warp 异步框架）
- **可用性**: 24/7（Cloudflare Tunnel）

---

*最后更新: 2025-09-30 20:45 +07*

---
---

# KMS 系统启动和使用指南

*最后更新: 2025-10-01 17:41*

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
