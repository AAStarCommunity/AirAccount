# KMS (Key Management System) ARM TEE Deployment Guide

*Created: 2025-09-27 17:38*

## Important Technical Clarification

During development, we used `mock_tee` as a testing strategy, but this was **NOT** due to limitations of QEMU OP-TEE.

**QEMU OP-TEE CAN provide a real TEE environment** - we chose mock_tee for development speed and iteration, not because QEMU lacks TEE capabilities.

## TEE Environment Comparison

| Environment | Security Level | Development Speed | Use Case |
|-------------|---------------|-------------------|----------|
| Mock TEE | Low (testing only) | Very High | Rapid prototyping, algorithm validation |
| QEMU OP-TEE | Medium (real TEE, virtualized) | Medium | Development, integration testing |
| Physical Hardware | High (production) | Low | Production deployment |

## Development Strategy

### Phase 1: Mock TEE Development (Current)
- **Purpose**: Rapid prototyping and algorithm validation
- **Benefits**: Fast iteration, easy debugging, no hardware dependencies
- **Implementation**: `mock_tee` module with system random and in-memory storage
- **Code location**: `kms/kms-ta-test/src/mock_tee.rs`

### Phase 2: QEMU OP-TEE Integration
- **Purpose**: Real TEE testing in virtualized environment
- **Benefits**: True TEE isolation, hardware crypto, secure storage
- **Requirements**: Switch from `mock_tee::Random` to `optee_utee::Random`
- **Build target**: Replace test build with actual TA build

### Phase 3: Physical Hardware Deployment
- **Purpose**: Production deployment on Raspberry Pi 5
- **Benefits**: Maximum security, real hardware crypto
- **Requirements**: OP-TEE enabled kernel, secure boot

## Switching from Mock to Real OP-TEE

To switch from our current mock environment to real QEMU OP-TEE:

1. **Update imports in wallet.rs**:
   ```rust
   // Change from:
   use crate::mock_tee::Random;

   // To:
   use optee_utee::Random;
   ```

2. **Update Cargo.toml dependencies**:
   ```toml
   # Add OP-TEE dependencies
   optee-utee = "0.6.0"
   optee-utee-macros = "0.6.0"
   ```

3. **Build as Trusted Application**:
   ```bash
   # Instead of standard build, use TA build
   cd kms/kms-ta
   make
   ```

## Current Status

- ✅ Core cryptographic algorithms implemented and tested
- ✅ AWS KMS-compatible API service deployed
- ✅ Public API accessible via Cloudflare Tunnel
- 🔄 Using mock_tee for development speed
- ⏳ Ready for QEMU OP-TEE integration when needed

The eth_wallet core code remains **completely unchanged** - only the TEE environment adapter layer differs between mock and real implementations.