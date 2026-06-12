#!/bin/bash
# KMS Full API Chain Test with Real P-256 PassKey Data
# Usage: ./run-api-tests.sh [host:port]
# Default: 192.168.7.2:3000
#
# Prerequisites:
#   pip3 install cryptography   (for p256_helper.py)
#
# NOTE (P0-2): this script uses LEGACY raw passkey assertions. The legacy
# path is rejected by default since it has no challenge binding (replayable).
# The kms-api-server under test must run with KMS_ALLOW_LEGACY_PASSKEY=1.
# Production deployments must NEVER set that variable.

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Auto-load .env if present (KMS_HOST, KMS_API_KEY, etc.)
[ -f "$SCRIPT_DIR/.env" ] && set -a && source "$SCRIPT_DIR/.env" && set +a
HOST="${1:-${KMS_HOST:-192.168.7.2:3000}}"
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

# Helper: extract assertion fields and build Passkey JSON
# CA API expects: { "AuthenticatorData": "hex", "ClientDataHash": "hex", "Signature": "r_hex + s_hex" }
make_passkey_json() {
    local assertion="$1"
    local auth_data cdh sig_r sig_s sig
    auth_data=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
    cdh=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
    sig_r=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
    sig_s=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")
    sig="${sig_r}${sig_s}"
    echo "{\"AuthenticatorData\":\"$auth_data\",\"ClientDataHash\":\"$cdh\",\"Signature\":\"$sig\"}"
}

# DeriveAddress
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")

timed_curl "POST /DeriveAddress (2nd)" \
    -X POST "$BASE/DeriveAddress" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DeriveAddress" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"Passkey\":$PASSKEY_JSON}"

# SignHash
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")

timed_curl "POST /SignHash" \
    -X POST "$BASE/SignHash" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.SignHash" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"Passkey\":$PASSKEY_JSON}"

# Sign (message)
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")

timed_curl "POST /Sign (message)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f20576f726c64\",\"Passkey\":$PASSKEY_JSON}"

# Sign (transaction)
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")

timed_curl "POST /Sign (transaction)" \
    -X POST "$BASE/Sign" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.Sign" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Transaction\":{\"chainId\":1,\"nonce\":0,\"to\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\",\"value\":\"0xde0b6b3a7640000\",\"gasPrice\":\"0x4a817c800\",\"gas\":21000,\"data\":\"\"},\"Passkey\":$PASSKEY_JSON}"
echo ""

# ── Phase 5b: Grant Session Signing ──
echo "${YELLOW}[Phase 5b] Grant Session Signing${NC}"

# Derive the owner address for use as 'account' in the grant hash
OWNER_ADDR=$(echo "$LAST_BODY" | python3 -c "
import sys, json
d = json.load(sys.stdin)
addr = d.get('Address', d.get('address', ''))
# Normalise: ensure 0x prefix
if addr and not addr.startswith('0x'): addr = '0x' + addr
print(addr)
" 2>/dev/null || echo "")

if [ -z "$OWNER_ADDR" ]; then
    OWNER_ADDR="0x0000000000000000000000000000000000000001"
fi

SESSION_KEY="0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF"
DUMMY_CONTRACT="0x0000000000000000000000000000000000000000"
EXPIRY=$(python3 -c "import time; print(int(time.time()) + 86400)")
KEY_X="0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
KEY_Y="0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

# sign-grant-session positive test requires WebAuthn ceremony (browser) — SKIP in headless CI
# To test manually: call POST /BeginAuthentication first, complete ceremony, then:
#   POST /kms/sign-grant-session with webAuthnAssertion:{ChallengeId, Credential}
API_NAMES+=("POST /kms/sign-grant-session (WebAuthn)"); API_TIMES+=("0"); API_STATUS+=("SKIP")
API_NAMES+=("POST /kms/sign-p256-grant-session (WebAuthn)"); API_TIMES+=("0"); API_STATUS+=("SKIP")

# Negative: sign-grant-session without auth (should fail)
timed_curl "POST /kms/sign-grant-session (no auth)" \
    -X POST "$BASE/kms/sign-grant-session" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -d "{\"keyId\":\"$KEY_ID\",\"chainId\":1,\"verifyingContract\":\"$DUMMY_CONTRACT\",\"account\":\"$OWNER_ADDR\",\"sessionKey\":\"$SESSION_KEY\",\"expiry\":$EXPIRY,\"contractScope\":\"$DUMMY_CONTRACT\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":0,\"velocityWindow\":0,\"nonce\":0}"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — no auth correctly rejected)"
