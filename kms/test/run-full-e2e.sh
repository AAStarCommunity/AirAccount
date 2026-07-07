#!/bin/bash
# Full E2E coverage — every KMS HTTP endpoint, via real WebAuthn ceremony
# (no legacy passkey, so no KMS_ALLOW_LEGACY_PASSKEY needed).
#
# Run ON THE BOARD (localhost) — Mac→board:3000 is intercepted by cloudflared (502).
#   scp -r kms/test root@<board>:/tmp/kmstest
#   ssh root@<board> 'cd /tmp/kmstest && bash run-full-e2e.sh'
#
# Requires: python3 + cryptography (for p256_helper.py).

set -uo pipefail
HOST="${1:-127.0.0.1:3000}"
BASE="http://$HOST"
# #145 fail-closed 认证:板子有 API key 时必须发 x-api-key,否则所有 auth 端点 401。
# 用数组保证含空格的 header 值不被 word-split。set KMS_API_KEY=... 传入。
AK=()
[ -n "${KMS_API_KEY:-}" ] && AK=(-H "x-api-key: $KMS_API_KEY")
DIR="$(cd "$(dirname "$0")" && pwd)"
HELPER="$DIR/p256_helper.py"
LASTF="$(mktemp)"; SCF="$(mktemp)"; echo 1 > "$SCF"; trap 'rm -f "$LASTF" "$SCF"' EXIT
RED='\033[0;31m'; GRN='\033[0;32m'; YEL='\033[1;33m'; NC='\033[0m'
PASS=0; FAIL=0; FAILED_NAMES=()

[ -f "$DIR/test-fixtures/user1.json" ] || python3 "$HELPER" gen-all >/dev/null 2>&1
PK=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['public_key_hex'])")
PEM=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['private_key_pem'])")
PK2=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user2.json'))['public_key_hex'])")

jbody() { python3 -c "import sys,json;d=json.load(open('$LASTF'));print(d$1)" 2>/dev/null; }

# *_code: write body to $LASTF, echo http_code
get_code()  { curl -s --max-time 15 -o "$LASTF" -w '%{http_code}' "${AK[@]}" "$BASE$1"; }
post_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' "${AK[@]}" -X POST "$BASE/$1" -H "Content-Type: application/json" -H "x-amz-target: TrentService.$1" -d "$2"; }
post_path_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' "${AK[@]}" -X POST "$BASE$1" -H "Content-Type: application/json" -H "x-amz-target: TrentService.$2" ${4:+-H "$4"} -d "$3"; }
# DeleteKey is exposed at path /DeleteKey but its AWS-KMS action name is ScheduleKeyDeletion
del_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' "${AK[@]}" -X POST "$BASE/DeleteKey" -H "Content-Type: application/json" -H "x-amz-target: TrentService.ScheduleKeyDeletion" -d "$1"; }

# chk <name> <got_code> <expect_code>  (body context read from $LASTF)
chk() {
  if [ "$2" = "$3" ]; then PASS=$((PASS+1)); printf "${GRN} OK ${NC} %-42s %s\n" "$1" "$2"
  else FAIL=$((FAIL+1)); FAILED_NAMES+=("$1"); printf "${RED}FAIL${NC} %-42s got=%s want=%s  %s\n" "$1" "$2" "$3" "$(head -c 90 "$LASTF")"; fi
}

# ceremony <keyid> → echoes WebAuthn assertion JSON {ChallengeId, Credential}
ceremony() {
  local kid="$1" ba cid chal cred sc
  sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc" > "$SCF"   # strictly increasing signCount
  ba=$(curl -s --max-time 15 "${AK[@]}" -X POST "$BASE/BeginAuthentication" -H "Content-Type: application/json" -H "x-amz-target: TrentService.BeginAuthentication" -d "{\"KeyId\":\"$kid\"}")
  cid=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['ChallengeId'])" 2>/dev/null)
  chal=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['Options']['challenge'])" 2>/dev/null)
  [ -z "$cid" ] && { echo "{}"; return 1; }
  cred=$(python3 "$HELPER" ceremony "$PEM" "$chal" "dGVzdC1jcmVkZW50aWFs" "$sc")
  echo "{\"ChallengeId\":\"$cid\",\"Credential\":$cred}"
}

