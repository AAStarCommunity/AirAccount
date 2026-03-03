# KMS Full Test Results

> Last updated: 2026-03-03 15:35

Board: STM32MP157F-DK2 (Cortex-A7 650MHz)
Branch: KMS-stm32

## Unit Tests

| Crate | Tests | Status |
|-------|-------|--------|
| proto | 26 passed | PASS |
| kms host (lib) | 52 passed | PASS |
| **Total** | **78** | **All PASS** |

### Test Breakdown

**proto (26 tests)**:
- Command enum: roundtrip, to_u32, from_u32, unknown mapping
- I/O structs: CreateWallet, RemoveWallet, DeriveAddress, DeriveAddressAuto, SignTransaction, SignMessage, SignHash, ExportPrivateKey, VerifyPasskey, WarmupCache, RegisterPasskeyTa — all with roundtrip serialization
- PasskeyAssertion roundtrip
- UUID constant validation
- JSON compatibility (EthTransaction, CreateWalletOutput)

**kms host - address_cache (11 tests)**:
- Address map: empty roundtrip, lookup hit/miss, overwrite, multiple entries
- Metadata: JSON roundtrip, all fields, deserialize from known JSON
- Error handling: invalid JSON, missing required field
- Timestamp freshness

**kms host - cli (11 tests)**:
- UUID: valid, invalid format, nil, empty, missing hyphens
- Hex address: with/without prefix, uppercase, all zeros, all ff, too short, longer than 20 bytes, invalid chars, only prefix, empty

**kms host - db (17 tests)**:
- Wallet CRUD: insert/get, not found, exists check, update derived, delete cascades address, list all, update status with error, update passkey, update sign count
- Address index: upsert and lookup
- Challenges: store/consume, not found, cleanup expired
- API keys: generate/validate, list/revoke
- Concurrent access: 10 threads parallel read/write

**kms host - webauthn (13 tests)**:
- Base64url: roundtrip
- Random challenge: 32 bytes length
- Registration options: structure validation
- Authentication options: structure validation
- Registration: bad challenge rejection
- **P-256 ECDSA verification (5 tests)**:
  - Valid signature: real P-256 keypair → PASS
  - Tampered signature: flipped bit → correctly rejected
  - Wrong key: different keypair → correctly rejected
  - Tampered auth_data: modified authenticator data → correctly rejected
  - Invalid pubkey hex: garbage input → correctly rejected

## API Tests

*Requires DK2 board. Run with:*
```bash
cd kms/test && ./run-api-tests.sh 192.168.7.2:3000
```

Test coverage:
- GET /health
- GET /QueueStatus
- POST /CreateKey (real P-256 passkey)
- GET /KeyStatus polling (background derivation)
- POST /ListKeys
- POST /DescribeKey
- POST /GetPublicKey
- POST /DeriveAddress (with real passkey assertion)
- POST /SignHash (with real passkey assertion)
- POST /Sign message (with real passkey assertion)
- POST /Sign transaction (with real passkey assertion)
- Negative: SignHash with bad signature (CA pre-verify rejects)
- Negative: DescribeKey with non-existent key
- CLI remove-wallet cleanup

## Performance Benchmark

*Requires DK2 board. Run with:*
```bash
cd kms/test && ./perf-test.sh 192.168.7.2:3000 5
```

### Real Benchmark — DK2 Direct (2026-03-03 15:35 +07, p256-m TA verify enabled)

Measured via USB Ethernet direct connection to DK2, no CDN. 5 rounds per API. Real P-256 ECDSA passkey assertions. **TA-side p256-m ECDSA verify active.**

