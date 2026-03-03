# KMS Test Suite

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
