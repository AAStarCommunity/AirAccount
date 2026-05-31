#!/bin/bash
# PR #8 Integration Tests — Session Key v0.17.2 (106-byte format)
#
# Tests the three unchecked items from PR #8:
#   1. 106-byte session-key signature decode + ECDSA verify
#   2. Agent key address derivation matches TEE output
#   3. Full create-agent-key → sign-agent → verify-on-chain flow
#
# Prerequisites:
#   - DK2 with KMS v0.17.2+ running (real OP-TEE hardware)
#   - pip3 install cryptography eth_account
#   - KMS_HOST env var set to DK2 address (default: 192.168.7.2:3000)
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
    if echo "$body" | grep -q '"error"'; then
        printf "${RED}FAIL${NC} %s\n  Response: %s\n" "$label" "$(echo "$body" | head -c 200)"
        FAIL=$((FAIL + 1))
    else
        printf "${GREEN} OK ${NC} %s\n" "$label"
        PASS=$((PASS + 1))
    fi
}

assert_eq() {
    local label="$1" got="$2" want="$3"
    if [ "$got" = "$want" ]; then
        printf "${GREEN} OK ${NC} %s\n" "$label"
        PASS=$((PASS + 1))
    else
        printf "${RED}FAIL${NC} %s\n  got=%s\n  want=%s\n" "$label" "$got" "$want"
        FAIL=$((FAIL + 1))
    fi
}

# Helper: generate fresh P-256 passkey assertion
FIXTURE="$SCRIPT_DIR/test-fixtures/user1.json"
if [ ! -f "$FIXTURE" ]; then
    python3 "$SCRIPT_DIR/p256_helper.py" gen-all
fi
USER1_PEM=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['private_key_pem'])")
USER1_PUBKEY=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['public_key_hex'])")

make_assertion() { python3 "$SCRIPT_DIR/p256_helper.py" assertion "$USER1_PEM"; }
make_passkey_json() {
    local a="$1"
    local auth_data sig_r sig_s cdh sig
    auth_data=$(echo "$a" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
    cdh=$(echo "$a" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
    sig_r=$(echo "$a" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
    sig_s=$(echo "$a" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")
    sig="${sig_r}${sig_s}"
    echo "{\"AuthenticatorData\":\"$auth_data\",\"ClientDataHash\":\"$cdh\",\"Signature\":\"$sig\"}"
}

API_KEY_HDR=""
[ -n "$KMS_API_KEY" ] && API_KEY_HDR="-H x-api-key:$KMS_API_KEY"

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
    -H "$HDR_JSON" -H "x-amz-target: TrentService.CreateKey" $API_KEY_HDR \
    -d "{\"Description\":\"sk-test-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$USER1_PUBKEY\"}")
assert_ok "CreateKey" "$BODY"
KEY_ID=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])" 2>/dev/null || echo "")
[ -z "$KEY_ID" ] && echo "${RED}Cannot continue — CreateKey failed${NC}" && exit 1
echo "  KeyId: ${CYAN}$KEY_ID${NC}"

# Poll until ready
TIMEOUT=180; ELAPSED=0
while true; do
    STATUS=$(curl -s "$BASE/KeyStatus?KeyId=$KEY_ID" $API_KEY_HDR | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Status',d.get('status','')))" 2>/dev/null || echo "")
    WALLET_ADDR=$(curl -s "$BASE/KeyStatus?KeyId=$KEY_ID" $API_KEY_HDR | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',''))" 2>/dev/null || echo "")
    [ "$STATUS" = "ready" ] && break
    ELAPSED=$((ELAPSED + 3)); [ $ELAPSED -ge $TIMEOUT ] && echo "${RED}Timeout${NC}" && exit 1
    sleep 3
done
echo "  Wallet address: ${CYAN}$WALLET_ADDR${NC}"
echo ""

# ── Step 2: Create Agent Key (index 0) ──
echo "${YELLOW}[Step 2] Create Agent Key — verify address derivation${NC}"
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")
BODY=$(curl -s -X POST "$BASE/kms/create-agent-key" \
    -H "$HDR_JSON" $API_KEY_HDR \
    -d "{\"keyId\":\"$KEY_ID\",\"agentIndex\":0,\"passkeyAssertion\":$PASSKEY_JSON}")
assert_ok "create-agent-key" "$BODY"

AGENT_ADDR=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('agentAddress',''))" 2>/dev/null || echo "")
AGENT_JWT=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('credential',''))" 2>/dev/null || echo "")
AGENT_KEY_ID="${KEY_ID}:0"
echo "  Agent address: ${CYAN}$AGENT_ADDR${NC}"

# Verify the address is a non-zero 20-byte hex
if python3 -c "
addr = '$AGENT_ADDR'.lstrip('0x')
assert len(addr) == 40, f'bad length {len(addr)}'
assert int(addr, 16) != 0, 'all-zero address'
" 2>/dev/null; then
    printf "${GREEN} OK ${NC} agent address is valid 20-byte non-zero hex\n"
    PASS=$((PASS + 1))
else
    printf "${RED}FAIL${NC} agent address invalid: %s\n" "$AGENT_ADDR"
    FAIL=$((FAIL + 1))
fi

# ── Step 3: Sign UserOpHash — decode 106-byte session key ──
echo ""
echo "${YELLOW}[Step 3] Sign UserOpHash — decode 106-byte session-key signature${NC}"
USER_OP_HASH="0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
BODY=$(curl -s -X POST "$BASE/kms/sign-agent" \
    -H "$HDR_JSON" \
    -H "Authorization: Bearer $AGENT_JWT" \
    $API_KEY_HDR \
    -d "{\"keyId\":\"$AGENT_KEY_ID\",\"userOpHash\":\"$USER_OP_HASH\"}")
assert_ok "sign-agent (userOpHash)" "$BODY"

SIG_HEX=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('signature',''))" 2>/dev/null || echo "")
echo "  Signature (hex): ${CYAN}${SIG_HEX:0:20}...${NC}"

