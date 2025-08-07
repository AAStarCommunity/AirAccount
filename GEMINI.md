# GEMINI.md: Your AI Assistant's Guide to AirAccount

This document provides a comprehensive guide for an AI assistant to understand, navigate, and contribute to the AirAccount TEE Module project.

## Project Overview

**AirAccount** is a decentralized, cross-platform Web3 account system that leverages Trusted Execution Environments (TEE) to provide hardware-enforced security. The goal is to create an open-source, privacy-preserving account system where users control their keys and data through a combination of biometrics and hardware security, abstracting away the complexities of private key management and gas payments.

The project is a monorepo containing all the core components for the AirAccount TEE-based Web3 account system. The core technologies used are:

*   **TEE:** OP-TEE on Raspberry Pi 5, Apache Teaclave
*   **Client-side Framework:** Tauri (Rust core with a web-based frontend)
*   **Blockchain:** EVM-compatible chains (Ethereum, Polygon, etc.)
*   **Smart Contracts:** Solidity
*   **Decentralized Networking:** `rust-libp2p`

## Building and Running

The project is developed in phases. The initial phase focuses on setting up a local development environment using QEMU on macOS.

### Phase 1: Local Development Environment Setup (QEMU on macOS)

**1. Prerequisites:**

*   Xcode Command Line Tools: `xcode-select --install`
*   Homebrew: `/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`
*   Required Packages: `brew install automake coreutils curl gmp gnutls libtool libusb make wget`

**2. Clone Repositories:**

The following repositories need to be cloned into the same directory:

```bash
git clone https://github.com/OP-TEE/build.git optee-build
git clone https://github.com/OP-TEE/optee_os.git
git clone https://github.com/OP-TEE/optee_client.git
git clone https://github.com/linaro-swg/optee_examples.git
git clone https://github.com/OP-TEE/toolchains.git
git clone https://github.com/linaro-swg/linux.git
```

**3. Build Toolchains:**

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) toolchains
cd ..
```

**4. Build QEMU Environment:**

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
cd ..
```

**5. Run QEMU Simulator:**

```bash
cd optee-build
make -f qemu_v8.mk run
```

**6. Compile and Test an Example TA:**

```bash
cd optee-build
make -f qemu_v8.mk ta-examples
cd ..
```

In the Normal World (Linux) terminal inside QEMU, run:

```bash
optee_example_hello_world
```

## Development Conventions

*   The project follows a phased development roadmap, starting with a simulated environment (QEMU) and progressing to hardware integration (Raspberry Pi), a centralized server MVP, and finally a decentralized network.
*   Code is structured to maximize reusability between different TEE technologies (Intel SGX and ARM TrustZone) by using a three-layer architecture:
    1.  **Core Logic Layer (90% Reusable):** A hardware-independent Rust crate.
    2.  **TEE Adapter Layer (Platform-Specific):** A thin wrapper for each TEE technology.
    3.  **TA Entry Point (Platform-Specific):** The final binary for each platform.
*   The project uses a dual-signature trust model, requiring both a client-side and a server-side signature for critical operations.
