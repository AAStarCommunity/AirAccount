# STM32MP157F-DK2 KMS Development Guide

> Last updated: 2026-03-03 17:59

## Endpoints & Test UI

| 端点 | 环境 | 版本 |
|------|------|------|
| `https://kms1.aastar.io` | **DK2 生产** (Cloudflare tunnel) | v0.16.0 |
| `https://kms.aastar.io` | **QEMU 模拟** (Cloudflare tunnel) | v0.16.0 |
| `http://192.168.7.2:3000` | DK2 本地直连 (USB Ethernet) | v0.16.0 |
| `http://localhost:3000` | QEMU 本地 (Docker port map) | v0.16.0 |

所有端点的 `/test` 路径提供交互式 Test UI：`https://kms1.aastar.io/test`

Test UI 功能：
- 左侧所有 API 按钮（GET/POST，标注 auth 要求）
- 右侧 JSON 编辑器 + 实时响应展示
- PassKey P-256 ECDSA assertion 浏览器内通过 SubtleCrypto 生成（私钥不离开页面）
- 自动从响应中提取 KeyId 填入后续请求

文件位置：`kms/test/kms-test-page.html` → 部署到目标机 `/root/shared/kms-test-page.html`

## Docker Containers

| 容器 | 镜像 | 用途 | 状态 |
|------|------|------|------|
| `stm32-builder` | ubuntu:22.04 | **DK2 交叉编译** (nightly-2024-05-15, xargo, v26 SDK) | 保留 |
| `teaclave_dev_env` | teaclave/...expand-memory | **QEMU OP-TEE 模拟器** (aarch64, port 3000) | 保留 |

> `stm32-builder-v20` (ubuntu:20.04) 和 `stm32-community` (jadiaconu) 已删除（未使用）。

### QEMU 启动脚本（三个 Terminal）

```bash
# Terminal 2: Guest VM Shell (先启动)
./scripts/terminal2-guest-vm.sh

# Terminal 3: Secure World Log (先启动)
./scripts/terminal3-secure-log.sh

# Terminal 1: QEMU (等 Terminal 2+3 就绪后启动)
./scripts/terminal1-qemu.sh
```

启动后在 QEMU guest 内：
```bash
mount -t 9p -o trans=virtio host /root/shared
cp /root/shared/*.ta /lib/optee_armtz/
cd /root/shared && ./kms-api-server
```

### QEMU 构建与部署

QEMU 使用 `teaclave_dev_env` 容器内的 Rust 工具链编译（aarch64 target），不用 `stm32-builder`：

```bash
# 进入 QEMU 容器
docker exec -it teaclave_dev_env bash

# 编译 TA + CA（容器内执行）
cd /root/AirAccount/kms/ta && cargo build --release
cd /root/AirAccount/kms/host && cargo build --release --bin kms-api-server

# 部署到共享目录（QEMU guest 通过 9p 挂载）
cp /root/AirAccount/kms/ta/target/release/*.ta /opt/teaclave/shared/
cp /root/AirAccount/kms/host/target/release/kms-api-server /opt/teaclave/shared/
```

## Network Setup

```
Mac Mini M4 (192.168.7.3) ──RJ45──> STM32MP157F-DK2 (192.168.7.2)
SSH: ssh root@192.168.7.2
```

Board: Cortex-A7 650MHz, ARMv7-A 32-bit, OP-TEE 3.16, OpenSTLinux kirkstone v22.06

## Docker Build Environment (DK2)

**统一使用 `stm32-builder` 容器**（基于 ubuntu:22.04，包含 nightly-2024-05-15 + xargo + v26 SDK）。

```bash
# 恢复容器
docker start stm32-builder

# 进入容器（如需交互）
docker exec -it stm32-builder bash
```

> 容器挂载：`-v /Volumes/UltraDisk/Dev2/aastar/AirAccount:/workspace`
> SDK 路径：`/opt/st/stm32mp1/6.6-v26.02.18/`
> Rust 工具链：`nightly-2024-05-15` (rustc 1.80.0) + `xargo 0.3.26`

### Container Setup (one-time)

```bash
# Fix optee-utee-build symlink to use our patched version
ln -sf /workspace/sdks/rust-sdk/optee-utee-build /optee-utee-build

# Install signing dependency
pip3 install cryptography

# TA GCC wrapper (with -nostartfiles for bare-metal TA)
cat > /tmp/arm-wrapper-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  -nostartfiles \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-wrapper-gcc

# CA GCC wrapper (without -nostartfiles, for Linux userspace)
cat > /tmp/arm-ca-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-ca-gcc
```

