# KMS Full Test Results

Date: 2026-03-03 10:09
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

### Expected Performance (p256-m optimized)

| Operation | Expected | Notes |
|-----------|----------|-------|
| SignHash (hot) | ~960ms | p256-m verify (~100ms) + secp256k1 sign (~800ms) |
| Sign (message) | ~1.0s | |
| Sign (transaction) | ~1.7s | includes RLP encode |
| DeriveAddress | ~920ms | |
| CreateKey | ~7.2s | includes PBKDF2 |
| Background derivation | ~90s | PBKDF2 (cold) + BIP32 |
| DescribeKey | ~23ms | CA-only (SQLite) |
| health | <5ms | |

### p256-m Impact

| Component | Before (OP-TEE native) | After (p256-m) | Speedup |
|-----------|----------------------|----------------|---------|
| P-256 ECDSA verify (TA) | ~2000ms | ~100ms | 20x |
| P-256 ECDSA verify (CA) | ~20ms | ~20ms | same |
| SignHash total | ~3000ms | ~960ms | 3.1x |

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

| Category | Count | Severity |
|----------|-------|----------|
| Production unwrap()/expect() | 5 | 3 critical, 2 safe-but-non-idiomatic |
| println! in production code | 22 | Medium (should be log::*) |
| Hardcoded values | 4 | Medium (port, URL, paths) |
| Missing security features | 2 | Rate limiting, constant-time API key compare |
