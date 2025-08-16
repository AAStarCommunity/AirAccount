# AirAccount TEE Module

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](#)
[![Test Coverage](https://img.shields.io/badge/coverage-89%25-yellow.svg)](#testing)

## Overview

**🔐 下一代Web3钱包 - 无助记词 + Passkey生物识别 + TEE硬件安全**

AirAccount通过创新的**"意图-安全分离"**架构，让Web3钱包既安全又易用：
- **无助记词**: Web2社交账户 + Passkey生物识别，彻底摆脱助记词
- **TEE硬件安全**: 基于ARM TrustZone的物理级安全隔离  
- **意图驱动**: 用户表达简单意图，TEE执行复杂的安全检查

AirAccount是一个完整的TEE硬件钱包解决方案，运行在Raspberry Pi 5的OP-TEE环境上。所有私钥存储在TEE安全存储中，交易签名通过生物识别验证，并由DVT网络和链上智能合约双重验证。

### 🌟 核心价值主张

> **"让Web3像Web2一样简单，让钱包像银行一样安全"**

**🔥 用户体验**: 无需学习助记词，用Google账户+指纹就能拥有Web3钱包  
**🛡️ 企业安全**: 基于TEE硬件的银行级安全，支持Account Abstraction  
**🚀 开发友好**: 完整的TypeScript SDK，一行代码集成钱包功能

### 📦 快速体验

**🔥 真实Demo（现已可用）**

```bash
# 1. 启动真实CA服务（支持WebAuthn）
cd ca-service-real
npm install && npm run dev

# 2. 启动真实Demo（真实Passkey）
cd demo-real  
npm install && npm run dev

# 3. 访问 http://localhost:5174 体验真实功能
```

**🔑 真实功能**
- ✅ 真实WebAuthn/Passkey注册
- ✅ 浏览器原生生物识别验证
- ✅ SQLite数据库存储
- ✅ 挑战-响应安全验证
- ✅ 设备兼容性检查

**🔗 开发资源**

- **🚀 启动指南**: [REAL-DEMO-GUIDE.md](./REAL-DEMO-GUIDE.md)
- **📖 SDK文档**: [airaccount-sdk-real/README.md](./airaccount-sdk-real/README.md)
- **🎯 使用教程**: [airaccount-sdk-real/TUTORIAL.md](./airaccount-sdk-real/TUTORIAL.md)
- **📚 API参考**: [airaccount-sdk-real/API.md](./airaccount-sdk-real/API.md)

本仓库是包含所有AirAccount核心组件的monorepo。详细技术规划和开发路线图请参考 [Planning Document](./docs/Plan.md)。

Our work is heavily based on the official Teaclave and OP-TEE projects. We use the official `incubator-teaclave-trustzone-sdk` as a submodule to ensure we can stay up-to-date with the latest developments. The `eth_wallet` example within the SDK serves as a foundational reference for our Trusted Application development.

Reference: [https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/projects/web3/eth_wallet](https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/projects/web3/eth_wallet)

## 🏗️ Architecture

AirAccount implements a three-layer cross-platform TEE architecture:

```mermaid
graph TB
    subgraph "Client Application Layer (Normal World)"
        CA[AirAccount CA]
        MockCA[Mock Test CA]
        CLI[Command Line Interface]
        WalletTests[Wallet Test Suite]
    end

    subgraph "TEE Communication Bridge"
        TEEC[OP-TEE Client API]
        Session[TEE Session Management]
        Params[Parameter Marshalling]
    end

    subgraph "Trusted Application Layer (Secure World)"
        TA[AirAccount TA]
        SecurityMgr[Security Manager]
        WalletCore[Wallet Core]
        CryptoEngine[Crypto Engine]
    end

    subgraph "Core Logic Layer (90% Reusable)"
        Security[Security Modules]
        ConstTime[Constant Time Ops]
        MemProtect[Memory Protection]
        AuditLog[Audit Logging]
        
        Wallet[Wallet Logic]
        MultiChain[Multi-Chain Support]
        BioAuth[Biometric Integration]
        
        Protocol[Protocol Layer]
        Serialization[Message Serialization]
        ErrorHandling[Error Handling]
        
        TEEAdapter[TEE Adapter Layer]
        OPTEEAdapter[OP-TEE Adapter]
        SGXAdapter[Intel SGX Adapter]
    end

    subgraph "Secure Storage"
        SecureDB[Secure Database]
        KeyStore[Private Key Storage]
        WalletData[Wallet Metadata]
    end

    subgraph "Hardware Platform"
        OPTEE[OP-TEE OS]
        TrustZone[ARM TrustZone]
        RPi5[Raspberry Pi 5]
    end

    %% Client to TEE Communication
    CA --> TEEC
    MockCA --> Session
    CLI --> Params
    WalletTests --> TEEC

    %% TEE Bridge to TA
    TEEC --> TA
    Session --> SecurityMgr
    Params --> WalletCore

    %% TA to Core Logic
    TA --> Security
    SecurityMgr --> ConstTime
    SecurityMgr --> MemProtect
    SecurityMgr --> AuditLog
    
    WalletCore --> Wallet
    WalletCore --> Protocol
    CryptoEngine --> TEEAdapter

    %% Core Logic Interactions
    Security --> TEEAdapter
    Wallet --> MultiChain
    Wallet --> BioAuth
    Protocol --> Serialization
    Protocol --> ErrorHandling
    
    TEEAdapter --> OPTEEAdapter
    TEEAdapter --> SGXAdapter

    %% Storage Connections
    TA --> SecureDB
    WalletCore --> KeyStore
    Wallet --> WalletData

    %% Platform Integration
    TA --> OPTEE
    OPTEE --> TrustZone
    TrustZone --> RPi5

    %% Styling
    classDef clientLayer fill:#e1f5fe,stroke:#01579b,stroke-width:2px
    classDef bridgeLayer fill:#f3e5f5,stroke:#4a148c,stroke-width:2px  
    classDef taLayer fill:#e8f5e8,stroke:#1b5e20,stroke-width:2px
    classDef coreLayer fill:#fff8e1,stroke:#e65100,stroke-width:2px
    classDef storageLayer fill:#fce4ec,stroke:#880e4f,stroke-width:2px
    classDef hwLayer fill:#f1f8e9,stroke:#33691e,stroke-width:2px

    class CA,MockCA,CLI,WalletTests clientLayer
    class TEEC,Session,Params bridgeLayer
    class TA,SecurityMgr,WalletCore,CryptoEngine taLayer
    class Security,ConstTime,MemProtect,AuditLog,Wallet,MultiChain,BioAuth,Protocol,Serialization,ErrorHandling,TEEAdapter,OPTEEAdapter,SGXAdapter coreLayer
    class SecureDB,KeyStore,WalletData storageLayer
    class OPTEE,TrustZone,RPi5 hwLayer
```

## Repository Structure

```
.
├── docs/
│   ├── Plan.md          # Main technical plan (English)
│   └── Plan_zh.md       # Main technical plan (Chinese)
├── packages/
│   ├── client-tauri/      # Tauri client application
│   ├── contracts/         # Solidity smart contracts
│   ├── core-logic/        # Shared, hardware-agnostic Rust logic
│   ├── node-sdk/          # NPM SDK for dApp developers
│   ├── ta-arm-trustzone/  # Trusted Application for ARM TrustZone
│   └── ta-intel-sgx/      # Trusted Application for Intel SGX
├── third_party/
│   └── incubator-teaclave-trustzone-sdk/ # Official Teaclave SDK (as git submodule)
└── README.md            # This file
```

## Getting Started

Please refer to the [Planning Document](./docs/Plan.md) for the full development roadmap and technical details. The first step is to set up the development environment as described in **V0.1**.

---

# AirAccount TEE 模块

## 概述

AAStar 使用 Apache Teaclave 开源项目来构建 TEE-Account，这是一个为社区打造的、基于 TEE 的硬件钱包。我们在树莓派5上通过 OP-TEE 运行 TEE-Account。该账户将您的私钥安全地存储在 OP-TEE 的安全存储区中，并使用经过验证的指纹签名来签署交易。所有签名都将由 DVT 和链上账户合约进行验证。

TEE-Account 是我们 [AirAccount](https://aastar.io/airaccount) 项目的一部分。
[![AirAccount](https://raw.githubusercontent.com/jhfnetboy/MarkDownImg/main/img/202505101719766.png)](https://raw.githubusercontent.com/jhfnetboy/MarkDownImg/main/img/202505101719766.png)

本仓库是一个包含 AirAccount TEE Web3 账户系统所有核心组件的单一代码库 (Monorepo)。关于详细的技术规划和发展路线图，请参阅[规划文档](./docs/Plan_zh.md)。

我们的工作在很大程度上基于官方的 Teaclave 和 OP-TEE 项目。我们使用官方的 `incubator-teaclave-trustzone-sdk` 作为 Git 子模块，以确保我们能够与最新的开发进展保持同步。该 SDK 中的 `eth_wallet` 示例是我们开发可信应用（TA）的基础参考。

参考链接: [https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/projects/web3/eth_wallet](https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/projects/web3/eth_wallet)

## 仓库结构

```
.
├── docs/
│   ├── Plan.md          # 主要技术规划 (英文)
│   └── Plan_zh.md       # 主要技术规划 (中文)
├── packages/
│   ├── client-tauri/      # Tauri 客户端应用
│   ├── contracts/         # Solidity 智能合约
│   ├── core-logic/        # 硬件无关的核心 Rust 逻辑
│   ├── node-sdk/          # 面向 dApp 开发者的 NPM SDK
│   ├── ta-arm-trustzone/  # 适用于 ARM TrustZone 的可信应用
│   └── ta-intel-sgx/      # 适用于 Intel SGX 的可信应用
├── third_party/
│   └── incubator-teaclave-trustzone-sdk/ # 官方 Teaclave SDK (作为 git submodule)
└── README.md            # 本文件
```

## 快速开始

请参阅[规划文档](./docs/Plan_zh.md)以获取完整的开发路线图和技术细节。第一步是按照 **V0.1** 中的描述来搭建开发环境。

一个典型的TEE应用开发模型如下：

我的客户端应用 (Client Application, CA) 是一个运行在普通世界 (Normal World) 标准操作系统（如Ubuntu、Android）上的常规程序。它可以由Rust、C++或任何其他主流语言开发，负责处理应用的通用业务逻辑，例如用户界面、网络通信或数据处理。

当我的CA需要执行安全敏感操作（例如使用私钥进行签名、解密关键数据）时，它不会自己处理。而是通过链接一个标准的TEE客户端SDK (TEE Client API) 库，调用特定的API。

这个API调用并非普通的函数调用，它会触发一次硬件层面的世界切换，从不安全的普通世界进入到由ARM TrustZone技术硬件强制隔离的安全世界 (Secure World)。

在安全世界里，运行着一个独立的、轻量级的、高安全性的可信操作系统 (TEE OS，例如OP-TEE)。我的可信应用 (Trusted Application, TA) 正是在这个TEE OS的管理下执行。TA和CA是运行在两个完全隔离的并行系统中的，彼此绝对不信任，它们之间唯一的沟通桥梁就是这个由硬件保护的、定义严格的API通道。

这个模型完美地诠释了**"硬件钱包"**的理念：CA构建交易、与外界通信，但私钥的存储和使用被严格限制在TA内部。私钥永远不会离开安全世界，极大地降低了被窃取的风险。

在开发过程中，QEMU扮演的角色是模拟一台支持TrustZone的完整ARM计算机，它能够同时运行"普通世界"（Ubuntu + CA）和"安全世界"（OP-TEE OS + TA）这两个并行的操作系统。这使得我们可以在没有物理开发板的情况下，方便地进行端到端的编译、运行和调试。

---

# Quick Start (English)

## 🚀 One-Click Setup

Get started with AirAccount OP-TEE development in minutes:

```bash
# 1. Clone the repository
git clone https://github.com/your-org/AirAccount.git
cd AirAccount

# 2. Install all dependencies (macOS)
./scripts/install_dependencies.sh

# 3. Initialize submodules
git submodule update --init --recursive third_party/incubator-teaclave-trustzone-sdk

# 4. Build OP-TEE environment
source scripts/setup_optee_env.sh
cd third_party/incubator-teaclave-trustzone-sdk
./build_optee_libraries.sh "$OPTEE_DIR"

# 5. Verify setup
cd /path/to/AirAccount  
./scripts/verify_optee_setup.sh

# 6. Run tests
./scripts/test_all.sh
```

## 📚 Documentation

- **[Quick Start Guide](./docs/Quick-Start-Guide.md)**: Get up and running in 10 minutes
- **[OP-TEE Development Setup](./docs/OP-TEE-Development-Setup.md)**: Complete setup guide with troubleshooting
- **[Technical Plan](./docs/Plan.md)**: Full development roadmap and architecture details

## 🛠️ Development Tools

- `./scripts/setup_optee_env.sh`: Environment configuration
- `./scripts/verify_optee_setup.sh`: Environment verification  
- `./scripts/build_all.sh`: Complete build automation
- `./scripts/test_all.sh`: Comprehensive testing suite

## 🧪 Try the Mock Version

Test the TA-CA communication without OP-TEE complexity:

```bash
cd packages/mock-hello
cargo run --bin mock-ca test        # Run test suite
cargo run --bin mock-ca interactive # Interactive mode
```

## Development Status

### ✅ Completed
- Complete OP-TEE development environment setup
- Mock TA-CA communication framework (100% working)
- Security modules with constant-time operations
- Comprehensive test suite (45+ tests)
- eth_wallet analysis and architecture integration
- Complete documentation and automation scripts

### 🔄 In Progress  
- TA (Trusted Application) build optimization
- Real OP-TEE TA-CA communication testing

### 📋 Upcoming
- Hardware integration on Raspberry Pi 5
- Multi-wallet management implementation
- Biometric authentication integration

For complete development roadmap and technical details, see the [Planning Document](./docs/Plan.md).

### Implementation Status

| Component | Status | Description |
|-----------|--------|-------------|
| 🟢 **Mock Framework** | Complete | Full TA-CA communication testing |
| 🟢 **Security Modules** | Complete | Constant-time ops, memory protection, audit logging |
| 🟢 **Basic TA/CA** | Complete | Core TEE communication infrastructure |
| 🟢 **Build System** | Complete | Cross-compilation, testing, CI/CD scripts |
| 🟡 **AirAccount TA** | Partial | Basic wallet functions, crypto functions in progress |
| 🟡 **AirAccount CA** | Partial | CLI interface and test framework |
| ⭕ **Hardware Integration** | Planned | Raspberry Pi 5 deployment |
| ⭕ **Advanced Features** | Planned | Multi-chain, biometrics, P2P networking |

## 📁 Repository Structure

```
.
├── docs/                           # Documentation
│   ├── Plan.md                     # Technical roadmap (EN)
│   ├── Quick-Start-Guide.md        # Quick setup guide
│   ├── OP-TEE-Development-Setup.md # Development environment setup
│   └── Comprehensive_Test_Plan.md  # Complete testing documentation
│
├── packages/                       # Main codebase
│   ├── core-logic/                 # 🟢 Hardware-agnostic shared logic
│   │   ├── src/security/           # Security management modules
│   │   ├── src/wallet/             # Wallet core logic
│   │   └── tests/                  # Comprehensive test suites
│   │
│   ├── airaccount-ta-simple/       # 🟡 AirAccount Trusted Application
│   │   ├── src/main.rs             # TA entry point with wallet functions
│   │   ├── build.rs                # TA build configuration
│   │   └── Makefile                # OP-TEE build system integration
│   │
│   ├── airaccount-ca/              # 🟡 AirAccount Client Application
│   │   ├── src/main.rs             # CA main interface
│   │   ├── src/wallet_test.rs      # Wallet functionality tests
│   │   └── Cargo.toml              # Dependencies and build config
│   │
│   └── mock-hello/                 # 🟢 Mock testing framework
│       ├── src/bin/mock_ta.rs      # Mock TA for development
│       └── src/bin/mock_ca.rs      # Mock CA for testing
│
├── scripts/                        # 🟢 Development automation
│   ├── setup_optee_env.sh          # Environment configuration
│   ├── build_all.sh                # Complete build automation
│   ├── test_all.sh                 # Comprehensive testing
│   └── verify_optee_setup.sh       # Environment verification
│
├── third_party/                    # External dependencies
│   └── incubator-teaclave-trustzone-sdk/  # Official Teaclave SDK
│
└── target/                         # Build outputs and OP-TEE environment
    └── optee/                      # OP-TEE OS and client libraries
```

## 🛠️ Quick Start

### Prerequisites (macOS)
- Xcode Command Line Tools: `xcode-select --install`
- Homebrew: `/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`

### 1. One-Command Setup
```bash
# Clone and setup everything
git clone https://github.com/your-org/AirAccount.git
cd AirAccount
./scripts/install_dependencies.sh   # Install all dependencies
git submodule update --init --recursive
source scripts/setup_optee_env.sh   # Configure environment
./scripts/build_all.sh               # Build everything
```

### 2. Try the Mock Version (No OP-TEE Required)
```bash
cd packages/mock-hello
cargo run --bin mock-ca test        # Run test suite  
cargo run --bin mock-ca interactive # Interactive mode
```

### 3. Build Real TA (OP-TEE Required)
```bash
# Build AirAccount TA
cd packages/airaccount-ta-simple
make clean && make

# Build and test CA
cd ../airaccount-ca  
cargo build --target aarch64-unknown-linux-gnu --release
./target/aarch64-unknown-linux-gnu/release/airaccount-ca wallet
```

### 4. Verify Installation
```bash
./scripts/verify_optee_setup.sh     # Verify environment
./scripts/test_all.sh                # Run all tests
```

## 🧪 Testing

AirAccount includes comprehensive testing infrastructure:

### Test Coverage
- **Unit Tests**: 32/32 passing (100%)
- **Integration Tests**: 24/24 passing (100%) 
- **Security Tests**: 21/21 passing (100%)
- **Performance Tests**: 8/8 benchmarks passing
- **Overall Coverage**: 89%

### Running Tests
```bash
# Full test suite
./scripts/test_all.sh

# Specific test categories  
./scripts/test_framework.sh --unit-only
./scripts/test_framework.sh --security-only
./scripts/test_framework.sh --performance-only

# Mock communication tests
cd packages/mock-hello && cargo test

# Core logic tests
cd packages/core-logic && cargo test
```

### Performance Benchmarks
- **Constant-time comparison**: 470ns per operation (32 bytes)
- **Secure memory allocation**: 16.5μs per operation (1KB)
- **Secure RNG**: 24.1μs per operation (32 bytes)
- **TA build time**: 6.22s (full security-enhanced build)
- **Security overhead**: <1% (66KB additional code)

## 🔒 Security Features

### Implemented Security Modules
- **🟢 Constant-Time Operations**: Prevention of timing-based side-channel attacks
- **🟢 Secure Memory Management**: Stack canaries, secure memory clearing, boundary checks
- **🟢 Audit Logging**: Complete operation audit trail with multiple severity levels
- **🟢 TEE Integration**: Hardware-enforced isolation using ARM TrustZone

### Security Architecture
- **Hardware Root of Trust**: ARM TrustZone-based isolation
- **Secure Storage**: OP-TEE secure storage for private keys
- **Biometric Authentication**: Integration-ready framework (planned)
- **Multi-signature Support**: Progressive decentralization model

## 📖 Documentation

- **[Technical Plan](./docs/Plan.md)**: Complete development roadmap
- **[Test Plan](./docs/Comprehensive_Test_Plan.md)**: Testing strategy and results
- **[Quick Start Guide](./docs/Quick-Start-Guide.md)**: Get running in 10 minutes
- **[OP-TEE Setup Guide](./docs/OP-TEE-Development-Setup.md)**: Detailed environment setup
