#!/usr/bin/env bash
# Delete "gap keys" from the KMS database on the IMX93 board.
#
# Gap keys are wallets created with a syntactically valid but
# cryptographically invalid P-256 public key (e.g. a random 65-byte
# string starting with 0x04 that doesn't lie on the P-256 curve).
# They cannot be deleted via the normal API because passkey verification
# always fails.  This script connects via SSH or serial and removes them
# directly from SQLite.
#
# Usage:
#   ./cleanup-gap-keys.sh              # serial
#   ./cleanup-gap-keys.sh <board-ip>   # SSH

SERIAL="${IMXSERIAL:-$(ls /dev/cu.usbmodem* 2>/dev/null | head -1)}"
BOARD_IP="${1:-}"
DB="/data/kms/kms.db"

# Gap keys created by e2e-test.py carry this description prefix.
# The P-256 validation added in v0.20.0 prevents new ones, but old ones
# may already be in the DB.
GAP_DESC="sec-gap-test%"

CMDS=$(cat <<SQL
-- Show gap keys before deletion
SELECT key_id, description, status, created_at
  FROM wallets
 WHERE description LIKE '${GAP_DESC}';

-- Delete them
DELETE FROM wallets WHERE description LIKE '${GAP_DESC}';

SELECT 'Deleted: ' || changes() || ' gap key(s).' AS result;
SQL
)

run_on_board() {
    # $1 = command to execute on board
    echo "$1"
}

cleanup_via_ssh() {
    echo "→ Connecting via SSH to $1 ..."
    ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no root@"$1" \
        "sqlite3 $DB \"$CMDS\""
}

cleanup_via_serial() {
    [[ -z "$SERIAL" || ! -e "$SERIAL" ]] && { echo "✗ No serial device."; exit 1; }
    echo "→ Connecting via serial $SERIAL ..."
    python3 - "$SERIAL" "$DB" "$GAP_DESC" <<'PYEOF'
import sys, time, termios, select, tty

dev, db, pattern = sys.argv[1], sys.argv[2], sys.argv[3]
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
    fd.write(s.encode()); time.sleep(0.4)

send(fd, '\r\n')
out = read_for(fd, 2)
if 'login:' in out:
    send(fd, 'root\r\n'); read_for(fd, 2)

# Show gap keys
send(fd, f"sqlite3 {db} \"SELECT key_id,description FROM wallets WHERE description LIKE '{pattern}';\"\r\n")
print(read_for(fd, 3).strip())

# Delete
send(fd, f"sqlite3 {db} \"DELETE FROM wallets WHERE description LIKE '{pattern}'; SELECT 'Deleted: '||changes()||' rows.' ;\"\r\n")
print(read_for(fd, 3).strip())
fd.close()
PYEOF
}

echo "=== Gap Key Cleanup ==="
echo "DB: $DB"
echo "Pattern: $GAP_DESC"
echo ""

if [[ -n "$BOARD_IP" ]]; then
    cleanup_via_ssh "$BOARD_IP" && echo "✓ Done." && exit 0
    echo "SSH failed, trying serial ..."
fi

for host in imx93.local frdm-imx93.local; do
    if ping -c1 -W1 "$host" &>/dev/null 2>&1; then
        cleanup_via_ssh "$host" && echo "✓ Done." && exit 0
    fi
done

cleanup_via_serial
echo "✓ Done."
