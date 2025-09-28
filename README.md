# KMS (Key Management System) on TEE

A private key management system built on Trusted Execution Environment (TEE) using the eth_wallet sample from Teaclave TrustZone SDK.

## Overview

This KMS provides secure key generation, storage, and cryptographic operations compatible with AWS KMS API patterns, running in TEE for hardware-level security.

## Architecture

- **KMS Core**: `kms/kms-core/` - Core cryptographic logic
- **KMS API**: `kms/kms-api/` - HTTP API server with AWS KMS compatibility
- **KMS Host**: `kms/kms-host/` - Host application for TEE communication
- **KMS TA Test**: `kms/kms-ta-test/` - Testing with mock TEE environment
- **Proto**: `kms/proto/` - Protocol definitions for TEE communication

## Key Features

- **Hardware Security**: Uses TEE (Trusted Execution Environment) for secure key operations
- **AWS KMS Compatible**: Implements TrentService API patterns for easy integration
- **Ethereum Support**: Full secp256k1 key generation and signing
- **BIP39/BIP32**: Mnemonic phrase generation and HD key derivation
- **Secure Storage**: Keys never leave the TEE environment

## API Endpoints

- `POST /` - AWS KMS TrentService actions (CreateKey, Sign, GetPublicKey, etc.)
- `GET /health` - Health check endpoint

## Current Status

- ✅ Core eth_wallet algorithms implemented (unchanged from original)
- ✅ AWS KMS-compatible API service
- ✅ Public deployment via Cloudflare Tunnel
- 🔄 Using mock_tee for development (ready for real OP-TEE)

## Documentation

- `docs/Changes.md` - Development changelog
- `docs/deploy-arm-kms.md` - TEE deployment guide and environment comparison
- `CLAUDE.md` - Project instructions for AI development assistant

## Quick Start

```bash
# Build and run KMS API service
cd kms/kms-api
cargo run

# Test basic functionality
curl -X POST http://localhost:3000/ \
  -H "Content-Type: application/x-amz-json-1.1" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}'
```

## Technology Stack

- **Rust**: Core implementation language
- **OP-TEE**: Trusted Execution Environment
- **secp256k1**: Ethereum-compatible elliptic curve cryptography
- **Axum**: HTTP server framework
- **UUID**: Key identification
- **BIP39**: Mnemonic phrase generation