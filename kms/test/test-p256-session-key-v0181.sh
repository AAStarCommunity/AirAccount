#!/bin/bash
# P256 Session Key v0.18.1 Integration Test
# Wire format: [0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)] = 149 bytes
#
# Prerequisites:
#   - Running KMS server with OP-TEE TA
#   - A pre-created wallet with known KEY_ID
#   - WebAuthn ceremony bypass: set AGENT_CREDENTIAL env var (pre-issued JWT)
#     OR run through a browser-based WebAuthn flow to obtain P256_SESSION_CRED
#
# Usage:
#   # With pre-issued JWT (bypassing WebAuthn browser flow):
#   P256_SESSION_CRED="<jwt>" P256_SESSION_KEY_ID="<uuid>:<idx>" ./test-p256-session-key-v0181.sh [host:port]
#
#   # Full flow (browser required for creation step):
#   ./test-p256-session-key-v0181.sh [host:port]

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
[ -f "$SCRIPT_DIR/.env" ] && set -a && source "$SCRIPT_DIR/.env" && set +a
HOST="${1:-${KMS_HOST:-192.168.7.2:3000}}"
BASE="http://$HOST"
HDR_JSON="Content-Type: application/json"

FIXTURE="$SCRIPT_DIR/test-fixtures/user1.json"
if [ ! -f "$FIXTURE" ]; then
    echo "Generating test fixtures..."
    python3 "$SCRIPT_DIR/p256_helper.py" gen-all
fi

USER1_PUBKEY=$(FIXTURE_PATH="$FIXTURE" python3 -c 'import json,os; print(json.load(open(os.environ["FIXTURE_PATH"]))["public_key_hex"])')
USER1_PEM=$(FIXTURE_PATH="$FIXTURE" python3 -c 'import json,os; print(json.load(open(os.environ["FIXTURE_PATH"]))["private_key_pem"])')

now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
make_assertion() { python3 "$SCRIPT_DIR/p256_helper.py" assertion "$USER1_PEM"; }

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

PASS=0; FAIL=0; SKIP=0

pass() { PASS=$((PASS+1)); printf "${GREEN} OK ${NC} %s\n" "$*"; }
fail() { FAIL=$((FAIL+1)); printf "${RED}FAIL${NC} %s\n" "$*"; }
skip() { SKIP=$((SKIP+1)); printf "${YELLOW}SKIP${NC} %s\n" "$*"; }

API_KEY_HDR=""
[ -n "$KMS_API_KEY" ] && API_KEY_HDR="-H x-api-key:$KMS_API_KEY"

echo ""
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  P256 Session Key v0.18.1 Integration Test${NC}"
echo "${BOLD}  Target: ${CYAN}$BASE${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}================================================================${NC}"
echo ""

# ── Step 1: Ensure we have a wallet ──
WALLET_KEY_ID="${WALLET_KEY_ID:-}"

if [ -z "$WALLET_KEY_ID" ]; then
    echo "${YELLOW}[Step 1] Creating wallet for P256 session key test${NC}"
    RESP=$(curl -s --max-time 30 -X POST "$BASE/CreateKey" \
        -H "$HDR_JSON" -H "x-amz-target: TrentService.CreateKey" $API_KEY_HDR \
        -d "{\"Description\":\"p256-test-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$USER1_PUBKEY\"}" 2>/dev/null)

    WALLET_KEY_ID=$(echo "$RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['KeyMetadata']['KeyId'])" 2>/dev/null || echo "")
    if [ -z "$WALLET_KEY_ID" ]; then
        fail "CreateKey failed — cannot continue"
        exit 1
    fi
    echo "  Wallet created: ${CYAN}$WALLET_KEY_ID${NC}"

    # Poll for ready
    echo "  Waiting for key derivation..."
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        sleep 5
        STATUS=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$WALLET_KEY_ID" $API_KEY_HDR 2>/dev/null | \
            python3 -c "import sys,json; print(json.load(sys.stdin).get('Status','unknown'))" 2>/dev/null || echo "unknown")
        [ "$STATUS" = "ready" ] && break
    done
    if [ "$STATUS" != "ready" ]; then
        fail "Key derivation timeout"
        exit 1
    fi
    ACCOUNT_ADDRESS=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$WALLET_KEY_ID" $API_KEY_HDR 2>/dev/null | \
        python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',d.get('address','')))" 2>/dev/null || echo "")
    pass "Wallet ready: addr=$ACCOUNT_ADDRESS"
