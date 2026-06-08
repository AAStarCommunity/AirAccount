#!/bin/bash
# Build CA only (kms-api-server release binary).
# Run ON THE BOARD. Restarts service after successful build.
# Usage: bash /root/AirAccount/scripts/mx93/build-ca.sh [--no-restart]
set -eo pipefail

PROJECT="/root/AirAccount"
RESTART=1
[[ "$1" == "--no-restart" ]] && RESTART=0

cd "$PROJECT"
echo "Building kms-api-server (release)..."
RUSTFLAGS="" cargo build --release --bin kms-api-server 2>&1 | grep -E 'Compiling kms|Finished|error|warning:.*unused'

BIN="target/release/kms-api-server"
echo "Binary: $(ls -lh $BIN | awk '{print $5, $6, $7, $8}')"

if [[ "$RESTART" -eq 1 ]]; then
    echo "Restarting kms-api..."
    systemctl restart kms-api
    sleep 3
    systemctl is-active kms-api && echo "✓ Service active" || echo "✗ Service failed"
fi
