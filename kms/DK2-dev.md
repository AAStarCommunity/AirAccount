# KMS Development & Deployment Guide

> Last updated: 2026-03-04

## Endpoints & Test UI

| 端点 | 环境 | 版本 |
|------|------|------|
| `https://kms1.aastar.io` | **DK2 生产** (Cloudflare tunnel) | v0.16.1 |
| `https://kms.aastar.io` | **QEMU 测试** (Cloudflare tunnel) | v0.16.1 |
| `http://192.168.7.2:3000` | DK2 本地直连 (USB Ethernet) | v0.16.1 |
| `http://localhost:3000` | QEMU 本地 (Docker port map) | v0.16.1 |

所有端点的 `/test` 路径提供交互式 Test UI：`https://kms1.aastar.io/test`

---

## 开发流程

### 正常迭代（推荐流程）

```
1. 修改代码（Mac 上编辑 kms/ 目录）
   ↓
2. QEMU 构建 + 测试（开发验证，可中断服务）
   ↓
3. DK2 构建 + 优雅部署（beta 生产，不中断服务）
   ↓
4. 提交 + 推送到 KMS 分支
```

### 完整命令

```bash
# Step 1: 编辑代码（Mac 本地，两个容器自动可见）
#   - stm32-builder: 挂载 /workspace = AirAccount/
#   - teaclave_dev_env: 挂载 kms/ = projects/web3/kms/
vim kms/host/src/api_server.rs   # 改代码

# Step 2: QEMU 构建 + 部署测试
docker exec teaclave_dev_env bash -lc "
  cd /root/teaclave_sdk_src/projects/web3/kms/host
  cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server
"
docker exec teaclave_dev_env bash -lc "
  cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/
"
# 然后在 QEMU guest 中手动重启（见下文）

# Step 3: DK2 构建 + 优雅部署
cd kms && ./scripts/build.sh ca && ./scripts/deploy.sh ca

# Step 4: 验证
curl -s http://192.168.7.2:3000/version   # DK2
curl -s https://kms.aastar.io/version     # QEMU (如果 guest 已重启)
```

### 使用脚本（三步走）

```bash
cd kms/

# Step 1: 构建（DK2 target）
./scripts/build.sh ca          # 只编译 CA
./scripts/build.sh ta          # 只编译 TA
./scripts/build.sh             # 编译 TA + CA

# Step 2: 部署到 DK2（优雅切换，~4s 停机）
./scripts/deploy.sh ca         # 只部署 CA
./scripts/deploy.sh            # 部署 TA + CA

# Step 3: 测试
./scripts/run-all-tests.sh     # 全链 API 测试
```

### 注意事项

- **Mac 编辑 → 容器即可见**: stm32-builder 挂载整个 AirAccount，teaclave_dev_env 挂载 kms/ 目录
- **不需要 docker cp**: volume mount 已配置好
- **两个 target 共用代码**: 源码完全一样，只是编译目标不同
- **版本号两处同步**: `Cargo.toml` 的 `version` + `api_server.rs` 的 `KMS_VERSION`

---

## Docker Containers

| 容器 | 用途 | Target | Volume Mount |
|------|------|--------|--------------|
| `stm32-builder` | **DK2 交叉编译** | `armv7-unknown-linux-gnueabihf` | `-v AirAccount:/workspace` |
| `teaclave_dev_env` | **QEMU 模拟器 + 编译** | `aarch64-unknown-linux-gnu` | `-v third_party/teaclave-trustzone-sdk:/root/teaclave_sdk_src` + `-v kms:/root/teaclave_sdk_src/projects/web3/kms` |

> 容器重建命令（仅在丢失时使用）:
> ```bash
> # teaclave_dev_env 必须双挂载：SDK + kms overlay
> docker run -d --name teaclave_dev_env \
>   -v /path/to/third_party/teaclave-trustzone-sdk:/root/teaclave_sdk_src \
>   -v /path/to/kms:/root/teaclave_sdk_src/projects/web3/kms \
>   -p 54320:54320 -p 54321:54321 -p 3000:3000 \
>   teaclave_dev_env_backup tail -f /dev/null
> ```