# keccak256 of a hex string → hex digest (Ethereum keccak, not NIST SHA3). pycryptodome.
keccak256_hex() { python3 -c "from Crypto.Hash import keccak; h=keccak.new(digest_bits=256); h.update(bytes.fromhex('$1')); print(h.hexdigest())"; }

# ceremony_payload <keyid> <payload_hex> → assertion for a SIGNING op (Issue #68).
# The WebAuthn challenge must COMMIT to the payload: challenge = SHA256(nonce||payload).
# payload_hex = the 32-byte digest the TA will actually sign (SignHash=hash;
# Sign message=keccak256(msg); Sign tx=RLP-keccak tx hash).
ceremony_payload() {
  local kid="$1" payload_hex="$2" ba cid chal committed cred sc
  sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc" > "$SCF"
  ba=$(curl -s --max-time 15 "${AK[@]}" -X POST "$BASE/BeginAuthentication" -H "Content-Type: application/json" -H "x-amz-target: TrentService.BeginAuthentication" -d "{\"KeyId\":\"$kid\"}")
  cid=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['ChallengeId'])" 2>/dev/null)
  chal=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['Options']['challenge'])" 2>/dev/null)
  [ -z "$cid" ] && { echo "{}"; return 1; }
  committed=$(python3 -c "
import hashlib,base64
chal='$chal'; payload=bytes.fromhex('$payload_hex')
nonce=base64.urlsafe_b64decode(chal+'='*(-len(chal)%4))
print(base64.urlsafe_b64encode(hashlib.sha256(nonce+payload).digest()).rstrip(b'=').decode())")
  cred=$(python3 "$HELPER" ceremony "$PEM" "$committed" "dGVzdC1jcmVkZW50aWFs" "$sc")
  echo "{\"ChallengeId\":\"$cid\",\"Credential\":$cred}"
}

# ceremony_grant <keyid> → assertion bound to a purpose="grant-session" challenge
# (sign-grant-session rejects the plain 'authentication' purpose to prevent cross-op replay)
ceremony_grant() {
  local kid="$1" ba cid chal cred sc
  sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc" > "$SCF"
  ba=$(curl -s --max-time 15 "${AK[@]}" "$BASE/kms/begin-grant-session-auth?keyId=$kid")
  cid=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['ChallengeId'])" 2>/dev/null)
  chal=$(echo "$ba" | python3 -c "import sys,json;print(json.load(sys.stdin)['Options']['challenge'])" 2>/dev/null)
  [ -z "$cid" ] && { echo "{}"; return 1; }
  cred=$(python3 "$HELPER" ceremony "$PEM" "$chal" "dGVzdC1jcmVkZW50aWFs" "$sc")
  echo "{\"ChallengeId\":\"$cid\",\"Credential\":$cred}"
}

echo "════════ Full E2E Coverage @ $BASE ════════"

echo -e "${YEL}[1] Infra${NC}"
chk "GET /health"          "$(get_code /health)" 200
chk "GET /version"         "$(get_code /version)" 200
chk "GET /QueueStatus"     "$(get_code /QueueStatus)" 200
chk "GET /RollbackCounter" "$(get_code /RollbackCounter)" 200
chk "GET /stats"           "$(get_code /stats)" 200
chk "GET / (dashboard)"    "$(get_code /)" 200
chk "GET /test (test UI)"  "$(get_code /test)" 200

echo -e "${YEL}[2] Wallet lifecycle${NC}"
chk "POST /CreateKey" "$(post_code CreateKey "{\"Description\":\"e2e\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}")" 200
KEYID=$(jbody "['KeyMetadata']['KeyId']"); echo "    KeyId=$KEYID"
sleep 2
chk "GET /KeyStatus" "$(get_code "/KeyStatus?KeyId=$KEYID")" 200

echo -e "${YEL}[3] Metadata${NC}"
chk "POST /ListKeys"     "$(post_code ListKeys '{}')" 200
chk "POST /DescribeKey"  "$(post_code DescribeKey "{\"KeyId\":\"$KEYID\"}")" 200
chk "POST /GetPublicKey" "$(post_code GetPublicKey "{\"KeyId\":\"$KEYID\"}")" 200

