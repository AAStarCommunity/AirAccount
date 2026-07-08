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

# #115 mint_label_digest = SHA256(tag ‖ wallet_id[16] ‖ SHA256(label)). $1=uuid $2=label $3=tag
mint_digest() { python3 -c "
import hashlib,uuid
u=uuid.UUID('$1').bytes
print(hashlib.sha256(b'$3'+u+hashlib.sha256(b'$2').digest()).hexdigest())"; }

# #115 refresh digest = SHA256("AA-AGENT-REFRESH-v2" ‖ wallet[16] ‖ agent_index_u32_be). $1=uuid $2=index
refresh_digest() { python3 -c "
import hashlib,uuid
print(hashlib.sha256(b'AA-AGENT-REFRESH-v2'+uuid.UUID('$1').bytes+int('$2').to_bytes(4,'big')).hexdigest())"; }

# EIP-712 digest for the harness's fixed typed data(domain Test/1/chain1/$ADDR,Mail{contents:"hello"})
# 匹配 TA eip712.rs:keccak(0x1901 ‖ domainSeparator ‖ hashStruct)。$1=verifyingContract(0x+40hex)
typed_data_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
addr=bytes.fromhex('$1'[2:])
dom_th=k(b'EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)')
ds=k(dom_th+k(b'Test')+k(b'1')+(1).to_bytes(32,'big')+b'\x00'*12+addr)
hs=k(k(b'Mail(string contents)')+k(b'hello'))
print(k(b'\x19\x01'+ds+hs).hex())"; }

# legacy tx signing hash (EIP-155): keccak(RLP([nonce,gasPrice,gas,to,value,data,chainId,0,0]))
# matches Wallet::tx_signing_hash. args via env TX_* (hex 0x or int).
tx_sign_hash() { python3 -c "
from Crypto.Hash import keccak
def rlp(x):
    if isinstance(x,list):
        b=b''.join(rlp(i) for i in x); return _len(0xc0,b)
    return _len(0x80,x) if not(len(x)==1 and x[0]<0x80) else x
def _len(off,b):
    if len(b)<56: return bytes([off+len(b)])+b
    L=len(b).to_bytes((len(b).bit_length()+7)//8,'big'); return bytes([off+55+len(L)])+L+b
def i2b(n): return b'' if n==0 else n.to_bytes((n.bit_length()+7)//8,'big')
h='$1'  # to address (0x + 40 hex)
fields=[i2b($2),i2b($3),i2b($4),bytes.fromhex(h[2:]),i2b($5),bytes.fromhex('$6'),i2b($7),b'',b'']
k=keccak.new(digest_bits=256); k.update(rlp(fields)); print(k.hexdigest())"; }

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

# ceremony_grant_payload <keyid> <payload_hex> — grant-session ceremony committed to payload (#68).
ceremony_grant_payload() {
  local kid="$1" payload_hex="$2" ba cid chal committed cred sc
  sc=$(cat "$SCF"); sc=$((sc+1)); echo "$sc" > "$SCF"
  ba=$(curl -s --max-time 15 "${AK[@]}" "$BASE/kms/begin-grant-session-auth?keyId=$kid")
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

# grant-session digest: eip191_hash(keccak(build_grant_session_inner))。$1=verifyingContract(=account=sessionKey in test)
grant_session_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
addr=bytes.fromhex('$1'[2:]); z20=b'\x00'*20
def u256(n): return n.to_bytes(32,'big')
def pa(a): return b'\x00'*12+a
eh=k(b'')
buf=bytearray(480)
buf[0:32]=u256(416); buf[32:64]=u256(1); buf[64:96]=pa(addr); buf[96:128]=pa(addr)
buf[128:160]=pa(addr); buf[160:192]=u256(9999999999); buf[192:224]=pa(z20)
buf[224:256]=b'\x00'*32; buf[256:288]=u256(10); buf[288:320]=u256(3600)
buf[320:352]=eh; buf[352:384]=eh; buf[384:416]=u256(0)
buf[416:448]=u256(16); buf[448:464]=b'GRANT_SESSION_V2'
inner=k(bytes(buf))
print(k(b'\x19Ethereum Signed Message:\n32'+inner).hex())"; }

# p256-grant-session digest。$1=verifyingContract(=account) $2=keyX(64hex) $3=keyY(64hex)
p256_grant_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
addr=bytes.fromhex('$1'[2:]); z20=b'\x00'*20; kx=bytes.fromhex('$2'); ky=bytes.fromhex('$3')
def u256(n): return n.to_bytes(32,'big')
def pa(a): return b'\x00'*12+a
eh=k(b'')
buf=bytearray(512)  # 512 字节:string word [480..512] = 21B 串 + 11B 零填充
buf[0:32]=u256(448); buf[32:64]=u256(1); buf[64:96]=pa(addr); buf[96:128]=pa(addr)
buf[128:160]=kx; buf[160:192]=ky; buf[192:224]=u256(9999999999); buf[224:256]=pa(z20)
buf[256:288]=b'\x00'*32; buf[288:320]=u256(10); buf[320:352]=u256(3600)
buf[352:384]=eh; buf[384:416]=eh; buf[416:448]=u256(0)
buf[448:480]=u256(21); buf[480:501]=b'GRANT_P256_SESSION_V2'
inner=k(bytes(buf))
print(k(b'\x19Ethereum Signed Message:\n32'+inner).hex())"; }

# P2 便利签名 op = host 构造 EIP-712 typed-data 发给 TA sign_typed_data。各 digest = eip712(domain,struct)。
# x402: SuperPaymaster/1 · PaymentPayload(bytes32 paymentId,uint256 amount,address recipient,uint256 deadline)
# $1=verifyingContract $2=paymentId(64hex) $3=amount $4=recipient $5=deadline
x402_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
def u(n): return int(n).to_bytes(32,'big')
def a(h): return b'\x00'*12+bytes.fromhex(h[2:] if h[:2]=='0x' else h)
dth=k(b'EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)')
ds=k(dth+k(b'SuperPaymaster')+k(b'1')+u(11155111)+a('$1'))
sth=k(b'PaymentPayload(bytes32 paymentId,uint256 amount,address recipient,uint256 deadline)')
hs=k(sth+bytes.fromhex('$2')+u('$3')+a('$4')+u('$5'))
print(k(b'\x19\x01'+ds+hs).hex())"; }

# micropayment: MicroPaymentChannel/1.0.0 · Voucher(bytes32 channelId,uint256 cumulativeAmount)
# $1=verifyingContract $2=channelId(64hex) $3=cumulativeAmount
micropay_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
def u(n): return int(n).to_bytes(32,'big')
def a(h): return b'\x00'*12+bytes.fromhex(h[2:] if h[:2]=='0x' else h)
dth=k(b'EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)')
ds=k(dth+k(b'MicroPaymentChannel')+k(b'1.0.0')+u(11155111)+a('$1'))
sth=k(b'Voucher(bytes32 channelId,uint256 cumulativeAmount)')
hs=k(sth+bytes.fromhex('$2')+u('$3'))
print(k(b'\x19\x01'+ds+hs).hex())"; }

# gtoken: GToken/1 · TransferWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)
# $1=gTokenAddress $2=from $3=to $4=value $5=validAfter $6=validBefore $7=nonce(64hex)
gtoken_digest() { python3 -c "
from Crypto.Hash import keccak
def k(b):
    h=keccak.new(digest_bits=256); h.update(b); return h.digest()
def u(n): return int(n).to_bytes(32,'big')
def a(h): return b'\x00'*12+bytes.fromhex(h[2:] if h[:2]=='0x' else h)
dth=k(b'EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)')
ds=k(dth+k(b'GToken')+k(b'1')+u(11155111)+a('$1'))
sth=k(b'TransferWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)')
hs=k(sth+a('$2')+a('$3')+u('$4')+u('$5')+u('$6')+bytes.fromhex('$7'))
print(k(b'\x19\x01'+ds+hs).hex())"; }

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
WA=$(ceremony_payload "$KEYID" "$(tx_sign_hash 0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18 0 0x4a817c800 21000 0xde0b6b3a7640000 "" 1)")  # #68: payload = RLP-keccak tx hash
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
WA=$(ceremony_payload "$HKID" "$(mint_digest "$HKID" e2e AA-AGENT-MINT-v2)")  # #68: mint_label_digest
chk "POST /kms/create-agent-key" "$(post_path_code /kms/create-agent-key CreateAgentKey "{\"humanKeyId\":\"$HKID\",\"label\":\"e2e\",\"webAuthnAssertion\":$WA}")" 200
ACRED=$(jbody "['agentCredential']"); AKID=$(jbody "['keyId']")
chk "POST /kms/sign-agent (Bearer JWT)" "$(post_path_code /kms/sign-agent SignAgent "{\"keyId\":\"$AKID\",\"payload\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"accountAddress\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18\"}" "Authorization: Bearer $ACRED")" 200
WA=$(ceremony_payload "$HKID" "$(refresh_digest "$HKID" "$(echo "$AKID" | cut -d: -f2)")")  # #68: agent_refresh_digest(wallet,index)
chk "POST /kms/refresh-agent-credential" "$(post_path_code /kms/refresh-agent-credential RefreshAgentCredential "{\"keyId\":\"$AKID\",\"webAuthnAssertion\":$WA}" "Authorization: Bearer $ACRED")" 200
WA=$(ceremony "$HKID")
chk "POST /kms/revoke-agent-credential" "$(post_path_code /kms/revoke-agent-credential RevokeAgentCredential "{\"keyId\":\"$AKID\",\"webAuthnAssertion\":$WA}")" 200

echo -e "${YEL}[7c] EIP-712 / grant-session / p256-session signing (ceremony)${NC}"
ADDR="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18"
ZERO="0x0000000000000000000000000000000000000000"
WA=$(ceremony_payload "$KEYID" "$(typed_data_digest "$ADDR")")  # #68: EIP-712 digest
chk "POST /kms/SignTypedData" "$(post_path_code /kms/SignTypedData SignTypedData "{\"keyId\":\"$KEYID\",\"domain\":{\"name\":\"Test\",\"version\":\"1\",\"chainId\":1,\"verifyingContract\":\"$ADDR\"},\"primaryType\":\"Mail\",\"types\":[{\"name\":\"Mail\",\"fields\":[{\"name\":\"contents\",\"type\":\"string\"}]}],\"message\":[{\"name\":\"contents\",\"value\":\"hello\"}],\"webAuthnAssertion\":$WA}")" 200
WA=$(ceremony_grant_payload "$KEYID" "$(grant_session_digest "$ADDR")")  # #68: eip191(grant_session_inner)
chk "POST /kms/sign-grant-session" "$(post_path_code /kms/sign-grant-session SignGrantSession "{\"keyId\":\"$KEYID\",\"chainId\":1,\"verifyingContract\":\"$ADDR\",\"account\":\"$ADDR\",\"sessionKey\":\"$ADDR\",\"expiry\":9999999999,\"contractScope\":\"$ZERO\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":10,\"velocityWindow\":3600,\"nonce\":0,\"webAuthnAssertion\":$WA}")" 200
KX="1111111111111111111111111111111111111111111111111111111111111111"
KY="2222222222222222222222222222222222222222222222222222222222222222"
WA=$(ceremony_grant_payload "$KEYID" "$(p256_grant_digest "$ADDR" "$KX" "$KY")")  # #68: eip191(p256_grant_inner)
chk "POST /kms/sign-p256-grant-session" "$(post_path_code /kms/sign-p256-grant-session SignP256GrantSession "{\"keyId\":\"$KEYID\",\"chainId\":1,\"verifyingContract\":\"$ADDR\",\"account\":\"$ADDR\",\"keyX\":\"0x$KX\",\"keyY\":\"0x$KY\",\"expiry\":9999999999,\"contractScope\":\"$ZERO\",\"selectorScope\":\"0x00000000\",\"velocityLimit\":10,\"velocityWindow\":3600,\"nonce\":0,\"webAuthnAssertion\":$WA}")" 200
# P256 session key: create (ceremony) → returns agentCredential → sign-p256-user-op (Bearer)
post_code CreateKey "{\"Description\":\"p256s\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$PK\"}" >/dev/null
PHKID=$(jbody "['KeyMetadata']['KeyId']"); sleep 2
WA=$(ceremony_payload "$PHKID" "$(mint_digest "$PHKID" e2e AA-P256-SESSION-MINT-v2)")  # #68: mint_label_digest
chk "POST /kms/create-p256-session-key" "$(post_path_code /kms/create-p256-session-key CreateP256SessionKey "{\"humanKeyId\":\"$PHKID\",\"label\":\"e2e\",\"webAuthnAssertion\":$WA}")" 200
PCRED=$(jbody "['agentCredential']"); PKID=$(jbody "['keyId']")
chk "POST /kms/sign-p256-user-op (Bearer JWT)" "$(post_path_code /kms/sign-p256-user-op SignP256UserOp "{\"keyId\":\"$PKID\",\"payload\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"accountAddress\":\"$ADDR\"}" "Authorization: Bearer $PCRED")" 200
WA=$(ceremony "$PHKID")
chk "POST /kms/revoke-p256-session-key" "$(post_path_code /kms/revoke-p256-session-key RevokeP256SessionKey "{\"keyId\":\"$PKID\",\"webAuthnAssertion\":$WA}")" 200

echo -e "${YEL}[7d] P2 SuperPaymaster convenience signers (WebAuthn ceremony)${NC}"
# Same auth as SignTypedData: replay-protected ceremony, no legacy passkey.
WA=$(ceremony_payload "$KEYID" "$(x402_digest "$ADDR" "$KX" 1000000 "$ADDR" 9999999999)")  # #68: EIP-712 PaymentPayload
chk "POST /kms/SignX402Payment" "$(post_path_code /kms/SignX402Payment SignX402Payment "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"verifyingContract\":\"$ADDR\",\"paymentId\":\"0x$KX\",\"amount\":\"1000000\",\"recipient\":\"$ADDR\",\"deadline\":\"9999999999\",\"webAuthnAssertion\":$WA}")" 200
WA=$(ceremony_payload "$KEYID" "$(micropay_digest "$ADDR" "$KY" 500000)")  # #68: EIP-712 Voucher
chk "POST /kms/SignMicropaymentVoucher" "$(post_path_code /kms/SignMicropaymentVoucher SignMicropaymentVoucher "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"verifyingContract\":\"$ADDR\",\"channelId\":\"0x$KY\",\"cumulativeAmount\":\"500000\",\"webAuthnAssertion\":$WA}")" 200
# #52: GToken `from` MUST equal the real address derived from keyId+default hdPath
# (cached by CreateKey's background derivation). Fetch it so `from` matches.
WA=$(ceremony "$KEYID")
post_code DeriveAddress "{\"KeyId\":\"$KEYID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"WebAuthn\":$WA}" >/dev/null
GADDR=$(jbody "['Address']")
WA=$(ceremony_payload "$KEYID" "$(gtoken_digest "$ADDR" "$GADDR" "$ADDR" 500000 0 9999999999 "$KX")")  # #68: EIP-712 TransferWithAuthorization
chk "POST /kms/SignGTokenAuthorization" "$(post_path_code /kms/SignGTokenAuthorization SignGTokenAuthorization "{\"keyId\":\"$KEYID\",\"chainId\":11155111,\"gTokenAddress\":\"$ADDR\",\"from\":\"$GADDR\",\"to\":\"$ADDR\",\"value\":\"500000\",\"validAfter\":\"0\",\"validBefore\":\"9999999999\",\"nonce\":\"0x$KX\",\"webAuthnAssertion\":$WA}")" 200
# #52 (0x normalization): the same `from` without the 0x prefix must still match
# (address compare strips 0x + lower-cases — a format diff is not a mismatch).
GADDR_NO0X=$(echo "$GADDR" | sed 's/^0x//')
WA=$(ceremony_payload "$KEYID" "$(gtoken_digest "$ADDR" "$GADDR_NO0X" "$ADDR" 500000 0 9999999999 "$KX")")  # #68: same digest(from 归一)
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
