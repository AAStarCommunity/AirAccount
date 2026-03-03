# KMS Beta Release Code Review

Date: 2026-03-03
Branch: KMS-stm32
Board: STM32MP157F-DK2 (Cortex-A7 650MHz)

## Test Coverage

| Crate | Tests | Status |
|-------|-------|--------|
| proto | 26 | All pass |
| kms (host lib) | 52 | All pass |
| **Total** | **78** | **All pass** |

### Test Breakdown
- **address_cache**: 11 tests (roundtrip, lookup, serialization)
- **cli**: 11 tests (UUID/address parsing)
- **db**: 17 tests (CRUD, cascade delete, concurrent access, challenge TTL, API keys)
- **webauthn**: 10 tests (b64url, options, registration, P-256 ECDSA verify)
- **proto**: 26 tests (command enum, I/O struct roundtrip, JSON compat)

### Not Covered (requires TEE)
- `ta_client.rs` — requires OP-TEE runtime
- `api_server.rs` handlers — requires full server + TEE
- Integration tests — covered by `kms/test/run-api-tests.sh`

## Code Quality Issues

### Critical (fix before beta)

| File | Line | Issue |
|------|------|-------|
| `ta_client.rs` | 532-534 | `Context::new().expect()`, `Uuid::parse_str().expect()`, `open_session().expect()` — TA 初始化失败会 panic |
| `ta/wallet.rs` | 141 | `cached_seed.as_ref().unwrap()` — 隐式依赖 `ensure_seed_cached()` 前置调用 |
| `ta/main.rs` | 96 | LRU eviction `.unwrap()` — 安全但不规范 |

### Medium (should fix for beta)

| File | Line | Issue |
|------|------|-------|
| `api_server.rs` | 全局 | 22 处 `println!/eprintln!` 应改为 `log::{info,warn,error}` |
| `api_server.rs` | 1580 | 端口 3000 硬编码，应配置化 |
| `api_server.rs` | 1577 | `https://kms.aastar.io` 硬编码 |
| `address_cache.rs` | 13 | `ADDRESS_MAP_PATH` 硬编码 `/root/shared/address_map.json` |

### Low (post-beta)

| File | Line | Issue |
|------|------|-------|
| `ta/main.rs` | 45 | `CACHE_CAPACITY = 200` 硬编码（可接受） |
| `ta/wallet.rs` | 85 | `MAX_ADDRESSES_PER_WALLET = 100` 硬编码 |
| `api_server.rs` | 786 | `base64::decode` deprecated warning |

## Security Checklist

- [x] CA 端 P-256 passkey 预验证（拦截无效请求）
- [x] TA 端 p256-m 独立验证（双重验证）
- [x] API Key 认证（DB 驱动 + env fallback）
- [x] WebAuthn 仪式完整验证（challenge/origin/rpIdHash/signCount）
- [x] 无明文密钥在日志中泄露
- [x] SQLite WAL mode + foreign keys
- [ ] API Key 比较应用常量时间（当前用字符串 ==）
- [ ] Rate limiting 未实现
- [ ] HTTPS 终止依赖外部 reverse proxy

## i.MX 95 Migration Assessment

### Hardware Comparison

| | STM32MP157F-DK2 (current) | i.MX 95 (target) |
|---|---|---|
| CPU | 2x Cortex-A7 @ 650MHz | 6x Cortex-A55 @ 2.0GHz |
| Architecture | ARMv7-A (32-bit) | ARMv8.2-A (64-bit) |
| Secure World | OP-TEE 3.16 (ST fork) | OP-TEE 4.4+ |
| Crypto HW | CAAM | EdgeLock Secure Enclave |
| Memory | 512MB DDR3 | 4-8GB LPDDR5 |

### Expected Performance

| Operation | DK2 (current) | i.MX 95 (est.) | Speedup |
|-----------|---------------|-----------------|---------|
| SignHash | ~960ms | ~100ms | ~10x |
| CreateKey | ~7.2s | ~0.8s | ~9x |
| DeriveAddress | ~920ms | ~100ms | ~9x |
| P-256 verify (p256-m) | ~100ms | ~10ms | ~10x |
| PBKDF2 seed derive | ~60s | ~6s | ~10x |

Basis: A55@2.0GHz vs A7@650MHz — clock ~3x, IPC ~2-3x, 64-bit ops ~1.5x.

### Migration Work Items (~10-14 days)

| Task | Effort | Notes |
|------|--------|-------|
| Target triple change | 1d | `armv7-unknown-linux-gnueabihf` → `aarch64-unknown-linux-gnu` |
| p256-m 64-bit 适配 | 1d | 库本身支持 64-bit，验证 FFI 接口 |
| OP-TEE 4.x API 差异 | 2-3d | TEE_Param 布局变化、新 crypto API |
| xargo → cargo (64-bit TA) | 1-2d | aarch64 TA 可能不需要 xargo |
| EdgeLock Secure Enclave | 2-3d | 评估是否替代 software crypto |
| BSP 集成 + boot | 2d | U-Boot、设备树、rootfs |
| 全面测试 | 2d | 全 API + 性能基准 |

### Code Changes for Portability

需要预留的抽象点：
1. **Target triple**: 构建脚本中参数化（`kms/scripts/build.sh` 已处理）
2. **p256-m.c**: 64-bit clean（`uint32_t` 数组而非 platform-dependent types）
3. **TA stack/heap size**: `build.rs` 中的 `ta_stack_size` / `ta_data_size` 需调整
4. **OP-TEE API**: Rust SDK wrapper 会变化，pin 到特定版本

## Current Performance (STM32MP157F-DK2, p256-m)

| Operation | Time | Notes |
|-----------|------|-------|
| CreateKey | ~7.2s | 含 PBKDF2 + 首次 BIP32 |
| Background derivation | ~90s | PBKDF2 + BIP32 + address |
| SignHash (hot) | ~960ms | seed cached + p256-m verify |
| Sign (message) | ~1.0s | |
| Sign (transaction) | ~1.7s | |
| DeriveAddress | ~920ms | |
| DescribeKey/ListKeys | ~23ms | CA-only, no TA |
| health | <5ms | |

## Files Modified (Recent Sprint)

| File | Changes |
|------|---------|
| `kms/ta/src/main.rs` | p256-m FFI, verify_passkey rework |
| `kms/ta/p256-m.c` + `.h` | New: minimal P-256 ECDSA (Apache-2.0) |
| `kms/ta/build.rs` | cc crate compiles p256-m.c |
| `kms/ta/Cargo.toml` | +cc, -p256 |
| `kms/host/src/api_server.rs` | CA pre-verify, WebAuthn, DB, API key |
| `kms/host/src/db.rs` | SQLite persistence layer |
| `kms/host/src/webauthn.rs` | WebAuthn ceremony + P-256 tests |
| `kms/host/Cargo.toml` | +p256, +sha2, +rusqlite, +ciborium |
| `kms/proto/src/in_out.rs` | PasskeyAssertion struct |
