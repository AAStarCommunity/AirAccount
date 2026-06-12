#!/usr/bin/env bash
# Get the local IP address of the IMX93 board
# Usage: ./get-imx93-ip.sh
#
# Method 1: query board via serial (most reliable)
# Method 2: scan local network ARP table

SERIAL="${IMXSERIAL:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -1)}"

get_ip_via_serial() {
    [[ -z "$SERIAL" || ! -e "$SERIAL" ]] && return 1
    echo "→ Querying board IP via serial $SERIAL ..."
    python3 - "$SERIAL" <<'PYEOF'
import sys, time, termios, select, tty

dev = sys.argv[1]
fd = open(dev, 'rb+', buffering=0)
tty.setraw(fd)
attrs = termios.tcgetattr(fd)
attrs[0] = termios.ICRNL
attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
attrs[3] = 0
attrs[4] = attrs[5] = termios.B115200
termios.tcsetattr(fd, termios.TCSANOW, attrs)

def read_for(fd, secs):
    buf = b''
    deadline = time.time() + secs
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            buf += fd.read(256)
    return buf.decode('utf-8', errors='replace')

def send(fd, s):
    fd.write(s.encode()); time.sleep(0.3)

send(fd, '\r\n')
out = read_for(fd, 2)
if 'login:' in out:
    send(fd, 'root\r\n')
    read_for(fd, 2)

send(fd, "ip route get 1.1.1.1 | awk '/src/{for(i=1;i<=NF;i++) if($i==\"src\") print $(i+1)}'\r\n")
out = read_for(fd, 3)

import re
ips = re.findall(r'\b(?:192\.168|10\.|172\.(?:1[6-9]|2[0-9]|3[01]))\.\d+\.\d+\b', out)
if ips:
    print(f"Board IP: {ips[0]}")
    print(f"\nSSH command:  ssh root@{ips[0]}")
    print(f"Poweroff:     ./poweroff-imx93.sh {ips[0]}")
else:
    print("Could not parse IP. Raw output:")
    print(out.strip())
fd.close()
PYEOF
}

get_ip_via_arp() {
    echo "→ Scanning ARP table (devices seen recently on local network) ..."
    # NXP i.MX93 EVK MACs often start with 00:04:9f (Freescale/NXP OUI)
    arp -a | grep -iE '(00:04:9f|NXP|imx|frdm)' || arp -a
    echo ""
    echo "Hint: connect via serial and run: ip addr show eth0"
}

if ! get_ip_via_serial; then
    get_ip_via_arp
fi
