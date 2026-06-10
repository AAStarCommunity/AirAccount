# MX93 FRDM-IMX93 完整刷机决策日志

> 持续更新中。记录从根因到最终修复的每一步决策、假设、尝试和教训。
> 最后更新：2026-06-10

---

## 根本问题起源：CreateKey 崩板

**症状**: 每次重启后 `POST /kms/CreateKey` 导致板子物理崩溃（无响应，需断电重启）

**根因调查**:
- 假设 A (错误): CAAM 硬件 RNG 问题 → 测试否定
- **真实根因**: ELE (Edge Lock Enclave) TRNG 必须在 U-Boot SPL 阶段启动
  - ELE 是 i.MX93 的安全子系统，不是 CAAM
  - TRNG 初始化需要特定 SPL 启动序列
  - 缺少这步 → OP-TEE TA 调用 TEE RNG → 触发 ELE fault → 板子崩溃

**修复**: 在 imx-boot 中确保 ele_start_rng 被调用（6.6.36-2.1.0 BSP 已包含，
         验证依据：在 imx-boot 二进制 0x2d6dd 处找到 "Fail to start RNG: %d\n" 紧跟 "Normal Boot\n"）

---

## 当前问题：eMMC 损坏需要刷机

**事故经过**:
- 原因：SSH 会话中执行 `dd if=imx-boot.bin of=/dev/mmcblk0`，**没有 seek=66**
- 结果：eMMC 的 MBR/分区表被覆盖
- 症状：板子启动停在 "M33 prepare ok"（eMMC 启动失败，SPL 在等 SDPV USB 或 eMMC）

**硬件架构**:
```
J1 (OpenSDA, 上面的 USB-C):
  - 连 Mac → /dev/cu.usbmodem5B6D0044901 (串口 115200)
  - 不是刷机口，是调试/串口口

J2 (i.MX93 直接 USB, 下面的 USB-C):  
  - 刷机口，SDPS/SDPV/FB 协议
  - PID 0x014e (ROM/SDPS) → 0x0151 (SPL/SDPV) → 0x0152 (U-Boot/FB)

SW1 DIP 拨码:
  - 0001 (label 1 ON, 234 OFF) = SDPS USB 下载模式
  - 0010 (label 2 ON, 134 OFF) = eMMC 启动
  - 0011 (label 1+2 ON, 34 OFF) = SD 卡启动 (USDHC2)
```

---

## WIC 镜像分析（关键数据）

```
文件: imx-image-full-imx93frdm.rootfs.wic
大小: 7.0GB
MBR: 有效 (55aa)

分区表:
  Part 1: FAT32 (0x0c) 起始 sector 16384 (8MB) 大小 83MB   ← 内核/dtb/启动文件
  Part 2: ext4  (0x83) 起始 sector 196608 (96MB) 大小 7107MB ← 根文件系统

bootloader 位置: sector 64 (字节偏移 32768 = 32KB)
  - 这是 i.MX93 SD 卡启动的正确偏移量 ✓
  - eMMC 用户区启动用 sector 66 (33KB)，与此不同
  
imx-boot binary 内容: 2,136,064 bytes
  所有三个变体 (singleboot/gdet/gdet_auto) 大小相同，可能内容完全一样
```

**结论**: WIC 的 bootloader 在正确的 SD 卡偏移（sector 64）。
写入 SD 卡后 SW1=0011 应该能启动。如果不能，是写入问题或 SD 卡问题，不是镜像问题。

---

## 尝试 1: Ubuntu VM uuu 1.4.193（系统默认版本）

**日期**: 2026-06-07 开始  
**假设**: 系统 uuu 能找到 i.MX93 设备并启动 SDPS  
**结果**: `Wait for Known USB Device Appear...` → 永远卡住  
**根因**: uuu 1.4.193 的内置设备表不包含 i.MX93 (1fc9:014e)  
**教训**: 必须用 uuu 1.5.243+，用 `uuu -V | grep MX93` 验证  

---

## 尝试 2: uuu 1.5.243 aarch64 下载

**假设**: 1.5.243 支持 i.MX93  
**踩坑**: `/mnt/images/uuu_new` 被下载为空文件（9 bytes ASCII），是 WebDAV 写问题  
**解决**: 下载到 `/tmp/uuu_new`（本地 tmpfs），验证 `uuu -V | grep MX93` 出现 ✅  

---

## 尝试 3: uuu 1.5.243 + .lst 脚本文件

