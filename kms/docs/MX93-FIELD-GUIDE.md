# MX93 现场操作手册 / Field Guide

> NXP FRDM-IMX93 (aarch64 Cortex-A55) · AirAccount KMS v0.27.3+
> 板子完全独立运行，不依赖 Mac

---

## 1. 硬件连接

### 必插线缆
| 线缆 | 接口 | 说明 |
|------|------|------|
| 电源 | USB-C 电源口 | 5V/3A 以上 |
| Debug USB | USB-C debug 口（靠近角落） | 串口控制台，接 Mac/PC |

> 凭据一律不存 repo，见下方说明。

### 板子启动后自动做的事
```
wpa_supplicant@mlan0.service  → WiFi 连接
kms-api.service               → KMS API (port 3000)
cloudflared.service           → Cloudflare Tunnel → kms.aastar.io
tailscaled.service            → Tailscale VPN → 远程 SSH 入口
```
**无需任何手动操作**，约 30 秒后 `https://kms.aastar.io/health` 可访问。

---

## 2. 串口连接（最差情况 / 调试用）

### 找串口设备

**macOS**:
```bash
ls /dev/cu.usbmodem*
# 通常是两个：
#   /dev/cu.usbmodem5B6D0044901  ← Linux 控制台（用这个）
#   /dev/cu.usbmodem5B6D0044903  ← M33 核心（不用）
```

**Linux**:
```bash
ls /dev/ttyUSB* /dev/ttyACM*
# 通常是 /dev/ttyUSB0 或 /dev/ttyACM0
```

### 连接命令
```bash
# macOS / Linux
screen /dev/cu.usbmodem5B6D0044901 115200
# 或
python3 -c "
import termios,tty,select,sys,time,os
fd=open('/dev/cu.usbmodem5B6D0044901','rb+',buffering=0)
# ...（见 scripts/mx93/serial-run.py）
"

# 退出 screen：Ctrl+A → K → Y
```

### 登录
```
login: root
password: <串口密码，见内部凭据管理>
```
> 密码不存 repo。

---

## 3. SSH 连接

### 3.1 Tailscale（首选，全球任意网络可用）

板子已安装 Tailscale，加入账号 `mushroomjiao82@`，**固定 IP `100.121.187.3`**，不随 DHCP 变化。

```bash
# Mac 上直接：
ssh mx93
# 等价于：
ssh root@100.121.187.3 -i ~/.ssh/id_ed25519

# ~/.ssh/config 已配置：
# Host mx93
#   HostName 100.121.187.3
#   User root
#   IdentityFile ~/.ssh/id_ed25519
#   StrictHostKeyChecking no
```

前提：Mac 和板子都在同一个 Tailscale 网络（`mushroomjiao82@`）。Mac 上运行 `tailscale status` 确认在线。

### 3.2 局域网 SSH（同一 WiFi 时）

```bash
# 扫网段找板子（CMU IoT 网段 10.82.8.0/22）
for ip in $(seq 1 254); do
  ( nc -z -w 1 10.82.8.$ip 22 2>/dev/null && echo "10.82.8.$ip OPEN" ) &
done; wait

ssh root@<IP>  # 公钥免密
```

---

## 4. WiFi 配置

### 当前配置文件位置（板子上）
```
/etc/wpa_supplicant.conf                                  ← 主配置
/etc/wpa_supplicant/wpa_supplicant-mlan0.conf             ← systemd 服务读取的（同步自主配置）
/etc/wpa_supplicant/action.sh                             ← 连接成功后自动 DHCP
```

### 当前已配置的网络（优先级从高到低）

> **注意：凭据不存 repo**。PSK/密码保存在板子 `/etc/wpa_supplicant.conf` 本地，查询方式见下方。

| SSID | 类型 | 密码来源 | 地点 |
|------|------|---------|------|
| `@JumboPlusIoT5GHz` | WPA-PSK | CMU IoT 设备入网门户申请，登板查 `/etc/wpa_supplicant.conf` | CMU Thailand 学校机房 |
| `@JumboPlusIoT` | WPA-PSK | 同上（2.4GHz 备用） | CMU Thailand |
| `@JumboPlus5GHz` | WPA2-EAP PEAP | EAP 账号（凭证待确认，暂不可用） | CMU |
| `eduroam` | WPA2-EAP PEAP | EAP 账号（凭证待确认，暂不可用） | CMU eduroam |
| `OneplusCMU3 5Ghz` | 开放 | 无密码（需门户登录） | CMU 区域 |
| `ChinaNet-AuRfsu-5G` | WPA-PSK | 家庭 WiFi，登板查配置 | 家（中国） |
| `ChinaNet-AuRfsu` | WPA-PSK | 家庭 WiFi，登板查配置 | 家（中国，2.4GHz） |

### 加新 WiFi
```bash
# 在板子上编辑（凭据只存板子本地，不要放 repo）
vi /etc/wpa_supplicant.conf

# 加一个 PSK 网络块：
# network={
#     ssid="新SSID"
#     psk="<PSK>"          ← 实际密码只填板子本地
#     key_mgmt=WPA-PSK
#     priority=40
# }

# 同步到 systemd 读取路径
cp /etc/wpa_supplicant.conf /etc/wpa_supplicant/wpa_supplicant-mlan0.conf

# 重启 WiFi 服务（不重启整个板子）
systemctl restart wpa_supplicant@mlan0.service
sleep 12
udhcpc -i mlan0 -n -q
```

