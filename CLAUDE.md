# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

AirAccount is a TEE-based (Trusted Execution Environment) cross-platform Web3 account system using OP-TEE on Raspberry Pi 5. This monorepo contains all core components for a hardware-based wallet that stores private keys in secure TEE storage and signs transactions with verified fingerprint signatures.

The system implements a dual-signature trust model requiring both client-side (user-controlled) and server-side (TEE-controlled) signatures for critical operations, following a progressive decentralization roadmap.

## Architecture

The codebase follows a three-layer architecture for cross-platform TEE support:

1. **Core Logic Layer (90% reusable)**: Hardware-independent Rust crate containing business logic
2. **TEE Adapter Layer**: Platform-specific wrappers for different TEE technologies (ARM TrustZone, Intel SGX)  
3. **TA Entry Point**: Platform-specific binaries for each TEE implementation

Key components (as planned):
- `packages/client-tauri/`: Tauri client application  
- `packages/contracts/`: Solidity smart contracts
- `packages/core-logic/`: Shared, hardware-agnostic Rust logic
- `packages/node-sdk/`: NPM SDK for dApp developers
- `packages/ta-arm-trustzone/`: Trusted Application for ARM TrustZone
- `packages/ta-intel-sgx/`: Trusted Application for Intel SGX
- `third_party/`: Git submodules for OP-TEE components

## Development Environment Setup

### Phase V0.1: QEMU Development Environment

The project uses Git submodules extensively for OP-TEE components. All third-party dependencies are in `third_party/` as submodules.

**Setup Commands:**
```bash
# Initialize submodules  
git submodule update --init --recursive

# Setup OP-TEE environment using the provided script
./scripts/setup_optee_env.sh

# Build toolchains
cd third_party/build && make -j$(sysctl -n hw.ncpu) toolchains

# Build QEMU environment  
cd third_party/build && make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all

# Run QEMU simulator
cd third_party/build && make -f qemu_v8.mk run

# Build and test example TAs
cd third_party/build && make -f qemu_v8.mk ta-examples
```

**Docker Alternative:**
Use `Dockerfile.optee` for containerized development environment.

### Prerequisites (macOS)
- Xcode Command Line Tools: `xcode-select --install`
- Homebrew packages: `brew install automake coreutils curl gmp gnutls libtool libusb make wget`

## Key Technical Details

- **TEE SDK**: Uses Apache Teaclave SDK for Rust-based TEE development
- **Target Hardware**: Raspberry Pi 5 with OP-TEE
- **Development Platform**: QEMU ARMv8 simulation for initial development
- **Client Framework**: Tauri (Rust core with web frontend)
- **Blockchain**: EVM-compatible chains (Ethereum, Polygon, Arbitrum)
- **Networking**: rust-libp2p for decentralized P2P networking

## Development Phases

1. **V0.1**: QEMU-based foundational R&D and prototyping
2. **V0.2**: Hardware integration on Raspberry Pi  
3. **V1.0**: Centralized server MVP with dual-signature
4. **V2.0**: Multi-node decentralized network
5. **V3.0**: Economic security with staking/slashing
6. **V4.0**: ZK-enhanced hybrid trust model

## Reference Documentation

- Main technical plan: `docs/Plan.md` (English) and Chinese version within same file
- Solution overview: `docs/Solution.md` 
- Deployment guides: `docs/Deploy.md` and `docs/Deploy_zh.md`
- AI assistant context: `GEMINI.md`

The project follows the eth_wallet example from the Teaclave TrustZone SDK as a foundational reference for Trusted Application development.