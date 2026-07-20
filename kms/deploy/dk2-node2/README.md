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
  > ⚠️ **安全**（#187 review）：DK2 是 keystore 节点,串口=物理接触=免密 root=可读密钥。dev 板默认如此、需物理接触,当前可接受;**生产化前**应硬化(串口登录加密码 / 禁 auto-login)。

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

## 3. dvt2 初始化 runbook（✅ 2026-07-20 已跑通上链）

DK2 联网后（学校 `@JumboPlusIoT` → `10.82.8.213`，或 Tailscale `dk2-node2`）全程走网络（串口太慢传不了 48M bundle）。**每一步 + 踩的坑**：

```bash
# ① SSH：DK2 用 dropbear(不是 openssh):  systemctl start dropbear.socket
# ② Mac 交叉打 armv7l bundle(板上永不 build,512MB 不够):
cd YetAnotherAA-Validator && ./deploy/dk2/build-bundle-dk2.sh   # → aastar-dvt-<ver>-linux-armv7l.tar.gz (~47M, node armv7l)
# ③ 传 + install(⚠️坑:/tmp 是 tmpfs=RAM,会满 → bundle 挪 rootfs + TMPDIR 指 rootfs):
scp bundle root@<dk2>:/home/root/dvt-install/
ssh <dk2> 'cd deploy/dk2 && TMPDIR=/home/root/dvt-install ./install-dk2.sh /home/root/dvt-install/<bundle> node2'
# ④ BLS keystore(本地加密,非 TEE):
node scripts/gen-node-state.mjs dvt2                                    # → node_state.json (明文 key)
KDF=pbkdf2 NODE_KEY_PASSPHRASE=$DVT2_SECRET node scripts/encrypt-node-key.mjs node_state.json  # ⚠️要传文件参数!
scp node_state.json root@<dk2>:/opt/aastar-dvt/state/node2/node_state.json  # chmod 600
# ⑤ node2.env:填 ETH_RPC_URL + NODE_KEY_PASSPHRASE(远程节点存 node2.env 600 自启,非 dvt3 的 tmpfs 手动解锁);start:
ssh <dk2> 'systemctl start aastar-dvt@node2 aastar-dvt-health@node2.timer'   # /health :4002 绿(armv7 启动慢,poll)
# ⑥ 链上注册(复用 dvt3 脚本,4 坑解法都在:scalar %r / gToken 实链 0x4c09aE57 / operator 先持久化):
cd aastar-sdk   # funder=PRIVATE_KEY_JASON(.env.sepolia);先 DRY_ONLY=1 验,再真跑
DVT3_KEYSTORE_JSON=<dvt2 node_state> DVT3_SECRET=$DVT2_SECRET DVT3_OPERATOR_PK=<fresh> PRIVATE_KEY_JASON=<funder> \
  SEPOLIA_RPC_URL=<rpc> pnpm exec tsx tests/regression/onchain-evidence/dvt3-register.ts
#   → registerWithProof + stake 30 GToken → isRegistered=true。operator key 务必持久备份(丢=管不了质押)。
```

> **op-sec**（#189 review）：① `gen-node-state.mjs` 产的明文 `node_state.json` + encrypt 后的 `.bak` 有落盘窗口 —— 加密后用 `shred -u`（非 `rm`，rm 可恢复）抹掉明文。② passphrase/私钥别**内联** env 传（`ps -E` / `/proc/PID/environ` 可见）—— 走文件/stdin，或至少确保只在 root-only 环境跑。

**生产硬化**（远程节点要稳）：
- **时钟**：学校网封 NTP 123 → `timesyncd` 同步不了、无 RTC → 用 host 时间手动 `date -u -s`（重启会再偏，靠 timesyncd 有网时纠）。
- **swap**：这镜像**无 `swapon/mkswap`、无 zram 模块** → 加不了 swap；DVT 靠 `MemoryMax=320M + --max-old-space-size=224` 在 512MB 内跑（idle ~28MB free,紧但活）。
- **可达**：Tailscale userspace 模式(缺 iptables) `--tun=userspace-networking + up --ssh`,做成 `tailscaled.service` enabled → 随地 `ssh root@<tailscale-ip>`。

> 对照 dvt3(板B)：`../../docs/dvt3-independent-node-onboarding.md`。dvt2 = 同套独立节点流程,armv7 + 512MB 特调。

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
- `wifi-up.sh` / `wifi-autoconnect.service` — 装到 DK2 的部署产物(接口自探测+wpa+dhcp+黑匣子 / systemd 自启)
