#!/bin/bash
# KMS Full API Chain Test — measures timing for every endpoint
# Usage: ./test-full-api.sh [host:port]
# Default: 192.168.7.2:3000

set -eo pipefail

HOST="${1:-192.168.7.2:3000}"
BASE="http://$HOST"
HDR_JSON="Content-Type: application/json"
LAST_BODY=""

# macOS-compatible millisecond timestamp
now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

# Result collection
declare -a API_NAMES=()
declare -a API_TIMES=()
declare -a API_STATUS=()

timed_curl() {
    local label="$1"; shift
    local start end elapsed body
    start=$(now_ms)
    body=$(curl -s -w '\n%{http_code}' --max-time 300 "$@" 2>&1)
    end=$(now_ms)
    body=$(echo "$body" | sed '$d')
    elapsed=$(( end - start ))

    API_NAMES+=("$label")
    API_TIMES+=("$elapsed")

    if echo "$body" | grep -q '"error"'; then
        API_STATUS+=("FAIL")
        printf "${RED}FAIL${NC} %-28s %6d ms  %s\n" "$label" "$elapsed" "$(echo "$body" | head -c 120)"
    else
        API_STATUS+=("OK")
        printf "${GREEN} OK ${NC} %-28s %6d ms  %s\n" "$label" "$elapsed" "$(echo "$body" | head -c 120)"
    fi
    # Export body for caller
    LAST_BODY="$body"
}

echo ""
echo "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo "${BOLD}  KMS Full API Chain Test${NC}"
echo "${BOLD}  Target: ${CYAN}$BASE${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo ""

# ─────────────────────────────────────────────
# 1. Health
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 1] Infrastructure${NC}"
timed_curl "GET  /health" "$BASE/health"
timed_curl "GET  /QueueStatus" "$BASE/QueueStatus"
echo ""

# ─────────────────────────────────────────────
# 2. CreateKey
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 2] Wallet Lifecycle${NC}"
timed_curl "POST /CreateKey" \
    -X POST "$BASE/CreateKey" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.CreateKey" \
    -d '{"Description":"api-bench-test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'

KEY_ID=$(echo "$LAST_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])" 2>/dev/null || echo "")
if [ -z "$KEY_ID" ]; then
    echo "${RED}CreateKey failed, cannot continue${NC}"
    exit 1
fi
echo "  KeyId: ${CYAN}$KEY_ID${NC}"
echo ""

# ─────────────────────────────────────────────
# 3. Poll KeyStatus until ready (measures first derivation + seed cache)
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 3] Background Derivation (PBKDF2 + BIP32, first call)${NC}"
POLL_START=$(now_ms)
POLL_COUNT=0
DERIVED_ADDR=""
while true; do
    POLL_COUNT=$((POLL_COUNT + 1))
    RESP=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$KEY_ID" 2>/dev/null || echo '{"status":"error"}')
    STATUS=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Status',d.get('status','unknown')))" 2>/dev/null || echo "unknown")

    if [ "$STATUS" = "ready" ]; then
        DERIVED_ADDR=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',d.get('address','')))" 2>/dev/null || echo "")
        break
    fi

    # Timeout after 3 minutes
    NOW=$(now_ms)
    if [ $((NOW - POLL_START)) -gt 180000 ]; then
        echo "${RED}  Timeout waiting for derivation (>180s)${NC}"
        break
    fi
    printf "  poll #%d  status=%s\r" "$POLL_COUNT" "$STATUS"
    sleep 3
done
POLL_END=$(now_ms)
DERIVE_TOTAL=$((POLL_END - POLL_START))

API_NAMES+=("KeyStatus poll (derivation)")
API_TIMES+=("$DERIVE_TOTAL")
if [ -n "$DERIVED_ADDR" ]; then
    API_STATUS+=("OK")
    printf "${GREEN} OK ${NC} %-28s %6d ms  addr=%s\n" "KeyStatus poll (derivation)" "$DERIVE_TOTAL" "$DERIVED_ADDR"
else
    API_STATUS+=("FAIL")
    printf "${RED}FAIL${NC} %-28s %6d ms\n" "KeyStatus poll (derivation)" "$DERIVE_TOTAL"
fi
echo ""

# ─────────────────────────────────────────────
# 4. Metadata queries
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 4] Metadata Queries${NC}"
timed_curl "POST /ListKeys" \
    -X POST "$BASE/ListKeys" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.ListKeys" \
    -d '{}'