### Linker 配置

`kms/host/.cargo/config.toml` 已配置 QEMU aarch64 linker：
```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
```

DK2 armv7 linker 通过 `build.sh` 的环境变量 `CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER` 设置，不写在 config.toml 中（避免冲突）。

---

## QEMU 测试环境

### 启动 QEMU（三个 Terminal）

```bash
# Terminal 2: Guest VM Shell (先启动)
docker exec -it teaclave_dev_env ./scripts/terminal2-guest-vm.sh

# Terminal 3: Secure World Log (先启动)
docker exec -it teaclave_dev_env ./scripts/terminal3-secure-log.sh

# Terminal 1: QEMU (等 Terminal 2+3 显示 "Listening on TCP port" 后启动)
docker exec -it teaclave_dev_env ./scripts/terminal1-qemu.sh
```

### QEMU Guest 内操作

```bash
# 登录
buildroot login: root

# 首次：挂载共享目录 + 系统路径
mkdir -p shared && mount -t 9p -o trans=virtio host shared && cd shared
mount --bind ta/ /lib/optee_armtz
mount --bind plugin/ /usr/lib/tee-supplicant/plugins/
cp /root/shared/*.ta /lib/optee_armtz/

# 创建本地 DB 目录（9p 不支持 SQLite WAL）
mkdir -p /data/kms

# 启动 KMS（生产 origin）
KMS_DB_PATH=/data/kms/kms.db ./kms-api-server

# 启动 KMS（开发模式，加 localhost）
KMS_DB_PATH=/data/kms/kms.db KMS_ORIGIN="https://*.aastar.io,http://localhost:5173" ./kms-api-server
```

### QEMU 重启 KMS（不重启 QEMU）

```bash
# 在 QEMU guest shell 中（没有 pkill，用 killall）
killall kms-api-server
KMS_DB_PATH=/data/kms/kms.db KMS_ORIGIN="https://*.aastar.io,http://localhost:5173" /root/shared/kms-api-server
```

### QEMU 构建（TA + CA）

```bash
# TA 构建（跳过 clippy，直接编译 + strip + sign）
docker exec teaclave_dev_env bash -lc '
cd /root/teaclave_sdk_src/projects/web3/kms/ta
LINKER_CFG="target.${TARGET_TA}.linker=\"${CROSS_COMPILE_TA}gcc\""
xargo build --target $TARGET_TA --release --config "$LINKER_CFG"
UUID=$(cat ../uuid.txt)
OUT=$PWD/target/${TARGET_TA}/release
${CROSS_COMPILE_TA}objcopy --strip-unneeded $OUT/ta $OUT/stripped_ta
python3 ${TA_DEV_KIT_DIR}/scripts/sign_encrypt.py \
  --uuid $UUID --key ${TA_DEV_KIT_DIR}/keys/default_ta.pem \
  --in $OUT/stripped_ta --out $OUT/${UUID}.ta
'

# CA 构建
docker exec teaclave_dev_env bash -lc "
  cd /root/teaclave_sdk_src/projects/web3/kms/host
  cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server
"

# 删除旧文件 + 部署新文件（必须先删后部署，避免残留旧版本）
docker exec teaclave_dev_env bash -lc '
UUID=$(cat /root/teaclave_sdk_src/projects/web3/kms/uuid.txt)
rm -f /opt/teaclave/shared/kms-api-server /opt/teaclave/shared/*.ta /opt/teaclave/shared/ta/*.ta
cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/${TARGET_TA}/release/${UUID}.ta /opt/teaclave/shared/
cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/${TARGET_TA}/release/${UUID}.ta /opt/teaclave/shared/ta/
cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/
'
```

> **注意**:
> - TA target 是 `$TARGET_TA`（aarch64-unknown-optee），不是 Makefile 默认的 `aarch64-unknown-linux-gnu`
> - `xargo clean` 会清除 sysroot 缓存，重建耗时 ~40s；非必要不要 clean
> - TA 改动后如果 target 没变，不需要 `xargo clean`，增量编译即可
> - 部署前**必须删除旧文件**，否则 guest 可能加载旧 TA

