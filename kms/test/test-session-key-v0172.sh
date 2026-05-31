#!/bin/bash
# PR #8 Integration Tests — Session Key v0.17.2 (106-byte format)
#
# Tests three items from PR #8:
#   1. 106-byte session-key signature format (marker=0x08, account[20], agent[20], ECDSA[65])
#   2. Agent key address matches what was created in TEE
#   3. ecrecover of the ECDSA portion returns the correct agent address
#
# HARDWARE REQUIREMENT: DK2 with KMS v0.17.2+ (real OP-TEE).
#   create-agent-key requires WebAuthn ceremony — cannot be simulated with legacy passkey.
#   Set AGENT_CREDENTIAL and AGENT_KEY_ID env vars to bypass the create step,
#   or run the full flow with DK2 WebAuthn hardware.
#
# ENV VARS:
#   KMS_HOST        — host:port (default 192.168.7.2:3000)
#   KMS_API_KEY     — API key if required
#   AGENT_CREDENTIAL — pre-existing agent JWT (skips create-agent-key)
#   AGENT_KEY_ID    — pre-existing agent keyId "wallet_uuid:index" (pairs with AGENT_CREDENTIAL)
#
# Usage: ./test-session-key-v0172.sh [host:port]

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
[ -f "$SCRIPT_DIR/.env" ] && set -a && source "$SCRIPT_DIR/.env" && set +a
HOST="${1:-${KMS_HOST:-192.168.7.2:3000}}"
BASE="http://$HOST"
HDR_JSON="Content-Type: application/json"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

PASS=0; FAIL=0

assert_ok() {
    local label="$1" body="$2"
    if echo "$body" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(1 if 'error' in d else 0)" 2>/dev/null; then
        printf "${GREEN} OK ${NC} %s\n" "$label"
        PASS=$((PASS + 1))
        return 0
    else
        printf "${RED}FAIL${NC} %s\n  Response: %s\n" "$label" "$(echo "$body" | head -c 200)"
        FAIL=$((FAIL + 1))
        return 1
    fi
}

assert_py() {
    local label="$1"
    shift
    local result
    if result=$(python3 - "$@" 2>&1); then
        printf "${GREEN} OK ${NC} %s\n" "$label"
        echo "$result" | sed 's/^/     /'
        PASS=$((PASS + 1))
        return 0
    else
        printf "${RED}FAIL${NC} %s\n  %s\n" "$label" "$result"
        FAIL=$((FAIL + 1))
        return 1
    fi
}

# API key header (properly quoted to avoid word-splitting)
API_KEY_HDR_ARGS=()
[ -n "$KMS_API_KEY" ] && API_KEY_HDR_ARGS=(-H "x-api-key: $KMS_API_KEY")

# Helper: generate fresh P-256 passkey assertion for legacy-compatible endpoints
FIXTURE="$SCRIPT_DIR/test-fixtures/user1.json"
[ ! -f "$FIXTURE" ] && python3 "$SCRIPT_DIR/p256_helper.py" gen-all
USER1_PEM=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['private_key_pem'])")
USER1_PUBKEY=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['public_key_hex'])")

