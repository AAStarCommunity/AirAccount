#!/bin/bash
# Issue #42 (#60) freeze/unfreeze end-to-end test — RUN ON THE BOARD (needs kms.db).
#
# The background dormant-sweep that flips lifecycle_status to 'frozen' is time-
# driven (days), so this test injects the 'frozen' state directly via SQLite to
# simulate the sweep result, then exercises the HTTP end-to-end path that the
# unit tests don't cover:
#   - a frozen key rejects signing (ensure_not_frozen gate), and
#   - POST /UnfreezeKey with an owner WebAuthn ceremony restores it.
# The "when to freeze" logic itself is covered by db.rs unit tests.
set -u
HOST="${1:-127.0.0.1:3000}"; BASE="http://$HOST"
DIR="$(cd "$(dirname "$0")" && pwd)"; HELPER="$DIR/p256_helper.py"
DB=/root/AirAccount/kms.db
SCF=$(mktemp); echo 1 > "$SCF"; trap 'rm -f "$SCF" /tmp/lf' EXIT
RED='\033[0;31m'; GRN='\033[0;32m'; NC='\033[0m'
PK=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['public_key_hex'])")
PEM=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['private_key_pem'])")
PASS=0; FAIL=0
chk(){ if [ "$2" = "$3" ]; then PASS=$((PASS+1)); printf "${GRN} OK ${NC} %-40s %s\n" "$1" "$2";
       else FAIL=$((FAIL+1)); printf "${RED}FAIL${NC} %-40s got=%s want=%s %s\n" "$1" "$2" "$3" "$(head -c 90 /tmp/lf)"; fi; }
post(){ curl -s -o /tmp/lf -w '%{http_code}' --max-time 30 -X POST "$BASE/$1" \
        -H "Content-Type: application/json" -H "x-amz-target: TrentService.$1" -d "$2"; }
ceremony(){ local kid="$1" ba cid chal cred sc; sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc">"$SCF"
  ba=$(curl -s --max-time 15 -X POST "$BASE/BeginAuthentication" -H "Content-Type: application/json" \
       -H "x-amz-target: TrentService.BeginAuthentication" -d "{\"KeyId\":\"$kid\"}")
  cid=$(echo "$ba"|python3 -c "import sys,json;print(json.load(sys.stdin)['ChallengeId'])" 2>/dev/null)
  chal=$(echo "$ba"|python3 -c "import sys,json;print(json.load(sys.stdin)['Options']['challenge'])" 2>/dev/null)
  cred=$(python3 "$HELPER" ceremony "$PEM" "$chal" "dGVzdC1jcmVkZW50aWFs" "$sc")
  echo "{\"ChallengeId\":\"$cid\",\"Credential\":$cred}"; }
lifecycle(){ sqlite3 "$DB" "SELECT lifecycle_status FROM wallets WHERE key_id='$1';" 2>/dev/null; }

echo "════════ #42 freeze/unfreeze E2E @ $BASE ════════"
post CreateKey "{\"Description\":\"freeze-e2e\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
KID=$(python3 -c "import json;print(json.load(open('/tmp/lf'))['KeyMetadata']['KeyId'])" 2>/dev/null)
echo "KeyId=$KID  (lifecycle=$(lifecycle "$KID"))"; sleep 2

WA=$(ceremony "$KID")
chk "1. active: DeriveAddress → 200" "$(post DeriveAddress "{\"KeyId\":\"$KID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"WebAuthn\":$WA}")" 200

sqlite3 "$DB" "UPDATE wallets SET lifecycle_status='frozen' WHERE key_id='$KID';"
echo "   (injected frozen; lifecycle=$(lifecycle "$KID"))"

WA=$(ceremony "$KID")
chk "2. frozen: DeriveAddress → 400 reject" "$(post DeriveAddress "{\"KeyId\":\"$KID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"WebAuthn\":$WA}")" 400

WA=$(ceremony "$KID")
chk "3. UnfreezeKey (owner ceremony) → 200" "$(post UnfreezeKey "{\"KeyId\":\"$KID\",\"WebAuthn\":$WA}")" 200
echo "   (lifecycle after unfreeze=$(lifecycle "$KID"))"

WA=$(ceremony "$KID")
chk "4. unfrozen: DeriveAddress → 200" "$(post DeriveAddress "{\"KeyId\":\"$KID\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"WebAuthn\":$WA}")" 200

# negative: unfreeze without auth must be rejected (not an unauthenticated state probe)
chk "5. UnfreezeKey no-auth → 400 reject" "$(post UnfreezeKey "{\"KeyId\":\"$KID\"}")" 400

echo "════════════════════════════════════"
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
