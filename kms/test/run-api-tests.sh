#!/bin/bash
# KMS Full API Chain Test with Real P-256 PassKey Data
# Usage: ./run-api-tests.sh [host:port]
# Default: 192.168.7.2:3000
#
# Prerequisites:
#   pip3 install cryptography   (for p256_helper.py)

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOST="${1:-192.168.7.2:3000}"
BASE="http://$HOST"
HDR_JSON="Content-Type: application/json"
LAST_BODY=""

# Load test fixture
FIXTURE="$SCRIPT_DIR/test-fixtures/user1.json"
if [ ! -f "$FIXTURE" ]; then
    echo "Generating test fixtures..."
    python3 "$SCRIPT_DIR/p256_helper.py" gen-all
fi

USER1_PUBKEY=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['public_key_hex'])")
USER1_PEM=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['private_key_pem'])")

# macOS-compatible millisecond timestamp
now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

# Result collection
declare -a API_NAMES=()
declare -a API_TIMES=()
declare -a API_STATUS=()
TOTAL_PASS=0
TOTAL_FAIL=0

timed_curl() {
    local label="$1"; shift
    local start end elapsed body http_code
    start=$(now_ms)
    body=$(curl -s -w '\n%{http_code}' --max-time 300 "$@" 2>&1)
    end=$(now_ms)
    http_code=$(echo "$body" | tail -1)
    body=$(echo "$body" | sed '$d')
    elapsed=$(( end - start ))

    API_NAMES+=("$label")
    API_TIMES+=("$elapsed")

    if echo "$body" | grep -q '"error"'; then
        API_STATUS+=("FAIL")
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
        printf "${RED}FAIL${NC} %-32s %6d ms  %s\n" "$label" "$elapsed" "$(echo "$body" | head -c 120)"
    else
        API_STATUS+=("OK")
        TOTAL_PASS=$((TOTAL_PASS + 1))
        printf "${GREEN} OK ${NC} %-32s %6d ms  %s\n" "$label" "$elapsed" "$(echo "$body" | head -c 120)"
    fi
    LAST_BODY="$body"
}

# Generate fresh assertion for signing requests
make_assertion() {
    python3 "$SCRIPT_DIR/p256_helper.py" assertion "$USER1_PEM"
}

# API key header (if configured)
API_KEY_HDR=""
if [ -n "$KMS_API_KEY" ]; then
    API_KEY_HDR="-H x-api-key:$KMS_API_KEY"
fi

echo ""
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  KMS Full API Test Suite (Real P-256 PassKey)${NC}"
echo "${BOLD}  Target: ${CYAN}$BASE${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}================================================================${NC}"
echo ""

# ── Phase 1: Infrastructure ──
echo "${YELLOW}[Phase 1] Infrastructure${NC}"
timed_curl "GET  /health" "$BASE/health" $API_KEY_HDR
timed_curl "GET  /QueueStatus" "$BASE/QueueStatus" $API_KEY_HDR
echo ""

# ── Phase 2: CreateKey (with real P-256 passkey) ──
echo "${YELLOW}[Phase 2] Wallet Lifecycle${NC}"
timed_curl "POST /CreateKey" \
    -X POST "$BASE/CreateKey" \
    -H "$HDR_JSON" \
    -H "x-amz-target: TrentService.CreateKey" \
    $API_KEY_HDR \
    -d "{\"Description\":\"api-test-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$USER1_PUBKEY\"}"

KEY_ID=$(echo "$LAST_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])" 2>/dev/null || echo "")
if [ -z "$KEY_ID" ]; then
    echo "${RED}CreateKey failed, cannot continue${NC}"
    exit 1
fi
echo "  KeyId: ${CYAN}$KEY_ID${NC}"
echo ""

