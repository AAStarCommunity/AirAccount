#!/usr/bin/env bash
# Graceful shutdown for NXP FRDM-IMX93
# Usage:
#   ./poweroff-imx93.sh               # serial (auto-detect /dev/cu.usbmodem*)
#   ./poweroff-imx93.sh <board-ip>    # SSH preferred, serial fallback

SERIAL="${IMXSERIAL:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -1)}"
BOARD_IP="${1:-}"

# ── helpers ──────────────────────────────────────────────────────────────────

poweroff_via_ssh() {
    echo "→ SSH to $1 ..."
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no root@"$1" \
        "systemctl stop kms-api.service 2>/dev/null; sync; poweroff" 2>/dev/null
}

poweroff_via_serial() {
    [[ -z "$SERIAL" ]] && { echo "✗ No serial device found."; exit 1; }
    [[ ! -e "$SERIAL" ]] && { echo "✗ $SERIAL not found. USB cable connected?"; exit 1; }
    echo "→ Serial shutdown via $SERIAL ..."
    python3 - "$SERIAL" <<'PYEOF'
import sys, time, termios, select, os

dev = sys.argv[1]
try:
    fd = open(dev, 'rb+', buffering=0)
except PermissionError:
    print(f"✗ Cannot open {dev}: permission denied. Try: sudo chmod 666 {dev}")
    sys.exit(1)

# configure 115200 8N1
attrs = termios.tcgetattr(fd)
import tty
tty.setraw(fd)
attrs = termios.tcgetattr(fd)
attrs[0] = termios.ICRNL                          # input
attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL  # control
attrs[3] = 0                                       # local
attrs[4] = attrs[5] = termios.B115200             # speed
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
    fd.write(s.encode())
    time.sleep(0.2)

# Wake up the console
send(fd, '\r\n')
out = read_for(fd, 2)
print(out.strip() or '(no output)')

# Login if needed
if 'login:' in out:
    print('[login prompt detected, logging in as root]')
    send(fd, 'root\r\n')
    out2 = read_for(fd, 3)
    print(out2.strip())
    if 'Password' in out2:
        print('✗ Board requires a password — connect manually via: screen ' + dev + ' 115200')
        fd.close(); sys.exit(1)

# Send the shutdown command
send(fd, 'systemctl stop kms-api.service 2>/dev/null; sync; poweroff\r\n')
print('Waiting for board to shut down ...')
out3 = read_for(fd, 12)
print(out3.strip())

if any(x in out3 for x in ['Power down', 'reboot:', 'Stopping', 'halt']):
    print('\n✓ Board is shutting down.')
else:
    print('\n⚠ No shutdown confirmation received — command was sent, wait 15s then cut power.')

fd.close()
PYEOF
}

# ── main ─────────────────────────────────────────────────────────────────────

if [[ -n "$BOARD_IP" ]]; then
    if poweroff_via_ssh "$BOARD_IP"; then
        echo "✓ Board shutdown initiated via SSH."
        exit 0
    fi
    echo "  SSH failed, trying serial ..."
fi

# Auto-discover via mDNS
for host in imx93.local frdm-imx93.local; do
    if ping -c1 -W1 "$host" &>/dev/null 2>&1; then
        if poweroff_via_ssh "$host"; then
            echo "✓ Board shutdown via SSH ($host)."
            exit 0
        fi
    fi
done

poweroff_via_serial
echo "Wait ~15s for board LEDs to go dark, then cut power."