else
    echo "${YELLOW}[Step 1] Using existing wallet: ${CYAN}$WALLET_KEY_ID${NC}"
    ACCOUNT_ADDRESS=$(curl -s --max-time 10 "$BASE/KeyStatus?KeyId=$WALLET_KEY_ID" $API_KEY_HDR 2>/dev/null | \
        python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Address',d.get('address','')))" 2>/dev/null || echo "0x0000000000000000000000000000000000000000")
fi
echo ""

# ── Step 2: Create P256 session key ──
echo "${YELLOW}[Step 2] Create P256 session key${NC}"

if [ -n "$P256_SESSION_CRED" ] && [ -n "$P256_SESSION_KEY_ID" ]; then
    skip "Using pre-provided P256_SESSION_CRED (skipping creation)"
    P256_KEY_ID="$P256_SESSION_KEY_ID"
    P256_JWT="$P256_SESSION_CRED"
    P256_PUBKEY_X=""
    P256_PUBKEY_Y=""
else
    # create-p256-session-key requires WebAuthn ceremony — cannot automate without browser
    echo "  ${YELLOW}NOTE: create-p256-session-key requires WebAuthn ceremony.${NC}"
    echo "  Set P256_SESSION_CRED and P256_SESSION_KEY_ID env vars to skip creation."
    echo "  To get these values, complete a WebAuthn flow in the test UI or SDK."
    skip "WebAuthn ceremony required — set P256_SESSION_CRED+P256_SESSION_KEY_ID"
    P256_KEY_ID=""
    P256_JWT=""
fi
echo ""

# ── Step 3: Sign P256 user op ──
echo "${YELLOW}[Step 3] Sign P256 UserOp${NC}"

if [ -z "$P256_JWT" ] || [ -z "$P256_KEY_ID" ]; then
    skip "No P256 credential available — skipping sign test"
else
    SAMPLE_HASH="a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
    # Use account address from wallet derivation
    ACCT="${ACCOUNT_ADDRESS:-0x742d35cc6634c0532925a3b844bc9e7595f2bd18}"

    T0=$(now_ms)
    SIGN_RESP=$(curl -s --max-time 30 -X POST "$BASE/kms/sign-p256-user-op" \
        -H "$HDR_JSON" \
        -H "Authorization: Bearer $P256_JWT" \
        $API_KEY_HDR \
        -d "{\"keyId\":\"$P256_KEY_ID\",\"payload\":\"0x$SAMPLE_HASH\",\"accountAddress\":\"$ACCT\"}" 2>/dev/null)
    T1=$(now_ms)
    ELAPSED=$((T1 - T0))

    if echo "$SIGN_RESP" | grep -q '"error"'; then
        fail "sign-p256-user-op failed (${ELAPSED}ms): $(echo "$SIGN_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("error","?"))' 2>/dev/null)"
    else
        SIG_HEX=$(echo "$SIGN_RESP" | python3 -c "
import sys, json
d = json.load(sys.stdin)
s = d.get('signature', '')
print(s[2:] if s.startswith('0x') else s)
" 2>/dev/null || echo "")
        SIG_BYTES=$((${#SIG_HEX} / 2))
        PUB_X=$(echo "$SIGN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('pubKeyX',''))" 2>/dev/null || echo "")
        PUB_Y=$(echo "$SIGN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('pubKeyY',''))" 2>/dev/null || echo "")

        if [ "$SIG_BYTES" -eq 149 ]; then
            pass "sign-p256-user-op (${ELAPSED}ms): ${SIG_BYTES} bytes ✓"
        else
            fail "sign-p256-user-op: unexpected sig length ${SIG_BYTES} bytes (want 149)"
        fi

        # ── Decode 149-byte format ──
        echo ""
        echo "  ${BOLD}Decoding 149-byte P256 session key signature:${NC}"
        SIG_HEX_ENV="$SIG_HEX" python3 - <<'PYEOF'
import sys, os

hex_str = os.environ['SIG_HEX_ENV']
if len(hex_str) != 298:  # 149 * 2
    print(f"  ERROR: expected 298 hex chars, got {len(hex_str)}")
    sys.exit(1)

data = bytes.fromhex(hex_str)
marker   = data[0]
account  = data[1:21].hex()
key_x    = data[21:53].hex()
key_y    = data[53:85].hex()
sig_r    = data[85:117].hex()
sig_s    = data[117:149].hex()

print(f"  marker  = 0x{marker:02x}  (expected 0x08)")
print(f"  account = 0x{account}")
print(f"  keyX    = 0x{key_x[:16]}...")
print(f"  keyY    = 0x{key_y[:16]}...")
print(f"  r       = 0x{sig_r[:16]}...")
print(f"  s       = 0x{sig_s[:16]}...")

assert marker == 0x08, f"marker mismatch: 0x{marker:02x} != 0x08"
assert len(data) == 149, f"length mismatch: {len(data)}"
print("  Format validation: ✓ all fields correct")
PYEOF
        DECODE_RC=$?
        if [ $DECODE_RC -eq 0 ]; then
            pass "149-byte format validated"
        else
            fail "149-byte format validation failed"
        fi

        # ── Verify P256 signature using Python cryptography ──
        echo ""
        echo "  ${BOLD}Verifying P256 ECDSA signature:${NC}"
        set +e
        VERIFY_RESULT=$(SIG_HEX_ENV="$SIG_HEX" PUB_X_ENV="$PUB_X" PUB_Y_ENV="$PUB_Y" SAMPLE_HASH_ENV="$SAMPLE_HASH" python3 - 2>&1 <<'PYEOF'
import sys, os
try:
    from cryptography.hazmat.primitives.asymmetric.ec import (
        EllipticCurvePublicKey, SECP256R1, ECDSA
    )
    from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature
    from cryptography.hazmat.backends import default_backend
    from cryptography.hazmat.primitives.hashes import SHA256
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric.ec import (
        EllipticCurvePublicNumbers, SECP256R1
    )
    from cryptography.hazmat.primitives.asymmetric import ec
    from hashlib import sha3_256
except ImportError:
    print("SKIP: pip install cryptography")
    sys.exit(2)

import struct

sig_hex = os.environ['SIG_HEX_ENV']
data = bytes.fromhex(sig_hex)
pub_x_hex = os.environ.get('PUB_X_ENV', '')
pub_y_hex = os.environ.get('PUB_Y_ENV', '')
user_op_hash = bytes.fromhex(os.environ['SAMPLE_HASH_ENV'])

# Reconstruct public key from X and Y
key_x_int = int(pub_x_hex, 16) if pub_x_hex else int(data[21:53].hex(), 16)
key_y_int = int(pub_y_hex, 16) if pub_y_hex else int(data[53:85].hex(), 16)
sig_r = int(data[85:117].hex(), 16)
sig_s = int(data[117:149].hex(), 16)

# EIP-191 prefix
eip191_prefix = b'\x19Ethereum Signed Message:\n32'
import hashlib
digest_input = eip191_prefix + user_op_hash
from sha3 import keccak_256 as keccak
h = keccak()
h.update(digest_input)
msg_hash = h.digest()

pub_numbers = EllipticCurvePublicNumbers(x=key_x_int, y=key_y_int, curve=SECP256R1())
pub_key = pub_numbers.public_key(default_backend())

# DER-encode (r, s) for verification
import struct
def encode_dss_sig(r, s):
    def encode_int(n):
        b = n.to_bytes((n.bit_length() + 7) // 8, 'big')
        if b[0] & 0x80: b = b'\x00' + b
        return bytes([0x02, len(b)]) + b
    ri = encode_int(r)
    si = encode_int(s)
    return bytes([0x30, len(ri) + len(si)]) + ri + si

dss_sig = encode_dss_sig(sig_r, sig_s)
from cryptography.hazmat.primitives.asymmetric import utils
from cryptography.hazmat.primitives.hashes import Prehashed
pub_key.verify(dss_sig, msg_hash, ec.ECDSA(Prehashed(hashes.SHA256())))
print("P256 signature VALID ✓")
PYEOF
        )
        VERIFY_RC=$?
        set -e

        if [ $VERIFY_RC -eq 2 ]; then
            skip "cryptography not installed (pip install cryptography sha3)"
        elif [ $VERIFY_RC -eq 0 ]; then
            pass "P256 ECDSA signature verified cryptographically"
            echo "  $VERIFY_RESULT"
        else
            fail "P256 ECDSA signature verification failed: $VERIFY_RESULT"
        fi
    fi
fi
echo ""

# ── Summary ──
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  SUMMARY  pass=$PASS  fail=$FAIL  skip=$SKIP${NC}"
echo "${BOLD}================================================================${NC}"

if [ "$FAIL" -gt 0 ]; then
    echo "${RED}${FAIL} test(s) FAILED${NC}"
    exit 1
else
    echo "${GREEN}All tests passed (${SKIP} skipped)${NC}"
fi
