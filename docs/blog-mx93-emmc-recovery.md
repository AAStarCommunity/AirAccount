# NXP FRDM-IMX93 eMMC 变砖全记录：从一条 dd 命令到完整恢复

> 时间：2026 年 6 月
> 作者：Jason（AAstar）
> 硬件：NXP FRDM-IMX93（aarch64 Cortex-A55，OP-TEE 4.8，生产 KMS 服务）

---

## 事故起因：一条少了参数的命令

我们在 NXP FRDM-IMX93 开发板上运行 AirAccount KMS（TEE 私钥管理服务），对外提供 `kms.aastar.io` 接口。某天在 SSH 会话中执行了：

```bash
dd if=imx-boot.bin of=/dev/mmcblk0
```

**少了 `seek=66`**。

这条命令把 imx-boot 二进制直接从 eMMC 起始位置（sector 0）写入，覆盖了 MBR 和分区表。正确写法应该是：

```bash
dd if=imx-boot.bin of=/dev/mmcblk0 seek=66 bs=512 conv=notrunc
```

eMMC 启动需要 bootloader 在 sector 66（字节偏移 0x8400），而 SD 卡是 sector 64（字节偏移 0x8000）。少了 `seek=66`，MBR 被覆盖，板子下次启动就卡在：

```
M33 prepare ok
（然后无输出，无响应，需断电）
```

## 硬件基础知识（踩坑前必读）

```
FRDM-IMX93 接口：
  J1（上方 USB-C）= CH342 双串口 → /dev/cu.usbmodem5B6D0044901
                     调试口，不供电，不是刷机口
  J2（下方 USB-C）= i.MX93 USB OTG
                     刷机口，SDPS/SDPV/FB 协议
  
  电源 = 单独的接口，J1/J2 均不供电
  
SW1 DIP 拨码：
  0001 = SDPS USB 下载模式（J2 刷机用）
  0010 = eMMC 启动
  0011 = SD 卡启动

eMMC vs SD 启动偏移：
  SD 卡：bootloader 在 sector 64（= 32KB）
  eMMC：bootloader 在 sector 66（= 33792 字节 = 0x8400）
  差 2 个 sector（1KB），这就是为什么 dd 必须加 seek=66
```

---

## 尝试一：UTM + uuu（彻底死路）

**思路**：用 NXP 官方工具 uuu 通过 J2 的 USB OTG 把 bootloader 下载进去，从 RAM 启动后再修 eMMC。

**踩坑 1：macOS IOHIDFamily 拦截**

Mac 上直接跑 uuu：

```
HID(W): LIBUSB_ERROR_TIMEOUT (-7)(20.16s)
```

卡在 14%，永远不动。根因：macOS 内核的 IOHIDFamily 驱动独占了 NXP USB HID 设备（VID=1FC9, PID=014E），libusb 无法写入。加 sudo 也没用——这是内核驱动级别的拦截，不是权限问题。

**踩坑 2：UTM LIBUSB_ERROR_ACCESS**

用 UTM（macOS 上的 QEMU 虚拟机）把 NXP USB 设备转发进 Ubuntu VM，让 VM 里的 uuu 来操作：

```
LIBUSB_ERROR_ACCESS (-3): could not claim interface 0 (configuration 2)
```

根因：QEMU/libusb 的 USB 设备配置索引有 off-by-one bug——NXP SDPS 设备只有 1 个 configuration（配置 0），但 UTM 的 libusb 尝试访问配置 2，直接拒绝。

这个问题没有外部修复方式，UTM 源码里的 bug。

**踩坑 3：UTM 无法自动转发重枚举设备**

i.MX93 在 SDPS → SDPV 阶段 USB PID 从 0x014E 变成 0x0151，UTM GUI 需要手动重新连接。手动操作根本跟不上时序。

**结论：macOS + UTM 路线彻底死路，不再尝试。**

---

## 尝试二：ELE Anti-Rollback（意外发现的深坑）

修好了 UTM 问题后，发现另一个更底层的问题：

```
SDPS 传输 100% 完成 → 等待 SDPV(0x0151) 出现 → 永远不出现
```

通过串口（J1，115200 baud）分析，发现 SPL 完全静默，没有任何输出。

根因：**ELE（Edge Lock Enclave）Anti-Rollback 机制**

i.MX93 的 ELE 是安全子系统，内部有一个 SNVS 单调计数器。每次运行更新版本的 ELE firmware，这个计数器就会不可逆地向前推进。一旦推进，旧版本的 ELE FW 就会被永久拒绝。

