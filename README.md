
  README.md

    1 # AirAccount TEE Module
    2 
    3 ## Overview
    4 
    5 AAStar uses the Apache Teaclave open-source project to build
      TEE-Account, a hardware-based wallet using TEE for the community. We
      run TEE-Account on OP-TEE on a Raspberry Pi 5. This account saves
      your private key in secure storage on OP-TEE and signs transactions
      with a verified fingerprint signature. All signatures will be
      verified by DVT and the on-chain account contract.
    6 
    7 TEE-Account is a part of our [AirAccount](
      https://aastar.io/airaccount) project.
    8 [![AirAccount](
      https://raw.githubusercontent.com/jhfnetboy/MarkDownImg/main/img/202
      505101719766.png)](
      https://raw.githubusercontent.com/jhfnetboy/MarkDownImg/main/img/202
      505101719766.png)
    9 
   10 This repository is a monorepo containing all the core components for
      the AirAccount TEE-based Web3 account system. For a detailed
      technical plan and development roadmap, please see the [Planning 
      Document](./docs/Plan.md).
   11 
   12 Our work is heavily based on the official Teaclave and OP-TEE
      projects. We use the official `incubator-teaclave-trustzone-sdk` as
      a submodule to ensure we can stay up-to-date with the latest
      developments. The `eth_wallet` example within the SDK serves as a
      foundational reference for our Trusted Application development.
   13 
   14 Reference: [
      https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/proje
      cts/web3/eth_wallet](
      https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/proje
      cts/web3/eth_wallet)
   15 
   16 ## Repository Structure
   17 
   18 ```
   19 .
   20 ├── docs/
   21 │   ├── Plan.md          # Main technical plan (English)
   22 │   └── Plan_zh.md       # Main technical plan (Chinese)
   23 ├── packages/
   24 │   ├── client-tauri/      # Tauri client application
   25 │   ├── contracts/         # Solidity smart contracts
   26 │   ├── core-logic/        # Shared, hardware-agnostic Rust logic
   27 │   ├── node-sdk/          # NPM SDK for dApp developers
   28 │   ├── ta-arm-trustzone/  # Trusted Application for ARM TrustZone
   29 │   └── ta-intel-sgx/      # Trusted Application for Intel SGX
   30 ├── third_party/
   31 │   └── incubator-teaclave-trustzone-sdk/ # Official Teaclave SDK
      (as git submodule)
   32 └── README.md            # This file
    1 
    2 ## Getting Started
    3 
    4 Please refer to the [Planning Document](./docs/Plan.md) for the full
      development roadmap and technical details. The first step is to set
      up the development environment as described in **V0.1**.
    5 
    6 ---
    7 
    8 # AirAccount TEE 模块
    9 
   10 ## 概述
   11 
   12 AAStar 使用 Apache Teaclave 开源项目来构建 
      TEE-Account，这是一个为社区打造的、基于 TEE 
      的硬件钱包。我们在树莓派5上通过 OP-TEE 运行 
      TEE-Account。该账户将您的私钥安全地存储在 OP-TEE 
      的安全存储区中，并使用经过验证的指纹签名来签署交易。所有签名都将由 
      DVT 和链上账户合约进行验证。
   13 
   14 TEE-Account 是我们 [AirAccount](https://aastar.io/airaccount) 
      项目的一部分。
   15 [![AirAccount](https://raw.githubusercontent.com/jhfnetboy
      /MarkDownImg/main/img/202505101719766.png)](https:/
      /raw.githubusercontent.com/jhfnetboy/MarkDownImg/main/img/
      202505101719766.png)
   16 
   17 本仓库是一个包含 AirAccount TEE Web3
      账户系统所有核心组件的单一代码库
      (Monorepo)。关于详细的技术规划和发展路线图，请参阅[规划文档]
      (./docs/Plan_zh.md)。
   18 
   19 我们的工作在很大程度上基于官方的 Teaclave 和 OP-TEE
      项目。我们使用官方的 `incubator-teaclave-trustzone-sdk` 作为 Git
      子模块，以确保我们能够与最新的开发进展保持同步。该 SDK 中的
      `eth_wallet` 示例是我们开发可信应用（TA）的基础参考。
   20 
   21 参考链接:
      [https://github.com/AAStarCommunity/TEE-Account/tree/aastar-dev/proj
      ects/web3/eth_wallet](https://github.com/AAStarCommunity/TEE-Account
      /tree/aastar-dev/projects/web3/eth_wallet)
   22 
   23 ## 仓库结构
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
  │   └── incubator-teaclave-trustzone-sdk/ # 官方 Teaclave SDK (作为 git
  submodule)
  └── README.md            # 本文件

   1 
   2 ## 快速开始
   3 
   4 请参阅[规划文档](./docs/Plan_zh.md
     )以获取完整的开发路线图和技术细节。第一步是按照 **V0.1**
     中的描述来搭建开发环境


一个典型的TEE应用开发模型如下：

我的客户端应用 (Client Application, CA) 是一个运行在普通世界 (Normal World) 标准操作系统（如Ubuntu、Android）上的常规程序。它可以由Rust、C++或任何其他主流语言开发，负责处理应用的通用业务逻辑，例如用户界面、网络通信或数据处理。

当我的CA需要执行安全敏感操作（例如使用私钥进行签名、解密关键数据）时，它不会自己处理。而是通过链接一个标准的TEE客户端SDK (TEE Client API) 库，调用特定的API。

这个API调用并非普通的函数调用，它会触发一次硬件层面的世界切换，从不安全的普通世界进入到由ARM TrustZone技术硬件强制隔离的安全世界 (Secure World)。

在安全世界里，运行着一个独立的、轻量级的、高安全性的可信操作系统 (TEE OS，例如OP-TEE)。我的可信应用 (Trusted Application, TA) 正是在这个TEE OS的管理下执行。TA和CA是运行在两个完全隔离的并行系统中的，彼此绝对不信任，它们之间唯一的沟通桥梁就是这个由硬件保护的、定义严格的API通道。

这个模型完美地诠释了**“硬件钱包”**的理念：CA构建交易、与外界通信，但私钥的存储和使用被严格限制在TA内部。私钥永远不会离开安全世界，极大地降低了被窃取的风险。

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
