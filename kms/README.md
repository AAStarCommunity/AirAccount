# KMS (Key Management Service) - OP-TEE STD模式开发

基于eth_wallet的企业级密钥管理服务，运行在OP-TEE TrustZone环境。

## 📁 项目架构

```
AirAccount/
├── kms/                                    # 📝 开发源码（在这里开发）
│   ├── host/                              # CA (Client Application)
│   │   ├── src/
│   │   │   ├── main.rs                    # CLI工具
│   │   │   ├── api_server.rs              # HTTP API服务器
│   │   │   ├── ta_client.rs               # TA通信客户端
│   │   │   ├── cli.rs                     # 命令行接口
│   │   │   ├── tests.rs                   # 测试模块
│   │   │   └── lib.rs                     # 共享库
│   │   └── Cargo.toml                     # 双二进制配置
│   ├── ta/                                # TA (Trusted Application)
│   ├── proto/                             # 协议定义（Host-TA共享）
│   └── uuid.txt                           # TA UUID
│
├── third_party/teaclave-trustzone-sdk/    # 🔧 SDK (git submodule)
│   ├── projects/web3/kms/                 # 构建目标（脚本自动同步）
│   ├── rust/                              # STD模式依赖（自动初始化）
│   └── optee-teec/                        # OP-TEE客户端库
│
└── scripts/
    ├── kms-deploy.sh                      # 🚀 一键部署
    └── kms-dev-env.sh                     # 开发环境管理
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

## 🔄 工作流程

```
📝 开发 (kms/)
  ↓ rsync同步
🔧 构建 (SDK/projects/web3/kms/)
  ↓ Docker编译
📦 部署 (/opt/teaclave/shared/)
  ↓ QEMU运行
🖥️ 测试 (Guest VM)
```

### 关键理解
1. **Docker挂载**: SDK目录挂载到容器，本地修改实时可见
2. **STD依赖**: rust/libc在`.gitignore`中，需要自动初始化
3. **自动同步**: 脚本rsync代码，Docker挂载实时同步

## 🐛 常见问题

**Q: 为何setup_std_dependencies.sh之前执行过，这次又需要？**
A: rust/目录在`.gitignore`中，git reset时被删除。脚本现已自动检测并初始化。

**Q: 为何在kms/开发，但Docker内编译的是SDK内的代码？**
A: 
1. rsync同步: `kms/` → `third_party/.../projects/web3/kms/`
2. Docker挂载: `third_party/...` → `/root/teaclave_sdk_src/`（实时同步）
3. 所以修改kms/后，SDK内立即可见，Docker内也立即可见

**Q: CA如何加载？**
A: CA是普通Linux程序，复制到共享目录即可运行。TA需复制到`/lib/optee_armtz/`

---

*最后更新: 2025-09-30 15:00*

## 📊 完整流程图

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
│  # cd shared                                                 │
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
│  │  # ./kms-api-server                          │            │
│  │    (HTTP :3000)  │                           │            │
│  │       ↓          │                           │            │
│  │  curl POST → API │                           │            │
│  └──────────────────┴──────────────────────────┘            │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 依赖初始化流程

```
启动开发环境: ./scripts/kms-dev-env.sh start
         ↓
检查rust/目录是否存在？
         ├─ 是 → ✅ 跳过初始化
         └─ 否 → 运行 setup_std_dependencies.sh
                  ↓
            从GitHub克隆:
            ├─ rust源码 (DemesneGH/rust:optee-xargo)
            ├─ libc库 (DemesneGH/libc:optee)
            └─ 保存到: third_party/.../rust/
                       ↓
                  ✅ 初始化完成
                  
注意: rust/目录在.gitignore中
      git reset/clean后会被删除
      脚本会自动重新初始化
```

### 文件挂载关系

```
Host macOS                          Docker Container
──────────────────────────────────────────────────────────
third_party/                   →    /root/teaclave_sdk_src/
├── teaclave-trustzone-sdk/    →    ├── (SDK根目录)
│   ├── projects/web3/kms/     →    │   ├── projects/web3/kms/
│   │   ├── host/              →    │   │   ├── host/
│   │   │   └── src/main.rs    →    │   │   │   └── src/main.rs  (实时同步)
│   │   └── ta/                →    │   │   └── ta/
│   ├── rust/                  →    │   ├── rust/
│   │   ├── rust/              →    │   │   ├── rust/
│   │   └── libc/              →    │   │   └── libc/
│   └── optee-teec/            →    │   └── optee-teec/
                                    
箭头表示: 双向实时同步（Docker -v 挂载）
修改左边文件 → 右边立即可见
修改右边文件 → 左边立即可见
```

## 🎯 测试示例

### CLI测试命令

```bash
# 在QEMU Guest VM中

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

### API测试命令 (curl)

```bash
# 确保API服务器运行
./kms-api-server &

# 1. 创建密钥
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "Description": "Test Key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'

# 2. 获取公钥
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.GetPublicKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "KeyId": "<key-id-from-step-1>"
  }'

# 3. 派生地址
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.DeriveAddress' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "KeyId": "<key-id>",
    "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0"
  }'

# 4. 签名交易
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "KeyId": "<key-id>",
    "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0",
    "Transaction": {
      "chainId": 1,
      "nonce": 0,
      "to": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
      "value": "0x0de0b6b3a7640000",
      "gasPrice": "0x04a817c800",
      "gas": 21000,
      "data": "0x"
    }
  }'

# 5. 列出所有密钥
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.ListKeys' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{}'

# 6. 删除密钥
curl -X POST http://localhost:3000/ \
  -H 'X-Amz-Target: TrentService.ScheduleKeyDeletion' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -d '{
    "KeyId": "<key-id>",
    "PendingWindowInDays": 7
  }'
```

---

*最后更新: 2025-09-30 15:15*
