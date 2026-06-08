# Cloudflare Tunnel：MX93 → kms.aastar.io

> 创建于：2026-06-07 | 适用：NXP FRDM-IMX93 部署

本文档说明如何将 MX93 板子上的 KMS API（`:3000`）通过 Cloudflare Tunnel 发布到 `kms.aastar.io`。

---

## 架构

```
Internet → Cloudflare Edge → Tunnel → MX93 板子:3000
                              ↑
                    cloudflared 运行在 MX93 板子上
```

cloudflared 在板子上直接连接 Cloudflare 网络（出站 443/7844），无需公网入站 IP。

---

## Step 1：Mac 上认证并创建 Tunnel

```bash
# 登录（会打开浏览器，在 Cloudflare Dashboard 点 Authorize）
cloudflared tunnel login
# 生成 ~/.cloudflared/cert.pem

# 创建新 tunnel（命名为 mx93-kms）
cloudflared tunnel create mx93-kms
# 输出：Created tunnel mx93-kms with id <TUNNEL_UUID>
# 生成 ~/.cloudflared/<TUNNEL_UUID>.json

# 配置 DNS CNAME（kms.aastar.io → tunnel）
cloudflared tunnel route dns mx93-kms kms.aastar.io
# 输出：Added CNAME kms.aastar.io which will route to this tunnel
```

---

## Step 2：准备 cloudflared aarch64 二进制

> **注意**：Yocto MX93 没有 `/usr/local/bin`，安装路径应为 `/usr/bin/cloudflared`。

### 推荐：直接在板子上 wget（板子有出站 WiFi）

WiFi AP 隔离导致 Mac 无法直连板子，但板子能访问互联网。通过串口执行：

```bash
# 在板子上（通过串口）
wget -q -O /tmp/cloudflared \
  'https://github.com/cloudflare/cloudflared/releases/download/2026.5.2/cloudflared-linux-arm64' &
# 文件约 36MB，约 5 分钟（WiFi 约 1MB/s）

# 下载完成后安装
ls -lh /tmp/cloudflared  # 确认 ~36MB
mv /tmp/cloudflared /usr/bin/cloudflared
chmod +x /usr/bin/cloudflared
cloudflared --version
```

### 备选：通过串口 base64 传输（非常慢，不推荐）

```bash
# 在 Mac 上下载
CFVER=$(curl -s https://api.github.com/repos/cloudflare/cloudflared/releases/latest | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])")
curl -L "https://github.com/cloudflare/cloudflared/releases/download/${CFVER}/cloudflared-linux-arm64" \
  -o /tmp/cloudflared-arm64
# 36MB 通过串口约需 95 小时 — 不可行
```

---

## Step 3：通过串口传输凭证和二进制到 MX93

由于 WiFi AP 隔离，Mac 无法直连板子，通过串口 base64 传输：

```bash
# 运行传输脚本
python3 scripts/mx93-setup-tunnel.py
```

或手动传输（以下命令在 Mac Python 运行）：

```python
import serial, time, base64, json, glob

ser = serial.Serial('/dev/cu.usbmodem5B6D0044901', 115200, timeout=2)

def slow_send(cmd, wait=3):
    for c in cmd:
        ser.write(c.encode()); time.sleep(0.007)
    ser.write(b'\r\n'); time.sleep(wait)
    return ser.read(ser.in_waiting).decode('utf-8', errors='replace')

def b64_transfer(local_path, remote_path):
    with open(local_path, 'rb') as f:
        data = base64.b64encode(f.read()).decode()
    lines = [data[i:i+60] for i in range(0, len(data), 60)]
    slow_send(f"cat > /tmp/_upload.b64 << 'EOF'", wait=1)
    for line in lines:
        for c in line:
            ser.write(c.encode()); time.sleep(0.007)
        ser.write(b'\r\n'); time.sleep(0.015)
    slow_send("EOF", wait=2)
    slow_send(f"base64 -d /tmp/_upload.b64 > {remote_path} && echo OK")

import os

# 1. 仅传输凭证（二进制已在板上通过 wget 下载）
# 不需要传输二进制（请用 Step 2 的 wget 方法）

# 2. 传输 cert.pem
slow_send("mkdir -p /root/.cloudflared")
b64_transfer(os.path.expanduser('~/.cloudflared/cert.pem'), '/root/.cloudflared/cert.pem')

# 3. 传输 tunnel JSON 凭证（找到 <UUID>.json 文件）
import glob
cred_files = glob.glob(os.path.expanduser('~/.cloudflared/*.json'))
tunnel_cred = [f for f in cred_files if '-' in os.path.basename(f)][0]
tunnel_uuid = os.path.basename(tunnel_cred).replace('.json', '')
b64_transfer(tunnel_cred, f'/root/.cloudflared/{tunnel_uuid}.json')

print(f"Tunnel UUID: {tunnel_uuid}")
```

