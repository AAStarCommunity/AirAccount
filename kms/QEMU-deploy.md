# QEMU KMS Deploy Guide

> Last updated: 2026-03-03

## Prerequisites

- Docker container `teaclave_dev_env` running
- Three terminal windows available
- Cloudflared tunnel configured (`kms.aastar.io → localhost:3000`)

## 1. Start QEMU (3 Terminals)

```bash
# Terminal 2 (先启动 — Guest VM Shell)
./scripts/terminal2-guest-vm.sh

# Terminal 3 (先启动 — Secure World Log)
./scripts/terminal3-secure-log.sh

# Terminal 1 (最后启动 — QEMU)
./scripts/terminal1-qemu.sh
```

Terminal 2 和 3 会显示 `Listening on TCP port 54320/54321`，确认后再启动 Terminal 1。

## 2. QEMU Guest Boot & Deploy

等待 Terminal 2 出现登录提示后：

```
Welcome to Buildroot, type root or test to login
buildroot login: root
```

执行以下命令：

```bash
# 挂载共享目录
mkdir -p shared && mount -t 9p -o trans=virtio host shared && cd shared

# 挂载 TA 和 plugin 到系统路径
mount --bind ta/ /lib/optee_armtz
mount --bind plugin/ /usr/lib/tee-supplicant/plugins/

# 拷贝最新 TA（如果 shared/ 下有新编译的 .ta）
cp /root/shared/4319f351-0b24-4097-b659-80ee4f824cdd.ta /lib/optee_armtz/

# 启动 KMS API Server
# 注意: DB 必须放在本地磁盘，9p 不支持 SQLite WAL 的 xShmMap
mkdir -p /data/kms
KMS_DB_PATH=/data/kms/kms.db ./kms-api-server
```

## 3. 启动成功输出

```
📦 SQLite DB opened: /data/kms/kms.db
💾 SQLite DB: /data/kms/kms.db
⏱️  Rate limiter: 100/min per API key
🔗 TeeHandle: worker thread spawned, session will be opened on first command
🛡️  Circuit breaker: threshold=3, recovery=30s
⚠️  API Key authentication: DISABLED (run `api-key generate` to enable)
🚀 KMS API Server starting on http://0.0.0.0:3000
📚 Supported APIs:
   GET  /              - Welcome page
   GET  /test          - Interactive test UI
   POST /CreateKey     - Create new TEE wallet
   POST /DescribeKey   - Query wallet metadata
   POST /ListKeys      - List all wallets
   POST /DeriveAddress - Derive Ethereum address
   POST /Sign          - Sign Ethereum transaction or message
   POST /SignHash      - Sign 32-byte hash directly
   POST /GetPublicKey  - Get public key
   POST /DeleteKey     - Delete wallet (requires PassKey)
   POST /ChangePasskey         - Change PassKey public key
   POST /BeginRegistration     - WebAuthn registration (step 1)
   POST /CompleteRegistration  - WebAuthn registration (step 2)
   POST /BeginAuthentication   - WebAuthn authentication challenge
   GET  /KeyStatus             - Key derivation status (polling)
   GET  /QueueStatus           - TEE queue depth
   GET  /health                - Health check
🔐 TA Mode: ✅ Real TA (OP-TEE Secure World required)
🆔 TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd
🌐 Public URL: https://kms.aastar.io
🔗 TEE worker: session opened
```

## 4. 验证

```bash
# 本地
curl http://localhost:3000/health
curl http://localhost:3000/version

# 公网 (Cloudflare Tunnel)
curl https://kms.aastar.io/health
curl https://kms.aastar.io/version   # 应返回 {"version":"0.15.22",...}

# Test UI
open https://kms.aastar.io/test
```

## 5. 重启 KMS（不重启 QEMU）

在 QEMU guest shell 中：

```bash
# 停掉旧进程
pkill kms-api-server

# 重新启动
KMS_DB_PATH=/data/kms/kms.db /root/shared/kms-api-server
```

## 6. 重启 QEMU

在 QEMU guest shell 中：

```bash
pkill kms-api-server
poweroff
```

等 QEMU 退出后，重复步骤 1-2。

## Network Path

```
Browser → kms.aastar.io
       → Cloudflare Tunnel (cloudflared on Mac)
       → localhost:3000 (Mac)
       → Docker port map 3000:3000 (teaclave_dev_env)
       → QEMU hostfwd tcp::3000-:3000
       → QEMU guest 0.0.0.0:3000 (kms-api-server)
```

## QEMU Build (in teaclave_dev_env container)

```bash
docker exec -it teaclave_dev_env bash -l

# TA (aarch64-unknown-optee via xargo)
cd /root/teaclave_sdk_src/projects/web3/kms/ta
CC=aarch64-linux-gnu-gcc xargo build --target aarch64-unknown-optee --release

# Sign TA
aarch64-linux-gnu-objcopy --strip-unneeded \
  target/aarch64-unknown-optee/release/ta \
  target/aarch64-unknown-optee/release/stripped_ta
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid 4319f351-0b24-4097-b659-80ee4f824cdd --ta-version 0 \
  --in target/aarch64-unknown-optee/release/stripped_ta \
  --out target/aarch64-unknown-optee/release/4319f351-0b24-4097-b659-80ee4f824cdd.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem

# CA (aarch64-unknown-linux-gnu)
cd /root/teaclave_sdk_src/projects/web3/kms/host
cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server

# Deploy to shared (QEMU guest mounts this via 9p)
cp ../ta/target/aarch64-unknown-optee/release/4319f351-0b24-4097-b659-80ee4f824cdd.ta /opt/teaclave/shared/
cp target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/
```

## Gotchas

- **SQLite WAL on 9p**: `xShmMap` 不支持，DB 必须放本地路径（`/data/kms/`），不能放 `/root/shared/`
- **QEMU 端口转发**: 启动脚本需包含 `hostfwd=tcp::3000-:3000`，否则容器外访问不到 guest 的 3000 端口
- **p256-m.c on aarch64**: ARM32 inline ASM 被 `!defined(__aarch64__)` 守卫跳过，走纯 C fallback
- **build.rs `-marm` flag**: 仅在 ARM32 target 时添加，aarch64 跳过
- **TA `.cargo/config.toml`**: 需要 `linker = "aarch64-linux-gnu-ld"`
- **CA `.cargo/config.toml`**: 需要 `linker = "aarch64-linux-gnu-gcc"`
- **Source path**: 容器内源码在 `projects/web3/kms/`（不是 `kms/`），相对路径 `../../../../` 才能找到 SDK 依赖
