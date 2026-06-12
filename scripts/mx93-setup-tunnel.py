#!/usr/bin/env python3
"""
mx93-setup-tunnel.py
把 cloudflared aarch64 二进制 + Cloudflare tunnel 凭证传输到 MX93 板子
并配置 systemd 服务。

前置条件：
  1. cloudflared tunnel login（生成 ~/.cloudflared/cert.pem）
  2. cloudflared tunnel create mx93-kms（生成 ~/.cloudflared/<UUID>.json）
  3. cloudflared tunnel route dns mx93-kms kms.aastar.io
  4. 下载 aarch64 cloudflared 到 /tmp/cloudflared-arm64

用法：
  python3 scripts/mx93-setup-tunnel.py
  python3 scripts/mx93-setup-tunnel.py --port /dev/cu.usbmodem5B6D0044901
"""

import argparse
import base64
import glob
import json
import os
import sys
import time

try:
    import serial
except ImportError:
    print("错误: 需要 pyserial — pip3 install pyserial")
    sys.exit(1)

SERIAL_PORT = '/dev/cu.usbmodem5B6D0044901'
BAUD_RATE = 115200
CHAR_DELAY = 0.007   # 7ms per character
LINE_DELAY = 0.015   # 15ms per line (for b64 transfer)


def parse_args():
    p = argparse.ArgumentParser(description='MX93 Cloudflare tunnel setup')
    p.add_argument('--port', default=SERIAL_PORT)
    p.add_argument('--cloudflared-bin', default='/tmp/cloudflared-arm64',
                   help='Path to cloudflared linux-arm64 binary')
    return p.parse_args()


def open_serial(port):
    ser = serial.Serial(port, BAUD_RATE, timeout=2,
                        xonxoff=False, rtscts=False, dsrdtr=False)
    time.sleep(0.3)
    ser.read(ser.in_waiting)
    return ser


def slow_send(ser, cmd, wait=4):
    for c in cmd:
        ser.write(c.encode())
        time.sleep(CHAR_DELAY)
    ser.write(b'\r\n')
    time.sleep(wait)
    return ser.read(ser.in_waiting).decode('utf-8', errors='replace')


def b64_transfer(ser, local_path, remote_path, desc=""):
    size = os.path.getsize(local_path)
    print(f"  传输 {desc or local_path} ({size/1024:.0f} KB) → {remote_path}")
    with open(local_path, 'rb') as f:
        data = base64.b64encode(f.read()).decode()
    lines = [data[i:i+60] for i in range(0, len(data), 60)]
    print(f"  总行数: {len(lines)}", end='', flush=True)

    slow_send(ser, "cat > /tmp/_upload.b64 << 'B64EOF'", wait=1)
    for i, line in enumerate(lines):
        for c in line:
            ser.write(c.encode())
            time.sleep(CHAR_DELAY)
        ser.write(b'\r\n')
        time.sleep(LINE_DELAY)
        if (i + 1) % 50 == 0:
            print(f'\r  进度: {i+1}/{len(lines)} 行', end='', flush=True)
    print()
    result = slow_send(ser, "B64EOF", wait=3)

    result = slow_send(ser, f"base64 -d /tmp/_upload.b64 > {remote_path} && echo TRANSFER_OK")
    if 'TRANSFER_OK' not in result:
        print(f"  ❌ 传输失败: {result}")
        return False
    print(f"  ✅ 传输成功")
    return True


def find_tunnel_creds():
    """Find tunnel UUID and credential JSON file."""
    cf_dir = os.path.expanduser('~/.cloudflared')
    cert_pem = os.path.join(cf_dir, 'cert.pem')
    if not os.path.exists(cert_pem):
        print("❌ ~/.cloudflared/cert.pem 不存在，请先运行: cloudflared tunnel login")
        return None, None, None

    jsons = glob.glob(os.path.join(cf_dir, '*.json'))
    # Filter to tunnel credential files (UUID format)
    creds = [f for f in jsons if len(os.path.basename(f)) == 41]  # UUID + .json
    if not creds:
        print("❌ 找不到 tunnel 凭证 JSON，请先运行: cloudflared tunnel create mx93-kms")
        return None, None, None

    # Use the most recently created one
    creds.sort(key=os.path.getmtime, reverse=True)
    cred_file = creds[0]
    uuid = os.path.basename(cred_file).replace('.json', '')
    print(f"  找到 tunnel 凭证: {uuid}")
    return cert_pem, cred_file, uuid


