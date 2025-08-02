# 部署与开发指南

**[English Version](./Deploy.md)**

本文档为设置开发环境、构建、测试和部署AirAccount TEE模块提供了分步指南，旨在对新手友好。

## 1. 背景与概念

### 什么是OP-TEE？
OP-TEE（Open Portable Trusted Execution Environment）是为ARM TrustZone技术设计的一个开源TEE实现。它提供了一个与普通操作系统（如Linux）并行运行的安全世界，我们用它来创建一个硬件隔离的环境，以保护我们的私钥和签名逻辑。

### 为什么首先使用QEMU？
QEMU是一个通用的开源机器模拟器和虚拟化器。OP-TEE项目支持在QEMU模拟的ARM环境中运行。我们从QEMU开始，因为：
- 它不需要特殊硬件。
- 它允许快速开发和测试。
- 它为所有开发者提供了一致的环境。

## 2. 阶段一：本地开发环境设置 (macOS上的QEMU)

此阶段对应于我们`Plan.md`中的**V0.1**。

### 2.1. macOS环境依赖

首先，请确保您在系统上安装了必要的依赖项。macOS的主要包管理器是[Homebrew](https://brew.sh/)。

1.  **安装Xcode命令行工具:**
    ```bash
    xcode-select --install
    ```
2.  **安装Homebrew** (如果您还没有安装):
    ```bash
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    ```
3.  **使用Homebrew安装所需软件包:**
    ```bash
    brew install automake coreutils curl gmp gnutls libtool libusb make wget
    ```

### 2.2. 克隆代码仓库

OP-TEE构建系统需要将多个仓库克隆到同一个目录中。我们会将它们放置在我们的`airaccount-tee` monorepo内部。

1.  **克隆构建仓库:** 包含主要的makefile和脚本。
    ```bash
    git clone https://github.com/OP-TEE/build.git optee-build
    ```
2.  **克隆其他核心仓库:**
    ```bash
    git clone https://github.com/OP-TEE/optee_os.git
    git clone https://github.com/OP-TEE/optee_client.git
    git clone https://github.com/linaro-swg/optee_examples.git
    git clone https://github.com/OP-TEE/toolchains.git
    # 这个仓库是Linux内核
    git clone https://github.com/linaro-sw-projects/linux.git
    ```

### 2.3. 构建工具链

此步骤将下载并构建ARM所需的交叉编译器。

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) toolchains
cd ..
```

### 2.4. 构建QEMU环境

此命令将构建QEMUv8平台所需的所有内容。

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
cd ..
```

### 2.5. 运行QEMU模拟器

构建完成后，您可以运行模拟器。

```bash
cd optee-build
make -f qemu_v8.mk run
```
您将看到两个终端弹出：一个用于安全世界（OP-TEE），另一个用于普通世界（Linux）。

### 2.6. 编译并测试一个示例TA

为了验证环境，我们将编译并运行一个提供的示例TA。

1.  **构建示例:**
    ```bash
    cd optee-build
    make -f qemu_v8.mk ta-examples
    cd ..
    ```
2.  **在QEMU中运行测试:** 在QEMU的普通世界（Linux）终端中，运行：
    ```bash
    optee_example_hello_world
    ```
    如果您看到成功输出，则表示您的QEMU开发环境已准备就绪。

## 3. 阶段二：硬件部署 (树莓派)

*(本节将在阶段一完成后详细填写。它将涵盖准备SD卡、构建树莓派特定的OP-TEE镜像、部署TA和测试等步骤。)*

## 4. 阶段三：链上部署

*(本节为未来工作占位。它将详细说明将智能合约部署到测试网和主网的过程。)*

## 5. 阶段四：生产环境

*(本节为未来工作占位。它将涵盖在生产环境中运行去中心化验证器节点的最佳实践，包括安全、监控和维护。)*
