# 在 macOS 上离线读写 Linux SD 卡（ext4 rootfs）

> 场景：板子（DK2 / STM32MP157F-DK2、MX93 等）串口/USB/网络都连不上时，
> 直接把它的 microSD 拔到 Mac 上，**离线**改 rootfs —— 加 SSH 公钥、配 WiFi、重置密码等。
> macOS 原生不认 ext4（插卡弹「不认识」是正常的），本流程用 `debugfs` 绕过。
>
> 实战验证：2026-07-15 给 DK2（STM32MP157F-DK2 / DVT2）加公钥 + 配 WiFi 自连（公寓+学校）。
> 背景与坑详见 memory `hardware_dk2_stm32mp1_access`。

---

## 一次性准备

```bash
brew install e2fsprogs         # 得到 debugfs / e2fsck / dumpe2fs
```

**授予「完全磁盘访问」（Full Disk Access, FDA）** —— 新版 macOS 连 root 读裸盘都要它：
1. 系统设置 → 隐私与安全性 → 完全磁盘访问
2. 把**宿主终端 App** 加进去并打开开关（本机是 **Ghostty.app**；用别的终端就加那个）
3. 退出并重开该终端 App（FDA 才生效）

> ⚠️ **关键坑**：`osascript "... with administrator privileges"` **不继承 FDA**（它把命令扔到系统 helper 进程里跑，报 `Operation not permitted`）。
> 必须用下面的 **SUDO_ASKPASS + `sudo -A`**，让 sudo 在**终端进程树内**跑才继承 FDA。

**GUI 密码助手**（避免 sudo 在非交互 shell 里 "a terminal is required"）：
```bash
cat > /tmp/askpass.sh <<'X'
#!/bin/bash
osascript -e 'display dialog "sudo 密码：" default answer "" with hidden answer' -e 'text returned of result' 2>/dev/null
X
chmod +x /tmp/askpass.sh
# 之后所有 sudo 都写成： SUDO_ASKPASS=/tmp/askpass.sh sudo -A <cmd>
```

---

## 核心工具变量

```bash
DBG=/opt/homebrew/opt/e2fsprogs/sbin/debugfs
FSCK=/opt/homebrew/opt/e2fsprogs/sbin/e2fsck
S(){ SUDO_ASKPASS=/tmp/askpass.sh sudo -A "$@"; }   # 提权包装
```

---

## 步骤

### 1. 找到卡和 rootfs 分区
```bash
diskutil list external physical
```
STM32MP1 SD 布局：`s1–7`=bootloader，`s8`=bootfs，`s9`=vendorfs，
**`s10`(~2.5G)=rootfs**（有 /etc /home /bin），`s11`=userfs。假设卡是 `disk6`。

确认哪个是 rootfs：
```bash
S $DBG -R "ls /" /dev/disk6s10       # 应看到 bin boot etc home lib root sbin ...
```

### 2. 卸载整卡释放 macOS 锁（不弹出、不抹除）
```bash
diskutil unmountDisk /dev/disk6
```

### 3. 读文件（诊断）
```bash
S $DBG -R "cat /etc/passwd" /dev/disk6s10 | grep '^root:'   # 注意 root home 可能是 /home/root
S $DBG -R "cat /etc/shadow" /dev/disk6s10 | grep '^root:'   # 字段2: 空=无密码 / ! * =锁 / $=hash
S $DBG -R "ls -l /home/root/.ssh" /dev/disk6s10
```

### 4. 写文件（改配置）

> ⚠️ **顺序**：卡若是从**运行中的板子拔的**=ext4 脏（journal 未回放）。
> **必须先 `e2fsck -fy` 清理，再写**，否则 journal 回放可能冲掉你新加的目录项。
> **e2fsck 必须用缓冲块设备 `/dev/disk6s10`（不是 `/dev/rdisk6s10`）** ——
> 裸 rdisk 写超级块要扇区对齐，会 `unable to set superblock flags`，journal 清不掉。

```bash
# a) 写前清理（脏卡会 recovering journal + 修错，跑到退出码 0/1）
S $FSCK -fy /dev/disk6s10

# b) debugfs 批量写（喂命令脚本）—— 例：加 SSH 公钥
cat ~/.ssh/id_ed25519.pub > /tmp/authkeys.txt
S $DBG -w /dev/disk6s10 <<'CMDS'
mkdir /home/root/.ssh
write /tmp/authkeys.txt /home/root/.ssh/authorized_keys
set_inode_field /home/root/.ssh mode 040700
set_inode_field /home/root/.ssh uid 0
set_inode_field /home/root/.ssh gid 0
set_inode_field /home/root/.ssh/authorized_keys mode 0100600
set_inode_field /home/root/.ssh/authorized_keys uid 0
set_inode_field /home/root/.ssh/authorized_keys gid 0
quit
CMDS

# c) 写后再 fsck 修计数（debugfs 写常留 free blocks/inodes count 不一致 + 收尾报
#    "ext2fs_close: Invalid argument"，属正常，fsck 会 fix）
S $FSCK -fy /dev/disk6s10        # 反复跑到退出码 0 = 干净
```

