# KMS API Reference (STM32 DK2)

Base URL: `https://kms1.aastar.io`

All POST endpoints accept `Content-Type: application/json` and require header `x-amz-target` with the corresponding service target.

## Typical Application Flow

```
1. CreateKey         → get KeyId (instant, ~4s)
2. Poll KeyStatus    → wait for "ready" (~71s first time, seed cached)
3. RegisterPasskey   → bind WebAuthn P-256 public key to wallet
4. Sign / SignHash   → sign tx or hash (~7s with cached seed)
```

```
┌──────────┐         ┌──────────────┐         ┌─────────────┐
│  Client  │         │  KMS API     │         │  TEE (TA)   │
└────┬─────┘         └──────┬───────┘         └──────┬──────┘
     │  POST /CreateKey     │                        │
     │─────────────────────>│  create_wallet()       │
     │                      │───────────────────────>│
     │  { KeyId, Status }   │                        │
     │<─────────────────────│                        │
     │                      │  spawn background      │
     │                      │  derive_address_auto() │
     │                      │───────────────────────>│
     │  GET /KeyStatus      │                        │
     │─────────────────────>│                        │
     │  { Status:"deriving"}│                        │
     │<─────────────────────│                        │
     │         ...poll...   │                        │
     │  GET /KeyStatus      │       done (~60-75s)   │
     │─────────────────────>│<───────────────────────│
     │  { Status:"ready",   │                        │
     │    Address, PubKey } │                        │
     │<─────────────────────│                        │
     │                      │                        │
     │  POST /RegisterPasskey                        │
     │─────────────────────>│  store P-256 pubkey    │
     │  { Registered:true } │                        │
     │<─────────────────────│                        │
     │                      │                        │
     │  POST /Sign (+ Passkey assertion)             │
     │─────────────────────>│  verify_passkey()      │
     │                      │───────────────────────>│
     │                      │  sign_message()        │
     │                      │───────────────────────>│
     │  { Signature }       │                        │
     │<─────────────────────│                        │
```

---

## Endpoints

### GET /health

Health check.

```bash
curl https://kms1.aastar.io/health
```

Response:
```json
{
  "status": "healthy",
  "service": "kms-api",
  "version": "0.1.0",
  "ta_mode": "real",
  "endpoints": {
    "POST": ["/CreateKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/DeleteKey", "/RegisterPasskey"],
    "GET": ["/health", "/KeyStatus?KeyId=xxx", "/QueueStatus"]
  }
}
```

---

### POST /CreateKey

Create a new HD wallet in TEE secure storage. Returns immediately with KeyId; address derivation runs in background.

```bash
curl -X POST https://kms1.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "my-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'
```

Response:
```json
{
  "KeyMetadata": {
    "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
    "Arn": "arn:aws:kms:region:account:key/c45a955b-...",
    "CreationDate": "2026-03-01T16:26:15Z",
    "Enabled": true,
    "Description": "my-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  },
  "Mnemonic": "[MNEMONIC_IN_SECURE_WORLD]"
}
```

After receiving the response, poll `/KeyStatus` until `Status` is `"ready"`.

---

### GET /KeyStatus?KeyId=\<uuid\>

Poll address derivation progress.

```bash
curl "https://kms1.aastar.io/KeyStatus?KeyId=c45a955b-2e50-41bf-8331-3a6de70b27e6"
```

Status values:
| Status | Meaning |
|--------|---------|
| `creating` | Wallet created, derivation not started |
| `deriving` | BIP32 key derivation in progress (~60-75s) |
| `ready` | Address and public key available |
| `error` | Derivation failed (see `Error` field) |

Response when ready:
```json
{
  "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
  "Status": "ready",
  "Address": "0x51671e4d896d718208b549d05bb6f6d8a7f5b89e",
  "PublicKey": "0x03799170bf8863a004acd475640b1588af391d9d79ff3d4d1c5a5b32669f64498b",
  "DerivationPath": "m/44'/60'/0'/0/0"
}
```

---

### GET /QueueStatus

Check TEE operation queue depth.

```bash
curl https://kms1.aastar.io/QueueStatus
```

Response:
```json
{
  "queue_depth": 1,
  "estimated_wait_seconds": 80
}
```

TEE is single-threaded; concurrent operations queue up.

---

### POST /RegisterPasskey

Register a WebAuthn P-256 public key for a wallet. Once registered, all Sign/SignHash requests for this wallet must include a valid PassKey assertion.

