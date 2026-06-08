#!/bin/bash
# Build TA (Trusted Application) — requires OP-TEE TA Dev Kit.
# The dev kit is NOT installed on the MX93 board by default.
# Until available, this script prints instructions and exits.
#
# To enable: set TA_DEV_KIT_DIR=/path/to/optee/export-ta_arm64
# and run from a machine with the OP-TEE build environment.
set -eo pipefail

TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
TA_DEST="/usr/lib/optee_armtz/${TA_UUID}.ta"
PROJECT="$(cd "$(dirname "$0")/../.." && pwd)"

if [ -z "$TA_DEV_KIT_DIR" ]; then
    echo "ERROR: TA build requires OP-TEE dev kit."
    echo ""
    echo "Requirements:"
    echo "  export TA_DEV_KIT_DIR=/path/to/optee/export-ta_arm64"
    echo "  The kit provides: sign_encrypt.py, default_ta.pem, tee_internal_api.h"
    echo ""
    echo "Current TA on board:"
    ls -la "$TA_DEST" 2>/dev/null || echo "  (not found at $TA_DEST)"
    echo ""
    echo "Workaround: CA gracefully handles old TA commands returning 0xffff0006."
    echo "  ForceRemoveWallet (cmd 23) will log a warning but SQLite cleanup still runs."
    exit 1
fi

echo "Building TA with dev kit: $TA_DEV_KIT_DIR"
cd "$PROJECT/kms/ta"
make CROSS_COMPILE=aarch64-linux-gnu- TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR" 2>&1

echo ""
echo "Deploying TA..."
TA_BIN=$(find . -name "${TA_UUID}.ta" | head -1)
if [ -z "$TA_BIN" ]; then
    echo "ERROR: built TA not found"
    exit 1
fi

cp "$TA_BIN" "$TA_DEST"
chown root:root "$TA_DEST"
chmod 444 "$TA_DEST"
echo "✓ TA deployed to $TA_DEST"
echo "Restarting tee-supplicant and kms-api..."
systemctl restart tee-supplicant 2>/dev/null || true
systemctl restart kms-api
sleep 3
systemctl is-active kms-api && echo "✓ kms-api active" || echo "✗ kms-api failed"