### QEMU 网络路径

```
Browser → kms.aastar.io
       → Cloudflare Tunnel (cloudflared on Mac)
       → localhost:3000 (Mac)
       → Docker port map 3000:3000 (teaclave_dev_env)
       → QEMU hostfwd tcp::3000-:3000
       → QEMU guest 0.0.0.0:3000 (kms-api-server)
```

---

## DK2 生产环境

### Network

```
Mac Mini M4 (192.168.7.3) ──USB Ethernet──> STM32MP157F-DK2 (192.168.7.2)
SSH: ssh root@192.168.7.2
```

Board: Cortex-A7 650MHz, ARMv7-A 32-bit, OP-TEE 3.16

### Systemd Service

文件: `/etc/systemd/system/kms.service`

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
Environment=KMS_ORIGIN=https://*.aastar.io,http://localhost:5173

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable kms        # 开机自启
journalctl -u kms -f        # 查看日志
journalctl -u kms | grep "Allowed origins"  # 确认 origin 配置
```

### 优雅部署（Graceful Deploy）

`deploy.sh` 采用 **Pre-stage + Atomic Swap** 策略，最小化停机时间：

```
Phase 1: Pre-stage（服务仍在运行，零影响）
  ├── scp kms-api-server → /usr/local/bin/kms-api-server.new
  └── scp *.ta → /lib/optee_armtz/*.ta.new

Phase 2: Switchover（~3-4s 停机窗口）
  ├── systemctl stop kms        # SIGTERM → warp drain in-flight requests
  ├── mv *.new → live binary    # 原子替换
  ├── systemctl daemon-reload   # 加载最新 service 文件
  └── systemctl start kms       # 启动新版本

Phase 3: Verify
  ├── health check 轮询（最多 15s）
  └── 输出版本对比 + 停机时长
```

实测停机 **~4 秒**。

#### 生产部署建议

对于 beta 阶段，当前 ~4s 停机已经足够。正式生产时可考虑：

1. **低峰期部署**: 选择凌晨或低流量时段
2. **提前公告**: 通过 API 返回 `X-Maintenance-Window` header 预告
3. **健康检查前置**: 部署前 `curl /QueueStatus` 确认无排队请求
4. **回滚**: 保留旧二进制为 `.bak`，出问题立即 `mv .bak` 回滚

```bash
# 完整生产部署流程
cd kms/

# 1. 确认无进行中的 TEE 操作
curl -s http://192.168.7.2:3000/QueueStatus

# 2. 构建
./scripts/build.sh ca

# 3. 优雅部署（自动 pre-stage + swap + verify）
./scripts/deploy.sh ca

# 4. 验证
curl -s http://192.168.7.2:3000/version
curl -s https://kms1.aastar.io/health
```

---

## WebAuthn Origin 配置

`KMS_ORIGIN` 控制 WebAuthn 允许的 origin 列表，**不需要重新编译**：

```bash
# 通配符：接受所有 *.aastar.io 子域名 + localhost（当前配置）
Environment=KMS_ORIGIN=https://*.aastar.io,http://localhost:5173

# 仅生产（关闭 localhost 调试）
Environment=KMS_ORIGIN=https://*.aastar.io

# 不设置时默认 https://{KMS_RP_ID}（即 https://aastar.io）
```

修改后：
```bash
vi /etc/systemd/system/kms.service
systemctl daemon-reload && systemctl restart kms
journalctl -u kms | grep "Allowed origins"
```

---

## Docker Build Environment (DK2)

**统一使用 `stm32-builder` 容器**

```bash
docker start stm32-builder
docker exec -it stm32-builder bash
```

> 挂载：`-v /Volumes/UltraDisk/Dev2/aastar/AirAccount:/workspace`
> SDK：`/opt/st/stm32mp1/6.6-v26.02.18/`
> 工具链：`nightly-2024-05-15` (rustc 1.80.0) + `xargo 0.3.26`

### Container Setup (one-time)

```bash
ln -sf /workspace/sdks/rust-sdk/optee-utee-build /optee-utee-build
pip3 install cryptography

# TA GCC wrapper
cat > /tmp/arm-wrapper-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  -nostartfiles \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-wrapper-gcc

# CA GCC wrapper
cat > /tmp/arm-ca-gcc << 'EOF'
#!/bin/bash
exec /opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-gcc \
  --sysroot=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
  -mthumb -mfpu=neon-vfpv4 -mfloat-abi=hard -mcpu=cortex-a7 "$@"
EOF
chmod +x /tmp/arm-ca-gcc
```

---

## Build Commands

### build.sh（推荐）

```bash
cd kms/
./scripts/build.sh           # TA + CA
./scripts/build.sh ca        # 只 CA
./scripts/build.sh ta        # 只 TA
```

### 手动构建

```bash
# DK2 CA
docker exec stm32-builder bash -c '
source /root/.cargo/env
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/tmp/arm-ca-gcc
export CC=/tmp/arm-ca-gcc
export OPTEE_CLIENT_EXPORT=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
cd /workspace/kms/host
cargo build --target armv7-unknown-linux-gnueabihf --release --bin kms-api-server
'

# QEMU CA（Mac 编辑后容器内直接编译）
docker exec teaclave_dev_env bash -lc "
cd /root/teaclave_sdk_src/projects/web3/kms/host
cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server
"
```

Output:
- DK2 CA: `kms/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server`
- QEMU CA: 容器内 `target/aarch64-unknown-linux-gnu/release/kms-api-server` → 复制到 `/opt/teaclave/shared/`

### 构建常见问题

| 错误 | 原因 | 解决 |
|------|------|------|
| `cannot represent machine 'aarch64'` | `TARGET_TA` 未设置 | `export TARGET_TA=arm-unknown-optee` |
| `xargo: command not found` | 用错了容器 | 用 `stm32-builder` |
| `OPTEE_CLIENT_EXPORT is not set` | CA 缺环境变量 | build.sh 自动设置 |
| `file in wrong format` | linker 不匹配 | 检查 `.cargo/config.toml` 或 `CC` 环境变量 |
| `xShmMap not supported` (QEMU) | 9p 不支持 SQLite WAL | `KMS_DB_PATH=/data/kms/kms.db` |

---

## 平台对比

| | DK2 (当前) | QEMU (测试) | i.MX 95 (未来) |
|---|---|---|---|
| CPU | Cortex-A7 650MHz | QEMU virt aarch64 | Cortex-A55 2.0GHz |
| Arch | ARMv7 32-bit | AArch64 64-bit | AArch64 64-bit |
| TA target | `arm-unknown-optee` | `aarch64-unknown-optee` | `aarch64-unknown-optee` |
| CA target | `armv7-unknown-linux-gnueabihf` | `aarch64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` |
| Build container | `stm32-builder` | `teaclave_dev_env` | 新容器（待建） |
| 代码 | 完全一样 | 完全一样 | 完全一样 |

i.MX 95 迁移只需：新建 Docker 编译容器 + 调整 deploy.sh 的 SSH 地址。代码零修改。

---

## Performance (DK2, p256-m, 2026-03-03)

| 操作 | 耗时 |
|------|------|
| health / QueueStatus | 3-5ms |
| ListKeys / DescribeKey | 5-6ms |
| GetPublicKey | 4ms |
| **DeriveAddress** | **1.16s** |
| **SignHash** | **1.26s** |
| **Sign (message)** | **1.27s** |
| **Sign (transaction)** | **1.93s** |
| CreateKey | 2.46s |
| ChangePasskey | 2.68s |
| DeleteKey | 2.88s |

---

## Key Fixes Applied

1. **ta_head at vaddr 0** — linker script places `.ta_head` before `.text`
2. **DT_HASH required** — `--hash-style=both` (OP-TEE 3.16 needs DT_HASH)
3. **No NOTE segment** — `/DISCARD/` for `.note` sections
4. **GNU_STACK RWE** — `fix_ta_elf.py` patches flags from RW (6) to RWE (7)
5. **p256-m aarch64** — `!defined(__aarch64__)` guard skips ARM32 inline ASM on 64-bit
6. **Wildcard origin** — `https://*.aastar.io` matches any subdomain
