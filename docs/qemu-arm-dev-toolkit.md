# QEMU ARM 开发套件

> 作者：AirAccount Team | 创建：2026-06-02
> 用途：AirAccount KMS 本地开发 / CI 验证环境（非生产）
> 生产方案：见 `docs/hardware-imx95-fetmx9596c.md`

---

## 架构说明

```
macOS Host (x86_64 或 Apple Silicon)
  └── Docker 容器 teaclave_dev_env (x86_64 linux)
        └── QEMU qemu-system-aarch64
              ├── Normal World: Linux (aarch64, cortex-a57)
              │     └── kms-api-server (CA, aarch64-linux-gnu)
              └── Secure World: OP-TEE 4.5.0
                    └── AirAccount TA (aarch64-unknown-linux-gnu)
```

**关键点**：
- Host macOS 是 x86_64（或 arm64 Apple Silicon），容器内仍是 x86_64 linux
- QEMU 模拟 `aarch64 cortex-a57`，和 i.MX 95 (Cortex-A55) **相同指令集**
- 编译目标：TA 和 CA 均为 `aarch64-unknown-linux-gnu`（见 `kms/Makefile` `TARGET_TA`）
- 历史文档中的"Intel"指的是 Docker 容器是 x86_64 宿主，**QEMU 本身一直是 ARM**

---

## 快速启动（三终端）

按顺序启动：先 T2、T3，再 T1。

```bash
# 终端 2（Guest VM Shell，先启动）
./scripts/terminal2-guest-vm.sh
# 等待显示：Listening on TCP port 54320

# 终端 3（Secure World Log，先启动）
./scripts/terminal3-secure-log.sh
# 等待显示：Listening on TCP port 54321

# 终端 1（QEMU，最后启动）
./scripts/terminal1-qemu.sh
# 按回车，等待 buildroot login: 出现在终端 2
```

---

## 完整开发周期

### Step 1：构建 TA（Trusted Application）

推荐直接使用 `make`（包含构建、strip、签名全流程）：

```bash
docker exec -it teaclave_dev_env bash -l
cd /root/teaclave_sdk_src/projects/web3/kms
make ta

# 产物路径（TARGET_TA = aarch64-unknown-linux-gnu）
# target/aarch64-unknown-linux-gnu/release/4319f351-0b24-4097-b659-80ee4f824cdd.ta

# 部署到 QEMU 共享目录
cp target/aarch64-unknown-linux-gnu/release/4319f351-*.ta /opt/teaclave/shared/ta/
```

如需手动逐步操作（调试用）：

```bash
# 构建 TA（TARGET_TA = aarch64-unknown-linux-gnu，见 kms/Makefile）
cd kms/ta
CC=aarch64-linux-gnu-gcc \
  xargo build --target aarch64-unknown-linux-gnu --release

# 签名 TA
UUID=4319f351-0b24-4097-b659-80ee4f824cdd
aarch64-linux-gnu-objcopy --strip-unneeded \
  target/aarch64-unknown-linux-gnu/release/ta \
  target/aarch64-unknown-linux-gnu/release/stripped_ta

python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid $UUID \
  --ta-version 1 \
  --in  target/aarch64-unknown-linux-gnu/release/stripped_ta \
  --out target/aarch64-unknown-linux-gnu/release/${UUID}.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem

cp target/aarch64-unknown-linux-gnu/release/${UUID}.ta /opt/teaclave/shared/ta/
```

### Step 2：构建 CA（KMS API Server）

```bash
# 同一 Docker 容器内
cd /root/teaclave_sdk_src/projects/web3/kms/host
cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server

# 部署到 QEMU 共享目录
cp target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/
```

### Step 3：在 QEMU guest 里部署

在终端 2（Guest VM Shell）里操作：

```bash
# QEMU guest 登录后
mkdir -p shared
mount -t 9p -o trans=virtio host shared
cd shared

# 挂载 TA
mount --bind ta/ /lib/optee_armtz

# 重新加载 TA（无需重启 QEMU）
cp ta/4319f351-0b24-4097-b659-80ee4f824cdd.ta /lib/optee_armtz/

# 启动 KMS
mkdir -p /data/kms
pkill kms-api-server 2>/dev/null || true
KMS_DB_PATH=/data/kms/kms.db ./kms-api-server &

# 验证
curl http://localhost:3000/health
```