### 在新学校/场地注册 IoT 设备
1. 确认板子 WiFi MAC：`ip link show mlan0 | grep ether`（`80:a1:97:50:21:2d`）
2. 找学校 IT/网络管理员填写 IoT 设备申请表单
3. 填写：MAC = `80:a1:97:50:21:2d`，名称 = `AirAccount-MX93`，点 GENERATE PSK
4. **PSK 只写到板子 `/etc/wpa_supplicant.conf`，不提交 repo**

---

## 5. 手动排查 WiFi

```bash
# 查看当前连接状态
wpa_cli -i mlan0 status

# 扫描附近网络
wpa_cli -i mlan0 scan; sleep 3; wpa_cli -i mlan0 scan_results

# 手动重连（不重启）
wpa_cli -i mlan0 reconnect

# 完全重启 WiFi 栈
killall wpa_supplicant 2>/dev/null
sleep 1
rm -f /var/run/wpa_supplicant/mlan0
wpa_supplicant -B -i mlan0 -c /etc/wpa_supplicant.conf -P /run/wpa_supplicant.pid
sleep 12
udhcpc -i mlan0 -n -q

# 查看 IP
ip addr show mlan0

# 测试外网
curl -o /dev/null -sw "HTTP_%{http_code}\n" --max-time 8 https://google.com
```

---

## 6. KMS 服务排查

```bash
# 服务状态
systemctl status kms-api.service
systemctl status cloudflared.service

# 本地健康检查
curl http://127.0.0.1:3000/health

# 重启 KMS
systemctl restart kms-api.service

# 重启 Cloudflare 隧道
systemctl restart cloudflared.service

# 查看日志
journalctl -u kms-api.service -n 50
journalctl -u cloudflared.service -n 30

# 验证公网
curl https://kms.aastar.io/health
```

---

## 7. 完整重置流程（板子无网络时）

```bash
# Step 1: 确认 WiFi 网卡存在
ip link show mlan0

# Step 2: 启动 wpa_supplicant（如果没跑）
systemctl start wpa_supplicant@mlan0.service
# 或手动：
killall wpa_supplicant 2>/dev/null; rm -f /var/run/wpa_supplicant/mlan0
wpa_supplicant -B -i mlan0 -c /etc/wpa_supplicant.conf -P /run/wpa_supplicant.pid

# Step 3: 等待连接（最多 30 秒）
for i in $(seq 1 30); do
  state=$(wpa_cli -i mlan0 status 2>/dev/null | grep wpa_state | cut -d= -f2)
  echo "$i: $state"
  [ "$state" = "COMPLETED" ] && break
  sleep 1
done

# Step 4: 拿 IP
udhcpc -i mlan0 -n -q

# Step 5: 恢复 KMS 隧道
systemctl restart cloudflared.service

# Step 6: 验证
curl https://kms.aastar.io/health
```

---

## 8. 系统信息速查

```bash
# 板子信息
hostname                              # imx93-11x11-lpddr4x-frdm
uname -m                              # aarch64
cat /etc/os-release | head -3

# KMS 版本
curl -s http://127.0.0.1:3000/health | python3 -m json.tool

# TEE 状态
ls /dev/tee*                          # /dev/tee0 存在 = OP-TEE 正常
systemctl is-active tee-supplicant

# WiFi MAC（填申请表用）
ip link show mlan0 | grep "link/ether"
# 固定：80:a1:97:50:21:2d

# 存储
df -h /
```

---

## 9. 远程部署（CA 二进制更新）

板子通过 Tailscale 可从任意地方 SSH，标准部署流程：

```bash
# Step 1：本地 Docker 交叉编译（Mac 上执行）
./scripts/mx93-build.sh ca
# 输出：build/mx93/kms-api-server（aarch64 ELF）

# Step 2：备份 + 上传 + 重启（一条命令）
BIN=/root/AirAccount/target/release/kms-api-server
scp build/mx93/kms-api-server mx93:/tmp/kms-api-server.new && \
ssh mx93 "
  cp \$BIN \${BIN}.bak-\$(date +%Y%m%d-%H%M%S) && \
  mv /tmp/kms-api-server.new \$BIN && \
  systemctl restart kms-api.service && \
  sleep 3 && curl -s http://127.0.0.1:3000/health
"

# Step 3：验证公网
curl https://kms.aastar.io/health
```

**回滚：**
```bash
ssh mx93 "ls /root/AirAccount/target/release/kms-api-server.bak-* | tail -1 | \
  xargs -I{} cp {} /root/AirAccount/target/release/kms-api-server && \
  systemctl restart kms-api.service"
```

---

## 10. 关键路径汇总

| 文件/路径 | 说明 |
|----------|------|
| `/etc/wpa_supplicant.conf` | WiFi 主配置（所有网络） |
| `/etc/wpa_supplicant/wpa_supplicant-mlan0.conf` | systemd 服务读取（与主配同步） |
| `/etc/wpa_supplicant/action.sh` | 连接成功后触发 DHCP |
| `/root/AirAccount/target/release/kms-api-server` | KMS 二进制 |
| `/etc/systemd/system/kms-api.service` | KMS systemd 单元 |
| `/root/.cloudflared/config.yml` | Cloudflare 隧道配置（含 SSH ingress） |
| `/var/lib/tailscale/tailscaled.state` | Tailscale 认证状态（持久化） |
| `/root/kms-check.sh` | 启动自检脚本 |
| `/var/log/kms-api.log` | KMS 服务日志 |
| `/var/log/kms-startup-check.log` | 启动自检日志 |

---

*最后更新: 2026-07-01 · MAC: 80:a1:97:50:21:2d · Tailscale: 100.121.187.3 · 公网: https://kms.aastar.io*
