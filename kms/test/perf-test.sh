#!/bin/bash
# KMS Performance Benchmark with Real P-256 PassKey
# Runs each operation N times, collects avg/min/max.
# Usage: ./perf-test.sh [host:port] [rounds]
# Default: 192.168.7.2:3000, 5 rounds

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOST="${1:-192.168.7.2:3000}"
ROUNDS="${2:-5}"
BASE="http://$HOST"
HDR_JSON="Content-Type: application/json"

# Load test fixture
FIXTURE="$SCRIPT_DIR/test-fixtures/user1.json"
if [ ! -f "$FIXTURE" ]; then
    python3 "$SCRIPT_DIR/p256_helper.py" gen-all
fi
USER1_PUBKEY=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['public_key_hex'])")
USER1_PEM=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['private_key_pem'])")

now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
make_assertion() { python3 "$SCRIPT_DIR/p256_helper.py" assertion "$USER1_PEM"; }

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

API_KEY_HDR=""
if [ -n "$KMS_API_KEY" ]; then
    API_KEY_HDR="-H x-api-key:$KMS_API_KEY"
fi

echo ""
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  KMS Performance Benchmark${NC}"
echo "${BOLD}  Target: ${CYAN}$BASE${NC}   Rounds: ${CYAN}$ROUNDS${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}================================================================${NC}"
echo ""

# ── Step 1: Create a test wallet ──
echo "${YELLOW}Setting up test wallet...${NC}"
CREATE_RESP=$(curl -s --max-time 60 \
    -X POST "$BASE/CreateKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.CreateKey" $API_KEY_HDR \
    -d "{\"Description\":\"perf-bench-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$USER1_PUBKEY\"}")

KEY_ID=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])" 2>/dev/null || echo "")
if [ -z "$KEY_ID" ]; then
    echo "${RED}CreateKey failed: $CREATE_RESP${NC}"
    exit 1
fi
echo "  KeyId: ${CYAN}$KEY_ID${NC}"

# Wait for background derivation
echo "  Waiting for derivation..."
for i in $(seq 1 60); do
    RESP=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$KEY_ID" $API_KEY_HDR 2>/dev/null)
    STATUS=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Status',''))" 2>/dev/null || echo "")
    if [ "$STATUS" = "ready" ]; then
        break
    fi
    printf "  poll #%d  status=%s\r" "$i" "$STATUS"
    sleep 3
done
echo "  Wallet ready.                    "
echo ""

# ── Step 2: Benchmark each operation ──
declare -A OP_TIMES
declare -A OP_SUMS
declare -A OP_MINS
declare -A OP_MAXS

