#!/usr/bin/env bash
# connect.sh — Open a serial console to the MX93 board via screen (auto-detects device).
#
# Baud: 115200. Login: root (no password).
#
# Usage:
#   ./connect.sh              # auto-detect /dev/cu.usbmodem*, connect at 115200
#   ./connect.sh /dev/cu.usbmodemXXXX   # force a specific device
#   SERIAL_BAUD=115200 ./connect.sh
#
# To EXIT screen:  Ctrl-A then k  (then y),  or  Ctrl-A then \  (then y).
# To DETACH (leave session running): Ctrl-A then d.
# If screen says the device is busy, run:  ./connect.sh --kill   to wipe stale sessions.

set -u
BAUD="${SERIAL_BAUD:-115200}"

if [ "${1:-}" = "--kill" ]; then
  echo "Wiping stale screen sessions..."
  screen -ls 2>/dev/null | grep -oE '[0-9]+\.[^[:space:]]+' | while read -r s; do
    echo "  killing $s"; screen -S "$s" -X quit 2>/dev/null
  done
  screen -wipe >/dev/null 2>&1
  echo "Done. Re-run ./connect.sh to connect."
  exit 0
fi

DEV="${1:-}"
if [ -z "$DEV" ]; then
  DEV=$(ls -1 /dev/cu.usbmodem* 2>/dev/null | head -1)
fi

if [ -z "$DEV" ] || [ ! -e "$DEV" ]; then
  echo "ERROR: no MX93 serial device found (looked for /dev/cu.usbmodem*)."
  echo ""
  echo "The board is OFF or crashed — its USB-serial chip only enumerates when booted."
  echo "Power-cycle the board, wait ~10s, then run:  ./scripts/mx93/list-serial.sh"
  exit 1
fi

echo "Connecting to MX93: $DEV @ ${BAUD} baud"
echo "  login: root (no password)"
echo "  exit screen:  Ctrl-A k y    |    detach:  Ctrl-A d"
echo ""
exec screen "$DEV" "$BAUD"