```bash
curl -X POST https://kms1.aastar.io/RegisterPasskey \
  -H "Content-Type: application/json" \
  -d '{
    "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
    "PasskeyPublicKey": "0x04<64-bytes-x><64-bytes-y>",
    "CredentialId": "optional-credential-id"
  }'
```

- `PasskeyPublicKey`: Uncompressed P-256 public key (65 bytes hex, starts with `04`)
- `CredentialId`: Optional WebAuthn credential ID for reference

Response:
```json
{
  "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
  "Registered": true
}
```

---

### POST /Sign

Sign an Ethereum transaction or arbitrary message. If a PassKey is registered for this wallet, a `Passkey` assertion must be included.

**Message signing:**
```bash
curl -X POST https://kms1.aastar.io/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d '{
    "KeyId": "c45a955b-...",
    "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0",
    "Message": "0x48656c6c6f",
    "SigningAlgorithm": "ECDSA_SHA_256",
    "Passkey": {
      "AuthenticatorData": "0x<hex>",
      "ClientDataHash": "0x<32-bytes-hex>",
      "Signature": "0x<DER-or-64-byte-r||s-hex>"
    }
  }'
```

**Transaction signing:**
```bash
curl -X POST https://kms1.aastar.io/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d '{
    "KeyId": "c45a955b-...",
    "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0",
    "Transaction": {
      "chainId": 11155111,
      "nonce": 0,
      "to": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
      "value": "0xde0b6b3a7640000",
      "gasPrice": "0x4a817c800",
      "gas": 21000,
      "data": ""
    }
  }'
```

**Address-based signing** (looks up KeyId and path from cache):
```bash
curl -X POST https://kms1.aastar.io/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d '{
    "Address": "0x51671e4d896d718208b549d05bb6f6d8a7f5b89e",
    "Message": "0x48656c6c6f",
    "SigningAlgorithm": "ECDSA_SHA_256"
  }'
```

Response:
```json
{
  "Signature": "3045022100...",
  "TransactionHash": "[TX_HASH_OR_MESSAGE_HASH]"
}
```

---

### POST /SignHash

Sign a raw 32-byte hash directly (no additional hashing).

```bash
curl -X POST https://kms1.aastar.io/SignHash \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.SignHash" \
  -d '{
    "KeyId": "c45a955b-...",
    "Hash": "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658",
    "Passkey": {
      "AuthenticatorData": "0x<hex>",
      "ClientDataHash": "0x<32-bytes-hex>",
      "Signature": "0x<hex>"
    }
  }'
```

Response:
```json
{
  "Signature": "3045022100..."
}
```

`Passkey` field is required only if a PassKey is registered for this wallet. Without registration, omit it for backward compatibility.

---

### DeleteKey (CLI only — not available via API)

DeleteKey is removed from the public API for security. Use the CLI tool on the DK2 board directly:

```bash
# SSH to DK2
ssh root@192.168.7.2

# List wallets (via API or inspect secure storage)
curl -s -X POST http://127.0.0.1:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" -d '{}'

# Delete a specific wallet
./kms remove-wallet -w c45a955b-2e50-41bf-8331-3a6de70b27e6
```

This permanently deletes the wallet, its entropy, and cached seed from TEE secure storage.

---

### POST /ListKeys

List all wallets in current server session (in-memory metadata store).

```bash
curl -X POST https://kms1.aastar.io/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}'
```

Response:
```json
{
  "Keys": [
    {
      "KeyId": "c45a955b-...",
      "KeyArn": "arn:aws:kms:region:account:key/c45a955b-..."
    }
  ]
}
```

---

### POST /DescribeKey

Get detailed metadata for a key.

```bash
curl -X POST https://kms1.aastar.io/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d '{"KeyId": "c45a955b-..."}'
```

Response:
```json
{
  "KeyMetadata": {
    "KeyId": "c45a955b-...",
    "Address": "0x51671e4d...",
    "PublicKey": "0x03799170...",
    "DerivationPath": "m/44'/60'/0'/0/0",
    "Arn": "arn:aws:kms:region:account:key/c45a955b-...",
    "CreationDate": "2026-03-01T16:26:15Z",
    "Enabled": true,
    "Description": "my-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }
}
```

---

## Full Example: Create Wallet + Register PassKey + Sign

