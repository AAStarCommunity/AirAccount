#!/usr/bin/env bash
# Connect to the MX93 board serial console (auto-detects the device).
# Delegates to scripts/mx93/connect.sh. Pass a device path to override.
exec "$(dirname "$0")/scripts/mx93/connect.sh" "$@"

# ssh 板子 192.168.2.30（50ms）
