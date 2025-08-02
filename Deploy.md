# Deployment and Development Guide

**[中文版](./Deploy_zh.md)**

This document provides a step-by-step guide for setting up the development environment, building, testing, and deploying the AirAccount TEE module. It is designed to be friendly for newcomers.

## 1. Background & Concepts

### What is OP-TEE?
OP-TEE (Open Portable Trusted Execution Environment) is an open-source implementation of a TEE designed for ARM TrustZone technology. It provides a secure world that runs in parallel with the normal operating system (like Linux). We use it to create a hardware-isolated environment to protect our private keys and signing logic.

### Why QEMU first?
QEMU is a generic and open-source machine emulator and virtualizer. The OP-TEE project provides support for running OP-TEE in a QEMU-emulated ARM environment. We start with QEMU because:
- It requires no special hardware.
- It allows for rapid development and testing.
- It provides a consistent environment for all developers.

## 2. Phase 1: Local Development Environment Setup (QEMU on macOS)

This phase corresponds to **V0.1** in our `Plan.md`.

### 2.1. Prerequisites for macOS

First, ensure you have the necessary dependencies installed. The primary package manager for macOS is [Homebrew](https://brew.sh/).

1.  **Install Xcode Command Line Tools:**
    ```bash
    xcode-select --install
    ```
2.  **Install Homebrew** (if you don't have it):
    ```bash
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    ```
3.  **Install Required Packages with Homebrew:**
    ```bash
    brew install automake coreutils curl gmp gnutls libtool libusb make wget
    ```

### 2.2. Cloning the Repositories

The OP-TEE build system requires several repositories to be cloned into the same directory. We will place them inside our `airaccount-tee` monorepo.

1.  **Clone the Build Repo:** This contains the main makefiles and scripts.
    ```bash
    git clone https://github.com/OP-TEE/build.git optee-build
    ```
2.  **Clone Other Core Repos:**
    ```bash
    git clone https://github.com/OP-TEE/optee_os.git
    git clone https://github.com/OP-TEE/optee_client.git
    git clone https://github.com/linaro-swg/optee_examples.git
    git clone https://github.com/OP-TEE/toolchains.git
    git clone https://github.com/linaro-sw-projects/linux.git
    ```

### 2.3. Build the Toolchains

This step downloads and builds the cross-compilers needed for ARM.

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) toolchains
cd ..
```

### 2.4. Build the QEMU Environment

This command builds everything needed for the QEMUv8 platform.

```bash
cd optee-build
make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
cd ..
```

### 2.5. Run the QEMU Simulator

Once the build is complete, you can run the simulator.

```bash
cd optee-build
make -f qemu_v8.mk run
```
You will see two terminals pop up: one for the Secure World (OP-TEE) and one for the Normal World (Linux).

### 2.6. Compile and Test an Example TA

To verify the environment, we will compile and run one of the provided example TAs.

1.  **Build the examples:**
    ```bash
    cd optee-build
    make -f qemu_v8.mk ta-examples
    cd ..
    ```
2.  **Run the test in QEMU:** In the Normal World (Linux) terminal inside QEMU, run:
    ```bash
    optee_example_hello_world
    ```
    If you see a successful output, your QEMU development environment is ready.

## 3. Phase 2: Hardware Deployment (Raspberry Pi)

*(This section will be filled out in detail once Phase 1 is complete. It will cover steps like preparing the SD card, building the RPi-specific OP-TEE image, deploying the TA, and testing.)*

## 4. Phase 3: On-Chain Deployment

*(This section is a placeholder for future work. It will detail the process of deploying the smart contracts to a testnet and mainnet.)*

## 5. Phase 4: Production Environment

*(This section is a placeholder for future work. It will cover best practices for running the decentralized validator nodes in a production setting, including security, monitoring, and maintenance.)*
