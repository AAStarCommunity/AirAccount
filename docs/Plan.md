#### **V0.1: Foundational R&D and Prototyping (QEMU)**

*   **Goal:** Validate the core TEE logic in a simulated environment.
*   **Key Tasks:**
    *   **1. Setup OP-TEE in QEMU:**
        *   [ ] 1.1. Clone all necessary OP-TEE repositories (`build`, `optee_os`, `optee_client`, etc.).
        *   [ ] 1.2. Build the cross-compilation toolchains.
        *   [ ] 1.3. Build the complete QEMU environment for the ARMv8 platform.
        *   [ ] 1.4. Run the QEMU simulator and verify the setup with an example TA.
    *   **2. Develop a minimal Key Management TA in Rust:**
        *   [ ] 2.1. Fork the `eth_wallet` example from the Teaclave SDK.
        *   [ ] 2.2. Implement basic key generation functionality (e.g., secp256k1).
        *   [ ] 2.3. Implement a signing function for a given message hash.
        *   [ ] 2.4. Implement a function to return the public key.
    *   **3. Develop a CLI client to test the TA:**
        *   [ ] 3.1. Create a new Rust project for the CLI client.
        *   [ ] 3.2. Use the `optee_client` APIs to connect to the TA.
        *   [ ] 3.3. Implement CLI commands to:
            *   [ ] a. Generate a new key pair.
            *   [ ] b. Retrieve the public key.
            *   [ ] c. Sign a test message.
*   **Outcome:** A working PoC demonstrating key generation and signing within a simulated TEE, with a command-line interface for interaction.

#### **V0.1: 基础研发与原型验证 (QEMU)**

*   **目标:** 在模拟环境中验证核心TEE逻辑。
*   **关键任务:**
    *   **1. 使用 Git Submodule 搭建QEMU中的OP-TEE环境:**
        *   [ ] 1.1. 将所有必需的 OP-TEE 仓库作为 Git Submodule 添加到 `third_party/` 目录中。
        *   [ ] 1.2. 初始化并递归更新子模块 (`git submodule update --init --recursive`)。
        *   [ ] 1.3. 在 `third_party/build` 目录中构建交叉编译工具链。
        *   [ ] 1.4. 为 ARMv8 平台构建完整的 QEMU 环境。
        *   [ ] 1.5. 运行 QEMU 模拟器并通过一个示例 TA 验证环境。
    *   **2. 开发一个最小化的密钥管理TA (Rust):**
        *   [ ] 2.1. 从 Teaclave SDK fork `eth_wallet` 示例。
        *   [ ] 2.2. 实现基本的密钥生成功能 (例如 secp256k1)。
        *   [ ] 2.3. 为给定的消息哈希实现签名功能。
        *   [ ] 2.4. 实现一个返回公钥的功能。
    *   **3. 开发一个CLI客户端以测试TA:**
        *   [ ] 3.1. 为 CLI 客户端创建一个新的 Rust 项目。
        *   [ ] 3.2. 使用 `optee_client` API 连接到 TA。
        *   [ ] 3.3. 实现 CLI 命令以:
            *   [ ] a. 生成新的密钥对。
            *   [ ] b. 检索公钥。
            *   [ ] c. 签署测试消息。
*   **产出:** 一个可在模拟TEE中生成密钥并签名，并带有命令行交互接口的PoC。