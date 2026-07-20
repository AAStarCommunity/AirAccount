<!-- Created: 2026-07-20 -->
# DK2 / node2 / dvt2 工具箱

> **DK2 = STMicro STM32MP157F-DK2**（armv7l / Cortex-A7 32-bit · 512MB · 板载 WiFi），
> Mycelium DVT 门限网络的 **node2 / dvt2**（独立本地 keystore 节点）。
> 本目录 = 通过 **micro-USB 串口** 访问、配置、初始化 DK2 的可复用工具 + 流程。

---

## 1. 怎么找到 DK2 的串口（micro-USB）

- DK2 串口 = **micro-USB 插 CN11**（板载 ST-LINK/V2-1 VCP）。⚠️**必须数据线**——充电线没 D+/D‑ 不出串口。
- 板子通电 + ST-LINK 枚举 → macOS 出 `/dev/cu.usbmodemXXXX`；`ioreg` 里可见 `STM32 STLink`（STMicroelectronics）。
- 串口 **自动 root 登录**（无密码，`root@stm32mp1`）。

```bash
./dk2.sh find              # 自动找到 DK2 串口设备路径(排除板B的 5B6D MCU-Link)
./dk2.sh console           # screen 交互(退出: Ctrl-a k y)
./dk2.sh run "<命令>" [秒]  # 跑命令读回显(非交互,脚本用)
./dk2.sh put <本地> <远程>  # 串口传【小】文件(base64,config 级;大文件走网络)
# 覆盖自动查找:  DK2_SERIAL=/dev/cu.usbmodemXXX ./dk2.sh ...
```

底层是 `dk2-serial.py`（pyserial;`pip3 install pyserial`）。串口 115200，`run` 用 marker 提取纯输出、去命令回显。

---

## 2. 配学校 WiFi 自动连接（已做）

DK2 的家在**学校/机房**（与板 A 同网 `@JumboPlusIoT`）。一条命令把它配成开机自动连：

```bash
./dk2-wifi-school.sh
```

它（全走串口，**不用拔卡**）：
1. **去掉公寓 MAC 克隆**（删 `macclone.conf` drop-in）→ DK2 用**真 MAC `24:cd:8d:4e:4f:28`**（学校 CMU 门户按真 MAC 注册的那个）。
2. 写 `wpa_supplicant-wlan0.conf`（`@JumboPlusIoT5GHz` + `@JumboPlusIoT`，**PSK/SSID 从 `~/Dev/.env` 读**：`DK2_WIFI_SSID_SCHOOL` / `DK2_WIFI_PSK_SCHOOL` / `DK2_WIFI_MAC`——不硬编码、不入库）。
3. 装 `wifi-up.sh` + `wifi-autoconnect.service`（接口自探测 + `wpa -B` + dhcp + 黑匣子）并 enable。

> **公寓这边它不会连**（学校配置 + 真 MAC，公寓 UniFi 门户没授权它）。到学校/机房上电即自动上线。
> 想在公寓也上线（为了 deploy）→ 在 UniFi 后台授权 DK2 真 MAC `24:cd`。

---

## 3. dvt2 初始化（下一步，需 DK2 联网）

DK2 联网后（学校，或授权后公寓）：
1. **SSH 起来**（现在 `sshd` inactive，串口 enable 一下）。
2. **交叉打包 armv7l DVT bundle**（Mac 上打，板上永不 build —— 512MB 跑不动 build）→ 传 DK2 解包。
   见 `YetAnotherAA-Validator/deploy/dk2/`（DVT 仓 CC-32，armv7 专用脚本）。
3. **本地 BLS keystore + operator key**（独立模式，非 TEE；密码走 tmpfs，勿用仓库测试键）。
4. `dvt2.env`（validator `0x539B96…`、PORT 4002、`MemoryMax=320M`、`--max-old-space-size=256`）。
5. systemd 起 + `/health` 绿 + **链上 `registerWithProof`**（dvt2 自己的 operator/pubkey）。
> 完整步骤见 `../../docs/go-live-dvt3-dk2-plan.md` §3。串口太慢传不了 48M bundle → 这步必须走网络。

---

## 4. 硬件/接入踩坑（速查）

- **两个 USB-C 是电源 + OTG，都不是串口**；串口在 **micro-USB CN11**。
- 直连登录死锁：root 无密码 + sshd 默认禁密码 + 无 key → 只能串口(自动 root)或先加 key。
- root home = **`/home/root`**（不是 `/root`）。
- armv7l:DVT 必须 `linux-armv7l` 产物（arm64 推上去 `Exec format error`）。
- 关联:`../../docs/dk2-school-wifi/`（旧的拔卡离线改法,现有串口后基本不需要）、
  `../../docs/go-live-dvt3-dk2-plan.md`（node2/node3 上线计划）。

## 文件
- `dk2.sh` — 串口访问(find/console/run/put)
- `dk2-serial.py` — pyserial 底层 helper
- `dk2-wifi-school.sh` — 配学校 wifi 自启(PSK 从 env)