**假设**: .lst 脚本文件可以正常使用  
**踩坑**: `Error: fail open file: >/tmp//mnt/images/imx-boot-...`  
**根因**: uuu 把脚本目录拼接到绝对路径前，这是 uuu 的 bug  
**修复**: 不用 .lst 文件，改用 `cd /mnt/images && uuu -b emmc_all boot.bin wic.zst`  
**结果**: SDPS 100% 成功（4.518秒）→ 但 SDPV (0x0151) 从不出现  

---

## 尝试 4: 串口分析（关键转折点）

**方法**: J1 连 Mac，分析 SPL 输出  
**踩坑**: screen 进程占串口 → `lsof /dev/cu.usbmodem*` → `kill -9 <pid>`  
**正确命令**: `stty -f /dev/cu.usbmodemXXX raw 115200 cs8 -cstopb -parenb -echo`（必须 raw 模式）

**关键发现**（serial 输出）:
```
SOC: 0xa1009300
LC: 0x2040010
PMIC: PCA9451A
PMIC: Over Drive Voltage Mode
DDR: 3733MTS
DDR: 3733MTS
M33 prepare ok
U-Boot SPL 2024.04+gde16f4f1722+p0 (Sep 02 2024 - 10:44:35 +0000)
[在 "Trying to boot from USB SDP" 出现之前就复位循环了]
```

**推翻假设**: 问题不是 UTM 转发，是 SPL 本身在 USB 初始化前就崩溃了  
**根因假设**: SPL 尝试将 USB 从 ROM SDPS HID 模式切换到 SPL gadget/SDPV 模式时挂起，watchdog 触发  

---

## 尝试 5: macOS 本地 uuu_mac_arm

**假设**: 绕过 UTM  
**结果**: `HID(W): LIBUSB_ERROR_TIMEOUT (-7)` → 永远不能写  
**根因**: macOS IOHIDFamily 内核驱动独占 USB HID 设备，libusb 无法写入  
**结论**: **macOS + libusb + NXP HID = 永久死路，不再尝试**  

---

## 尝试 6: UTM 配置文件修改（添加 0x0151/0x0152 规则）

**假设**: 找到 UTM 配置自动转发新设备  
**结论**: UTM 配置在 sandbox 内，外部无法修改  
**教训**: UTM SPICE USB 转发无法自动化，**不再尝试 UTM 方案**  

---

## 尝试 7: flash_singleboot（无 GPIO 检测）二进制

**假设**: `gdet_auto` 的 GPIO 检测在 SDPS RAM boot 时导致崩溃  
**前置**: 发现 `.bin` 扩展名问题 → uuu 无法识别 `flash_singleboot_gdet_auto` 文件名 → 需要 symlink 到 `boot.bin`  
**结果**: flash_singleboot 崩溃更早（7字节串口乱码），而非更少  
**结论**: 二进制变体不是崩溃根因  

---

## 尝试 8: UTM authorized=0 导致致命错误

**操作失误**: 在 UTM 转发设备上执行 `echo 0 > /sys/bus/usb/devices/3-3/authorized`  
**结果**: UTM 报 "USB Device [1fc9:014e] fatal IO error"，设备从 VM 永久断开  
**恢复**: 需要物理断电板子 + 重启 UTM  
**教训**: **永远不要在 UTM 转发的 USB 设备上执行 authorized=0**  

---

## 尝试 9: SD 卡启动（SW1=0011）

**时间**: 前一个 session（context 已压缩，细节不完整）  
**操作**:
1. 使用 Balena Etcher（或 dd）将 WIC 写入 microSD 卡
2. 设置 SW1=0011（1号+2号 ON）
3. 尝试启动

**已确认的数据**:
- WIC bootloader 在 sector 64（正确 SD 偏移）✓
- WIC 分区表有效（MBR 55aa）✓
- WIC 总大小 7.0GB，需要至少 7.2GB SD 卡

**失败原因**: **未知** — 前一个 session 没有捕获串口输出，不知道确切失败模式

**假设（按可能性排序）**:
1. WIC 写入不完整（Etcher 或 dd 中途中断）→ 内核找不到根文件系统 → 串口会显示 kernel panic
2. SD 卡未正确弹出/写入验证 → 数据损坏
3. Linux 实际上启动了但网络未配置 → 用户无法 SSH 连接，误以为失败
4. SD 卡物理故障
5. SW1 设置有误（仅 1 或 2 ON，而不是 1+2 同时 ON）

