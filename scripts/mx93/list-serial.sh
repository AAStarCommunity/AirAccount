#!/usr/bin/env bash
# list-serial.sh — List all serial / USB-modem devices on macOS, highlight the MX93 board.
#
# The MX93 (NXP FRDM-IMX93) shows up as /dev/cu.usbmodem<XXXX> ONLY while the board
# is powered and booted. If no usbmodem device appears, the board is OFF or crashed
# (its onboard USB-serial chip is powered by the board and stops enumerating on a hang).
#
# Usage: ./list-serial.sh

set -u

echo "=== All callout (cu.*) serial devices ==="
# cu.* are the call-out devices you use with screen; tty.* are dial-in.
ls -1 /dev/cu.* 2>/dev/null || echo "(none)"

echo ""
echo "=== USB-modem devices (MX93 board appears here when booted) ==="
MODEMS=$(ls -1 /dev/cu.usbmodem* 2>/dev/null)
if [ -n "$MODEMS" ]; then
  echo "$MODEMS"
  echo ""
  echo ">> MX93 candidate: $(echo "$MODEMS" | head -1)"
  echo ">> Connect with:   ./scripts/mx93/connect.sh"
else
  echo "(none)"
  echo ""
  echo ">> No usbmodem device found."
  echo ">> The board is OFF / crashed, or the USB cable is unplugged."
  echo ">> Power-cycle the board; the device re-enumerates a few seconds after boot."
fi

echo ""
echo "=== USB serial bridges via system_profiler (FTDI / CP210x / onboard) ==="
system_profiler SPUSBDataType 2>/dev/null \
  | grep -iA6 -e "serial" -e "uart" -e "imx" -e "modem" \
  | grep -iE "Product ID|Vendor|Serial Number|Manufacturer|:$" \
  | head -40
echo ""
echo "(If the section above is empty, no USB-serial bridge is enumerated.)"
