# 带外串口控制台（Out-of-Band Console via mac-mini）

> 目的：当板子**断网**（WiFi 掉、Tailscale 隧道挂、DNS/门户变更）时，仍能救板。
> 办法：把板子的 USB 串口常挂在一台**常在线的 mac-mini**（它在 Tailscale 上）上，
> 串口用 `tmux + screen` 挂成**常驻、可远程 attach、断线自动重连**的控制台。
> 我（或任何运维者）ssh 进 mac-mini → `tmux attach` → 直接进板子串口，网络挂了也不怕。
>
> 更新：2026-07-17

---

## 为什么需要它

Tailscale VPN 只能连「自己也在线」的节点。板子一旦丢了网络，`tailscaled` 就掉线，
**VPN 也进不去**——正是最需要救的时候够不着。串口是**带外**通道：只要板子通电，
串口就活，跟网络状态无关。mac-mini 当跳板（它自己联网正常），就把这条带外通道
桥接到了 Tailscale 上。

真实案例（2026-07-17）：板A 在学校 IoT 网上 Tailscale 掉线 3 小时，ping/ssh 全超时，
无法判断是断电还是网络侧变更，也无法从它扫 DK2。若当时有本方案，一条命令就进串口看清了。

---

## 口位对照（关键！插错口拿不到串口）

| 板 | 型号 | 串口控制台在哪 | 接 mac-mini | 波特率 |
|----|------|---------------|-------------|--------|
| **板A** | MX93 / FRDM-IMX93 | 板载 MCU-Link 调试口，**走 USB-C** | ✅ USB-C 直连 | 115200 |
| **板B** | MX93B / FRDM-IMX93 / kms1 | 同上，**走 USB-C** | ✅ USB-C 直连 | 115200 |
| DK2 | STM32MP157F-DK2 / DVT2 | **micro-USB CN11（ST-LINK VCP）** | ⚠️ **不是 type-C**！两个 type-C 是「电源 + OTG」 | 115200 |
| 板C（未来） | 预计 MX93 同款 | 同板A | ✅ USB-C | 115200 |

- **板A / 板B / 板C = FRDM-IMX93 家族**，串口走 USB-C 调试口 → 本方案两个脚本直接适用。
- **DK2 是例外**：串口在 micro-USB CN11，且 DK2 会**带去学校移动**，不常驻 mac-mini →
  DK2 救板靠**现场插 micro-USB 数据线**（不是充电线），不进本方案。

---

## 部署拓扑与时间线

- 现在：板A 在学校/机房；板B 在泰国公寓；DK2 带去学校临时用 ~1 个月。
- ~1 个月后：**板C 到货，挂机房，和板A 同处**。到时把 `board-a` 脚本复制成 `board-c`
  （改 `BOARD="board-c"` + 确认 `DEV_GLOB`），同一台机房 mac-mini 可同时挂 A、C 两个会话。
- 每个脚本在**与该板同处一地的 mac-mini** 上跑。板A/板C 一台机房 mac-mini；板B 一台公寓
  mac-mini。**USB 线就一两米，板子和 mac-mini 必须物理挨着**——会移动的 DK2 覆盖不了。

---

## 前置条件（在 mac-mini 上，一次性）

1. **装 tmux**：`brew install tmux`（screen 用系统自带的即可）。
2. **给远程运维者 ssh 权限**：把运维者（我的 Mac / jason）的公钥加入 mac-mini 的
   `~/.ssh/authorized_keys`。否则只有本机能 attach，远程帮不上。
   > 当前 `mac-mini-nicolas` 登录用户是 `nicolusshuaishuai`，需要它给我加公钥，我才能远程 attach。
3. **开机自启（建议）**：把 `up` 挂到登录项 / `launchd`，mac-mini 重启后自动拉起控制台。

---

## 用法

```bash
# 先核对串口设备名(把板子 USB 插好、通电后)
./mac-mini-console-board-a.sh list

# 启动常驻控制台(幂等,已在跑就只打印 attach 命令)
./mac-mini-console-board-a.sh up

# 本机附着(退出附着但不杀会话: 先 Ctrl-b 再 d)
./mac-mini-console-board-a.sh attach

# 停掉
./mac-mini-console-board-a.sh down
```

**远程附着（从我的 Mac，经 Tailscale）**：

```bash
ssh mac-mini -t '/opt/homebrew/bin/tmux attach -t board-a'   # Apple Silicon mac-mini
ssh mac-mini -t '/usr/local/bin/tmux  attach -t board-a'     # Intel mac-mini
```

- 串口输出**同时落盘**到 `~/airaccount-console-logs/board-a.log`（哪怕没人 attach 也在记），
  板子半夜掉线的启动日志事后可翻。
- 板子重启/USB 拔插导致串口消失，脚本会**自动等待并重连**，tmux 会话一直在。

---

## 键位备忘（tmux + screen 双层，前缀键不冲突）

| 想干嘛 | 按键 |
|--------|------|
| 退出附着、保留会话 | `Ctrl-b` 然后 `d`（tmux detach）|
| 给板子敲命令 | 直接打字即可（串口自动 root 登录）|
| 退掉当前 screen 触发重连 | `Ctrl-a` 然后 `k` |

> tmux 前缀是 `Ctrl-b`，screen 前缀是 `Ctrl-a`，两者不撞。

---

## 安全提示

这些板的串口**自动 root 登录**。常挂在 mac-mini 上 = **谁能进 mac-mini，谁就有板子的 root shell**
（绕过 SSH key/密码）。TEE（TrustZone）里的私钥仍安全，但**节点 OS 完全暴露**。
仅限**可信物理环境**（家/机房）使用；mac-mini 本身的账号与磁盘加密要看好。

---

## 局限

- 只覆盖**与 mac-mini 同处的固定板**。会移动的板（DK2）靠现场 micro-USB。
- USB 串口只能被一个进程独占；本方案用 tmux 里单个 screen，多人 attach 共享同一 tmux，
  不会互抢 tty。若之前有人手动 `screen` 占了口，脚本会重连失败并重试——先 `down` 清干净。

---

## 文件

- `mac-mini-console-board-a.sh` — 板A(MX93) 控制台
- `mac-mini-console-board-b.sh` — 板B(MX93B/kms1) 控制台
- 板C 到货后：`cp mac-mini-console-board-a.sh mac-mini-console-board-c.sh`，改 `BOARD` 与 `DEV_GLOB`