# ── Phase 3: Poll KeyStatus ──
echo "${YELLOW}[Phase 3] Background Derivation${NC}"
POLL_START=$(now_ms)
POLL_COUNT=0
DERIVED_ADDR=""
while true; do
    POLL_COUNT=$((POLL_COUNT + 1))
    RESP=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$KEY_ID" $API_KEY_HDR 2>/dev/null || echo '{"status":"error"}')
    STATUS=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Status',d.get('status','unknown')))" 2>/dev/null || echo "unknown")

    if [ "$STATUS" = "ready" ]; then
        DERIVED_ADDR=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',d.get('address','')))" 2>/dev/null || echo "")
        break
    fi

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
    TOTAL_PASS=$((TOTAL_PASS + 1))
    printf "${GREEN} OK ${NC} %-32s %6d ms  addr=%s\n" "KeyStatus poll" "$DERIVE_TOTAL" "$DERIVED_ADDR"
else
    API_STATUS+=("FAIL")
    TOTAL_FAIL=$((TOTAL_FAIL + 1))
    printf "${RED}FAIL${NC} %-32s %6d ms\n" "KeyStatus poll" "$DERIVE_TOTAL"
fi
echo ""

# ── Phase 4: Metadata (CA-only, no passkey needed) ──
echo "${YELLOW}[Phase 4] Metadata Queries${NC}"
timed_curl "POST /ListKeys" \
    -X POST "$BASE/ListKeys" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.ListKeys" $API_KEY_HDR \
    -d '{}'

timed_curl "POST /DescribeKey" \
    -X POST "$BASE/DescribeKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DescribeKey" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\"}"

timed_curl "POST /GetPublicKey" \
    -X POST "$BASE/GetPublicKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.GetPublicKey" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\"}"
echo ""

# ── Phase 5: Key Operations (with real passkey assertion) ──
echo "${YELLOW}[Phase 5] Key Operations (real passkey)${NC}"

# DeriveAddress
ASSERTION=$(make_assertion)
AUTH_DATA=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
CDH=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
SIG_R=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
SIG_S=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")

timed_curl "POST /DeriveAddress (2nd)" \
    -X POST "$BASE/DeriveAddress" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DeriveAddress" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"Passkey\":{\"AuthenticatorData\":\"$AUTH_DATA\",\"ClientDataHash\":\"$CDH\",\"SignatureR\":\"$SIG_R\",\"SignatureS\":\"$SIG_S\"}}"

# SignHash
ASSERTION=$(make_assertion)
AUTH_DATA=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
CDH=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
SIG_R=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
SIG_S=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")

timed_curl "POST /SignHash" \
    -X POST "$BASE/SignHash" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.SignHash" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"Passkey\":{\"AuthenticatorData\":\"$AUTH_DATA\",\"ClientDataHash\":\"$CDH\",\"SignatureR\":\"$SIG_R\",\"SignatureS\":\"$SIG_S\"}}"

# Sign (message)
ASSERTION=$(make_assertion)
AUTH_DATA=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
CDH=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
SIG_R=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
SIG_S=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")

timed_curl "POST /Sign (message)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f20576f726c64\",\"Passkey\":{\"AuthenticatorData\":\"$AUTH_DATA\",\"ClientDataHash\":\"$CDH\",\"SignatureR\":\"$SIG_R\",\"SignatureS\":\"$SIG_S\"}}"

# Sign (transaction)
ASSERTION=$(make_assertion)
AUTH_DATA=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
CDH=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
SIG_R=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
SIG_S=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")

timed_curl "POST /Sign (transaction)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Transaction\":{\"ChainId\":1,\"Nonce\":0,\"To\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\",\"Value\":1000000000000000000,\"GasPrice\":20000000000,\"GasLimit\":21000},\"Passkey\":{\"AuthenticatorData\":\"$AUTH_DATA\",\"ClientDataHash\":\"$CDH\",\"SignatureR\":\"$SIG_R\",\"SignatureS\":\"$SIG_S\"}}"
echo ""

# ── Phase 6: Negative Tests ──
echo "${YELLOW}[Phase 6] Negative Tests${NC}"