echo -e "${YEL}[4] Key ops (WebAuthn ceremony)${NC}"
WA=$(ceremony "$KEYID")
chk "POST /DeriveAddress" "$(post_code DeriveAddress "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"WebAuthn\":$WA}")" 200
WA=$(ceremony_payload "$KEYID" "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2")  # #68: payload = the hash
chk "POST /SignHash" "$(post_code SignHash "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"WebAuthn\":$WA}")" 200
WA=$(ceremony_payload "$KEYID" "$(keccak256_hex 48656c6c6f)")  # #68: payload = keccak256(message)
chk "POST /Sign (message)" "$(post_code Sign "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f\",\"WebAuthn\":$WA}")" 200
WA=$(ceremony "$KEYID")
chk "POST /Sign (transaction)" "$(post_code Sign "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Transaction\":{\"chainId\":1,\"nonce\":0,\"to\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\",\"value\":\"0xde0b6b3a7640000\",\"gasPrice\":\"0x4a817c800\",\"gas\":21000,\"data\":\"\"},\"WebAuthn\":$WA}")" 200

echo -e "${YEL}[5] ChangePasskey (isolated key — avoids changing main key's passkey)${NC}"
post_code CreateKey "{\"Description\":\"cp\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
TMPKEY=$(jbody "['KeyMetadata']['KeyId']"); sleep 2
WA=$(ceremony "$TMPKEY")
chk "POST /ChangePasskey" "$(post_code ChangePasskey "{\"KeyId\":\"$TMPKEY\",\"PasskeyPublicKey\":\"$PK2\",\"WebAuthn\":$WA}")" 200

echo -e "${YEL}[6] WebAuthn registration + auth ceremony${NC}"
chk "POST /BeginRegistration"   "$(post_code BeginRegistration '{"UserName":"e2e","UserDisplayName":"E2E"}')" 200
RCID=$(jbody "['ChallengeId']"); RCHAL=$(jbody "['Options']['challenge']")
REGCRED=$(python3 "$HELPER" registration "$PEM" "$RCHAL" "cmVnY3JlZA")
chk "POST /CompleteRegistration" "$(post_code CompleteRegistration "{\"ChallengeId\":\"$RCID\",\"Credential\":$REGCRED,\"KeySpec\":\"ECC_SECG_P256K1\"}")" 200
chk "POST /BeginAuthentication" "$(post_code BeginAuthentication "{\"KeyId\":\"$KEYID\"}")" 200
chk "GET /kms/begin-grant-session-auth" "$(get_code "/kms/begin-grant-session-auth?keyId=$KEYID")" 200

echo -e "${YEL}[7] Negative — auth gates reject correctly${NC}"
chk "SignTypedData no-auth → reject"      "$(post_path_code /kms/SignTypedData SignTypedData '{"domain":{},"types":{},"primaryType":"X","message":{}}')" 400
chk "sign-grant-session no-auth → reject" "$(post_path_code /kms/sign-grant-session SignGrantSession '{}')" 400
# Release build (no `admin-purge` feature) compiles out /admin/purge-key entirely,
# so it must be unreachable → 404. (A dev/test build with --features admin-purge
# would instead return 400 for a missing admin token; this suite targets release.)
chk "admin/purge-key absent in release → 404" "$(post_path_code /admin/purge-key AdminPurge '{"key_id":"00000000-0000-0000-0000-000000000000","reason":"e2e-neg"}')" 404