```bash
# Step 1: Create wallet
KEY_ID=$(curl -s -X POST https://kms1.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{"Description":"user-wallet","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' \
  | jq -r '.KeyMetadata.KeyId')
echo "Created: $KEY_ID"

# Step 2: Poll until ready
while true; do
  STATUS=$(curl -s "https://kms1.aastar.io/KeyStatus?KeyId=$KEY_ID" | jq -r '.Status')
  echo "Status: $STATUS"
  [ "$STATUS" = "ready" ] && break
  sleep 5
done

# Step 3: Get address and public key
INFO=$(curl -s "https://kms1.aastar.io/KeyStatus?KeyId=$KEY_ID")
ADDRESS=$(echo "$INFO" | jq -r '.Address')
PUBKEY=$(echo "$INFO" | jq -r '.PublicKey')
echo "Address: $ADDRESS"
echo "PublicKey: $PUBKEY"

# Step 4: Register PassKey (from WebAuthn registration ceremony)
curl -X POST https://kms1.aastar.io/RegisterPasskey \
  -H "Content-Type: application/json" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"PasskeyPublicKey\": \"0x04<your-p256-pubkey-hex>\"
  }"

# Step 5: Sign with PassKey assertion (from WebAuthn authentication ceremony)
curl -X POST https://kms1.aastar.io/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d "{
    \"KeyId\": \"$KEY_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\",
    \"Message\": \"0x48656c6c6f\",
    \"SigningAlgorithm\": \"ECDSA_SHA_256\",
    \"Passkey\": {
      \"AuthenticatorData\": \"0x<from-webauthn>\",
      \"ClientDataHash\": \"0x<sha256-of-clientDataJSON>\",
      \"Signature\": \"0x<der-signature-from-webauthn>\"
    }
  }"

# Step 6: Clean up (delete wallet from secure storage)
curl -X POST https://kms1.aastar.io/DeleteKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ScheduleKeyDeletion" \
  -d "{\"KeyId\": \"$KEY_ID\", \"PendingWindowInDays\": 0}"
```

---

## Performance Notes (STM32MP157F-DK2, Cortex-A7 @ 650MHz)

PBKDF2-HMAC-SHA512 (2048 rounds) is the bottleneck. Seed caching stores the 64-byte PBKDF2 output in secure storage, eliminating re-computation on subsequent operations.

| Operation | Without cache | With seed cache (v2) |
|-----------|-----------|------------------------|
| CreateKey (wallet only) | ~4s | ~4s |
| Address derivation (first, background) | ~71s | ~71s (PBKDF2 runs once) |
| Address derivation (subsequent) | ~75s | **~7s** |
| Sign (message) | ~80s | **~7s** |
| SignHash | ~83s | **~7s** |
| RegisterPasskey | instant | instant |
| PassKey verify (P-256 ECDSA) | <1s | <1s |
| KeyStatus / QueueStatus | instant | instant |

Seed cache is automatically populated on first wallet operation (CreateKey → auto-derive). All subsequent operations (Sign, SignHash, DeriveAddress) skip PBKDF2 and run in ~7s (BIP32 derivation + secp256k1 signing on 32-bit ARM).

- TEE is single-threaded; concurrent operations queue up sequentially
- Check `/QueueStatus` to estimate wait time before submitting operations

## Secure Storage Capacity (STM32MP157F-DK2)

Storage model: one OP-TEE persistent object per wallet + one metadata index object.

Per-wallet storage:
| Field | Size |
|-------|------|
| UUID | 16 bytes |
| entropy | 32 bytes |
| next_address_index | 4 bytes |
| next_account_index | 4 bytes |
| bincode overhead | ~8 bytes |
| **Subtotal (current)** | **~64 bytes** |
| cached_seed | +64 bytes |
| **Subtotal (with cache)** | **~128 bytes** |

OP-TEE secure storage on STM32MP157 uses REE filesystem (`/data/tee/`) encrypted with per-TA HUK-derived keys. Storage is limited by:
- SD card space (effectively unlimited for wallet data)
- OP-TEE object ID max length: 64 bytes (`"Wallet#<uuid>"` = ~43 bytes, OK)
- Metadata index object grows with wallet count (HashSet of keys)

Practical capacity: **thousands of wallets** — each wallet is ~128 bytes, the bottleneck is the metadata index object (key list) which grows linearly but remains small.

## Important Notes

- **Secure Storage**: Wallets persist in OP-TEE secure storage across reboots. Delete unused wallets via CLI on the board: `ssh root@192.168.7.2 ./kms remove-wallet -w <wallet-id>`
- **In-Memory Metadata**: The metadata store (ListKeys, DescribeKey) is in-memory only. After server restart, wallets still exist in TEE secure storage but won't appear in ListKeys until re-derived.
- **PassKey Store**: PassKey registrations are in-memory only. After server restart, passkeys must be re-registered.
- **HD Path**: Default derivation path is `m/44'/60'/0'/0/0` (Ethereum standard BIP-44).