**诊断缺失**: 这次尝试**没有串口监控**，所以无法确认是哪种失败  
**下一步**: 重试，这次先开启串口监控再启动

---

## 当前卡死的核心问题：SPL 在 SDPS 模式下崩溃

```
已确认死路（不再尝试）:
  ✗ macOS 直接 uuu — IOHIDFamily 永久阻塞
  ✗ UTM SPICE 自动转发 — 不可能对重枚举设备自动化
  ✗ flash_singleboot 变体 — 崩溃更快，无改善
  ✗ authorized=0 — UTM fatal IO error
  
SDPS 状态：
  ✓ SDPS（ROM 接收 imx-boot.bin）: 每次 100% 成功，4.4-4.5秒
  ✗ SDPV（SPL USB gadget 模式）: 从未出现，SPL 在 USB init 前崩溃
  
SPL 串口输出（SDPS 模式）：
  SOC/DDR/M33 初始化成功，U-Boot SPL 启动 banner 出现
  "Trying to boot from USB SDP" 从未出现 → 复位循环
```

---

## 业界标准方案对比分析

| 方案 | 工具 | 需要 USB? | 需要 SPL 正常? | 难度 | 状态 |
|------|------|----------|--------------|------|------|
| **SD 卡启动** | Etcher/dd | 否 | 否 | 低 | 已尝试，失败原因未知 |
| **SDPS→SDPV via uuu** | uuu 1.5.243 | 是 (J2) | 是 | 中 | **确认死路** (SPL崩溃) |
| **macOS uuu** | uuu_mac_arm | 是 (J2) | 是 | 中 | **确认死路** (IOHIDFamily) |
| **JTAG via OpenSDA** | pyocd/OpenOCD | 是 (J1) | 否 | 高 | 未尝试 |
| **NXP SPSDK** | nxpdebugmbox | 是 (J1/J2) | 部分 | 高 | 未尝试 |
| **更新 BSP 版本** | 不同 imx-boot | 是 (J2) | 是 | 低 | 未尝试 |

### 方案详解

#### A. SD 卡启动（最高优先级，业界标准首选）
**为什么是首选**: 完全绕过所有 USB/SPL 问题。写 SD → 启动 → 从 Linux 内部修 eMMC。
**NXP 官方支持**: SW1=0011 是 FRDM-IMX93 标准 SD 启动模式。
**关键命令（eMMC 修复）**:
```bash
# 从 SD 启动的 Linux 内部执行
dd if=/boot/imx-boot*.bin of=/dev/mmcblk0 seek=66 bs=512 conv=notrunc
# 或用 mmcblk0boot0（eMMC 专用 boot 分区）
```
**当前障碍**: 前一次尝试失败，原因未知（无串口日志）。需要诊断。

#### B. JTAG via OpenSDA（备选核武器）
**工具**: pyocd（开源，支持 CMSIS-DAP）或 JLink
**原理**: J1 上的 OpenSDA 固件提供 CMSIS-DAP probe，通过 JTAG 直接访问 Cortex-A55。
可以从 CPU 端操作 USDHC 控制器写 eMMC，完全绕过 ROM/SPL。
**安装**:
```bash
pip3 install pyocd
pyocd list  # 应该看到 FRDM-IMX93 OpenSDA
pyocd gdbserver -t imx93  # 或类似
```
**限制**: 需要写 USDHC HAL 代码或使用 NXP SPSDK 的 debug mailbox 功能。
**适用场景**: SD 卡方案完全失败时的最后备选。

#### C. NXP SPSDK (Security Provisioning SDK)
**工具**: https://github.com/NXPmicro/spsdk
**原理**: NXP 官方量产工具，支持 debug mailbox 访问 ELE。
**限制**: 对于 eMMC 完整刷机，仍然需要 SDPS→SDPV 路径，不能绕过 SPL 崩溃。
**有用之处**: 读 eFUSE 状态、ELE 配置诊断。
**结论**: 对我们的问题帮助有限。

#### D. 更换 imx-boot 二进制版本
**假设**: 当前 Sep 2024 BSP 的 SPL 在 SDPS RAM boot 时有 USB PHY 初始化 bug
**未验证版本**: LF_v6.6.52, LF_v6.12.x
**操作**: 从 NXP 下载更新 BSP，只换 imx-boot 文件，重试 uuu
**风险**: 不同 BSP 的 imx-boot 可能与我们的 rootfs 不兼容

---

## 下一步行动计划（按优先级）