def main():
    args = parse_args()

    print("=== MX93 Cloudflare Tunnel 配置脚本 ===\n")

    # 1. Check prerequisites
    if not os.path.exists(args.cloudflared_bin):
        print(f"❌ cloudflared 二进制不存在: {args.cloudflared_bin}")
        print("  下载命令:")
        print("  CFVER=$(curl -s https://api.github.com/repos/cloudflare/cloudflared/releases/latest | python3 -c \"import sys,json; print(json.load(sys.stdin)['tag_name'])\")")
        print(f"  curl -L \"https://github.com/cloudflare/cloudflared/releases/download/${{CFVER}}/cloudflared-linux-arm64\" -o {args.cloudflared_bin}")
        sys.exit(1)

    cert_pem, cred_file, tunnel_uuid = find_tunnel_creds()
    if not tunnel_uuid:
        sys.exit(1)

    print(f"  cloudflared 二进制: {args.cloudflared_bin} ({os.path.getsize(args.cloudflared_bin)/1024/1024:.1f} MB)")
    print(f"  Tunnel UUID: {tunnel_uuid}")
    print(f"  串口: {args.port}")
    print()
    input("按 Enter 开始传输...")

    ser = open_serial(args.port)

    # 2. Transfer cloudflared binary
    print("\n[1/4] 传输 cloudflared 二进制...")
    slow_send(ser, "mkdir -p /usr/local/bin", wait=1)
    if not b64_transfer(ser, args.cloudflared_bin, '/usr/local/bin/cloudflared', 'cloudflared'):
        sys.exit(1)
    result = slow_send(ser, "chmod +x /usr/local/bin/cloudflared && cloudflared --version")
    print(f"  版本: {result.strip().splitlines()[-2] if result.strip() else '未知'}")

    # 3. Transfer cert.pem
    print("\n[2/4] 传输 Cloudflare 证书...")
    slow_send(ser, "mkdir -p /root/.cloudflared", wait=1)
    if not b64_transfer(ser, cert_pem, '/root/.cloudflared/cert.pem', 'cert.pem'):
        sys.exit(1)

    # 4. Transfer tunnel credential JSON
    print("\n[3/4] 传输 Tunnel 凭证...")
    if not b64_transfer(ser, cred_file, f'/root/.cloudflared/{tunnel_uuid}.json', f'{tunnel_uuid}.json'):
        sys.exit(1)

    # 5. Write config.yml
    print("\n[4/4] 写入配置文件和 systemd 服务...")
    config_lines = [
        f"tunnel: {tunnel_uuid}",
        f"credentials-file: /root/.cloudflared/{tunnel_uuid}.json",
        "",
        "ingress:",
        "  - hostname: kms.aastar.io",
        "    service: http://localhost:3000",
        "  - service: http_status:404",
    ]
    slow_send(ser, "printf '' > /root/.cloudflared/config.yml", wait=1)
    for line in config_lines:
        slow_send(ser, f"printf '%s\\n' '{line}' >> /root/.cloudflared/config.yml", wait=0.5)

    # 6. Write systemd service
    svc_lines = [
        "[Unit]",
        "Description=Cloudflare Tunnel (kms.aastar.io)",
        "After=network.target kms-api.service",
        "Wants=kms-api.service",
        "",
        "[Service]",
        "Type=simple",
        "User=root",
        "ExecStart=/usr/local/bin/cloudflared tunnel run mx93-kms",
        "Restart=on-failure",
        "RestartSec=10",
        "StandardOutput=append:/var/log/cloudflared.log",
        "StandardError=append:/var/log/cloudflared.log",
        "",
        "[Install]",
        "WantedBy=multi-user.target",
    ]
    slow_send(ser, "printf '' > /etc/systemd/system/cloudflared.service", wait=1)
    for line in svc_lines:
        slow_send(ser, f"printf '%s\\n' '{line}' >> /etc/systemd/system/cloudflared.service", wait=0.5)

    # 7. Enable and start
    print("\n启用 systemd 服务...")
    print(slow_send(ser, "systemctl daemon-reload && systemctl enable cloudflared.service && echo ENABLED"))
    print(slow_send(ser, "systemctl start cloudflared.service && echo STARTED", wait=8))
    print(slow_send(ser, "systemctl status cloudflared.service --no-pager -l | head -10"))

    print("\n=== 完成 ===")
    print("验证:")
    print("  curl -s https://kms.aastar.io/health")
    print("  tail -f /var/log/cloudflared.log  (在板子上)")

    ser.close()


if __name__ == '__main__':
    main()
