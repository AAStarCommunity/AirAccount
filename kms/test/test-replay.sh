#!/bin/bash
# #49 anti-replay regression — RUN ON THE BOARD.
# After the DoS-on-nonce fix (peek-then-consume), the nonce must still be strictly
# one-time: submitting the SAME assertion twice must fail the second time, and a
# request with a WRONG challenge must NOT burn a valid pending nonce.
set -u
HOST="${1:-127.0.0.1:3000}"; BASE="http://$HOST"
DIR="$(cd "$(dirname "$0")" && pwd)"; HELPER="$DIR/p256_helper.py"
SCF=$(mktemp); echo 1 > "$SCF"; trap 'rm -f "$SCF" /tmp/r' EXIT
RED='\033[0;31m'; GRN='\033[0;32m'; NC='\033[0m'
PK=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['public_key_hex'])")
PEM=$(python3 -c "import json;print(json.load(open('$DIR/test-fixtures/user1.json'))['private_key_pem'])")
PASS=0; FAIL=0
chk(){ if [ "$2" = "$3" ]; then PASS=$((PASS+1)); printf "${GRN} OK ${NC} %-44s %s\n" "$1" "$2";
       else FAIL=$((FAIL+1)); printf "${RED}FAIL${NC} %-44s got=%s want=%s %s\n" "$1" "$2" "$3" "$(head -c 80 /tmp/r)"; fi; }
chkne(){ if [ "$2" != "$3" ]; then PASS=$((PASS+1)); printf "${GRN} OK ${NC} %-44s %s (≠%s)\n" "$1" "$2" "$3";
       else FAIL=$((FAIL+1)); printf "${RED}FAIL${NC} %-44s got=%s (must differ) %s\n" "$1" "$2" "$(head -c 80 /tmp/r)"; fi; }
post(){ curl -s -o /tmp/r -w '%{http_code}' --max-time 30 -X POST "$BASE/$1" -H "Content-Type: application/json" -H "x-amz-target: TrentService.$1" -d "$2"; }
da(){ post DeriveAddress "{\"KeyId\":\"$1\",\"DerivationPath\":\"m/44'/60'/0'/0/1\",\"WebAuthn\":$2}"; }
ceremony(){ local kid="$1" ba cid chal cred sc; sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc">"$SCF"
  ba=$(curl -s --max-time 15 -X POST "$BASE/BeginAuthentication" -H "Content-Type: application/json" -H "x-amz-target: TrentService.BeginAuthentication" -d "{\"KeyId\":\"$kid\"}")
  cid=$(echo "$ba"|python3 -c "import sys,json;print(json.load(sys.stdin)['ChallengeId'])" 2>/dev/null)
  chal=$(echo "$ba"|python3 -c "import sys,json;print(json.load(sys.stdin)['Options']['challenge'])" 2>/dev/null)
  cred=$(python3 "$HELPER" ceremony "$PEM" "$chal" "dGVzdC1jcmVkZW50aWFs" "$sc")
  echo "{\"ChallengeId\":\"$cid\",\"Credential\":$cred}"; }

echo "════════ #49 anti-replay + DoS-on-nonce @ $BASE ════════"
post CreateKey "{\"Description\":\"replay\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
KID=$(python3 -c "import json;print(json.load(open('/tmp/r'))['KeyMetadata']['KeyId'])" 2>/dev/null)
echo "KeyId=$KID"; sleep 2

# (A) anti-replay: same assertion twice — first OK, replay rejected.
WA=$(ceremony "$KID")
chk   "1. first use of assertion → 200"        "$(da "$KID" "$WA")" 200
chkne "2. replay same assertion → rejected"    "$(da "$KID" "$WA")" 200

# (B) DoS-on-nonce fix: a wrong-challenge request must NOT burn a fresh nonce.
#     Issue a fresh challenge, submit a STALE assertion (old challenge) to fail
#     verification, then a correct assertion for the fresh challenge must succeed.
STALE=$(ceremony "$KID")              # nonce#1 issued; STALE binds nonce#1
da "$KID" "$STALE" >/dev/null          # consumes nonce#1 (valid use)
OLD=$(ceremony "$KID")                 # nonce#2 issued; capture assertion for it
da "$KID" "$OLD" >/dev/null            # consume nonce#2 so OLD is now stale
FRESH=$(ceremony "$KID")               # nonce#3 issued (pending)
chkne "3. stale assertion vs fresh nonce → reject" "$(da "$KID" "$OLD")" 200
chk   "4. correct assertion still works (nonce not burned) → 200" "$(da "$KID" "$FRESH")" 200

echo "════════════════════════════════════"
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