### 立即行动：诊断 SD 卡启动（5分钟内可验证）

**目的**: 确认 SD 卡启动状态，捕获串口输出

```bash
# Step 1: 在 Mac 上开启串口监控（先于启动板子）
stty -f /dev/cu.usbmodem5B6D0044901 raw 115200 cs8 -cstopb -parenb -echo
nohup cat /dev/cu.usbmodem5B6D0044901 > /tmp/sd-boot.log 2>&1 &
echo "Serial capture PID: $!"

# Step 2: 硬件操作
# - 确认 SD 卡已插入板子 microSD 槽
# - SW1 拨到: 1号ON + 2号ON + 3号OFF + 4号OFF (即 0011)
# - 板子断电重启

# Step 3: 等 60 秒后读日志
sleep 60
cat /tmp/sd-boot.log
```

**期望输出**（如果成功）:
```
SOC: 0xa1009300
...
U-Boot 2024.04...
=> booting from SD
...
root@imx93frdm:~#
```

**如果 SD 启动成功，从 SD 修复 eMMC**:
```bash
# SSH 到板子（找 IP: arp -a 或看串口）
ssh root@<imx93-ip>  # 默认密码: root 或 见手册

# 修复 eMMC bootloader
dd if=/run/media/boot/imx-boot*.bin of=/dev/mmcblk0 seek=66 bs=512 conv=notrunc
# 或者重新从 SD 找到 boot binary

# 改回 eMMC 启动: SW1=0010
```

### 如果 SD 串口无任何输出
**诊断**: SD 卡没有被识别，可能是写入问题
**操作**: 重新用 dd 写入（不用 Etcher）

```bash
# Mac 上重新写 SD 卡
diskutil list  # 找 SD 设备号
diskutil unmountDisk /dev/diskX
sudo dd if=/Users/jason/Dev/aastar/AirAccount/LF_v6.6.36-2.1.0_images_FRDM_4.0_IMX93/imx-image-full-imx93frdm.rootfs.wic \
    of=/dev/rdiskX bs=4m status=progress
sync
diskutil eject /dev/diskX
```

**验证写入**:
```bash
# 重新插入后
diskutil list  # 应该看到 FAT32 + Linux Filesystem 两个分区
```

### 如果 SD 启动显示 kernel panic 根文件系统错误
**诊断**: WIC 写入不完整
**操作**: 重新完整写入 SD 卡（7.0GB 全部写完）

### 如果 SD 全部失败（卡坏/板子不识别）
**下一步**: JTAG via pyocd
```bash
pip3 install pyocd
pyocd list
```

---

## 环境配置参考

```bash
# Ubuntu VM 恢复步骤（VM 重启后）
sshpass -p ubuntu scp /tmp/uuu_aarch64 ubuntu@192.168.64.2:/tmp/uuu_new
sshpass -p ubuntu ssh ubuntu@192.168.64.2 'chmod +x /tmp/uuu_new'

# 串口监控（Mac，必须 raw 模式）
stty -f /dev/cu.usbmodem5B6D0044901 raw 115200 cs8 -cstopb -parenb -echo
nohup cat /dev/cu.usbmodem5B6D0044901 > /tmp/serial.log &

# uuu 刷机（仅当 SDPV 问题解决后才有用）
cd /mnt/images && sudo /tmp/uuu_new -v -b emmc_all boot.bin rootfs.wic.zst
# boot.bin 必须是 .bin 扩展名（symlink 到实际文件）
```

---

## 串口捕获的历史教训

| 错误 | 根因 | 修复 |
|------|------|------|
| screen 占着串口 | screen PID 没有清理 | `lsof /dev/cu.usbmodem*` → `kill -9 <pid>` |
| cat 收到 0 字节 | stty 不是 raw 模式（cooked 模式缓存到换行） | `stty raw` |
| heredoc 后台进程死亡 | SSH 会话退出时子进程被杀 | 用 `nohup cat ... &` |
| uuu 文件名错误 | SPL 崩溃时 SDPV 从来不出现，串口是诊断方法 | 先开串口，后跑 uuu |

---

## 刷机后恢复 KMS 服务

```bash
# 在板子 Linux 上执行
cd / && tar xzf /path/to/mx93-backup.tgz
systemctl enable --now kms-api cloudflared
```

备份在 Mac 上: `/Users/jason/mx93-backup/mx93-backup.tgz`  
包含: cloudflared 隧道凭证、kms.db、kms-api-server、TA、systemd 服务、WiFi 配置
