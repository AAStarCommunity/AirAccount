#!/bin/bash
# Step 2: Deploy to DK2 (graceful — zero-data-loss)
#
# Usage: ./deploy.sh [ta|ca|all] [host]
# Default: all, 192.168.7.2
#
# Graceful deployment flow:
#   1.  Pre-stage: upload new binary to .new path (service still running)
#   1.5 Drain: poll /QueueStatus, wait for pending TEE ops to finish (max 30s)
#   2.  Stop: systemctl stop kms (SIGTERM → warp drains in-flight HTTP)
#   3.  Swap: atomic mv .new → live binary
#   4.  Start: systemctl start kms
#   5.  Verify: health check
#   Total downtime: ~3-4s (after queue drained)

set -eo pipefail

MODE="${1:-all}"
DK2="${2:-192.168.7.2}"
YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
KMS_DIR="$(dirname "$SCRIPT_DIR")"
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

echo -e "${YELLOW}[Step 2] Deploy to DK2 ($DK2, mode: $MODE)${NC}"
echo ""

# Pre-check: connectivity
ssh -o ConnectTimeout=5 root@$DK2 "true" 2>/dev/null || {
    echo -e "${RED}Cannot connect to DK2 ($DK2)${NC}"
    exit 1
}

# Get current version before deploy
OLD_VER=$(curl -s --max-time 3 "http://$DK2:3000/version" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('version','unknown'))" 2>/dev/null || echo "unknown")
echo "  Current version: $OLD_VER"

# === Phase 1: Pre-stage (service still running) ===
echo -e "${YELLOW}Phase 1: Pre-staging binaries...${NC}"

if [ "$MODE" = "ta" ] || [ "$MODE" = "all" ]; then
    TA_FILE="$KMS_DIR/ta/target/arm-unknown-optee/release/${TA_UUID}.ta"
    if [ ! -f "$TA_FILE" ]; then
        echo -e "${RED}TA binary not found: $TA_FILE${NC}"
        echo "Run ./build.sh first"
        exit 1
    fi
    scp -q "$TA_FILE" root@$DK2:/lib/optee_armtz/${TA_UUID}.ta.new
    echo "  TA staged: /lib/optee_armtz/${TA_UUID}.ta.new"
fi

if [ "$MODE" = "ca" ] || [ "$MODE" = "all" ]; then
    CA_FILE="$KMS_DIR/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server"
    if [ ! -f "$CA_FILE" ]; then
        echo -e "${RED}CA binary not found: $CA_FILE${NC}"
        echo "Run ./build.sh first"
        exit 1
    fi
    scp -q "$CA_FILE" root@$DK2:/usr/local/bin/kms-api-server.new
    echo "  CA staged: /usr/local/bin/kms-api-server.new"

    APIKEY_FILE="$KMS_DIR/host/target/armv7-unknown-linux-gnueabihf/release/api-key"
    if [ -f "$APIKEY_FILE" ]; then
        scp -q "$APIKEY_FILE" root@$DK2:/usr/local/bin/api-key.new
    fi
fi

# === Phase 1.5: Drain queue (wait for pending TEE operations) ===
echo -e "${YELLOW}Phase 1.5: Draining TEE queue...${NC}"
DRAIN_OK=false
for i in $(seq 1 30); do
    QS=$(curl -s --max-time 2 "http://$DK2:3000/QueueStatus" 2>/dev/null || echo "{}")
    PENDING=$(echo "$QS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('PendingRequests',0))" 2>/dev/null || echo "0")
    ACTIVE=$(echo "$QS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(1 if d.get('TeeStatus','')=='busy' else 0)" 2>/dev/null || echo "0")
    if [ "$PENDING" = "0" ] && [ "$ACTIVE" = "0" ]; then
        DRAIN_OK=true
        echo "  Queue idle (waited ${i}s)"
        break
    fi
    echo "  Waiting... pending=$PENDING active=$ACTIVE (${i}/30)"
    sleep 1
done
if [ "$DRAIN_OK" != "true" ]; then
    echo -e "${YELLOW}  Queue not idle after 30s, proceeding anyway${NC}"
fi

# === Phase 2: Stop + Swap + Start (downtime window) ===
echo -e "${YELLOW}Phase 2: Switchover (expect ~3s downtime)...${NC}"
SWITCH_START=$(date +%s)

ssh root@$DK2 "
    # Stop gracefully (SIGTERM, warp drains in-flight HTTP connections)
    systemctl stop kms 2>/dev/null || true

    # Atomic swap
    [ -f /lib/optee_armtz/${TA_UUID}.ta.new ] && mv /lib/optee_armtz/${TA_UUID}.ta.new /lib/optee_armtz/${TA_UUID}.ta
    [ -f /usr/local/bin/kms-api-server.new ] && mv /usr/local/bin/kms-api-server.new /usr/local/bin/kms-api-server
    [ -f /usr/local/bin/api-key.new ] && mv /usr/local/bin/api-key.new /usr/local/bin/api-key

    # Reload unit file (in case it changed)
    systemctl daemon-reload

    # Start
    systemctl start kms
"

# === Phase 3: Verify ===
echo -e "${YELLOW}Phase 3: Verifying...${NC}"

# Wait for health
HEALTHY=false
for i in $(seq 1 15); do
    if curl -s --max-time 2 "http://$DK2:3000/health" > /dev/null 2>&1; then
        HEALTHY=true
        break
    fi
    sleep 1
done

SWITCH_END=$(date +%s)
DOWNTIME=$((SWITCH_END - SWITCH_START))

if [ "$HEALTHY" = "true" ]; then
    NEW_VER=$(curl -s --max-time 3 "http://$DK2:3000/version" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('version','unknown'))" 2>/dev/null || echo "unknown")
    STATUS=$(ssh root@$DK2 "systemctl is-active kms")
    echo -e "${GREEN}  Service: $STATUS${NC}"
    echo -e "${GREEN}  Version: $OLD_VER → $NEW_VER${NC}"
    echo -e "${GREEN}  Switchover: ${DOWNTIME}s${NC}"
else
    echo -e "${RED}  Health check failed after 15s!${NC}"
    echo "  Check logs: ssh root@$DK2 journalctl -u kms -n 20"
    exit 1
fi

echo ""
echo -e "${GREEN}[Step 2] Deploy complete${NC}"
