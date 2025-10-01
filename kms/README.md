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

### 1. 启动开发环境
```bash
./scripts/kms-dev-env.sh start  # 自动检查并初始化STD依赖
```

### 2. 开发代码
在 `kms/` 目录下编辑代码

### 3. 构建和部署
```bash
./scripts/kms-deploy.sh        # 一键构建和部署
./scripts/kms-deploy.sh clean  # 完整构建
```

### 4. 启动监控系统（查看调用链）

**方案 A: 真实日志监控（推荐，显示 CA/TA 真实日志）**
```bash
# 一次性配置（在单独终端）
socat - TCP:localhost:54320
# 在 QEMU 中执行:
killall kms-api-server
cd /root/shared
./kms-api-server > /root/shared/kms-api.log 2>&1 &
# 退出 (Ctrl+C)

# 启动监控
./scripts/monitor-all-tmux-direct.sh
```

**方案 B: 稳定版监控（无需配置，从 Cloudflared 推断）**
```bash
./scripts/start-cloudflared-debug.sh      # 启用 debug 日志
./scripts/monitor-all-tmux-v2.sh          # 启动监控
```

详细说明：
- [监控系统使用指南](../docs/Monitoring-Guide.md)
- [真实日志配置](../docs/Enable-Real-Logging.md)
- [故障排查](../docs/Monitoring-Troubleshooting.md)

### 5. 测试 API

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

### 开发指南
- [完整开发指南](../docs/KMS-Development-Guide.md) - Docker、QEMU、部署完整流程
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