---

## One-Command Build (推荐)

完整构建 TA + CA + 签名，一条命令搞定：

```bash
docker exec stm32-builder bash -c '
source /root/.cargo/env

# ===== 环境变量 =====
export TA_DEV_KIT_DIR=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include/optee/export-user_ta
export RUST_TARGET_PATH=/workspace/sdks/rust-sdk
export CC_arm_unknown_optee=/tmp/arm-wrapper-gcc
export TARGET_TA=arm-unknown-optee
export OPTEE_CLIENT_EXPORT=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/tmp/arm-ca-gcc
export CC=/tmp/arm-ca-gcc
CROSS_STRIP=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-strip
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd

# ===== 1. Build TA =====
echo ">>> Building TA..."
cd /workspace/kms/ta
HOST_CC=gcc xargo build --target arm-unknown-optee --release \
  --config "target.arm-unknown-optee.linker=\"/tmp/arm-wrapper-gcc\"" || exit 1

# ===== 2. Post-build: strip + fix + sign =====
echo ">>> Post-build pipeline..."
TARGET_DIR=target/arm-unknown-optee/release
cp $TARGET_DIR/ta $TARGET_DIR/ta.stripped
$CROSS_STRIP $TARGET_DIR/ta.stripped
python3 fix_ta_elf.py $TARGET_DIR/ta.stripped $TARGET_DIR/ta.fixed
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid $TA_UUID --ta-version 0 \
  --in $TARGET_DIR/ta.fixed \
  --out $TARGET_DIR/${TA_UUID}.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem || exit 1
echo ">>> TA done: $TARGET_DIR/${TA_UUID}.ta"

# ===== 3. Build CA =====
echo ">>> Building CA..."
cd /workspace/kms/host
cargo build --target armv7-unknown-linux-gnueabihf --release --bin kms-api-server || exit 1
echo ">>> CA done: target/armv7-unknown-linux-gnueabihf/release/kms-api-server"

echo ">>> ALL BUILD DONE"
'
```

Output binaries (在 Mac 本地文件系统的路径)：
- TA: `kms/ta/target/arm-unknown-optee/release/4319f351-0b24-4097-b659-80ee4f824cdd.ta` (538K)
- CA: `kms/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server` (4.9M)

---

## Build TA (单独构建)

```bash
docker exec stm32-builder bash -c '
source /root/.cargo/env
export TA_DEV_KIT_DIR=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include/optee/export-user_ta
export RUST_TARGET_PATH=/workspace/sdks/rust-sdk
export CC_arm_unknown_optee=/tmp/arm-wrapper-gcc
export TARGET_TA=arm-unknown-optee
cd /workspace/kms/ta
HOST_CC=gcc xargo build --target arm-unknown-optee --release \
  --config "target.arm-unknown-optee.linker=\"/tmp/arm-wrapper-gcc\""
'
```

> **关键**: `TARGET_TA=arm-unknown-optee` 必须设置！否则 linker script 会生成 aarch64 格式导致链接失败。

### Post-Build: Strip -> Fix -> Sign

```bash
docker exec stm32-builder bash -c '
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd
TA_DEV_KIT_DIR=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include/optee/export-user_ta
CROSS_STRIP=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-strip
cd /workspace/kms/ta
TARGET_DIR=target/arm-unknown-optee/release
cp $TARGET_DIR/ta $TARGET_DIR/ta.stripped
$CROSS_STRIP $TARGET_DIR/ta.stripped
python3 fix_ta_elf.py $TARGET_DIR/ta.stripped $TARGET_DIR/ta.fixed
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid $TA_UUID --ta-version 0 \
  --in $TARGET_DIR/ta.fixed \
  --out $TARGET_DIR/${TA_UUID}.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem
'
```

## Build CA (单独构建)

```bash
docker exec stm32-builder bash -c '
source /root/.cargo/env
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/tmp/arm-ca-gcc
export CC=/tmp/arm-ca-gcc
export OPTEE_CLIENT_EXPORT=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
cd /workspace/kms/host
cargo build --target armv7-unknown-linux-gnueabihf --release --bin kms-api-server
'
```

Output binaries:
- `target/armv7-unknown-linux-gnueabihf/release/kms` — CLI tool
- `target/armv7-unknown-linux-gnueabihf/release/kms-api-server` — API server

### 构建常见问题