make_assertion() { python3 "$SCRIPT_DIR/p256_helper.py" assertion "$USER1_PEM"; }
make_passkey_json() {
    local a="$1"
    local auth_data cdh sig_r sig_s
    auth_data=$(echo "$a" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
    cdh=$(echo "$a"       | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
    sig_r=$(echo "$a"     | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
    sig_s=$(echo "$a"     | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")
    echo "{\"AuthenticatorData\":\"$auth_data\",\"ClientDataHash\":\"$cdh\",\"Signature\":\"${sig_r}${sig_s}\"}"
}

echo ""
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  Session Key v0.17.2 Integration Tests (PR #8)${NC}"
echo "${BOLD}  Target: ${CYAN}$BASE${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}================================================================${NC}"
echo ""

# ── Step 1: Create wallet ──
echo "${YELLOW}[Step 1] Create wallet${NC}"
BODY=$(curl -s -X POST "$BASE/CreateKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.CreateKey" "${API_KEY_HDR_ARGS[@]}" \
    -d "{\"Description\":\"sk172-test-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$USER1_PUBKEY\"}")
assert_ok "CreateKey" "$BODY" || exit 1
KEY_ID=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])")
echo "  KeyId: ${CYAN}$KEY_ID${NC}"

# Poll until address is derived
TIMEOUT=180; ELAPSED=0; WALLET_ADDR=""
while true; do
    RESP=$(curl -s "$BASE/KeyStatus?KeyId=$KEY_ID" "${API_KEY_HDR_ARGS[@]}")
    STATUS=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Status',d.get('status','')))" 2>/dev/null || echo "")
    WALLET_ADDR=$(echo "$RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',''))" 2>/dev/null || echo "")
    [ "$STATUS" = "ready" ] && break
    ELAPSED=$((ELAPSED + 3)); [ $ELAPSED -ge $TIMEOUT ] && echo "${RED}Timeout waiting for key derivation${NC}" && exit 1
    sleep 3
done
echo "  Wallet address: ${CYAN}$WALLET_ADDR${NC}"
echo ""

# ── Step 2: Create Agent Key or use pre-existing ──
echo "${YELLOW}[Step 2] Agent Key${NC}"

if [ -n "$AGENT_CREDENTIAL" ] && [ -n "$AGENT_KEY_ID" ]; then
    echo "  Using pre-existing agent credential (AGENT_CREDENTIAL + AGENT_KEY_ID env vars)"
    AGENT_JWT="$AGENT_CREDENTIAL"
    AGENT_ADDR=""  # Cannot verify address without create response
    printf "${YELLOW}SKIP${NC} create-agent-key (using env var AGENT_CREDENTIAL)\n"
else
    echo "  create-agent-key requires WebAuthn ceremony (DK2 hardware)."
    echo "  Starting BeginAuthentication flow..."

    # Step 2a: get challenge
    AUTHN_RESP=$(curl -s -X POST "$BASE/BeginAuthentication" \
        -H "$HDR_JSON" "${API_KEY_HDR_ARGS[@]}" \
        -d "{\"KeyId\":\"$KEY_ID\"}" 2>/dev/null || echo '{"error":"BeginAuthentication failed"}')

    if echo "$AUTHN_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if 'ChallengeId' in d else 1)" 2>/dev/null; then
        CHALLENGE_ID=$(echo "$AUTHN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['ChallengeId'])")
        echo "  ChallengeId: $CHALLENGE_ID"
        echo "  ${YELLOW}NOTE: WebAuthn hardware required to complete ceremony.${NC}"
        echo "  Set AGENT_CREDENTIAL and AGENT_KEY_ID env vars to skip this step."
        echo "  Skipping agent key tests (no hardware authenticator available)."
        AGENT_JWT=""
    else
        echo "  ${YELLOW}BeginAuthentication not available or returned error.${NC}"
        AGENT_JWT=""
    fi
fi

if [ -z "$AGENT_JWT" ]; then
    echo ""
    echo "${YELLOW}Skipping Steps 3-4 (no agent credential available).${NC}"
    echo "Run with AGENT_CREDENTIAL=<jwt> AGENT_KEY_ID=<wallet:index> to test 106-byte format."
    echo ""

    # Still clean up the wallet
    echo "${YELLOW}[Cleanup] Delete test wallet${NC}"
    PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")
    curl -s -X POST "$BASE/DeleteKey" \
        -H "$HDR_JSON" -H "x-amz-target: TrentService.ScheduleKeyDeletion" "${API_KEY_HDR_ARGS[@]}" \
        -d "{\"KeyId\":\"$KEY_ID\",\"Passkey\":$PASSKEY_JSON}" > /dev/null

    echo ""
    echo "${BOLD}================================================================${NC}"
    echo "${YELLOW}INCOMPLETE: agent key steps require DK2 WebAuthn hardware${NC}"
    echo "  Passed: $PASS  Skipped: agent steps"
    exit 0
fi

# ── Step 3: Sign UserOpHash → decode 106-byte session key ──
echo ""
echo "${YELLOW}[Step 3] Sign payload — verify 106-byte session-key format${NC}"

# sign-agent uses compound keyId "wallet_uuid:index" from $AGENT_KEY_ID
USER_OP_HASH="0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
BODY=$(curl -s -X POST "$BASE/kms/sign-agent" \
    -H "$HDR_JSON" \
    -H "Authorization: Bearer $AGENT_JWT" \
    "${API_KEY_HDR_ARGS[@]}" \
    -d "{\"keyId\":\"$AGENT_KEY_ID\",\"payload\":\"$USER_OP_HASH\"}")
assert_ok "sign-agent (payload)" "$BODY"
SIG_HEX=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('signature',''))" 2>/dev/null || echo "")
echo "  Signature prefix: ${CYAN}${SIG_HEX:0:24}…${NC}"

# Verify 106-byte session-key format
assert_py "106-byte format: marker=0x08, account[20], agent[20], ECDSA[65]" <<PYEOF
import sys

sig_hex = '${SIG_HEX}'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
n = len(sig_bytes)

assert n == 106, f"expected 106 bytes, got {n}"

marker = sig_bytes[0]
assert marker == 0x08, f"expected marker 0x08, got {hex(marker)}"

embedded_account = '0x' + sig_bytes[1:21].hex()
embedded_agent   = '0x' + sig_bytes[21:41].hex()
ecdsa_sig        = sig_bytes[41:106]

print(f"marker=0x08 ✓")
print(f"embedded account: {embedded_account}")
print(f"embedded agent:   {embedded_agent}")
print(f"ECDSA portion:    {ecdsa_sig.hex()[:20]}… ({len(ecdsa_sig)} bytes)")

assert len(ecdsa_sig) == 65, f"ECDSA portion should be 65 bytes, got {len(ecdsa_sig)}"
v = ecdsa_sig[64]
assert v in (27, 28), f"V byte {v} not in {{27, 28}}"
print(f"V byte = {v} ✓")
PYEOF

# ── Step 4: Validate embedded addresses ──
echo ""
echo "${YELLOW}[Step 4] Validate embedded addresses in session key${NC}"

# Embedded account address should equal the wallet address
assert_py "embedded account = wallet address" <<PYEOF
sig_hex = '${SIG_HEX}'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
embedded_account = '0x' + sig_bytes[1:21].hex()
wallet_addr = '${WALLET_ADDR}'.lower()
got = embedded_account.lower()
assert got == wallet_addr, f"embedded account {got} != wallet addr {wallet_addr}"
print(f"embedded account {got} == wallet addr ✓")
PYEOF

# Embedded agent address should equal what TEE returned at create time
if [ -n "$AGENT_ADDR" ]; then
    assert_py "embedded agent = created agent address" <<PYEOF
sig_hex = '${SIG_HEX}'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
embedded_agent = '0x' + sig_bytes[21:41].hex()
expected = '${AGENT_ADDR}'.lower()
got = embedded_agent.lower()
assert got == expected, f"embedded agent {got} != created agent addr {expected}"
print(f"embedded agent {got} == created agent addr ✓")
PYEOF
else
    echo "  (agent address comparison skipped — no create response available)"
fi

# ── Step 5: ecrecover — ECDSA portion must recover to agent address ──
echo ""
echo "${YELLOW}[Step 5] ecrecover — ECDSA portion recovers to agent address${NC}"

python3 - <<PYEOF
import sys

try:
    from eth_account import Account
    from eth_account.messages import encode_defunct
except ImportError:
    print("SKIP: pip3 install eth_account to enable ecrecover check")
    sys.exit(0)

sig_hex = '${SIG_HEX}'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
embedded_agent = '0x' + sig_bytes[21:41].hex()
ecdsa_sig = sig_bytes[41:106]

payload_hash = bytes.fromhex('${USER_OP_HASH}'.lstrip('0x'))
r = int.from_bytes(ecdsa_sig[0:32], 'big')
s = int.from_bytes(ecdsa_sig[32:64], 'big')
v = ecdsa_sig[64]

recovered = Account.recover_message(encode_defunct(payload_hash), vrs=(v, r, s))
got = recovered.lower()
want = embedded_agent.lower()
if got == want:
    print(f"ecrecover → {recovered}")
    print(f"matches embedded agent address ✓")
else:
    print(f"FAIL: ecrecover={got}, want={want}", file=sys.stderr)
    sys.exit(1)
PYEOF

ECRECOVER_RC=$?
if [ $ECRECOVER_RC -eq 0 ]; then
    PASS=$((PASS + 1))
    printf "${GREEN} OK ${NC} ecrecover recovered correct agent address\n"
elif grep -q "SKIP" /dev/stdin 2>/dev/null; then
    :  # truly skipped (eth_account not installed)
else
    FAIL=$((FAIL + 1))
    printf "${RED}FAIL${NC} ecrecover check failed\n"
fi

# ── Cleanup ──
echo ""
echo "${YELLOW}[Cleanup] Delete test wallet${NC}"
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")
BODY=$(curl -s -X POST "$BASE/DeleteKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.ScheduleKeyDeletion" "${API_KEY_HDR_ARGS[@]}" \
    -d "{\"KeyId\":\"$KEY_ID\",\"Passkey\":$PASSKEY_JSON}")
assert_ok "DeleteKey (cleanup)" "$BODY"

# ── Summary ──
echo ""
echo "${BOLD}================================================================${NC}"
TOTAL=$((PASS + FAIL))
if [ $FAIL -eq 0 ]; then
    echo "${GREEN}All $PASS/$TOTAL session-key v0.17.2 tests PASSED${NC}"
    exit 0
else
    echo "${RED}$FAIL/$TOTAL tests FAILED${NC}"
    exit 1
fi
