#!/usr/bin/env bash
# Graceful shutdown for NXP FRDM-IMX93
# Usage: ./poweroff-imx93.sh [board-ip]
#
# Method 1 (preferred): SSH if board IP is reachable on local network
# Method 2 (fallback):  serial port via `expect`

SERIAL=/dev/cu.usbmodem5B6D0044901
BOARD_IP="${1:-}"

poweroff_via_ssh() {
    echo "→ Trying SSH to $1 ..."
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no root@"$1" \
        "systemctl stop kms-api.service; sync; poweroff" 2>/dev/null
    return $?
}

poweroff_via_serial() {
    echo "→ Sending poweroff via serial $SERIAL ..."
    if ! command -v expect &>/dev/null; then
        echo "  expect not found. Install with: brew install expect"
        echo "  Fallback: open a screen session and run: poweroff"
        echo "    screen $SERIAL 115200"
        exit 1
    fi
    expect -c "
        set timeout 10
        spawn -open [open $SERIAL r+]
        stty -f $SERIAL 115200 cs8 -cstopb -parenb
        send \"\r\"
        expect -re {#|\\$}
        send \"systemctl stop kms-api.service && sync && poweroff\r\"
        expect {
            \"reboot\" { puts \"Board is shutting down.\" }
            timeout    { puts \"Timeout — board may already be off or in another state.\" }
        }
    "
}

set -e

# Try SSH first if IP provided or discoverable via mDNS
if [[ -n "$BOARD_IP" ]]; then
    if poweroff_via_ssh "$BOARD_IP"; then
        echo "✓ Board shutdown initiated via SSH."
        exit 0
    fi
    echo "  SSH failed, falling back to serial."
fi

# Try mDNS hostname (sometimes shows up as imx93.local)
if ping -c1 -W1 imx93.local &>/dev/null 2>&1; then
    if poweroff_via_ssh imx93.local; then
        echo "✓ Board shutdown initiated via SSH (mDNS)."
        exit 0
    fi
fi

# Serial fallback
if [[ ! -e "$SERIAL" ]]; then
    echo "✗ Serial device $SERIAL not found. Is the USB cable connected?"
    exit 1
fi

poweroff_via_serial
echo "✓ Shutdown command sent. Wait ~10s before cutting power."