| Operation | Avg | Min | Max | HTTP | Notes |
|-----------|-----|-----|-----|------|-------|
| GET /health | **5ms** | 3ms | 7ms | 200 | CA-only |
| GET /QueueStatus | **3ms** | 3ms | 3ms | 200 | CA-only |
| POST /ListKeys | **6ms** | 5ms | 7ms | 200 | CA-only (SQLite) |
| POST /DescribeKey | **5ms** | 4ms | 6ms | 200 | CA-only (SQLite) |
| POST /GetPublicKey | **4ms** | 4ms | 4ms | 200 | CA-only (cache) |
| POST /DeriveAddress | **1.16s** | 1.15s | 1.17s | 200 | TEE BIP32 + p256-m verify |
| **POST /SignHash** | **1.26s** | 1.25s | 1.27s | 200 | TEE secp256k1 + p256-m verify |
| **POST /Sign (message)** | **1.27s** | 1.26s | 1.28s | 200 | EIP-191 + sign + p256-m verify |
| POST /CreateKey | **3.5s** | — | — | 200 | TA create + secure storage |
| Background derivation | **~90s** | — | — | — | PBKDF2 + BIP32 (cold) |

> CA-only 操作 <10ms。TEE 签名操作 ~1.2-1.3s（含 p256-m ~320ms）。PBKDF2 ~90s（seed 缓存后跳过）。

### Via HTTPS/Cloudflare (2026-03-03 earlier)

Network latency ~180ms (Singapore edge).

| Operation | Avg | Notes |
|-----------|-----|-------|
| GET /health | 338ms | CA-only + 180ms network |
| POST /ListKeys | 220ms | CA-only + 180ms network |
| POST /SignHash | 1.03s | TEE + 180ms network |
| POST /Sign (message) | 1.11s | TEE + 180ms network |
| POST /Sign (transaction) | 1.13s | TEE + 180ms network |

### Historical Performance Comparison

| Metric | No PassKey (v0.1) | OP-TEE native P-256 | CA-only (removed) | p256-m fixed (current) |
|--------|-------------------|---------------------|--------------------|------------------------|
| SignHash (hot) | **0.83~1.12s** | **~3.0s** | **~936ms** | **~1.26s** |
| Sign (message) | ~1s | ~3.1s | ~1.07s | **~1.27s** |
| DeriveAddress | ~0.9s | ~2.9s | ~0.9s | **~1.16s** |
| P-256 verify | N/A | TA optee (~2s) | CA only (~20ms) | **CA (~20ms) + TA p256-m (~320ms)** |

> **p256-m crash 已修复 (2026-03-03)**: 编译 flags `-O1 -fPIC -fno-common -marm` 解决了 Secure World 内存布局问题。
> CA pre-verify + TA p256-m verify 双重 defense-in-depth。

## Test Infrastructure

| File | Purpose |
|------|---------|
| `kms/test/p256_helper.py` | P-256 keypair generation + assertion creation |
| `kms/test/test-fixtures/user{1,2,3}.json` | Pre-generated test users with real P-256 keys |
| `kms/test/test-fixtures/transactions.json` | EIP-155 transaction templates |
| `kms/test/run-api-tests.sh` | Full API chain test with real passkey data |
| `kms/test/perf-test.sh` | Performance benchmark (configurable rounds) |
| `kms/scripts/build.sh` | Step 1: Build TA + CA |
| `kms/scripts/deploy.sh` | Step 2: Deploy to DK2 |
| `kms/scripts/run-all-tests.sh` | Step 3: Run all tests + collect results |

## Code Review Summary

See `docs/BETA-REVIEW.md` for full report.

| Category | Count | Status |
|----------|-------|--------|
| Production unwrap()/expect() | 5 | 3 critical, 2 safe-but-non-idiomatic |
| println! in production code | 22 | Medium (systemd journal captures) |
| Hardcoded values | 4 | Medium (port, URL, paths) |
| ~~Rate limiting~~ | — | **已实现**: 60 req/min per API key |
| ~~Circuit breaker~~ | — | **已实现**: 3 failures → 30s block |
| ~~CA input validation~~ | — | **已实现**: path/hash/message/UUID |
| ~~Log rotation~~ | — | **已配置**: journald 50MB/30天 |