echo -e "${YEL}[7b] Agent key flow (ceremony → Bearer JWT)${NC}"
post_code CreateKey "{\"Description\":\"ak\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
HKID=$(jbody "['KeyMetadata']['KeyId']"); sleep 2
WA=$(ceremony "$HKID")
chk "POST /kms/create-agent-key" "$(post_path_code /kms/create-agent-key CreateAgentKey "{\"humanKeyId\":\"$HKID\",\"label\":\"e2e\",\"webAuthnAssertion\":$WA}")" 200
ACRED=$(jbody "['agentCredential']"); AKID=$(jbody "['keyId']")
chk "POST /kms/sign-agent (Bearer JWT)" "$(post_path_code /kms/sign-agent SignAgent "{\"keyId\":\"$AKID\",\"payload\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"accountAddress\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\"}" "Authorization: Bearer $ACRED")" 200
WA=$(ceremony "$HKID")
chk "POST /kms/refresh-agent-credential" "$(post_path_code /kms/refresh-agent-credential RefreshAgentCredential "{\"keyId\":\"$AKID\",\"webAuthnAssertion\":$WA}" "Authorization: Bearer $ACRED")" 200
WA=$(ceremony "$HKID")
chk "POST /kms/revoke-agent-credential" "$(post_path_code /kms/revoke-agent-credential RevokeAgentCredential "{\"keyId\":\"$AKID\",\"webAuthnAssertion\":$WA}")" 200

echo -e "${YEL}[7c] EIP-712 / grant-session / p256-session signing (ceremony)${NC}"
ADDR="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18"
ZERO="0x0000000000000000000000000000000000000000"
WA=$(ceremony "$KEYID")
chk "POST /kms/SignTypedData" "$(post_path_code /kms/SignTypedData SignTypedData "{\"keyId\":\"$KEYID\",\"domain\":{\"name\":\"Test\",\"version\":\"1\",\"chainId\":1,\"verifyingContract\":\"$ADDR\"},\"primaryType\":\"Mail\",\"types\":[{\"name\":\"Mail\",\"fields\":[{\"name\":\"contents\",\"type\":\"string\"}]}],\"message\":[{\"name\":\"contents\",\"value\":\"hello\"}],\"webAuthnAssertion\":$WA}")" 200
WA=$(ceremony_grant "$KEYID")
chk "POST /kms/sign-grant-session" "$(post_path_code /kms/sign-grant-session SignGrantSession "{\"keyId\":\"$KEYID\",\"chainId\":1,\"verifyingContract\":\"$ADDR\",\"account\":\"$ADDR\",\"sessionKey\":\"$ADDR\",\"expiry\":9999999999,\"contractScope\":\"$ZERO\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":10,\"velocityWindow\":3600,\"nonce\":0,\"webAuthnAssertion\":$WA}")" 200
KX="1111111111111111111111111111111111111111111111111111111111111111"
KY="2222222222222222222222222222222222222222222222222222222222222222"
WA=$(ceremony_grant "$KEYID")
chk "POST /kms/sign-p256-grant-session" "$(post_path_code /kms/sign-p256-grant-session SignP256GrantSession "{\"keyId\":\"$KEYID\",\"chainId\":1,\"verifyingContract\":\"$ADDR\",\"account\":\"$ADDR\",\"keyX\":\"0x$KX\",\"keyY\":\"0x$KY\",\"expiry\":9999999999,\"contractScope\":\"$ZERO\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":10,\"velocityWindow\":3600,\"nonce\":0,\"webAuthnAssertion\":$WA}")" 200
# P256 session key: create (ceremony) → returns agentCredential → sign-p256-user-op (Bearer)
post_code CreateKey "{\"Description\":\"p256s\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
PHKID=$(jbody "['KeyMetadata']['KeyId']"); sleep 2
WA=$(ceremony "$PHKID")
chk "POST /kms/create-p256-session-key" "$(post_path_code /kms/create-p256-session-key CreateP256SessionKey "{\"humanKeyId\":\"$PHKID\",\"label\":\"e2e\",\"webAuthnAssertion\":$WA}")" 200
PCRED=$(jbody "['agentCredential']"); PKID=$(jbody "['keyId']")
chk "POST /kms/sign-p256-user-op (Bearer JWT)" "$(post_path_code /kms/sign-p256-user-op SignP256UserOp "{\"keyId\":\"$PKID\",\"payload\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"accountAddress\":\"$ADDR\"}" "Authorization: Bearer $PCRED")" 200
WA=$(ceremony "$PHKID")
chk "POST /kms/revoke-p256-session-key" "$(post_path_code /kms/revoke-p256-session-key RevokeP256SessionKey "{\"keyId\":\"$PKID\",\"webAuthnAssertion\":$WA}")" 200