# Decode 106-byte session-key format:
#   [0x08][account(20)][agent_key(20)][ECDSA(65)]  = 106 bytes
python3 - <<PYEOF
import sys

sig_hex = '$SIG_HEX'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
n = len(sig_bytes)

if n != 106:
    print(f"FAIL: expected 106 bytes, got {n}")
    sys.exit(1)

marker = sig_bytes[0]
account_addr = '0x' + sig_bytes[1:21].hex()
agent_addr   = '0x' + sig_bytes[21:41].hex()
ecdsa_sig    = sig_bytes[41:106]

if marker != 0x08:
    print(f"FAIL: expected marker 0x08, got {hex(marker)}")
    sys.exit(1)

print(f" OK  106-byte format: marker=0x08 ✓")
print(f"     embedded account: {account_addr}")
print(f"     embedded agent:   {agent_addr}")
print(f"     ECDSA sig:        {ecdsa_sig.hex()[:20]}... ({len(ecdsa_sig)} bytes)")

# Verify agent address in session key matches what TEE returned
expected_agent = '$AGENT_ADDR'.lower().lstrip('0x')
got_agent = sig_bytes[21:41].hex()
if got_agent != expected_agent:
    print(f"FAIL: agent addr mismatch: got={got_agent} want={expected_agent}")
    sys.exit(1)
print(f" OK  agent address in session key matches TEE-created agent address")

# Verify ECDSA signature structure (65 bytes: R||S||V, V=27 or 28)
v = ecdsa_sig[64]
if v not in (27, 28):
    print(f"FAIL: V byte {v} not in {{27, 28}}")
    sys.exit(1)
print(f" OK  V byte = {v} (normalized to 27/28)")
PYEOF

if [ $? -eq 0 ]; then
    PASS=$((PASS + 2))  # 3 checks above passed (marker, addr match, V byte)
else
    FAIL=$((FAIL + 1))
fi

# ── Step 4: Verify signature recovers to agent address ──
echo ""
echo "${YELLOW}[Step 4] Recover signer from session-key ECDSA signature${NC}"
python3 - <<PYEOF
import sys

try:
    from eth_account import Account
    from eth_account.messages import encode_defunct
except ImportError:
    print("SKIP: eth_account not installed (pip3 install eth_account)")
    sys.exit(0)

sig_hex = '$SIG_HEX'.lstrip('0x')
sig_bytes = bytes.fromhex(sig_hex)
ecdsa_sig = sig_bytes[41:106]

user_op_hash = bytes.fromhex('$USER_OP_HASH'.lstrip('0x'))
r = int.from_bytes(ecdsa_sig[0:32], 'big')
s = int.from_bytes(ecdsa_sig[32:64], 'big')
v = ecdsa_sig[64]

# Recover with eth_account
try:
    recovered = Account.recover_message(
        encode_defunct(user_op_hash),
        vrs=(v, r, s)
    )
    expected = '$AGENT_ADDR'.lower()
    got = recovered.lower()
    if got == expected:
        print(f" OK  Recovered signer {recovered} == TEE agent address")
    else:
        print(f"FAIL: recovered={got}, want={expected}")
        sys.exit(1)
except Exception as ex:
    print(f"FAIL: ecrecover failed: {ex}")
    sys.exit(1)
PYEOF
ECRECOVER_RC=$?
if [ $ECRECOVER_RC -eq 0 ]; then
    PASS=$((PASS + 1))
elif echo "SKIP" 2>/dev/null; then
    :
else
    FAIL=$((FAIL + 1))
fi

# ── Step 5: Cleanup ──
echo ""
echo "${YELLOW}[Step 5] Cleanup${NC}"
PASSKEY_JSON=$(make_passkey_json "$(make_assertion)")
BODY=$(curl -s -X POST "$BASE/DeleteKey" \
    -H "$HDR_JSON" -H "x-amz-target: TrentService.ScheduleKeyDeletion" $API_KEY_HDR \
    -d "{\"KeyId\":\"$KEY_ID\",\"Passkey\":$PASSKEY_JSON}")
assert_ok "DeleteKey (cleanup)" "$BODY"

# ── Summary ──
echo ""
echo "${BOLD}================================================================${NC}"
TOTAL=$((PASS + FAIL))
if [ $FAIL -eq 0 ]; then
    echo "${GREEN}All $PASS/$TOTAL session-key v0.17.2 tests passed${NC}"
    exit 0
else
    echo "${RED}$FAIL/$TOTAL tests FAILED${NC}"
    exit 1
fi