timed_curl "POST /DescribeKey" \
    -X POST "$BASE/DescribeKey" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.DescribeKey" \
    -d "{\"KeyId\":\"$KEY_ID\"}"

timed_curl "POST /GetPublicKey" \
    -X POST "$BASE/GetPublicKey" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.GetPublicKey" \
    -d "{\"KeyId\":\"$KEY_ID\"}"
echo ""

# ─────────────────────────────────────────────
# 5. Key operations (with seed cache — the critical benchmark)
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 5] Key Operations (seed cached)${NC}"

timed_curl "POST /DeriveAddress (2nd)" \
    -X POST "$BASE/DeriveAddress" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.DeriveAddress" \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\"}"

timed_curl "POST /Sign (message)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.Sign" \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f20576f726c64\",\"MessageType\":\"DIGEST\",\"SigningAlgorithm\":\"ECDSA_SHA_256\"}"

timed_curl "POST /SignHash" \
    -X POST "$BASE/SignHash" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.SignHash" \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\"}"
echo ""

# ─────────────────────────────────────────────
# 6. PassKey registration
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 6] PassKey${NC}"
# Generate a dummy P-256 uncompressed public key (65 bytes = 04 + 32x + 32y)
DUMMY_P256_PUBKEY="04$(openssl rand -hex 32)$(openssl rand -hex 32)"

timed_curl "POST /ChangePasskey" \
    -X POST "$BASE/ChangePasskey" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.ChangePasskey" \
    -d "{\"KeyId\":\"$KEY_ID\",\"PasskeyPublicKey\":\"$DUMMY_P256_PUBKEY\",\"CredentialId\":\"test-cred-001\"}"
echo ""

# ─────────────────────────────────────────────
# 7. Cleanup test wallet via CLI
# ─────────────────────────────────────────────
echo "${YELLOW}[Phase 7] Cleanup${NC}"
DEL_START=$(now_ms)
DEL_RESULT=$(ssh -o ConnectTimeout=5 root@192.168.7.2 "/usr/local/bin/kms remove-wallet -w $KEY_ID" 2>&1 || echo "FAIL")
DEL_END=$(now_ms)
DEL_TIME=$((DEL_END - DEL_START))

API_NAMES+=("CLI remove-wallet")
API_TIMES+=("$DEL_TIME")
if echo "$DEL_RESULT" | grep -qi "fail\|error"; then
    API_STATUS+=("FAIL")
    printf "${RED}FAIL${NC} %-28s %6d ms  %s\n" "CLI remove-wallet" "$DEL_TIME" "$DEL_RESULT"
else
    API_STATUS+=("OK")
    printf "${GREEN} OK ${NC} %-28s %6d ms  %s\n" "CLI remove-wallet" "$DEL_TIME" "$(echo "$DEL_RESULT" | head -c 80)"
fi
echo ""

# ─────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────
echo "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo "${BOLD}  SUMMARY${NC}"
echo "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
printf "%-34s %8s %6s\n" "Endpoint" "Time" "Status"
printf "%-34s %8s %6s\n" "──────────────────────────────────" "────────" "──────"

TOTAL_MS=0
FAIL_COUNT=0
for i in "${!API_NAMES[@]}"; do
    ms=${API_TIMES[$i]}
    st=${API_STATUS[$i]}
    TOTAL_MS=$((TOTAL_MS + ms))
    [ "$st" = "FAIL" ] && FAIL_COUNT=$((FAIL_COUNT + 1))

    if [ "$ms" -ge 10000 ]; then
        time_str="$(echo "scale=1; $ms/1000" | bc)s"
    else
        time_str="${ms}ms"
    fi

    if [ "$st" = "OK" ]; then
        printf "%-34s ${GREEN}%8s${NC} ${GREEN}%6s${NC}\n" "${API_NAMES[$i]}" "$time_str" "$st"
    else
        printf "%-34s ${RED}%8s${NC} ${RED}%6s${NC}\n" "${API_NAMES[$i]}" "$time_str" "$st"
    fi
done
printf "%-34s %8s %6s\n" "──────────────────────────────────" "────────" "──────"
TOTAL_SEC=$(echo "scale=1; $TOTAL_MS/1000" | bc)
printf "%-34s %8s  %d/%d pass\n" "TOTAL" "${TOTAL_SEC}s" "$((${#API_NAMES[@]} - FAIL_COUNT))" "${#API_NAMES[@]}"
echo ""

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "${RED}${FAIL_COUNT} test(s) failed${NC}"
    exit 1
else
    echo "${GREEN}All tests passed${NC}"
fi
