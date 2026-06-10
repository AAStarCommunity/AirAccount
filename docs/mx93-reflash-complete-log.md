# MX93 FRDM-IMX93 完整刷机决策日志

> 持续更新中。记录从根因到最终修复的每一步决策、假设、尝试和教训。

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
  - 不是刷机口

J2 (i.MX93 直接 USB, 下面的 USB-C):  
  - 刷机口，SDPS/SDPV/FB 协议
  - PID 0x014e (ROM/SDPS) → 0x0151 (SPL/SDPV) → 0x0152 (U-Boot/FB)

SW1 DIP:
  - 0001 (label 1 ON) = SDPS USB 下载模式
  - 0010 (label 2 ON) = eMMC 启动
```

---

## 尝试 1: Ubuntu VM uuu 1.4.193（系统默认版本）

**日期**: 2026-06-07 开始  
**假设**: 系统 uuu 能找到 i.MX93 设备并启动 SDPS  
**配置**:
- Ubuntu VM (UTM, Apple Silicon, aarch64)
- J2 通过 UTM SPICE 转发到 VM
- 脚本: 各种 .lst 文件

**结果**: `Wait for Known USB Device Appear...` → 永远卡住  
**调试过程**:
1. `uuu -V` 输出里没有 MX93/0x014e 条目
2. 只有: `SDPS: MX8ULP 0x1fc9 0x014b`
3. 确认: uuu 1.4.193 的内置设备表不包含 i.MX93

**结论**: uuu 1.4.193 不支持 i.MX93，需要升级

---

## 尝试 2: 下载 uuu 1.5.243 aarch64

**假设**: 1.5.243 支持 i.MX93  
**操作**: `curl -kL` 下载（需要 `-k` 跳过 GitHub CDN SSL 证书超时问题）

**踩坑**: `/mnt/images/uuu_new` 被下载为空文件（9 bytes ASCII），是 WebDAV 写问题  
**解决**: 下载到 `/tmp/uuu_new`（本地 tmpfs），SCP 方式传输

**验证**: `uuu -V | grep MX93` → 出现 `SDPS: MX93 0x1fc9 0x014e` ✅

---

## 尝试 3: uuu 1.5.243 + .lst 脚本文件（SDPS only）

**假设**: 只需要 SDPS 阶段就能让板子进入 SPL USB 模式  
**配置**:
```
uuu_version 1.5.243
SDPS: boot -scanterm -f /mnt/images/imx-boot-...-flash_singleboot_gdet_auto -scanlimited 0x800000
```

**结果 A**: `Error: fail open file: >/tmp//mnt/images/imx-boot-...`  
**根因 A**: uuu 把脚本目录 (`/tmp/`) 拼接到绝对路径前 → bug  
**质疑**: 这是 uuu 的设计还是 bug？（已确认是 bug，绝对路径也被拼接）

**修复**: 用 `-b emmc_all` 内置脚本，文件路径基于 CWD 而非脚本目录

**结果 B** (用 `-b emmc_all`):
- SDPS: 100% 成功，4.518 秒
- 然后等待 SDPV (0x0151) → **永远等不到**

**质疑 1**: 是否是 `-scanterm` 导致阻塞？（J1 未连 Ubuntu VM）  
→ 否，`-b emmc_all` 内置脚本里有 `-scanterm` 但 SDPS 仍然成功

**质疑 2**: 是 UTM 没有转发 SDPV 设备？  
→ 部分正确，但不是根本原因（见尝试 4）

---

## 尝试 4: 串口分析（关键转折点）

