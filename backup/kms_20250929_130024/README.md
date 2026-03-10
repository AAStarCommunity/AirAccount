# KMS - TEE-based Key Management System

A lightweight, secure Key Management System built on TEE (Trusted Execution Environment) using OP-TEE and Rust.

## Architecture

```
kms/
├── kms-core/     # Hardware-agnostic core logic
├── kms-ta/       # Trusted Application (OP-TEE)
├── kms-host/     # Host application & CLI
└── Cargo.toml    # Workspace configuration
```

## Features

- **Secure Key Generation**: Keys are generated within TEE and never exposed
- **Hardware-Agnostic Core**: Reusable logic across different TEE implementations
- **Multi-Algorithm Support**: secp256k1, Ed25519 (planned)
- **CLI Interface**: Easy-to-use command-line tool

## Quick Start

### Prerequisites

- OP-TEE development environment (QEMU or hardware)
- Rust toolchain with appropriate targets

### Building

```bash
# Build all components
cargo build --workspace

# Build specific component
cargo build -p kms-core
cargo build -p kms-ta
cargo build -p kms-host
```

### Usage

```bash
# Generate a new key
./target/debug/kms-cli generate -k "0011223344556677889900112233445566778899001122334455667788990011" -a secp256k1

# Get public key
./target/debug/kms-cli get-pub-key -k "0011223344556677889900112233445566778899001122334455667788990011"

# Sign a message
./target/debug/kms-cli sign -k "0011223344556677889900112233445566778899001122334455667788990011" -m "deadbeef"
```

## Development Status

- ✅ Core types and interfaces
- ✅ CLI framework
- 🚧 TEE integration (pending eth_wallet code integration)
- 🚧 Cryptographic operations
- ⏳ Hardware deployment
- ⏳ Advanced features (key recovery, backup)

## Integration with eth_wallet

This KMS is designed to extend the `eth_wallet` example from Teaclave TrustZone SDK. The actual TEE integration code will be added after user confirmation.

---

*Created: 2025-09-27*