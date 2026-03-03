# KMS Test Suite

Updated: 2026-03-03

## Production Environment

| Item | Value |
|------|-------|
| Endpoint | `https://kms1.aastar.io` |
| API Key | `kms_****（run `api-key list` on DK2 to get key）` |
| Board | STM32MP157F-DK2 (Cortex-A7 650MHz) |
| TA Mode | Real OP-TEE Secure World |

### Quick API Test

```bash
# Health check (no auth)
curl https://kms1.aastar.io/health

# List wallets (requires API key)
curl -X POST https://kms1.aastar.io/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-api-key: kms_****（run `api-key list` on DK2 to get key）" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}'
```

## Prerequisites

```bash
pip3 install cryptography   # for P-256 key generation
```

## Test Fixtures

Pre-generated P-256 test users in `test-fixtures/`:
- `user1.json`, `user2.json`, `user3.json` — real P-256 keypairs with sample assertions
- `transactions.json` — EIP-155 transaction templates

Regenerate:
```bash
python3 p256_helper.py gen-all
```

## Running Tests

### Unit Tests (local, no DK2 needed)

```bash
cd kms/proto && cargo test
cd kms/host && cargo test --no-default-features --lib
```

### API Tests (requires DK2)

```bash
./run-api-tests.sh [host:port]   # default: 192.168.7.2:3000
```

### Performance Tests (requires DK2)

```bash
./perf-test.sh [host:port] [rounds]   # default: 192.168.7.2:3000, 5 rounds
```

### All Tests

```bash
cd kms/scripts && ./run-all-tests.sh
```

## P-256 Helper

```bash
python3 p256_helper.py gen                    # generate keypair JSON
python3 p256_helper.py assertion <pem>        # create signed assertion
python3 p256_helper.py fixture out.json label # generate test user file
python3 p256_helper.py gen-all                # generate all fixtures
```

## CLI Tools (on DK2)

```bash
# Export private key (admin, no passkey needed)
export_key <wallet_id> [derivation_path]

# API key management
cd / && api-key generate --label <name>
cd / && api-key list
cd / && api-key revoke <key>
```

Note: `api-key` must run from `/` directory (same as kms service working directory) to use the correct SQLite DB.

## Known Issues (Beta)

- **p256-m removed from TA**: p256-m C library `.text`/`.data` segments corrupt OP-TEE Secure World memory layout even when functions aren't called, causing TEE_ERROR_TARGET_DEAD on all operations. Removed entirely; CA-side P-256 verify (Rust p256 crate) is active.
- **ExportPrivateKey API**: Suspended for beta. Use `export_key` CLI on DK2.
- **OP-TEE secure storage corrupts TLS**: `PersistentObject::create()` (write) corrupts `thread_local!` segments. `cache_put` must run before `db.put`.