**debugfs 常用写命令**：
- `write <本地文件> <卡内路径>` — 写入文件
- `mkdir <路径>` — 建目录
- `symlink <链接路径> <目标>` — 建符号链接（如 systemd enable：
  `symlink /etc/systemd/system/multi-user.target.wants/xxx.service /lib/systemd/system/xxx.service`）
- `set_inode_field <路径> mode 0100600` / `uid 0` / `gid 0` — 设权限/属主
- `rm <路径>` — 删文件

### 5. 验证 + 安全弹出
```bash
S $DBG -R "cat /home/root/.ssh/authorized_keys" /dev/disk6s10   # 读回确认
S $FSCK -fy /dev/disk6s10        # 最终确认退出码 0
sync; diskutil eject /dev/disk6  # 刷盘 + 安全弹（务必，否则缓冲写没落盘）
```

---

## 实战配方

### 配方 A：加 SSH 公钥（解「无密码账户 + sshd 禁密码登录」死锁）
见步骤 4b。root home 注意是 `/home/root`。加完插回板子，`ssh -i ~/.ssh/id_ed25519 root@<ip>` 免密进。

### 配方 B：配 WiFi 开机自连（systemd-networkd + wpa_supplicant@wlan0）
「重启就丢」根因通常是：无持久 wpa 配置 / `wpa_supplicant@wlan0` 没 enable / 无 `wlan0.network`。三样都补：

```bash
# 1) wpa 配置（多网络带优先级）
cat > /tmp/wpa-wlan0.conf <<'EOF'
ctrl_interface=/var/run/wpa_supplicant
ctrl_interface_group=0
update_config=1
network={
	ssid="<SSID>"
	psk="<PSK>"
	key_mgmt=WPA-PSK
	scan_ssid=1
	priority=50
}
EOF
# 2) DHCP
cat > /tmp/wlan0.network <<'EOF'
[Match]
Name=wlan0
[Network]
DHCP=yes
[DHCPv4]
RouteMetric=20
EOF
# 3) 写入 + enable 服务
S $DBG -w /dev/disk6s10 <<'CMDS'
mkdir /etc/wpa_supplicant
write /tmp/wpa-wlan0.conf /etc/wpa_supplicant/wpa_supplicant-wlan0.conf
write /tmp/wlan0.network /etc/systemd/network/wlan0.network
symlink /etc/systemd/system/multi-user.target.wants/wpa_supplicant@wlan0.service /lib/systemd/system/wpa_supplicant@.service
set_inode_field /etc/wpa_supplicant/wpa_supplicant-wlan0.conf mode 0100600
set_inode_field /etc/wpa_supplicant/wpa_supplicant-wlan0.conf uid 0
set_inode_field /etc/wpa_supplicant/wpa_supplicant-wlan0.conf gid 0
quit
CMDS
S $FSCK -fy /dev/disk6s10
```
先确认镜像有 WiFi 固件：`S $DBG -R "ls /lib/firmware/brcm" /dev/disk6s10`（找 `brcmfmac*.bin`）。
接口名一般 `wlan0`（brcmfmac）；老 Marvell 板是 `mlan0`。

---

## 排错

| 现象 | 原因 / 解法 |
|---|---|
| `Operation not permitted`（即使 root） | 缺 FDA，或用了 osascript-admin（不继承 FDA）→ 改 SUDO_ASKPASS + `sudo -A` |
| `unable to set superblock flags` | e2fsck 用了 `/dev/rdisk...`（裸盘不对齐）→ 改缓冲设备 `/dev/disk...` |
| `ext2fs_close: Invalid argument`（debugfs 写后） | 正常，计数不一致，再 `e2fsck -fy` 修 |
| 反复 `recovering journal` | journal 没清 → 用缓冲设备 e2fsck 跑到退出码 0 |
| sudo `a terminal is required` | 非交互 shell → 用 SUDO_ASKPASS GUI 助手 |
| 读到的文件是空/旧 | debugfs 不回放 journal；先 e2fsck 清 journal 再读关键文件 |
