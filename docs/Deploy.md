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

## 2. Local Development Environment Setup

This section provides two methods for setting up your local development environment. The **Podman-based method is strongly recommended** as it provides a consistent, reproducible environment and avoids host-specific configuration issues.

### 2.1. Method 1: Podman-Based Setup (Recommended)

This method uses Podman to create a self-contained build environment with all necessary dependencies and repositories.

**Prerequisites:**
- [Podman](https://podman.io/getting-started/installation) installed on your system.

**Setup and Build:**

1.  **Build the Podman Image:**
    From the root of the AirAccount project, run the following command. This will build the Podman image, which includes cloning all required OP-TEE repositories and compiling the toolchain and QEMU environment. This process can take a significant amount of time.
    ```bash
    podman build -f Dockerfile.optee -t airaccount-dev .
    ```

2.  **Run the Podman Container:**
    Once the image is built, you can start an interactive container session:
    ```bash
    podman run -it --rm airaccount-dev
    ```
    This will drop you into a bash shell inside the container at `/home/optee`.

3.  **Run the QEMU Simulator:**
    Inside the container, the entire OP-TEE environment is already built. To run the QEMU simulator, execute:
    ```bash
    cd optee-build
    make -f qemu_v8.mk run
    ```
    Two new xterm windows will pop up: one for the Secure World (OP-TEE) and one for the Normal World (Linux).

4.  **Verify the Environment:**
    In the **Normal World (Linux)** xterm window, run the pre-built "hello world" example to confirm everything is working:
    ```bash
    optee_example_hello_world
    ```
    A successful output means your development environment is ready.

### 2.2. Method 2: Manual Setup on macOS (Advanced)

This method is for developers who prefer to set up the environment directly on their macOS host. It is more prone to environment-specific issues.

**Prerequisites for macOS:**

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

**Setup and Build:**

1.  **Clone Repositories:**
    Run the setup script to clone all the necessary OP-TEE repositories into the `third_party/` directory.
    ```bash
    bash ./setup_optee_env.sh
    ```

2.  **Build the Toolchains and Environment:**
    The `setup_optee_env.sh` script also handles building the toolchains and the QEMU environment. If you need to run the steps manually, they are:
    ```bash
    # Build toolchains
    cd third_party/build
    make -j$(sysctl -n hw.ncpu) toolchains
    cd ../..

    # Build QEMU environment
    cd third_party/build
    make -j$(sysctl -n hw.ncpu) -f qemu_v8.mk all
    cd ../..
    ```

3.  **Run and Verify:**
    Follow steps 2.5 and 2.6 from the `GEMINI.md` guide to run the QEMU simulator and test the example TA.

## 3. Phase 2: Hardware Deployment (Raspberry Pi)

*(This section will be filled out in detail once Phase 1 is complete. It will cover steps like preparing the SD card, building the RPi-specific OP-TEE image, deploying the TA, and testing.)*

## 4. Phase 3: On-Chain Deployment

*(This section is a placeholder for future work. It will detail the process of deploying the smart contracts to a testnet and mainnet.)*

## 5. Phase 4: Production Environment

*(This section is a placeholder for future work. It will cover best practices for running the decentralized validator nodes in a production setting, including security, monitoring, and maintenance.)*