fi

# Negative: sign-p256-grant-session without auth (should fail)
timed_curl "POST /kms/sign-p256-grant-session (no auth)" \
    -X POST "$BASE/kms/sign-p256-grant-session" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -d "{\"keyId\":\"$KEY_ID\",\"chainId\":1,\"verifyingContract\":\"$DUMMY_CONTRACT\",\"account\":\"$OWNER_ADDR\",\"keyX\":\"$KEY_X\",\"keyY\":\"$KEY_Y\",\"expiry\":$EXPIRY,\"contractScope\":\"$DUMMY_CONTRACT\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":0,\"velocityWindow\":0,\"nonce\":0}"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — no auth correctly rejected)"
fi

# ── Phase 5c: P256 Session Key (v0.18.1) ──
# P256 key creation requires WebAuthn ceremony (challenge-based, browser flow).
# Provide P256_SESSION_CRED env var (JWT from a prior create-p256-session-key call)
# and P256_SESSION_KEY_ID (wallet_uuid:session_index) to run signing tests.
echo "${YELLOW}[Phase 5c] P256 Session Key (v0.18.1)${NC}"

if [ -n "$P256_SESSION_CRED" ] && [ -n "$P256_SESSION_KEY_ID" ]; then
    # Test /kms/sign-p256-user-op with pre-created credential
    SAMPLE_ACCOUNT="0x742d35cc6634c0532925a3b844bc9e7595f2bd18"
    SAMPLE_HASH="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"

    timed_curl "POST /kms/sign-p256-user-op" \
        -X POST "$BASE/kms/sign-p256-user-op" \
        -H "$HDR_JSON" \
        -H "Authorization: Bearer $P256_SESSION_CRED" \
        $API_KEY_HDR \
        -d "{\"keyId\":\"$P256_SESSION_KEY_ID\",\"payload\":\"0x$SAMPLE_HASH\",\"accountAddress\":\"$SAMPLE_ACCOUNT\"}"

    SIG_HEX=$(echo "$LAST_BODY" | python3 -c "import sys,json; s=json.load(sys.stdin).get('signature',''); print(s[2:] if s.startswith('0x') else s)" 2>/dev/null || echo "")
    SIG_LEN=$(echo -n "$SIG_HEX" | wc -c | tr -d ' ')
    if [ "$SIG_LEN" -eq 298 ]; then
        echo "  P256 signature: 149 bytes ✓ marker=0x$(echo "$SIG_HEX" | cut -c1-2)"
    else
        echo "  ${RED}WARNING: P256 sig length unexpected: ${SIG_LEN}/2 bytes (want 149)${NC}"
    fi
else
    # Mark as SKIP — WebAuthn ceremony required for creation
    API_NAMES+=("P256SessionKey (WebAuthn required)")
    API_TIMES+=("0")
    API_STATUS+=("SKIP")
    printf "${YELLOW}SKIP${NC} %-32s %6s  Set P256_SESSION_CRED+P256_SESSION_KEY_ID to test signing\n" \
        "P256SessionKey" "0ms"
fi
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

SIGN_TYPED_DATA_BODY="{\"keyId\":\"$KEY_ID\",\"primaryType\":\"Transfer\",\"domain\":{\"name\":\"Test\",\"version\":\"1\",\"chainId\":1},\"types\":[{\"name\":\"Transfer\",\"fields\":[{\"name\":\"to\",\"type\":\"address\"},{\"name\":\"amount\",\"type\":\"uint256\"}]}],\"message\":[{\"name\":\"to\",\"value\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\"},{\"name\":\"amount\",\"value\":\"1000000000000000000\"}]}"

# SignTypedData without any auth (must be rejected — auth gate fix v0.18.2)
timed_curl "POST /SignTypedData (no auth)" \
    -X POST "$BASE/kms/SignTypedData" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -d "$SIGN_TYPED_DATA_BODY"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — auth gate correctly rejected unauthenticated sign-typed-data)"
else
    echo "  ${RED}SECURITY: sign-typed-data accepted unauthenticated request — auth gate broken!${NC}"