benchmark_op() {
    local name="$1"
    shift
    local curl_args=("$@")

    OP_SUMS[$name]=0
    OP_MINS[$name]=999999
    OP_MAXS[$name]=0

    for r in $(seq 1 $ROUNDS); do
        # Generate fresh assertion for each call
        ASSERTION=$(make_assertion)
        AUTH_DATA=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
        CDH=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
        SIG_R=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
        SIG_S=$(echo "$ASSERTION" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")

        # Replace PASSKEY_PLACEHOLDER in args
        local final_args=()
        for arg in "${curl_args[@]}"; do
            arg="${arg//AUTH_DATA_PH/$AUTH_DATA}"
            arg="${arg//CDH_PH/$CDH}"
            arg="${arg//SIGR_PH/$SIG_R}"
            arg="${arg//SIGS_PH/$SIG_S}"
            final_args+=("$arg")
        done

        local start end elapsed
        start=$(now_ms)
        curl -s --max-time 300 "${final_args[@]}" > /dev/null 2>&1
        end=$(now_ms)
        elapsed=$((end - start))

        OP_SUMS[$name]=$(( ${OP_SUMS[$name]} + elapsed ))
        [ "$elapsed" -lt "${OP_MINS[$name]}" ] && OP_MINS[$name]=$elapsed
        [ "$elapsed" -gt "${OP_MAXS[$name]}" ] && OP_MAXS[$name]=$elapsed

        printf "  %-24s round %d/%d: %dms\r" "$name" "$r" "$ROUNDS" "$elapsed"
    done

    local avg=$(( ${OP_SUMS[$name]} / ROUNDS ))
    printf "${GREEN} OK ${NC} %-24s  avg=%4dms  min=%4dms  max=%4dms\n" "$name" "$avg" "${OP_MINS[$name]}" "${OP_MAXS[$name]}"
}

echo "${YELLOW}Running benchmarks ($ROUNDS rounds each)...${NC}"
echo ""

# SignHash
benchmark_op "SignHash" \
    -X POST "$BASE/SignHash" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.SignHash" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"Passkey\":{\"AuthenticatorData\":\"AUTH_DATA_PH\",\"ClientDataHash\":\"CDH_PH\",\"SignatureR\":\"SIGR_PH\",\"SignatureS\":\"SIGS_PH\"}}"

# Sign (message)
benchmark_op "Sign (message)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f20576f726c64\",\"Passkey\":{\"AuthenticatorData\":\"AUTH_DATA_PH\",\"ClientDataHash\":\"CDH_PH\",\"SignatureR\":\"SIGR_PH\",\"SignatureS\":\"SIGS_PH\"}}"

# Sign (transaction)
benchmark_op "Sign (transaction)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Transaction\":{\"ChainId\":1,\"Nonce\":0,\"To\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\",\"Value\":1000000000000000000,\"GasPrice\":20000000000,\"GasLimit\":21000},\"Passkey\":{\"AuthenticatorData\":\"AUTH_DATA_PH\",\"ClientDataHash\":\"CDH_PH\",\"SignatureR\":\"SIGR_PH\",\"SignatureS\":\"SIGS_PH\"}}"

# DeriveAddress
benchmark_op "DeriveAddress" \
    -X POST "$BASE/DeriveAddress" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DeriveAddress" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/2\",\"Passkey\":{\"AuthenticatorData\":\"AUTH_DATA_PH\",\"ClientDataHash\":\"CDH_PH\",\"SignatureR\":\"SIGR_PH\",\"SignatureS\":\"SIGS_PH\"}}"

# CA-only ops (no passkey needed)
echo ""
echo "${YELLOW}CA-only operations:${NC}"
for r in $(seq 1 $ROUNDS); do
    start=$(now_ms)
    curl -s "$BASE/health" > /dev/null
    end=$(now_ms)
    if [ "$r" -eq 1 ]; then HEALTH_SUM=$((end - start)); HEALTH_MIN=$((end - start)); HEALTH_MAX=$((end - start))
    else
        HEALTH_SUM=$((HEALTH_SUM + end - start))
        [ $((end - start)) -lt $HEALTH_MIN ] && HEALTH_MIN=$((end - start))
        [ $((end - start)) -gt $HEALTH_MAX ] && HEALTH_MAX=$((end - start))
    fi
done
printf "${GREEN} OK ${NC} %-24s  avg=%4dms  min=%4dms  max=%4dms\n" "health" "$((HEALTH_SUM / ROUNDS))" "$HEALTH_MIN" "$HEALTH_MAX"

# DescribeKey
for r in $(seq 1 $ROUNDS); do
    start=$(now_ms)
    curl -s -X POST "$BASE/DescribeKey" -H "$HDR_JSON" -H "x-amz-target: TrentService.DescribeKey" $API_KEY_HDR -d "{\"KeyId\":\"$KEY_ID\"}" > /dev/null
    end=$(now_ms)
    if [ "$r" -eq 1 ]; then DESC_SUM=$((end - start)); DESC_MIN=$((end - start)); DESC_MAX=$((end - start))
    else
        DESC_SUM=$((DESC_SUM + end - start))
        [ $((end - start)) -lt $DESC_MIN ] && DESC_MIN=$((end - start))
        [ $((end - start)) -gt $DESC_MAX ] && DESC_MAX=$((end - start))
    fi
done
printf "${GREEN} OK ${NC} %-24s  avg=%4dms  min=%4dms  max=%4dms\n" "DescribeKey" "$((DESC_SUM / ROUNDS))" "$DESC_MIN" "$DESC_MAX"

echo ""

# ── Step 3: Cleanup ──
echo "${YELLOW}Cleaning up...${NC}"
ssh -o ConnectTimeout=5 root@192.168.7.2 "/usr/local/bin/kms remove-wallet -w $KEY_ID" 2>/dev/null || true
echo "Done."
echo ""

# ── Summary table for markdown ──
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  Performance Results (Markdown)${NC}"
echo "${BOLD}================================================================${NC}"
echo ""
echo "| Operation | Avg | Min | Max | Rounds |"
echo "|-----------|-----|-----|-----|--------|"
for name in "SignHash" "Sign (message)" "Sign (transaction)" "DeriveAddress"; do
    if [ -n "${OP_SUMS[$name]+x}" ]; then
        avg=$(( ${OP_SUMS[$name]} / ROUNDS ))
        echo "| $name | ${avg}ms | ${OP_MINS[$name]}ms | ${OP_MAXS[$name]}ms | $ROUNDS |"
    fi
done
echo "| health | $((HEALTH_SUM / ROUNDS))ms | ${HEALTH_MIN}ms | ${HEALTH_MAX}ms | $ROUNDS |"
echo "| DescribeKey | $((DESC_SUM / ROUNDS))ms | ${DESC_MIN}ms | ${DESC_MAX}ms | $ROUNDS |"
echo ""