| 版本 | 结果 |
|------|------|
| LF_v6.6.36 | ELE 拒绝，SDPS 成功但 SPL 永远不运行 |
| LF_v6.12.34 | 无串口输出（ELE 也可能拒绝，或 DDR init 崩溃） |
| LF_v6.18.2 | ELE 接受 ✓ |

我们的板子之前运行过 v6.18.2，ELE 计数器已经推进，v6.6.36 被永久封锁。

---

## 尝试三：SD 卡启动（转机出现）

放弃 USB 刷机路线，转向 SD 卡启动。

**踩坑 1：v6.18.2 普通 singleboot 变体 DDR 崩溃**

写入 SD 卡后启动，串口收到：

```
（乱码，然后停止）
```

大约 1417 字节的乱码后停止，这是 DDR 初始化崩溃的特征——DDR 还没初始好，UART 就乱输出了。

**踩坑 2：v6.18.2 gdet_auto 才是正解**

NXP 的 BSP 提供了多个 bootloader 变体，其中 `gdet_auto` 后缀表示"自动检测 GPIO"——它能探测不同的 board revision 并自动选择正确的 DDR 时序参数。

文件名：`imx-boot-imx93-11x11-lpddr4x-frdm-sd.bin-flash_singleboot_gdet_auto`

写入 SD 卡 sector 64，SW1=0011，加电：**蓝色 LED 亮了。**

串口在 115200 baud 仍然是乱码（实际上是更高波特率或不同编码），但通过以太网 SSH 进去，板子完全正常运行，Linux 4 启动，KMS 服务运行。

---

## 从 SD 启动修复 eMMC

**第一步：确认 eMMC 分区表完整**

```bash
ssh root@192.168.2.37 "fdisk -l /dev/mmcblk0"
# p1: FAT32  sector 16384  256MB   ← kernel + DTB ✓
# p2: ext4   sector 540672 8.5GB   ← rootfs ✓
```

eMMC 里的内核和根文件系统完好，只有 bootloader 区域（sector 0-65）损坏了。

**第二步：把 v6.18.2 gdet_auto 写入 eMMC sector 66**

```bash
ssh root@192.168.2.37 "
dd if=/dev/mmcblk1 of=/dev/mmcblk0 skip=64 seek=66 bs=512 count=8192 conv=notrunc
sync
"
# mmcblk1 = SD 卡（bootloader 在 sector 64）
# mmcblk0 = eMMC（写入 sector 66）
```

验证 sector 66 开头为 AHAB 容器标识 `00 20 02 87`（tag=0x87）✓

**第三步：设置 PARTITION_CONFIG — 踩坑**

检查 eMMC 启动配置：

```bash
mmc extcsd read /dev/mmcblk0 | grep PARTITION_CONFIG
# PARTITION_CONFIG: 0x00
```

`0x00` 表示 `BOOT_PARTITION_ENABLE=0`（未配置启动源）。设置为 user area（值=7）：

```bash
mmc bootpart enable 7 1 /dev/mmcblk0
# PARTITION_CONFIG: 0x00 → 0x78
```

断电，拔 SD，SW1=0010，加电——**没有蓝灯，没有串口输出，完全无响应。**

---

## 关键诊断：sector 0 里是什么？

回到 SD 启动，检查 eMMC 的 sector 0：

```bash
dd if=/dev/mmcblk0 bs=512 count=4 2>/dev/null | od -A x -t x1 | head -4
000000 fa b8 00 10 8e d0 bc 00 b0 b8 00 00 8e d8 8e c0
```

`fa b8 00 10 8e d0` — **这是 x86 MBR 引导代码（cli; mov ax, 0x10; ...）**，不是 AHAB 容器。

真相：
1. eMMC 原本有完整的 WIC 镜像（MBR 在 sector 0，bootloader 在 sector 66）
2. 用户执行 `dd if=imx-boot.bin of=/dev/mmcblk0` 确实把 sector 0 写成了 AHAB 头
3. 但后来某次 WIC 镜像写入操作恢复了 sector 0 的 MBR
4. **结果：sector 0 = x86 MBR，sector 66 = AHAB（正确位置）**

问题在于：**i.MX93 ROM 在 user area 启动模式下，读取的起始地址是 sector 0**，而不是 sector 66。sector 0 的 x86 MBR 对 ARM ROM 来说是完全无效的数据，解析失败，启动终止——在 UART 初始化之前就挂了，所以什么输出都没有。

---

## 最终解决方案：Hardware Boot Partition

eMMC 有一个独立于 user area 的**硬件 boot partition**（`mmcblk0boot0`），专门用于存放 bootloader，不会被 MBR 或分区表操作影响。