| 错误 | 原因 | 解决 |
|------|------|------|
| `cannot represent machine 'aarch64'` | `TARGET_TA` 未设置 | `export TARGET_TA=arm-unknown-optee` |
| `xargo: command not found` | 用错了容器 | 用 `stm32-builder`（不是 stm32-builder-v20） |
| `OPTEE_CLIENT_EXPORT is not set` | CA 构建缺少环境变量 | `export OPTEE_CLIENT_EXPORT=...sysroots/cortexa7t2hf...` |
| `file in wrong format` | CA 用了 aarch64 的 cc | 设置 `CC=/tmp/arm-ca-gcc` |
| `sign_encrypt.py: error` | 缺少 `sign-enc` 子命令 | `sign_encrypt.py sign-enc --uuid ...` |

## Deploy to Board

```bash
# From Mac (outside Docker, files are in shared volume)
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd

# Deploy TA
scp kms/ta/target/arm-unknown-optee/release/${TA_UUID}.ta root@192.168.7.2:/lib/optee_armtz/

# Deploy CA binaries
scp kms/host/target/armv7-unknown-linux-gnueabihf/release/kms root@192.168.7.2:/usr/local/bin/
scp kms/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server root@192.168.7.2:/usr/local/bin/

# Restart service
ssh root@192.168.7.2 "systemctl restart kms-api"
```

## Systemd Service

File: `/etc/systemd/system/kms.service`

```ini
[Unit]
Description=KMS API Server
After=tee-supplicant.service network.target
Requires=tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/local/bin/kms-api-server
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
Environment=KMS_ORIGIN=https://kms1.aastar.io,http://localhost:5173

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable kms
systemctl start kms
journalctl -u kms -f  # View logs
```

## WebAuthn 多源 Origin 配置

`KMS_ORIGIN` 环境变量控制 WebAuthn 允许的 origin 列表（逗号分隔），**不需要重新编译**，改环境变量重启即可：

```bash
# 生产 + localhost 调试（当前 DK2 配置）
Environment=KMS_ORIGIN=https://kms1.aastar.io,http://localhost:5173

# 仅生产（关闭 localhost）
Environment=KMS_ORIGIN=https://kms1.aastar.io

# 不设置时默认 https://{KMS_RP_ID}（即 https://aastar.io）
```

修改后需要：
```bash
# 在 DK2 上
vi /etc/systemd/system/kms.service   # 编辑 KMS_ORIGIN 行
systemctl daemon-reload
systemctl restart kms
journalctl -u kms | grep "Allowed origins"  # 确认生效
```

QEMU 直接在启动命令中设置：
```bash
KMS_DB_PATH=/data/kms/kms.db KMS_ORIGIN="https://kms.aastar.io,http://localhost:5173" /root/shared/kms-api-server
```

## Testing

```bash
# Health check
curl http://192.168.7.2:3000/health

# Create wallet (fast, entropy only)
curl -X POST http://192.168.7.2:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{"Description":"test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'

# Derive address (slow, BIP39+BIP32+secp256k1 on 650MHz ARM)
curl -X POST http://192.168.7.2:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d '{"KeyId":"<wallet-id>","DerivationPath":"m/44h/60h/0h/0/0"}'

# CLI
ssh root@192.168.7.2 "kms create-wallet"
ssh root@192.168.7.2 "kms derive-address --wallet-id <id> --hd-path m/44h/60h/0h/0/0"
```

## Key Fixes Applied

1. **ta_head at vaddr 0** — linker script places `.ta_head` before `.text`
2. **DT_HASH required** — `--hash-style=both` linker flag (OP-TEE 3.16 needs DT_HASH, not just GNU_HASH)
3. **No NOTE segment** — `/DISCARD/` for `.note` sections
4. **No build-id** — `--build-id=none` removes PT_NOTE from build-id
5. **GNU_STACK RWE** — `fix_ta_elf.py` patches flags from RW (6) to RWE (7)

## Performance Notes

Seed caching 已实现，PBKDF2 只在首次操作时运行，结果缓存在 secure storage。

| 操作 | 首次 (PBKDF2) | 后续 (有缓存) | 提速 |
|------|---------------|---------------|------|
| CreateKey | ~4s | ~4s | - |
| DeriveAddress | ~71s | **~7s** | 10x |
| Sign | ~80s | **~7s** | 11x |
| SignHash | ~83s | **~7s** | 12x |

- 首次 auto-derive 后 seed 自动缓存
- ~7s 主要消耗在 BIP32 derivation + secp256k1 signing（纯 Rust on ARMv7）
- TEE concurrency: limited to 1 concurrent operation (semaphore)