---

## Step 4：在板子上配置 cloudflared

```bash
# 在板子上执行（通过串口）
TUNNEL_UUID="<从 Step 1 获取>"

# 创建 cloudflared 配置文件
mkdir -p /root/.cloudflared
cat > /root/.cloudflared/config.yml << EOF
tunnel: $TUNNEL_UUID
credentials-file: /root/.cloudflared/$TUNNEL_UUID.json

ingress:
  - hostname: kms.aastar.io
    service: http://localhost:3000
  - service: http_status:404
EOF

# 测试运行
cloudflared tunnel run mx93-kms

# 应看到：
# INF Registered tunnel connection connIndex=0 location=sin15
# INF Registered tunnel connection connIndex=1 ...
```

---

## Step 4b：WiFi 和 DNS 持久化（MX93 必做）

MX93 使用 NXP IW612 WiFi 芯片（`mlan0`），connman 管理 WiFi 存在问题。需用 wpa_supplicant 独立管理，且 ISP 路由器 DNS 无法解析 argotunnel.com SRV 记录，必须强制 DNS 为 1.1.1.1。

```bash
# 1. 排除 mlan0 不由 connman 管理
mkdir -p /etc/connman
cat > /etc/connman/main.conf << 'EOF'
[General]
NetworkInterfaceBlacklist=mlan0,uap0,wfd0
EOF
systemctl restart connman

# 2. 写 WiFi 配置（替换 SSID 和密码）
cat > /etc/wpa_supplicant.conf << 'EOF'
ctrl_interface=/var/run/wpa_supplicant
ctrl_interface_group=0
update_config=1

network={
    ssid="ChinaNet-AuRfsu-5G"
    psk="ucdb4543"
    key_mgmt=WPA-PSK
    priority=10
}

network={
    ssid="ChinaNet-AuRfsu"
    psk="ucdb4543"
    key_mgmt=WPA-PSK
    priority=5
}
EOF

# 3. 创建 WiFi systemd 服务
cat > /etc/systemd/system/wifi-mlan0.service << 'EOF'
[Unit]
Description=WiFi (wpa_supplicant + udhcpc) on mlan0
After=tee-supplicant@teepriv0.service
Before=cloudflared.service
Wants=cloudflared.service

[Service]
Type=forking
ExecStartPre=/sbin/ip link set mlan0 up
ExecStart=/sbin/wpa_supplicant -B -i mlan0 -c /etc/wpa_supplicant.conf -P /run/wpa_supplicant.pid
ExecStartPost=/bin/sh -c 'sleep 5 && /sbin/udhcpc -i mlan0 -b -p /run/udhcpc-mlan0.pid -q'
ExecStop=/bin/sh -c 'kill $(cat /run/wpa_supplicant.pid 2>/dev/null) 2>/dev/null; kill $(cat /run/udhcpc-mlan0.pid 2>/dev/null) 2>/dev/null; /sbin/ip link set mlan0 down'
RemainAfterExit=yes
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# 4. DNS 持久化钩子（udhcpc 每次续租都会覆盖 resolv.conf，这里强制 1.1.1.1）
# 原因：ISP 路由器 DNS（192.168.2.1）不能解析 argotunnel.com SRV 记录
mkdir -p /etc/udhcpc.d
cat > /etc/udhcpc.d/60dns << 'EOF'
#!/bin/sh
case "$1" in
  bound|renew)
    echo "nameserver 1.1.1.1" > /etc/resolv.conf
    echo "nameserver 8.8.8.8" >> /etc/resolv.conf
    ;;
esac
EOF
chmod +x /etc/udhcpc.d/60dns

# 5. 启用并启动 WiFi 服务
systemctl daemon-reload
systemctl enable wifi-mlan0.service
systemctl start wifi-mlan0.service

# 6. 验证 WiFi 连接
wpa_cli -i mlan0 status | grep -E "ssid|wpa_state|ip_address"
cat /etc/resolv.conf  # 应显示 1.1.1.1
```