```bash
# 解锁 boot0 的只读保护（Linux 默认开启只读）
echo 0 > /sys/class/block/mmcblk0boot0/force_ro

# 将 v6.18.2 gdet_auto bootloader 写入 hardware boot0
dd if=/dev/mmcblk1 bs=512 skip=64 count=8192 | \
    dd of=/dev/mmcblk0boot0 bs=512 seek=0 conv=notrunc
sync

# 设置 BOOT_PARTITION_ENABLE=1（从 hardware boot0 启动）
mmc bootpart enable 1 1 /dev/mmcblk0
# PARTITION_CONFIG: 0x48（BOOT_ACK=1, BOOT_PARTITION_ENABLE=1）
```

验证写入：sector 0 of boot0 = `00 20 02 87 01 00 00 00 00 00 02 01 90 00 00 00`（AHAB，tag=0x87）✓

断电，拔 SD，SW1=0010，加电——**蓝色 LED 亮了。**

```bash
ssh root@192.168.2.39 "
echo 'boot_dev:' $(cat /proc/cmdline | grep -oP 'root=\S+')
echo 'sd_card:' $(ls /dev/mmcblk1 2>/dev/null && echo present || echo absent)
"
# boot_dev: root=/dev/mmcblk0p2  ← 从 eMMC 启动 ✓
# sd_card: absent                ← SD 卡不在 ✓
```

---

## 验证：KMS 服务完整恢复

```bash
curl https://kms.aastar.io/health
```

```json
{
  "service": "kms-api",
  "status": "healthy",
  "ta_mode": "real",
  "version": "0.19.0"
}
```

- `ta_mode: "real"` — OP-TEE TrustZone 真实硬件，非模拟 ✓
- cloudflared 隧道：4 路连接（sjc10/lax08/lax07/sjc11）✓
- kms.aastar.io 公网可访问 ✓

---

## 总结：坑的完整列表

| 坑 | 根因 | 教训 |
|----|------|------|
| `dd` 少了 `seek=66` | 手滑 | SD 用 seek=64，eMMC 用 seek=66，永远不一样 |
| macOS uuu 14% 超时 | IOHIDFamily 内核驱动独占 HID 设备 | Mac 直连 uuu 是死路，别再试 |
| UTM LIBUSB_ERROR_ACCESS | QEMU libusb 的 off-by-one 配置号 bug | UTM USB 转发对 NXP SDPS 不可靠 |
| v6.6.36 被 ELE 拒绝 | ELE SNVS 计数器不可逆推进 | 运行过高版本后，低版本永久封锁 |
| v6.18.2 singleboot DDR 崩 | board revision 差异导致 DDR 时序不匹配 | 必须用 `gdet_auto` 变体，自动检测 DDR |
| eMMC user area 启动失败 | sector 0 是 x86 MBR，ROM 无法解析 | ROM 从 user area sector 0 读起，不是 sector 66 |
| PARTITION_CONFIG=0x78 无效 | 即使 user area boot 启用，sector 0 仍然是 MBR | 应使用 hardware boot partition，彻底隔离 |

## 正确的 eMMC 恢复流程（总结）

1. 准备 SD 卡，写入 **v6.18.2 gdet_auto** bootloader（sector 64）+ rootfs
2. SW1=0011，SD 卡启动进 Linux
3. SSH 进去，写入 hardware boot partition：
   ```bash
   echo 0 > /sys/class/block/mmcblk0boot0/force_ro
   dd if=/dev/mmcblk1 bs=512 skip=64 count=8192 | \
       dd of=/dev/mmcblk0boot0 bs=512 seek=0 conv=notrunc
   sync
   mmc bootpart enable 1 1 /dev/mmcblk0
   ```
4. 断电，拔 SD，SW1=0010，加电，验证蓝灯 ✓

**不要写 user area 的 sector 66——会被 MBR/分区工具覆盖。用 hardware boot partition。**

---

## 附：关键硬件信息

- **板子**：NXP FRDM-IMX93（aarch64 Cortex-A55 @ 1.7GHz，LPDDR4x）
- **TEE**：OP-TEE 4.8，TrustZone A55 EL3
- **安全芯片**：ELE（Edge Lock Enclave），独立 Cortex-M33
- **工作 bootloader**：LF_v6.18.2-1.0.0 `flash_singleboot_gdet_auto`（NXP BSP 包内）
- **eMMC 布局**：sector 0 = MBR，sector 66 = user area bootloader（已弃用），hardware boot0 = bootloader（正确），sector 16384 = FAT32（kernel/dtb），sector 540672 = ext4（rootfs）
