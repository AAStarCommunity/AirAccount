# MX93 FRDM-IMX93 eMMC Reflash — Troubleshooting Log

## 背景

板子因意外 `dd` 写坏了 eMMC bootloader 区域（offset 错误），需要从 SDPS USB 模式完整刷机。

## 硬件配置

- 板子: NXP FRDM-IMX93 (aarch64, OP-TEE 4.8)
- J1 (OpenSDA, USB-C): 连接 Mac → `/dev/cu.usbmodem5B6D0044901` (115200 baud)
- J2 (i.MX93 USB-C): 刷机用，连 Ubuntu VM (UTM SPICE) 或直连 Mac
- SW1 DIP: `0001` = SDPS 模式，`0010` = eMMC 启动
- Ubuntu VM: aarch64 (Apple Silicon + UTM), `192.168.64.2`
- WIC 镜像: `imx-image-full-imx93frdm.rootfs.wic.zst` (951MB)

## uuu 版本说明

| 版本 | 结果 | 原因 |
|------|------|------|
| uuu 1.4.193 (系统) | ❌ 永远等待 | 内置设备表无 i.MX93 PID 0x014e |
| uuu 1.5.243 Ubuntu VM | ✅ SDPS 100% OK | 支持 0x014e |
| uuu_mac_arm 1.5.243 | ❌ LIBUSB_ERROR_TIMEOUT | macOS IOHIDFamily 锁住 HID 设备，libusb 无法写 |

## 尝试记录

### 尝试 1: Ubuntu VM 用 .lst 脚本文件
**假设**: 用自定义 .lst 脚本控制 SDPS  
**结果**: `Error: fail open file: >/tmp//mnt/images/imx-boot-...`  
**根因**: uuu 把脚本目录拼接到所有路径前（绝对路径也不例外）  
**教训**: **不要用 .lst 脚本文件**，用 `-b emmc_all` 内置脚本

### 尝试 2: Ubuntu VM -b emmc_all，只有 SDPS
**假设**: 单阶段 SDPS 能让板子进 SPL  
**结果**: SDPS 成功 100% (4.518s)，但 SDPV 设备 (0x0151) 永远不出现  
**根因**: UTM SPICE 只转发了 0x014e，板子重枚举为 0x0151 后 UTM 没有自动转发  
**假设被推翻**: 以为 `-scanterm` 是问题（后来发现不是）

### 尝试 3: macOS 本地 uuu_mac_arm
**假设**: 绕过 UTM，在 Mac 直接运行能处理所有 USB 阶段  
**结果**: `HID(W): LIBUSB_ERROR_TIMEOUT (-7)` 失败  
**根因**: macOS IOHIDFamily 内核驱动独占 HID 设备，libusb 写请求超时  
**教训**: macOS 上 libusb 无法用于 NXP HID SDPS 协议（IOHIDFamily 冲突），**不要再试**

### 尝试 4: 修改 UTM 配置自动转发全部 1fc9 设备
**假设**: 可以找到 UTM .utm 配置文件修改 USB 设备规则  
**结果**: UTM 配置在 sandbox container 里，无法读取/修改  
**教训**: UTM 通过 SPICE 动态转发，每次设备重枚举需要用户手动点击，无法自动化  
**决定**: 放弃 UTM 方案

### 尝试 5: SPL 串口分析（关键发现）
**通过 J1 串口 (screen /dev/cu.usbmodem5B6D0044901) 观察到**:
```
SOC: 0xa1009300
LC: 0x2040010
PMIC: PCA9451A
PMIC: Over Drive Voltage Mode
DDR: 3733MTS
DDR: 3733MTS
M33 prepare ok

U-Boot SPL 2024.04+gde16f4f1722+p0 (Sep 02 2024 - 10:44:35 +0000)
[reset loop...]
```
**关键发现**: SPL 在 M33 prepare ok 之后、"Trying to boot from USB SDP" 之前就重置了  
**含义**: SDPV 设备 (0x0151) 从未枚举，因为 SPL 崩溃在 USB init 之前  
**UTM 无法转发 0x0151 的真正原因是：板子根本没有枚举出 0x0151**

### 尝试 6: 换 flash_singleboot（无 gdet）
**假设**: `gdet_auto` 变体的 GPIO 检测逻辑在 SDPS RAM boot 时状态不对，导致 SPL 崩溃  
**预期**: `flash_singleboot`（无 GPIO 检测）应该在 SDPS 模式下更稳定  
**结果**: 进行中...

## 正确的刷机命令

```bash
# 在 Ubuntu VM (192.168.64.2) 上执行
# 前提：J2 连接到 Ubuntu VM，SW1=0001，板子已断电重启
cd /mnt/images && sudo /tmp/uuu_new -b emmc_all \
  imx-boot-imx93frdm-sd.bin-flash_singleboot_gdet_auto \
  imx-image-full-imx93frdm.rootfs.wic.zst
```

## 串口调试

```bash
# Mac 上查看 J1 串口输出
# 先检查是否有进程占用
lsof /dev/cu.usbmodem5B6D0044901

# 配置并读取
stty -f /dev/cu.usbmodem5B6D0044901 115200 cs8 -cstopb -parenb -echo
cat /dev/cu.usbmodem5B6D0044901

# 或直接用 screen (会独占端口，记得退出)
screen /dev/cu.usbmodem5B6D0044901 115200
```

## 绝对不要做

- ❌ 在 SSH 会话中 `dd if=imx-boot.bin of=/dev/mmcblk0`（没有 seek=66 会覆盖 MBR）
- ❌ 下载 LF_v6.12.34 EVK 镜像（那是 i.MX93 EVK，不是 FRDM 板）
- ❌ macOS uuu（IOHIDFamily 冲突，已确认无解）
- ❌ UTM 方案用于 SDPV/FB 阶段（UTM 无法自动转发）