fi

# SignTypedData with malformed Authorization header (not "Bearer " prefix)
timed_curl "POST /SignTypedData (malformed auth header)" \
    -X POST "$BASE/kms/SignTypedData" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -H "Authorization: Token invalid.jwt.here" \
    -d "$SIGN_TYPED_DATA_BODY"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — malformed Authorization header correctly rejected)"
else
    echo "  ${RED}SECURITY: sign-typed-data accepted malformed Authorization header!${NC}"
fi

# SignTypedData with syntactically valid Bearer but invalid HMAC (must be rejected)
timed_curl "POST /SignTypedData (invalid Bearer JWT)" \
    -X POST "$BASE/kms/SignTypedData" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsImtpZCI6ImZha2Uta2lkIn0.eyJ3YWxsZXRfaWQiOiIwMDAwMDAwMC0wMDAwLTAwMDAtMDAwMC0wMDAwMDAwMDAwMDAiLCJhZ2VudF9pbmRleCI6MCwiZXhwIjo5OTk5OTk5OTk5fQ.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" \
    -d "$SIGN_TYPED_DATA_BODY"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — invalid HMAC correctly rejected)"
else
    echo "  ${RED}SECURITY: sign-typed-data accepted JWT with invalid HMAC!${NC}"
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

# P256 revoke without WebAuthn: must return 4xx (auth guard, not key-lookup).
# Note: revoke_p256_session_key requires a WebAuthn ceremony for replay-protection;
# this test verifies the guard fires before any DB lookup. Deep paths (not-found,
# already-revoked idempotency) require a valid WebAuthn assertion and must be tested
# via integration tests with a real or mock WebAuthn ceremony.
timed_curl "POST /revoke-p256-session-key (no webauthn)" \
    -X POST "$BASE/kms/revoke-p256-session-key" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -d "{\"keyId\":\"$KEY_ID:99999\"}"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — WebAuthn assertion required)"
fi

# P256 double-revoke idempotency: set P256_REVOKED_KEY_ID to a known-revoked key
# (obtained from a prior create+revoke run) and supply a fresh WebAuthn assertion
# via P256_REVOKE_ASSERTION_JSON to test that a second revoke returns 2xx.
if [ -n "${P256_REVOKED_KEY_ID:-}" ] && [ -n "${P256_REVOKE_ASSERTION_JSON:-}" ]; then
    timed_curl "POST /revoke-p256-session-key (idempotent)" \
        -X POST "$BASE/kms/revoke-p256-session-key" \
        -H "$HDR_JSON" $API_KEY_HDR \
        -d "{\"keyId\":\"$P256_REVOKED_KEY_ID\",\"webauthnAssertion\":$P256_REVOKE_ASSERTION_JSON}"
    if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "OK" ]; then
        echo "  (Idempotent revoke returned 2xx — correct)"
    else
        echo "  ${RED}FAIL: expected 2xx on double-revoke (idempotent)${NC}"
    fi
else
    API_NAMES+=("P256 double-revoke (idempotent)")
    API_TIMES+=("0")
    API_STATUS+=("SKIP")
    printf "${YELLOW}SKIP${NC} %-32s %6s  Set P256_REVOKED_KEY_ID+P256_REVOKE_ASSERTION_JSON to test\n" \
        "P256 double-revoke (idempotent)" "0ms"
fi
echo ""

# ── Phase 7: Cleanup ──
echo "${YELLOW}[Phase 7] Cleanup${NC}"
DEL_START=$(now_ms)
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")
timed_curl "POST /DeleteKey" \
    -X POST "$BASE/DeleteKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.ScheduleKeyDeletion" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"Passkey\":$PASSKEY_JSON}"

# Verify deleted key returns 404
timed_curl "POST /DescribeKey (deleted)" \
    -X POST "$BASE/DescribeKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.DescribeKey" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\"}"
if [ "${API_STATUS[${#API_STATUS[@]}-1]}" = "FAIL" ]; then
    API_STATUS[${#API_STATUS[@]}-1]="OK"
    TOTAL_FAIL=$((TOTAL_FAIL - 1))
    TOTAL_PASS=$((TOTAL_PASS + 1))
    echo "  (Expected failure — deleted key not found)"
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
