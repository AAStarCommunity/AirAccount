# KMS Full Test Results

> Last updated: 2026-03-03 13:47

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

### Real Benchmark — DK2 Direct (2026-03-03 15:00 +07)

Measured via USB Ethernet direct connection to DK2, no CDN. 3 rounds per API. Real P-256 ECDSA passkey assertions.

| Operation | Avg | Min | Median | Max | HTTP | Notes |
|-----------|-----|-----|--------|-----|------|-------|
| GET /health | **5ms** | 3ms | 4ms | 7ms | 200 | CA-only |
| GET /QueueStatus | **3ms** | 3ms | 3ms | 3ms | 200 | CA-only |
| POST /ListKeys | **6ms** | 5ms | 6ms | 7ms | 200 | CA-only (SQLite) |
| POST /DescribeKey | **5ms** | 4ms | 5ms | 6ms | 200 | CA-only (SQLite) |
| POST /GetPublicKey | **4ms** | 4ms | 4ms | 4ms | 200 | CA-only (cache) |
| POST /DeriveAddress | **900ms** | 891ms | 894ms | 915ms | 200 | TEE BIP32 + passkey |
| **POST /SignHash** | **936ms** | 916ms | 919ms | 974ms | 200 | TEE secp256k1 ECDSA |
| **POST /Sign (message)** | **1.066s** | 1.063s | 1.064s | 1.070s | 200 | EIP-191 hash + sign |
| **POST /Sign (transaction)** | **1.931s** | 1.920s | 1.921s | 1.952s | 200 | RLP encode + sign |
| POST /CreateKey | **2.460s** | — | — | — | 200 | TA create + secure storage |
| POST /ChangePasskey | **2.678s** | — | — | — | 200 | TA update passkey |
| POST /DeleteKey | **2.882s** | — | — | — | 200 | TA secure storage delete |
| Background derivation | **91.8s** | — | — | — | — | PBKDF2 + BIP32 (cold) |

> CA-only 操作 <10ms。TEE 签名操作 ~0.9-1.9s。首次 PBKDF2 ~90s（seed 缓存后跳过）。

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

| Metric | No PassKey (v0.1) | OP-TEE native P-256 | p256-m (removed) | CA-only P-256 (current) |
|--------|-------------------|---------------------|-------------------|-------------------------|
| SignHash (hot) | **0.83~1.12s** | **~3.0s** | **~960ms** | **~1.03s** |
| Sign (message) | ~1s | ~3.1s | ~1.0s | **~1.11s** |
| CreateKey | ~3.5s | ~6.1s | ~7.2s | **~6.5s** |
| P-256 verify location | N/A | TA (~2s) | TA (~100ms) | **CA (~20ms)** |

> **p256-m 已移除**: p256-m C 库的 .text/.data 段在 OP-TEE Secure World 中破坏内存布局，
> 导致所有 TA 操作触发 TEE_ERROR_TARGET_DEAD (0xffff3024)。P-256 验证移至 CA 端。

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
