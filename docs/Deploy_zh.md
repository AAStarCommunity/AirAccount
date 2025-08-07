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

## 2. 本地开发环境设置

本节提供两种设置本地开发环境的方法。**强烈推荐使用基于Podman的方法**，因为它提供了一致、可复现的环境，并避免了特定于主机的配置问题。

### 2.1. 方法一：基于Podman的设置 (推荐)

此方法使用Podman创建一个包含所有必要依赖项和代码仓库的自包含构建环境。

**先决条件:**
- 系统中已安装 [Podman](https://podman.io/getting-started/installation)。

**设置与构建:**

1.  **构建Podman镜像:**
    在AirAccount项目的根目录下，运行以下命令。这将构建Podman镜像，其中包括克隆所有必需的OP-TEE代码仓库、编译工具链和QEMU环境。此过程可能需要相当长的时间。
    ```bash
    podman build -f Dockerfile.optee -t airaccount-dev .
    ```

2.  **运行Podman容器:**
    镜像构建完成后，您可以启动一个交互式容器会话：
    ```bash
    podman run -it --rm airaccount-dev
    ```
    这会让你进入容器内位于`/home/optee`的bash shell。

3.  **运行QEMU模拟器:**
    在容器内部，整个OP-TEE环境已经构建完毕。要运行QEMU模拟器，请执行：
    ```bash
    cd optee-build
    make -f qemu_v8.mk run
    ```
    将会弹出两个新的xterm窗口：一个用于安全世界 (OP-TEE)，另一个用于普通世界 (Linux)。

4.  **验证环境:**
    在**普通世界 (Linux)** 的xterm窗口中，运行预构建的“hello world”示例以确认一切正常：
    ```bash
    optee_example_hello_world
    ```
    成功的输出意味着您的开发环境已准备就绪。

### 2.2. 方法二：在macOS上手动设置 (高级)

此方法适用于希望直接在macOS主机上设置环境的开发人员。它更容易出现特定于环境的问题。

**macOS环境依赖:**

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

### 2.3. 手动设置与构建

1.  **克隆代码仓库:**
    运行设置脚本，将所有必需的OP-TEE代码仓库克隆到`third_party/`目录中。
    ```bash
    bash ./setup_optee_env.sh
    ```

2.  **构建工具链和环境:**
    `setup_optee_env.sh`脚本也会处理构建工具链和QEMU环境。如果您需要手动运行这些步骤，它们是：
    ```bash
    # 构建工具链
    cd third_party/build
    make -j$(sysctl -n hw.ncpu) toolchains
    cd ../..

    # 构建QEMU环境
    cd third_party/build
    make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
    cd ../..
    ```

3.  **运行和验证:**
    遵循`GEMINI.md`指南中的步骤2.5和2.6来运行QEMU模拟器并测试示例TA。

## 3. 阶段二：硬件部署 (树莓派)

*(本节将在阶段一完成后详细填写。它将涵盖准备SD卡、构建树莓派特定的OP-TEE镜像、部署TA和测试等步骤。)*

## 4. 阶段三：链上部署

*(本节为未来工作占位。它将详细说明将智能合约部署到测试网和主网的过程。)*

## 5. 阶段四：生产环境

*(本节为未来工作占位。它将涵盖在生产环境中运行去中心化验证器节点的最佳实践，包括安全、监控和维护。)*
