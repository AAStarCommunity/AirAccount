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
get_code()  { curl -s --max-time 15 -o "$LASTF" -w '%{http_code}' "$BASE$1"; }
post_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' -X POST "$BASE/$1" -H "Content-Type: application/json" -H "x-amz-target: TrentService.$1" -d "$2"; }
post_path_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' -X POST "$BASE$1" -H "Content-Type: application/json" -H "x-amz-target: TrentService.$2" ${4:+-H "$4"} -d "$3"; }
# DeleteKey is exposed at path /DeleteKey but its AWS-KMS action name is ScheduleKeyDeletion
del_code() { curl -s --max-time 30 -o "$LASTF" -w '%{http_code}' -X POST "$BASE/DeleteKey" -H "Content-Type: application/json" -H "x-amz-target: TrentService.ScheduleKeyDeletion" -d "$1"; }

# chk <name> <got_code> <expect_code>  (body context read from $LASTF)
chk() {
  if [ "$2" = "$3" ]; then PASS=$((PASS+1)); printf "${GRN} OK ${NC} %-42s %s\n" "$1" "$2"
  else FAIL=$((FAIL+1)); FAILED_NAMES+=("$1"); printf "${RED}FAIL${NC} %-42s got=%s want=%s  %s\n" "$1" "$2" "$3" "$(head -c 90 "$LASTF")"; fi
}

# ceremony <keyid> → echoes WebAuthn assertion JSON {ChallengeId, Credential}
ceremony() {
  local kid="$1" ba cid chal cred sc
  sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc" > "$SCF"   # strictly increasing signCount
  ba=$(curl -s --max-time 15 -X POST "$BASE/BeginAuthentication" -H "Content-Type: application/json" -H "x-amz-target: TrentService.BeginAuthentication" -d "{\"KeyId\":\"$kid\"}")
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
WA=$(ceremony "$KEYID")
chk "POST /SignHash" "$(post_code SignHash "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"WebAuthn\":$WA}")" 200
WA=$(ceremony "$KEYID")
chk "POST /Sign (message)" "$(post_code Sign "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f\",\"WebAuthn\":$WA}")" 200

echo -e "${YEL}[5] ChangePasskey (isolated key — avoids changing main key's passkey)${NC}"
post_code CreateKey "{\"Description\":\"cp\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
TMPKEY=$(jbody "['KeyMetadata']['KeyId']"); sleep 2
WA=$(ceremony "$TMPKEY")
chk "POST /ChangePasskey" "$(post_code ChangePasskey "{\"KeyId\":\"$TMPKEY\",\"PasskeyPublicKey\":\"$PK2\",\"WebAuthn\":$WA}")" 200

echo -e "${YEL}[6] WebAuthn ceremony endpoints${NC}"
chk "POST /BeginRegistration"   "$(post_code BeginRegistration '{"UserName":"e2e","UserDisplayName":"E2E"}')" 200
chk "POST /BeginAuthentication" "$(post_code BeginAuthentication "{\"KeyId\":\"$KEYID\"}")" 200

echo -e "${YEL}[7] Negative — auth gates reject correctly${NC}"
chk "SignTypedData no-auth → reject"      "$(post_path_code /kms/SignTypedData SignTypedData '{"domain":{},"types":{},"primaryType":"X","message":{}}')" 400
chk "sign-grant-session no-auth → reject" "$(post_path_code /kms/sign-grant-session SignGrantSession '{}')" 400

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

echo -e "${YEL}[8] Cleanup${NC}"
WA=$(ceremony "$KEYID")
chk "POST /DeleteKey (ScheduleKeyDeletion)" "$(del_code "{\"KeyId\":\"$KEYID\",\"WebAuthn\":$WA}")" 200

echo "════════════════════════════════════════════"
echo -e "Total: ${GRN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
if [ $FAIL -gt 0 ]; then printf "Failed: %s\n" "${FAILED_NAMES[*]}"; exit 1; else echo -e "${GRN}ALL PASSED${NC}"; fi
