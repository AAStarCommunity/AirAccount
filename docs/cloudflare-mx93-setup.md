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

```bash
# 下载 aarch64 版本（在 Mac 上下载，然后传到板子）
CFVER=$(curl -s https://api.github.com/repos/cloudflare/cloudflared/releases/latest | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])")
curl -L "https://github.com/cloudflare/cloudflared/releases/download/${CFVER}/cloudflared-linux-arm64" \
  -o /tmp/cloudflared-arm64
# 文件约 40MB
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

# 1. 传输 cloudflared 二进制
b64_transfer('/tmp/cloudflared-arm64', '/usr/local/bin/cloudflared')
slow_send("chmod +x /usr/local/bin/cloudflared && cloudflared --version")

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

## Step 5：systemd 服务（开机自启）

```bash
# 在板子上
cat > /etc/systemd/system/cloudflared.service << 'EOF'
[Unit]
Description=Cloudflare Tunnel (kms.aastar.io)
After=network.target kms-api.service
Wants=kms-api.service

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/cloudflared tunnel run mx93-kms
Restart=on-failure
RestartSec=10
StandardOutput=append:/var/log/cloudflared.log
StandardError=append:/var/log/cloudflared.log

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