echo -e "${YEL}[7d] P2 SuperPaymaster convenience signers (WebAuthn ceremony)${NC}"
# Same auth as SignTypedData: replay-protected ceremony, no legacy passkey.
WA=$(ceremony "$KEYID")
chk "POST /kms/SignX402Payment" "$(post_path_code /kms/SignX402Payment SignX402Payment "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"verifyingContract\":\"$ADDR\",\"paymentId\":\"0x$KX\",\"amount\":\"1000000\",\"recipient\":\"$ADDR\",\"deadline\":\"9999999999\",\"webAuthnAssertion\":$WA}")" 200
WA=$(ceremony "$KEYID")
chk "POST /kms/SignMicropaymentVoucher" "$(post_path_code /kms/SignMicropaymentVoucher SignMicropaymentVoucher "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"verifyingContract\":\"$ADDR\",\"channelId\":\"0x$KY\",\"cumulativeAmount\":\"500000\",\"webAuthnAssertion\":$WA}")" 200
# #52: GToken `from` MUST equal the real address derived from keyId+default hdPath
# (cached by CreateKey's background derivation). Fetch it so `from` matches.
WA=$(ceremony "$KEYID")
post_code DeriveAddress "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"WebAuthn\":$WA}" >/dev/null
GADDR=$(jbody "['Address']")
WA=$(ceremony "$KEYID")
chk "POST /kms/SignGTokenAuthorization" "$(post_path_code /kms/SignGTokenAuthorization SignGTokenAuthorization "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"gTokenAddress\":\"$ADDR\",\"from\":\"$GADDR\",\"to\":\"$ADDR\",\"value\":\"500000\",\"validAfter\":\"0\",\"validBefore\":\"9999999999\",\"nonce\":\"0x$KX\",\"webAuthnAssertion\":$WA}")" 200
# #52 (0x normalization): the same `from` without the 0x prefix must still match
# (address compare strips 0x + lower-cases — a format diff is not a mismatch).
GADDR_NO0X=$(echo "$GADDR" | sed 's/^0x//')
WA=$(ceremony "$KEYID")
chk "GTokenAuth from sans-0x → 200 (normalized)" "$(post_path_code /kms/SignGTokenAuthorization SignGTokenAuthorization "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"gTokenAddress\":\"$ADDR\",\"from\":\"$GADDR_NO0X\",\"to\":\"$ADDR\",\"value\":\"500000\",\"validAfter\":\"0\",\"validBefore\":\"9999999999\",\"nonce\":\"0x$KX\",\"webAuthnAssertion\":$WA}")" 200
# #52 negative: a `from` that is NOT the derived address must be rejected pre-sign.
WA=$(ceremony "$KEYID")
chk "GTokenAuth wrong from → reject" "$(post_path_code /kms/SignGTokenAuthorization SignGTokenAuthorization "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"gTokenAddress\":\"$ADDR\",\"from\":\"$ADDR\",\"to\":\"$ADDR\",\"value\":\"500000\",\"validAfter\":\"0\",\"validBefore\":\"9999999999\",\"nonce\":\"0x$KX\",\"webAuthnAssertion\":$WA}")" 400
# Negative: no auth → reject (same gate as SignTypedData)
chk "SignX402Payment no-auth → reject" "$(post_path_code /kms/SignX402Payment SignX402Payment "{\"keyId\":\"$KEYID\",\"chainId\":1,\"verifyingContract\":\"$ADDR\",\"paymentId\":\"0x$KX\",\"amount\":\"1\",\"recipient\":\"$ADDR\",\"deadline\":\"9999999999\"}")" 400

echo -e "${YEL}[8] Cleanup${NC}"
WA=$(ceremony "$KEYID")
chk "POST /DeleteKey (ScheduleKeyDeletion)" "$(del_code "{\"KeyId\":\"$KEYID\",\"WebAuthn\":$WA}")" 200

echo "════════════════════════════════════════════"
echo -e "Total: ${GRN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
if [ $FAIL -gt 0 ]; then printf "Failed: %s\n" "${FAILED_NAMES[*]}"; exit 1; else echo -e "${GRN}ALL PASSED${NC}"; fi