---

## Step 5：systemd 服务（开机自启）

> **注意**：cloudflared.service 必须在 wifi-mlan0.service 之后启动，否则隧道启动时 WiFi/DNS 尚未就绪。

```bash
# 在板子上
cat > /etc/systemd/system/cloudflared.service << 'EOF'
[Unit]
Description=Cloudflare Tunnel - kms.aastar.io
After=network-online.target wifi-mlan0.service
Wants=network-online.target

[Service]
Type=simple
User=root
ExecStart=/usr/bin/cloudflared tunnel --config /root/.cloudflared/config.yml run
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable cloudflared.service
systemctl start cloudflared.service
```

---

## Step 6：验证

```bash
# 从任何网络验证公网访问
curl -s https://kms.aastar.io/health
# 期望: {"status":"healthy","ta_mode":"real","version":"0.19.0",...}

# 测试 CreateKey
TEST_PK="04$(openssl rand -hex 32)$(openssl rand -hex 32)"
curl -s -X POST https://kms.aastar.io/CreateKey \
  -H 'Content-Type: application/json' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d "{\"KeySpec\":\"ECC_SECG_P256K1\",\"KeyUsage\":\"SIGN_VERIFY\",\"Description\":\"tunnel-test\",\"Origin\":\"EXTERNAL\",\"PasskeyPublicKey\":\"$TEST_PK\"}"

# 测试 UI
open https://kms.aastar.io/test
```

---

## 服务管理

```bash
# 查看 tunnel 状态
systemctl status cloudflared

# 查看 tunnel 日志
tail -f /var/log/cloudflared.log

# 查看连接质量
journalctl -u cloudflared --no-pager -n 20

# Cloudflare Dashboard
# https://dash.cloudflare.com → Zero Trust → Tunnels → mx93-kms
```

---

## 故障排查

| 问题 | 排查 |
|------|------|
| tunnel 不通 | `cloudflared tunnel info mx93-kms`（Mac 上） |
| DNS 未生效 | `dig kms.aastar.io CNAME` 检查 CNAME 记录 |
| 503 Service Unavailable | kms-api.service 是否运行：`systemctl status kms-api` |
| cert.pem 过期 | 重新 `cloudflared tunnel login`，传 cert.pem 到板子 |
| 预检报告 FAIL 但实际连通 | 正常现象。WiFi AP 块 UDP/TCP port 7844 预检，但 IPv6 QUIC 连接能成功。看 journalctl 有 "Registered tunnel connection" 就说明隧道建立成功 |
| DNS 无法解析 argotunnel.com | ISP DNS 不支持 SRV 记录。检查 `/etc/resolv.conf`，应为 `1.1.1.1`。执行 Step 4b 安装 DNS 钩子 `/etc/udhcpc.d/60dns` |
| cloudflared 早于 WiFi 启动失败 | 检查 `cloudflared.service` 的 `After=` 是否包含 `wifi-mlan0.service` |
| DNS 已存在时路由失败 | `cloudflared tunnel route dns --overwrite-dns mx93-kms kms.aastar.io` |
