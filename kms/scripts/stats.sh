#!/bin/bash
# KMS Daily Statistics — one-key run
#
# Usage: ./stats.sh [host]
# Default host: 192.168.7.2:3000
#
# Env vars:
#   KMS_API_KEY  — required if API keys are configured on the server

set -eo pipefail

HOST="${1:-192.168.7.2:3000}"
BASE="http://$HOST"
API_KEY="${KMS_API_KEY:-}"
BOLD='\033[1m'; DIM='\033[2m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'

# --- helpers ---
kms_get() {
    curl -sf --max-time 5 "$BASE$1" 2>/dev/null
}

kms_post() {
    local target="$1"
    local body="${2:-"{}"}"
    local headers=(-H "Content-Type: application/json" -H "x-amz-target: TrentService.$target")
    [ -n "$API_KEY" ] && headers+=(-H "x-api-key: $API_KEY")
    curl -sf --max-time 10 "${headers[@]}" -d "$body" "$BASE/$target" 2>/dev/null
}

py() { python3 -c "$1" 2>/dev/null; }

# --- connectivity check ---
VER_JSON=$(kms_get /version) || { echo -e "${RED}Cannot reach $BASE${NC}"; exit 1; }
VERSION=$(echo "$VER_JSON" | py "import sys,json; print(json.load(sys.stdin)['version'])")

HEALTH_JSON=$(kms_get /health)
TA_MODE=$(echo "$HEALTH_JSON" | py "import sys,json; print(json.load(sys.stdin).get('ta_mode','?'))")
STATUS=$(echo "$HEALTH_JSON" | py "import sys,json; print(json.load(sys.stdin).get('status','?'))")

QS_JSON=$(kms_get /QueueStatus || echo '{}')
QUEUE_DEPTH=$(echo "$QS_JSON" | py "import sys,json; print(json.load(sys.stdin).get('queue_depth',0))")
CB_OPEN=$(echo "$QS_JSON" | py "import sys,json; print(json.load(sys.stdin).get('circuit_breaker_open',False))")
CONSEC_FAIL=$(echo "$QS_JSON" | py "import sys,json; print(json.load(sys.stdin).get('consecutive_failures',0))")

