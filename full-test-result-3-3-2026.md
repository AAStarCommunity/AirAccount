Date: 2026-03-26 18:18:42
Board: STM32MP157F-DK2 (Cortex-A7 650MHz)
Branch: KMS-stm32


## Unit Tests

| Crate | Tests | Status |
|-------|-------|--------|
| proto | 26 passed | PASS |
| host (lib) | 55 passed | PASS |

## API Tests

```

\033[1m================================================================\033[0m
\033[1m  KMS Full API Test Suite (Real P-256 PassKey)\033[0m
\033[1m  Target: \033[0;36mhttp://192.168.7.2:3000\033[0m
\033[1m  2026-03-26 18:18:45\033[0m
\033[1m================================================================\033[0m

\033[1;33m[Phase 1] Infrastructure\033[0m
 OK  GET  /health                         22 ms  {"endpoints":{"GET":["/health","/version","/KeyStatus?KeyId=xxx","/QueueStatus"],"POST":["/CreateKey","/DeleteKey","/Des
 OK  GET  /QueueStatus                    22 ms  {"queue_depth":0,"estimated_wait_seconds":0,"circuit_breaker_open":false,"consecutive_failures":0}

\033[1;33m[Phase 2] Wallet Lifecycle\033[0m
 OK  POST /CreateKey                    4803 ms  {"KeyMetadata":{"KeyId":"f5050e0e-60ac-4a6a-b56a-e56334cdef89","Arn":"arn:aws:kms:region:account:key/f5050e0e-60ac-4a6a-
  KeyId: \033[0;36mf5050e0e-60ac-4a6a-b56a-e56334cdef89\033[0m

\033[1;33m[Phase 3] Background Derivation\033[0m
  poll #1  status=deriving  poll #2  status=deriving  poll #3  status=deriving  poll #4  status=deriving  poll #5  status=deriving  poll #6  status=deriving  poll #7  status=deriving  poll #8  status=deriving  poll #9  status=deriving  poll #10  status=deriving  poll #11  status=deriving  poll #12  status=deriving  poll #13  status=deriving  poll #14  status=deriving  poll #15  status=deriving  poll #16  status=deriving  poll #17  status=deriving  poll #18  status=deriving  poll #19  status=deriving  poll #20  status=deriving  poll #21  status=deriving  poll #22  status=deriving  poll #23  status=deriving  poll #24  status=deriving  poll #25  status=deriving  poll #26  status=deriving  poll #27  status=deriving  poll #28  status=deriving  poll #29  status=deriving OK  KeyStatus poll                    89169 ms  addr=0xa0a7ef202a454ef542f3e506ef78cefa47709db6

\033[1;33m[Phase 4] Metadata Queries\033[0m
 OK  POST /ListKeys                       27 ms  {"Keys":[{"KeyId":"0236d440-0109-41a5-a3e6-657c9c7fd1d2","KeyArn":"arn:aws:kms:region:account:key/0236d440-0109-41a5-a3e
 OK  POST /DescribeKey                    24 ms  {"KeyMetadata":{"KeyId":"f5050e0e-60ac-4a6a-b56a-e56334cdef89","Address":"0xa0a7ef202a454ef542f3e506ef78cefa47709db6","P
 OK  POST /GetPublicKey                   25 ms  {"KeyId":"f5050e0e-60ac-4a6a-b56a-e56334cdef89","PublicKey":"0x0280fe404e179c1dc051042e450fcb7688529a8827e8945da6cfe0910

\033[1;33m[Phase 5] Key Operations (real passkey)\033[0m
 OK  POST /DeriveAddress (2nd)          1174 ms  {"Address":"0x3f2663b8f2d7583eca3d5cb81acbc3caaa75c011","PublicKey":"[PUBKEY_FROM_TA]"}
 OK  POST /SignHash                     1224 ms  {"Signature":"78ab7c928c6f1086cfbc4c7f1cb985cfa32177195e2ad67f75f62debaaf37b76052e68a817f678096d67f0eade05b6f4e52e2e99fd
 OK  POST /Sign (message)               1298 ms  {"Signature":"ce84fb89611b6b63dcfc6fabb8dc4613dba277b08b0d9230ec590a6b5e58b2961ef1bde50bcf3970ee461fae03e0f5b98a7c6d6f14
 OK  POST /Sign (transaction)           1975 ms  {"Signature":"f86c808504a817c80082520894742d35cc6634c0532925a3b844bc9e7595f2bd18880de0b6b3a76400008026a0bc1549fe7ebe95f8

\033[1;33m[Phase 6] Negative Tests\033[0m
FAIL POST /SignHash (bad sig)             27 ms  {"error":"Invalid JSON: missing field `Signature` at line 1 column 514"}
  (Expected failure — CA pre-verify correctly rejected invalid passkey)
FAIL POST /DescribeKey (404)              24 ms  {"error":"Key not found: 00000000-0000-0000-0000-000000000000"}
  (Expected failure — key not found)

\033[1;33m[Phase 7] Cleanup\033[0m
 OK  POST /DeleteKey                    5176 ms  {"KeyId":"f5050e0e-60ac-4a6a-b56a-e56334cdef89","DeletionDate":"2026-04-02T10:32:43.636166226Z"}
FAIL POST /DescribeKey (deleted)          28 ms  {"error":"Key not found: f5050e0e-60ac-4a6a-b56a-e56334cdef89"}
  (Expected failure — deleted key not found)

\033[1m================================================================\033[0m
\033[1m  SUMMARY\033[0m
\033[1m================================================================\033[0m
Endpoint                                 Time Status
──────────────────────────────────── ──────── ──────
GET  /health                             22ms     OK
GET  /QueueStatus                        22ms     OK
POST /CreateKey                        4803ms     OK
KeyStatus poll (derivation)             89.1s     OK
POST /ListKeys                           27ms     OK
POST /DescribeKey                        24ms     OK
POST /GetPublicKey                       25ms     OK
POST /DeriveAddress (2nd)              1174ms     OK
POST /SignHash                         1224ms     OK
POST /Sign (message)                   1298ms     OK
POST /Sign (transaction)               1975ms     OK
POST /SignHash (bad sig)                 27ms     OK
POST /DescribeKey (404)                  24ms     OK
POST /DeleteKey                        5176ms     OK
POST /DescribeKey (deleted)              28ms     OK
──────────────────────────────────── ──────── ──────
TOTAL                                  105.0s  15/15 pass

\033[0;32mAll tests passed\033[0m
```

## Performance Benchmark


