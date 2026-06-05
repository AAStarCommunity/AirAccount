# STM32MP157F-DK2 部署指南

> 创建：2026-06-02
> 用途：i.MX 95 到货前的 DK2 过渡部署方案
> 参考：`docs/STM32imigration.md`（详细背景）、`docs/hardware-ARM.md`（已弃用，历史参考）

---

## 架构说明

DK2 与 QEMU 的核心区别：

| 项目 | QEMU | STM32MP157F-DK2 |
|------|------|-----------------|
| 架构 | aarch64（ARMv8 64-bit） | ARMv7-A 32-bit（Cortex-A7）|
| 编译 target | `aarch64-unknown-linux-gnu` | `armv7-unknown-linux-gnueabihf` |
| 部署方式 | 9p 共享目录 | SCP → SSH |
| TA Dev Kit | 来自 Teaclave Docker 镜像 | 来自 ST SDK 或板上 OP-TEE 构建产物 |
| Secure Storage | REE-FS（虚拟磁盘） | REE-FS（SD 卡）或 eMMC RPMB |

**代码不需要改**，只需重新交叉编译。

---

## Step 0：获取 TA Dev Kit（关键前置）

TA Dev Kit (`export-ta_arm32`) 提供 STM32 平台的 OP-TEE 头文件和链接库，必须和目标板的 OP-TEE 版本匹配。

### 方案 A：从板子上提取（推荐，版本保证匹配）

板子运行起来后，OP-TEE build 产物通常在 `/usr/include/optee/` 或 BSP 的构建目录里：

```bash
# SSH 到板子
ssh root@192.168.7.2
find / -name "ta_dev_kit.mk" 2>/dev/null
# 示例输出: /usr/include/optee/export-ta_arm32/mk/ta_dev_kit.mk
# 则 TA_DEV_KIT_DIR = /usr/include/optee/export-ta_arm32

# 拷回 Mac
scp -r root@192.168.7.2:/usr/include/optee/export-ta_arm32 /opt/dk2-ta-dev-kit/
export DK2_TA_DEV_KIT_DIR=/opt/dk2-ta-dev-kit/export-ta_arm32
```

### 方案 B：从 ST 官方 SDK 获取

下载 OpenSTLinux Developer Package SDK（约 1.5GB）：

```bash
# ST 官网下载 SDK installer（根据你的 BSP 版本）
# https://www.st.com/en/embedded-software/stm32mp1dev.html
# 文件名类似: en.SDK-x86_64-stm32mp1-openstlinux-6.6-yocto-scarthgap-mpu-v26.02.18.tar.gz

tar xf en.SDK-*.tar.gz -C /tmp/
chmod +x /tmp/*/sdk/st-image-weston-*.sh
/tmp/*/sdk/st-image-weston-*.sh -d /opt/STM32MP1-SDK

# 找 TA Dev Kit 路径
find /opt/STM32MP1-SDK/sysroots -name "ta_dev_kit.mk" 2>/dev/null
export DK2_TA_DEV_KIT_DIR=<上面 find 到的路径，去掉末尾 /mk/ta_dev_kit.mk>
```

---

## Step 1：首次构建 Docker 镜像

```bash
cd ~/Dev/aastar/AirAccount

# 构建交叉编译镜像（仅首次，约 5 分钟）
docker build --platform linux/amd64 \
  -f docker/Dockerfile.stm32-builder \
  -t stm32-builder .
```

---

## Step 2：编译

```bash
# 设置 TA Dev Kit 路径（见 Step 0）
export DK2_TA_DEV_KIT_DIR=/opt/dk2-ta-dev-kit/export-ta_arm32

# 构建（TA + CA，产物到 build/dk2/）
./scripts/dk2-build.sh

# 查看产物
ls -lh build/dk2/
# 4319f351-0b24-4097-b659-80ee4f824cdd.ta  ← TA（约 200-400KB）
# kms-api-server                            ← CA（约 10-20MB）

# 验证是 32-bit ARM
file build/dk2/kms-api-server
# 应显示: ELF 32-bit LSB executable, ARM, ...
```

---

## Step 3：首次部署

板子通过网线直连 Mac Mini（Mac: `192.168.7.3`，DK2: `192.168.7.2`）：

```bash
# 确认 SSH 免密登录（首次需要）
ssh-copy-id root@192.168.7.2

# 首次部署（安装 systemd service + 部署二进制）
DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh --first-run
```

---

## Step 4：日常迭代

代码改了之后：

```bash
# 重新编译
DK2_TA_DEV_KIT_DIR=... ./scripts/dk2-build.sh

# 推送到板子（不需要 --first-run）
DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh
```

---

## 验证

```bash
# 健康检查
ssh root@192.168.7.2 'curl http://127.0.0.1:3000/health'

# 查看服务日志
ssh root@192.168.7.2 'journalctl -u kms-api-server -f'

# TEE 调用计数（确认 TA 真正被调用）
ssh root@192.168.7.2 'cat /sys/kernel/debug/optee/call_count'

# 跑 API 测试（在 Mac 上）
DK2_KMS_URL=http://192.168.7.2:3000 ./kms/test/run-api-tests.sh
```

---

## 常见问题

**`Exec format error`**：推了 aarch64 二进制到 ARMv7 板子。重新运行 `dk2-build.sh` 确保产物是 32-bit。

**`tee-supplicant` 没启动**：`ssh root@板子 'ps aux | grep tee-supplicant'`，手动 `tee-supplicant &` 后重试。

**`ta_dev_kit.mk` 找不到**：使用方案 A 从板子直接提取，或下载 ST SDK（方案 B）。

**RPMB 问题**：DK2 有 eMMC RPMB，但 ST 官方 BSP 默认用 SD 卡的 REE-FS。如需启用 RPMB，需重新编译 OP-TEE 加 `CFG_RPMB_FS=y`。对于过渡阶段，REE-FS 已满足需求。

---

## 与 i.MX 95 的切换

当 i.MX 95 到货后，切换方式：

```bash
# imx95 是 aarch64，直接用 QEMU 产物部署
# TA 二进制和 CA 二进制都不需要重新编译
scp build/qemu/4319f351-*.ta root@imx95:/lib/optee_armtz/
scp build/qemu/kms-api-server root@imx95:/usr/bin/
ssh root@imx95 'systemctl restart tee-supplicant kms-api-server'
```

详见 `docs/hardware-imx95-fetmx9596c.md` § Phase 1。