# --- list all keys ---
KEYS_JSON=$(kms_post ListKeys '{}') || { echo -e "${RED}ListKeys failed (need KMS_API_KEY?)${NC}"; exit 1; }
KEY_IDS=($(echo "$KEYS_JSON" | py "
import sys, json
d = json.load(sys.stdin)
keys = d.get('Keys', [])
for k in keys:
    print(k['KeyId'])
"))
TOTAL_KEYS=${#KEY_IDS[@]}

# --- describe each key ---
KEYS_WITH_ADDR=0
KEYS_WITH_PASSKEY=0
KEYS_ENABLED=0
KEYS_DISABLED=0
CREATION_DATES=()
KEY_DETAILS=()

for kid in "${KEY_IDS[@]}"; do
    DESC=$(kms_post DescribeKey "{\"KeyId\":\"$kid\"}" 2>/dev/null || echo '{}')
    META=$(echo "$DESC" | py "
import sys, json
d = json.load(sys.stdin)
m = d.get('KeyMetadata', {})
addr = m.get('Address', '')
pk = m.get('PasskeyPublicKey', '')
enabled = m.get('Enabled', False)
created = m.get('CreationDate', '?')
desc = m.get('Description', '')
has_addr = 1 if addr and addr != '' else 0
has_pk = 1 if pk and pk != '' else 0
en = 1 if enabled else 0
# short_id | has_addr | has_passkey | enabled | created | description
print(f'{m.get(\"KeyId\",\"?\")[:8]}|{has_addr}|{has_pk}|{en}|{created}|{desc}')
")
    IFS='|' read -r SHORT HAS_ADDR HAS_PK ENABLED CREATED DESC_TEXT <<< "$META"
    if [ "$HAS_ADDR" = "1" ]; then KEYS_WITH_ADDR=$((KEYS_WITH_ADDR+1)); fi
    if [ "$HAS_PK" = "1" ]; then KEYS_WITH_PASSKEY=$((KEYS_WITH_PASSKEY+1)); fi
    if [ "$ENABLED" = "1" ]; then KEYS_ENABLED=$((KEYS_ENABLED+1)); else KEYS_DISABLED=$((KEYS_DISABLED+1)); fi
    CREATION_DATES+=("$CREATED")
    KEY_DETAILS+=("$SHORT|$HAS_ADDR|$HAS_PK|$ENABLED|$CREATED|$DESC_TEXT")
done

# --- DK2 system info (if SSH reachable) ---
UPTIME="n/a"
DISK="n/a"
MEM="n/a"
TEE_STORAGE="n/a"
DK2_IP=$(echo "$HOST" | cut -d: -f1)
if ssh -o ConnectTimeout=2 -o BatchMode=yes root@$DK2_IP true 2>/dev/null; then
    UPTIME=$(ssh root@$DK2_IP "uptime -p" 2>/dev/null || echo "n/a")
    DISK=$(ssh root@$DK2_IP "df -h / | tail -1 | awk '{print \$3\"/\"\$2\" (\"\$5\" used)\"}'" 2>/dev/null || echo "n/a")
    MEM=$(ssh root@$DK2_IP "free -m | awk '/Mem:/{printf \"%dM/%dM (%d%%)\", \$3, \$2, \$3*100/\$2}'" 2>/dev/null || echo "n/a")
    TEE_FILES=$(ssh root@$DK2_IP "ls /data/tee/ 2>/dev/null | wc -l" 2>/dev/null || echo "?")
    TEE_SIZE=$(ssh root@$DK2_IP "du -sh /data/tee/ 2>/dev/null | cut -f1" 2>/dev/null || echo "?")
    TEE_STORAGE="${TEE_FILES} files, ${TEE_SIZE}"
fi

# --- output ---
NOW=$(date '+%Y-%m-%d %H:%M:%S')

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║           KMS Daily Statistics Report            ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}Generated:${NC}  $NOW"
echo -e "${CYAN}Endpoint:${NC}   $BASE"
echo ""

echo -e "${BOLD}── Service ─────────────────────────────────────────${NC}"
printf "  %-24s %s\n" "Version:" "$VERSION"
printf "  %-24s %s\n" "Status:" "$STATUS"
printf "  %-24s %s\n" "TA Mode:" "$TA_MODE"
printf "  %-24s %s\n" "Queue Depth:" "$QUEUE_DEPTH"
printf "  %-24s %s\n" "Circuit Breaker:" "$CB_OPEN"
printf "  %-24s %s\n" "Consecutive Failures:" "$CONSEC_FAIL"
echo ""

echo -e "${BOLD}── System ──────────────────────────────────────────${NC}"
printf "  %-24s %s\n" "Uptime:" "$UPTIME"
printf "  %-24s %s\n" "Disk:" "$DISK"
printf "  %-24s %s\n" "Memory:" "$MEM"
printf "  %-24s %s\n" "TEE Secure Storage:" "$TEE_STORAGE"
echo ""

echo -e "${BOLD}── Keys Summary ────────────────────────────────────${NC}"
printf "  %-24s %s\n" "Total Keys:" "$TOTAL_KEYS"
printf "  %-24s %s\n" "Enabled:" "$KEYS_ENABLED"
printf "  %-24s %s\n" "Disabled:" "$KEYS_DISABLED"
printf "  %-24s %s\n" "With Address:" "$KEYS_WITH_ADDR"
printf "  %-24s %s\n" "With PassKey:" "$KEYS_WITH_PASSKEY"
printf "  %-24s %s\n" "Without Address:" "$((TOTAL_KEYS - KEYS_WITH_ADDR))"
echo ""

if [ "$TOTAL_KEYS" -gt 0 ]; then
    echo -e "${BOLD}── Key Details ─────────────────────────────────────${NC}"
    printf "  ${DIM}%-10s %-6s %-8s %-8s %-22s %s${NC}\n" "KeyId" "Addr" "PassKey" "Enabled" "Created" "Description"
    printf "  ${DIM}%-10s %-6s %-8s %-8s %-22s %s${NC}\n" "--------" "----" "-------" "-------" "--------------------" "-----------"
    for detail in "${KEY_DETAILS[@]}"; do
        IFS='|' read -r SHORT HAS_ADDR HAS_PK ENABLED CREATED DESC_TEXT <<< "$detail"
        ADDR_SYM=$( [ "$HAS_ADDR" = "1" ] && echo -e "${GREEN}yes${NC}" || echo -e "${DIM}no${NC}" )
        PK_SYM=$( [ "$HAS_PK" = "1" ] && echo -e "${GREEN}yes${NC}" || echo -e "${DIM}no${NC}" )
        EN_SYM=$( [ "$ENABLED" = "1" ] && echo -e "${GREEN}yes${NC}" || echo -e "${RED}no${NC}" )
        printf "  %-10s %-15s %-17s %-17s %-22s %s\n" "${SHORT}…" "$ADDR_SYM" "$PK_SYM" "$EN_SYM" "$CREATED" "$DESC_TEXT"
    done
    echo ""
fi

echo -e "${DIM}── End of report ───────────────────────────────────${NC}"
echo ""
