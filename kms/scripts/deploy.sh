#!/bin/bash
# Step 2: Deploy to DK2
# Usage: ./deploy.sh [ta|ca|all] [host]
# Default: all, 192.168.7.2

set -eo pipefail

MODE="${1:-all}"
DK2="${2:-192.168.7.2}"
YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
KMS_DIR="$(dirname "$SCRIPT_DIR")"
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

echo "${YELLOW}[Step 2] Deploy to DK2 ($DK2, mode: $MODE)${NC}"
echo ""

# Pre-check: stop service to avoid file-in-use errors
echo "Stopping kms service..."
ssh -o ConnectTimeout=5 root@$DK2 "systemctl stop kms 2>/dev/null || true"

if [ "$MODE" = "ta" ] || [ "$MODE" = "all" ]; then
    TA_FILE="$KMS_DIR/ta/target/arm-unknown-optee/release/${TA_UUID}.ta"
    if [ ! -f "$TA_FILE" ]; then
        echo "${RED}TA binary not found: $TA_FILE${NC}"
        echo "Run ./build.sh first"
        exit 1
    fi
    echo "Deploying TA..."
    scp "$TA_FILE" root@$DK2:/lib/optee_armtz/
    echo "${GREEN}  TA deployed: /lib/optee_armtz/${TA_UUID}.ta${NC}"
fi

if [ "$MODE" = "ca" ] || [ "$MODE" = "all" ]; then
    CA_FILE="$KMS_DIR/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server"
    if [ ! -f "$CA_FILE" ]; then
        echo "${RED}CA binary not found: $CA_FILE${NC}"
        echo "Run ./build.sh first"
        exit 1
    fi
    echo "Deploying CA..."
    scp "$CA_FILE" root@$DK2:/usr/local/bin/

    # Deploy api-key tool if exists
    APIKEY_FILE="$KMS_DIR/host/target/armv7-unknown-linux-gnueabihf/release/api-key"
    if [ -f "$APIKEY_FILE" ]; then
        scp "$APIKEY_FILE" root@$DK2:/usr/local/bin/
    fi

    echo "${GREEN}  CA deployed: /usr/local/bin/kms-api-server${NC}"
fi

# Restart service
echo "Starting kms service..."
ssh root@$DK2 "systemctl start kms"
sleep 2

# Verify
STATUS=$(ssh root@$DK2 "systemctl is-active kms 2>/dev/null || echo inactive")
if [ "$STATUS" = "active" ]; then
    echo "${GREEN}  kms service: active${NC}"
else
    echo "${RED}  kms service: $STATUS${NC}"
    echo "  Check logs: ssh root@$DK2 journalctl -u kms -n 20"
    exit 1
fi

# Wait for API server to come up
echo "Waiting for API server..."
for i in $(seq 1 10); do
    if curl -s --max-time 2 "http://$DK2:3000/health" > /dev/null 2>&1; then
        echo "${GREEN}  API server: healthy${NC}"
        break
    fi
    sleep 1
done

echo ""
echo "${GREEN}[Step 2] Deploy complete${NC}"