**方法**: J1 连 Mac，`screen /dev/cu.usbmodem5B6D0044901 115200` 观察 SPL 输出  
**踩坑**: screen 进程占着串口 (`lsof /dev/cu.usbmodem...` 找到 PID 74577)  
**修复**: `kill -9 74577`，然后 `stty -f ... 115200 cs8 -cstopb -parenb -echo`

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
[重置循环...]
```

**分析**:
- SPL 确实在运行（DDR 3733MHz 初始化成功）
- 在 `M33 prepare ok` 之后、`Trying to boot from USB SDP` **之前**就重置
- 重置循环 → SDPV 设备 (0x0151) **从来没有枚举**
- UTM 无法转发 0x0151 的真实原因：**板子根本没有枚举出这个设备**

**推翻之前假设**: 问题不是 UTM 转发，而是 SPL 在 USB 初始化前就崩溃了

---

## 尝试 5: macOS 本地 uuu_mac_arm

**假设**: 绕过 UTM，直接在 Mac 运行，不需要三阶段设备转发  
**下载**: `uuu_mac_arm` 1.5.243 (Mach-O arm64, 1.9MB, NXP GitHub)  
**依赖**: 动态链接到 Homebrew libusb (`/opt/homebrew/opt/libusb/lib/libusb-1.0.0.dylib`)

**第一次运行** (J2 仍在 UTM 里):
```
HID(W): LIBUSB_ERROR_IO (-1)
```

**第二次运行** (从 UTM 断开 J2，直连 Mac):
```
1:1-123EBC50 1/ 1 [HID(W): LIBUSB_ERROR_TIMEOUT (-7)] SDPS: boot...
```

**根因**:
- macOS IOHIDFamily 内核驱动独占 USB HID 设备
- libusb 无法向已被 IOHIDFamily 占用的 HID 端点写数据
- `LIBUSB_ERROR_TIMEOUT` = HID OUT 报文发出但 10s 内无 ACK

**质疑**: 能绕过 IOHIDFamily 吗？
- 选项 A: SIP disable + unload kext → 不可接受（安全风险）
- 选项 B: 用 Python hid 模块（macOS HID Manager）→ 需要实现完整 SDPS 协议，复杂
- 选项 C: 修改 libusb 用 IOUSBHost.framework → 需要重新编译 uuu
- 选项 D: IOKit personality 注入 → 需要 SIP disable

**结论**: macOS 上 libusb + NXP HID SDPS = 无解（不修改 kernel/SIP）**不再尝试**

---

## 尝试 6: 修改 UTM 配置（0x0151/0x0152）

**假设**: 找到 UTM .utm 配置文件，添加 USB 设备规则  
**搜索**:
- UTM group container: `~/Library/Group Containers/WDNLXAD4W8.com.utmapp.UTM/` → sandbox 无法访问
- `find ~ -name "*.utm"` → 未找到
- UTM AppleScript: `path of virtual machine` → 错误 -1728

**结论**: UTM 配置在 sandbox 内，无法在外部修改  
**教训**: UTM SPICE USB 转发是动态的，每次设备重枚举需要用户手动点击 → **不可自动化**，**不再尝试 UTM 方案**

---

## 尝试 7: flash_singleboot（无 GPIO 检测）

**假设**: `gdet_auto` 变体在 SDPS RAM boot 时 GPIO 状态不对 → SPL 崩溃  
**依据**:
- `flash_singleboot` = 无 GPIO 检测
- `flash_singleboot_gdet` = 有 GPIO 检测（检测 eMMC 还是 SD）
- `flash_singleboot_gdet_auto` = 自动 GPIO 检测
- 在 SDPS（USB ROM boot）模式下，GPIO 可能读到意外状态

**质疑这个假设**:
1. gdet_auto 曾在 eMMC 启动模式下工作 ✓
2. 但 eMMC 启动 ≠ SDPS RAM boot（不同的 GPIO 上下文）
3. SPL 崩溃点（M33 prepare ok 之后）对应 USB PHY init 或 ELE init

**备选假设（更可能）**: SPL 崩溃是 ELE TRNG 初始化问题（不是 gdet）

**当前状态**: 进行中，等待串口输出验证

---

## 未来尝试方向（待验证）

### 方案 A: 从 SD 卡启动（最干净）

```bash
# Mac 上写 SD 卡
diskutil list  # 找到 SD 卡设备
# zstd -d wic.zst -o - | sudo dd of=/dev/rdiskX bs=4m
# 或直接用 SD 卡 WIC
```

**前提**:
1. 有 microSD 卡（FRDM-IMX93 有 microSD 槽）
2. SW1 对应 SD 启动的拨码设置（需查手册）

**优势**: 完全不需要 USB flashing，从 SD 启动后可直接 `dd` 修复 eMMC  
**待确认**: FRDM-IMX93 SD 启动的 SW1 设置

### 方案 B: 解决 SPL 崩溃根因

**调试方法**:
1. 连接 JTAG (J1 有 OpenSDA/JLink)
2. 在 M33 prepare ok 之后设断点
3. 查看 PC/LR/fault_reason

**可能崩溃原因**:
- ELE TRNG fault（最可能，与之前 CreateKey bug 同根）
- USB PHY init hang
- Watchdog 到期

### 方案 C: 修复 eMMC bootloader 而不是完整刷机

```bash
# 如果能进入任何 Linux shell（SD 卡启动），只需写 bootloader 区
dd if=imx-boot-imx93frdm-sd.bin-flash_singleboot_gdet_auto \
   of=/dev/mmcblk0 seek=66 bs=512
# 不需要刷完整 7GB WIC
```

---

## 环境配置参考

```bash
# Ubuntu VM 恢复步骤（VM 重启后）
sshpass -p ubuntu scp /tmp/uuu_aarch64 ubuntu@192.168.64.2:/tmp/uuu_new
sshpass -p ubuntu ssh ubuntu@192.168.64.2 'chmod +x /tmp/uuu_new'
# sudoers 配置（已持久化）: /etc/sudoers.d/ubuntu-nopasswd

# J1 串口监控（Mac）
stty -f /dev/cu.usbmodem5B6D0044901 115200 cs8 -cstopb -parenb -echo
cat /dev/cu.usbmodem5B6D0044901  # 或 screen ...

# uuu 刷机命令（Ubuntu VM，J2 连接后）
cd /mnt/images && sudo /tmp/uuu_new -b emmc_all \
  imx-boot-imx93frdm-sd.bin-flash_singleboot_gdet_auto \
  imx-image-full-imx93frdm.rootfs.wic.zst
```

---

*最后更新: 2026-06-10*