# Bad passkey assertion (wrong signature)
timed_curl "POST /SignHash (bad sig)" \
    -X POST "$BASE/SignHash" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.SignHash" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"Passkey\":{\"AuthenticatorData\":\"0000000000000000000000000000000000000000000000000000000000000000000000000000\",\"ClientDataHash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"SignatureR\":\"0000000000000000000000000000000000000000000000000000000000000001\",\"SignatureS\":\"0000000000000000000000000000000000000000000000000000000000000001\"}}"
# This should FAIL — we expect it to fail (CA pre-verify should catch it)
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    # "FAIL" is expected here — flip status
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — CA pre-verify correctly rejected invalid passkey)"
fi

# Non-existent key
timed_curl "POST /DescribeKey (404)" \
    -X POST "$BASE/DescribeKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DescribeKey" $API_KEY_HDR \
    -d '{"KeyId":"00000000-0000-0000-0000-000000000000"}'
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — key not found)"
fi
echo ""

# ── Phase 7: Cleanup ──
echo "${YELLOW}[Phase 7] Cleanup${NC}"
DEL_START=$(now_ms)
DEL_RESULT=$(ssh -o ConnectTimeout=5 root@192.168.7.2 "/usr/local/bin/kms remove-wallet -w $KEY_ID" 2>&1 || echo "SKIP")
DEL_END=$(now_ms)
DEL_TIME=$((DEL_END - DEL_START))

API_NAMES+=("CLI remove-wallet")
API_TIMES+=("$DEL_TIME")
if echo "$DEL_RESULT" | grep -qi "SKIP\|error\|fail"; then
    API_STATUS+=("SKIP")
    printf "${YELLOW}SKIP${NC} %-32s %6d ms  %s\n" "CLI remove-wallet" "$DEL_TIME" "$(echo "$DEL_RESULT" | head -c 80)"
else
    API_STATUS+=("OK")
    TOTAL_PASS=$((TOTAL_PASS + 1))
    printf "${GREEN} OK ${NC} %-32s %6d ms\n" "CLI remove-wallet" "$DEL_TIME"
fi
echo ""

# ── Summary ──
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  SUMMARY${NC}"
echo "${BOLD}================================================================${NC}"
printf "%-36s %8s %6s\n" "Endpoint" "Time" "Status"
printf "%-36s %8s %6s\n" "────────────────────────────────────" "────────" "──────"

TOTAL_MS=0
for i in "${!API_NAMES[@]}"; do
    ms=${API_TIMES[$i]}
    st=${API_STATUS[$i]}
    TOTAL_MS=$((TOTAL_MS + ms))

    if [ "$ms" -ge 10000 ]; then
        time_str="$(echo "scale=1; $ms/1000" | bc)s"
    else
        time_str="${ms}ms"
    fi

    if [ "$st" = "OK" ]; then
        printf "%-36s ${GREEN}%8s${NC} ${GREEN}%6s${NC}\n" "${API_NAMES[$i]}" "$time_str" "$st"
    elif [ "$st" = "SKIP" ]; then
        printf "%-36s ${YELLOW}%8s${NC} ${YELLOW}%6s${NC}\n" "${API_NAMES[$i]}" "$time_str" "$st"
    else
        printf "%-36s ${RED}%8s${NC} ${RED}%6s${NC}\n" "${API_NAMES[$i]}" "$time_str" "$st"
    fi
done
printf "%-36s %8s %6s\n" "────────────────────────────────────" "────────" "──────"
TOTAL_SEC=$(echo "scale=1; $TOTAL_MS/1000" | bc)
TOTAL_COUNT=${#API_NAMES[@]}
printf "%-36s %8s  %d/%d pass\n" "TOTAL" "${TOTAL_SEC}s" "$TOTAL_PASS" "$TOTAL_COUNT"
echo ""

if [ "$TOTAL_FAIL" -gt 0 ]; then
    echo "${RED}${TOTAL_FAIL} test(s) failed${NC}"
    exit 1
else
    echo "${GREEN}All tests passed${NC}"
fi