### Step 4：测试

```bash
# 本地 Mac 上
curl http://localhost:3000/health
curl http://localhost:3000/version

# 跑集成测试
cd kms/test
./run-api-tests.sh

# P256 session key 测试（需要先获取 credential）
./test-p256-session-key-v0181.sh
```

---

## 常见问题

### SQLite 必须放本地，不能放 9p 挂载目录

```bash
# ❌ 错误：会触发 xShmMap 不支持
KMS_DB_PATH=/root/shared/kms.db ./kms-api-server

# ✅ 正确：放 QEMU guest 本地磁盘
KMS_DB_PATH=/data/kms/kms.db ./kms-api-server
```

原因：9p 文件系统不支持 SQLite WAL 模式的 `xShmMap` 共享内存。

### 端口冲突（54320/54321 被占用）

```bash
# terminal2/3 脚本已内置端口清理，手动清理：
docker exec teaclave_dev_env bash -c "kill -9 \$(lsof -ti:54320) 2>/dev/null; true"
docker exec teaclave_dev_env bash -c "kill -9 \$(lsof -ti:54321) 2>/dev/null; true"
```

### QEMU 端口转发确认

QEMU guest 的 3000 端口需通过 hostfwd 映射到容器：
```
QEMU guest :3000  →  容器 :3000  →  Mac localhost:3000
```

确认 `terminal1-qemu.sh` 里的 start_qemuv8 脚本包含 `hostfwd=tcp::3000-:3000`。

### TA 更新后 tee-supplicant 缓存

```bash
# guest 里，新 TA 替换后强制 tee-supplicant 重新加载
pkill tee-supplicant
tee-supplicant &
sleep 2
# 然后再启动 kms-api-server
```

---

## QEMU vs i.MX 95 对比（决策参考）

| 维度 | QEMU | i.MX 95 |
|------|------|---------|
| 用途 | 开发 / CI | 生产 |
| TrustZone | 软件模拟 | 硬件实现 |
| Secure Storage | REE-FS（9p 或本地文件） | eMMC RPMB（硬件防回滚） |
| 网络 | Docker 端口转发 | 直连 |
| 启动 | ~30-60 秒 | ~8-12 秒 |
| 密钥安全 | ❌ 不适合生产 | ✅ 生产级 |
| 代码兼容 | ✅ | ✅（同 aarch64）|

---

## 快速 tmux 单窗口开发（推荐）

```bash
# 一行启动三窗格（需要 tmux）
tmux new-session -d -s kms -x 220 -y 50 \; \
  send-keys './scripts/terminal2-guest-vm.sh' Enter \; \
  split-window -h \; \
  send-keys './scripts/terminal3-secure-log.sh' Enter \; \
  split-window -v \; \
  send-keys 'sleep 3 && ./scripts/terminal1-qemu.sh' Enter \; \
  attach
```

或使用现有脚本：
```bash
./scripts/monitor-all-tmux.sh
```

---

## 与 i.MX 95 的迁移对接

QEMU 是 i.MX 95 的**镜像开发环境**：
- QEMU 验证通过的 TA 二进制，直接 `scp` 到 imx95 的 `/lib/optee_armtz/` 即可运行
- 无需重新编译，无需修改代码
- 唯一区别：imx95 使用 RPMB 存储，需确认 tee-supplicant 启用 RPMB backend

迁移命令：
```bash
# 从 QEMU 共享目录迁移到 imx95
scp /opt/teaclave/shared/ta/4319f351-*.ta root@imx95:/lib/optee_armtz/
scp /opt/teaclave/shared/kms-api-server  root@imx95:/usr/bin/
ssh root@imx95 "systemctl restart tee-supplicant kms-api-server"
```

详细迁移步骤见 `docs/hardware-imx95-fetmx9596c.md` § Phase 1。
