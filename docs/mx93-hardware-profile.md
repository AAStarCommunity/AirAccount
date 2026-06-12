# NXP FRDM-IMX93 硬件档案

> 采集时间：2026-06-07  
> 采集方式：串口控制台自动脚本  
> 用途：AirAccount KMS v0.19.x 部署参考

---

## 基础信息

| 项目 | 值 |
|------|-----|
| 板型 | NXP FRDM-IMX93（imx93-11x11-lpddr4x-frdm）|
| OS | NXP i.MX Release Distro 6.18-whinlatter |
| 内核 | 6.18.2-1.0.0-gf49f45233f7b，aarch64，编译于 2026-02-11 |
| 登录 | `root`，无密码 |

---

## CPU

| 项目 | 值 |
|------|-----|
| 架构 | aarch64（ARMv8-A）|
| 核心数 | 2 × Cortex-A55（CPU part 0xd05）|
| BogoMIPS | 48.00 / 核 |
| 实现者 | ARM（0x41）|

---

## 内存

| 项目 | 值 |
|------|-----|
| 总量 | 1985620 kB ≈ **2 GB LPDDR4x** |
| 空闲（采集时）| ~1.6 GB |
| Swap | 无 |

---

## 存储

### eMMC

| 项目 | 值 |
|------|-----|
| 设备 | `/dev/mmcblk0` |
| 型号 | DV4032（Sandisk/WD，Manufacturer ID 0x000045）|
| 总容量 | 29.1 GB |
| 分区布局 | mmcblk0p1（256MB，boot，/run/media/boot-mmcblk0p1）<br>mmcblk0p2（8.5GB，rootfs，/ ）<br>mmcblk0boot0 / boot1（各 4MB）|
| 根分区使用率 | 4.8G used / 8.2G total（61%，剩余 **3.1 GB**）|

### RPMB ✅

| 项目 | 值 |
|------|-----|
| 设备 | `/dev/mmcblk0rpmb` |
| 容量 | **16 MB** |
| 内核日志 | `mmcblk0rpmb: mmc0:0001 DV4032 16.0 MiB, chardev (511:0)` |
| 用途 | OP-TEE 安全存储后端（替代 QEMU/DK2 的 REE-FS）|

> **关键结论**：RPMB 存在且可用，AirAccount KMS 私钥可使用 RPMB 存储，安全性高于 DK2 的 REE-FS。

---

## OP-TEE / TEE

| 项目 | 值 |
|------|-----|
| TEE 设备 | `/dev/tee0`、`/dev/teepriv0` ✅ |
| tee-supplicant | `/sbin/tee-supplicant` ✅ |
| TA 目录 | `/lib/optee_armtz/`，已预装 **28 个 TA** |

预装 TA 列表（部分）：
```
023f8f1a-...ta  a4c04d50-...ta  b3091a65-...ta
02a42f43-...ta  a720ccbb-...ta  c3f6e2c0-...ta
...（共 28 个）
```

---

## EdgeLock Enclave（ELE）

| 项目 | 值 |
|------|-----|
| 驱动 | `fsl-se secure-enclave` 已加载 |
| 保留内存 | 0xa4120000，1 MB，nomap |
| TRNG | `ele-trng` 注册成功 ✅ |
| 用户空间设备 | `/dev/ele*` 未暴露（驱动加载，无字符设备）|

> ELE 是 i.MX93 的硬件安全子系统，提供可信随机数、密钥存储等能力，OP-TEE 通过 ELE 访问硬件安全服务。

---

## 网络

| 接口 | 状态 | MAC |
|------|------|-----|
| eth0 | NO-CARRIER（未插网线）| 90:a9:f7:80:39:15 |
| eth1 | NO-CARRIER（未插网线）| 90:a9:f7:80:39:16 |
| mlan0 | DOWN（WiFi，未配置）| 80:a1:97:50:21:2d |
| uap0 | DOWN（WiFi AP 模式）| — |

> **当前无网络**。部署需要先接网线（eth0/eth1）或配置 WiFi，才能通过 scp 传输编译产物。

---

## 工具链

| 工具 | 状态 | 版本 |
|------|------|------|
| gcc | ✅ `/bin/gcc` | GCC 15.2.0 |
| python3 | ✅ `/bin/python3` | Python 3.13.9 |
| rustc | ❌ 未安装 | — |

> 板子上有 GCC，**可以本地编译 C 代码**（TA 的 Makefile 用 gcc）。Rust 需要通过 `rustup` 安装或交叉编译后传入。

---

## 部署策略建议

### 推荐路径：网络传输

1. 给板子接网线（eth0 或 eth1）
2. 在 Mac 上交叉编译（aarch64 target）
3. `scp` 传输到板子
4. 板子上运行

```bash
# Mac 上编译（需 aarch64 cross toolchain）
cargo build --target aarch64-unknown-linux-gnu --release

# 传输
scp target/aarch64-unknown-linux-gnu/release/kms-api-server root@<板子IP>:/usr/local/bin/
```

### 备用路径：串口传输（无网络）

使用 lrzsz 工具通过串口 YMODEM 协议传输（速度慢，~115KB/s）：

```bash
# Mac 端发送
brew install lrzsz
sz -b build/mx93/kms-api-server   # 在 screen 里输入 rz 后执行
```

### TA 编译

TA 需要 OP-TEE TA Dev Kit（`export-ta_aarch64`）。候选路径：

```bash
# 在板子上找
find / -name "ta_dev_kit.mk" 2>/dev/null
find /usr -name "export-ta_aarch64" 2>/dev/null
```

---

## 与 DK2 / QEMU 对比

| 项目 | QEMU | DK2（STM32MP157）| **MX93（本机）** |
|------|------|------|------|
| 架构 | aarch64 | ARMv7-A 32-bit | **aarch64** ✅ |
| RAM | 可配置 | 512MB | **2GB** ✅ |
| RPMB | 模拟 | ❌（REE-FS）| **✅ 真实 16MB** |
| OP-TEE | 软件 | 真实 | **真实** ✅ |
| EdgeLock | ❌ | ❌ | **✅ ELE** |
| 网络 | virtio | USB Eth | **Eth + WiFi** |
