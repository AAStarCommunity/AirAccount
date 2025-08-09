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

---

## 📋 重新规划: 基于eth_wallet架构的AirAccount开发计划

> **核心策略变更**: 将Apache Teaclave eth_wallet作为架构模板和核心参考，深度学习其TEE设计模式、安全机制和实现逻辑，在此基础上扩展AirAccount的高级功能。

## 📋 架构确认后的实施TODO清单

> **当前状态**: 架构设计和技术选型阶段，待确认后执行
> **核心原则**: 基于eth_wallet架构，批判性融合我们的安全增强，实现AirAccount业务模型

### Phase 1: eth_wallet学习与核心架构确立 🔴 P0 (预计3周)

#### Task 1.8.1: eth_wallet深度研究与授权机制分析 🔴 P0-Critical (5天)
- [ ] **1.8.1.1 eth_wallet完整代码分析**
  - 克隆和构建Apache Teaclave eth_wallet项目
  - 深入分析TA授权验证机制 (重点关注如何控制私钥访问)
  - 理解OP-TEE会话管理和权限控制
  - 分析BIP32/BIP39/secp256k1密码学实现

- [ ] **1.8.1.2 TEE授权机制对比设计**
  - 对比eth_wallet vs AirAccount的授权需求差异
  - 设计四层授权架构：TA访问控制→会话管理→用户认证→操作授权
  - 确定WebAuthn/Passkey集成方案 (非生物特征提取)
  - 设计用户-钱包权限绑定机制

- [ ] **1.8.1.3 架构融合策略确定**
  - 确定保留我们安全模块的具体内容
  - 确定采用eth_wallet的具体组件  
  - 设计融合后的目录结构和接口
  - 制定分阶段实施计划

#### Task 1.8.2: 业务模型与技术架构融合设计 🔴 P0 (4天)
- [ ] **1.8.2.1 用户账户生命周期设计**
  - 设计Web2用户注册→TEE钱包创建的完整流程
  - 设计用户ID与钱包ID的安全绑定机制
  - 确定多钱包管理策略 (主钱包+子钱包)
  - 设计账户恢复和迁移机制

- [ ] **1.8.2.2 WebAuthn/Passkey授权集成**
  - 设计Node.js前端→系统生物识别→TEE验证的完整流程
  - 确定Passkey签名验证在TEE中的实现方式
  - 设计会话管理和授权令牌机制
  - 制定多因素认证策略

- [ ] **1.8.2.3 数据模型和接口设计**
  - 设计用户表、钱包绑定表、权限表的数据库结构
  - 定义Proto层的命令和数据结构扩展
  - 设计RESTful API接口规范
  - 制定前后端通信协议

#### Task 1.8.3: 代码融合策略确定 🟡 P1 (5天)
- [ ] **1.8.3.1 安全模块保留决策**
  - 确认保留我们的: constant_time, memory_protection, audit系统
  - 确认采用eth_wallet的: BIP32/BIP39/secp256k1, TA架构, 存储接口
  - 设计安全模块与eth_wallet核心的集成方式
  - 制定性能基准和安全测试要求

- [ ] **1.8.3.2 实施优先级排序**
  - P0: TEE授权机制实现 (关键安全基础)
  - P1: 基础钱包功能集成 (创建、签名、地址派生)
  - P2: WebAuthn/Passkey用户认证
  - P3: 多签钱包和高级功能

**Phase 1 验收标准**: 
- ✅ 完成架构设计文档和技术选型确认
- ✅ TEE授权机制清晰定义，安全风险可控
- ✅ 前后端接口规范确定，数据模型设计完成
- ✅ 代码融合策略明确，实施计划可执行

---

### Phase 2: AirAccount核心功能扩展 🟡 P1 (预计4周)

#### Task 2.1: 多重签名钱包实现
- [ ] **2.1.1 多签架构设计** (3天)
  ```rust
  pub struct MultiSigWallet {
      threshold: u8,           // 签名阈值
      signers: Vec<PublicKey>, // 签名者公钥
      pending_txs: HashMap<TxHash, PartialSignature>, // 待签名交易
  }
  ```
- [ ] **2.1.2 分布式签名协议** (5天)
- [ ] **2.1.3 签名聚合和验证** (4天)

#### Task 2.2: 生物识别集成  
- [ ] **2.2.1 指纹识别TA模块** (4天)
- [ ] **2.2.2 安全模板存储** (3天)  
- [ ] **2.2.3 活体检测机制** (3天)

#### Task 2.3: 跨链支持扩展
- [ ] **2.3.1 多链适配器设计** (4天)
- [ ] **2.3.2 BTC/其他UTXO链支持** (5天)

**Phase 2 验收标准**: 完整的多签钱包功能，生物识别验证，多链交易签名

---

### Phase 3: 分布式网络与高级功能 🟢 P2 (预计3周)

#### Task 3.1: P2P网络层实现
- [ ] **3.1.1 libp2p集成** (5天)
- [ ] **3.1.2 节点发现和通信** (4天)  
- [ ] **3.1.3 消息加密和认证** (3天)

#### Task 3.2: 密钥分片和恢复
- [ ] **3.2.1 Shamir秘密分享** (4天)
- [ ] **3.2.2 社交恢复机制** (3天)

#### Task 3.3: 零知识证明支持
- [ ] **3.3.1 zk-SNARK集成** (5天)
- [ ] **3.3.2 隐私交易支持** (4天)

---

### Phase 4: 安全加固与生产优化 🟢 P2 (预计2周)

#### Task 4.1: 基于审查报告的安全修复
- [ ] **4.1.1 密钥派生函数(KDF)实现** (3天)
  - 集成Argon2id替代直接密钥使用
  - 增强熵源质量检查
- [ ] **4.1.2 审计系统加固** (2天)  
  - 防篡改日志链
  - 敏感信息脱敏
- [ ] **4.1.3 性能优化** (3天)
  - 批量操作优化
  - SIMD加速

#### Task 4.2: 测试与文档完善
- [ ] **4.2.1 安全渗透测试** (2天)
- [ ] **4.2.2 性能基准测试** (2天)
- [ ] **4.2.3 API文档和用户指南** (2天)

---

### 关键里程碑和验收标准

| 阶段 | 里程碑 | 验收标准 | 时间 |
|------|--------|----------|------|
| Phase 1 | eth_wallet学习完成 | PoC运行，安全风险修复 | 3周 |
| Phase 2 | 核心功能实现 | 多签+生物识别+多链 | 7周 |  
| Phase 3 | 高级功能开发 | P2P网络+密钥恢复+ZK | 10周 |
| Phase 4 | 生产就绪 | 安全审计通过+性能达标 | 12周 |

**总预计工期**: 12周 (3个月)
**核心原则**: 以eth_wallet为模板，学习其精华，修复其不足，扩展我们需要的功能

---

## V0.1 详细执行计划：基于TEE开发指南的可执行子任务

### Phase 1: 环境准备与基础设施搭建

#### Task 1.1: 系统环境配置
- [ ] **1.1.1** 确认操作系统版本 (Ubuntu 20.04/22.04 LTS 或 macOS)
- [ ] **1.1.2** 安装基础开发工具
  ```bash
  # Ubuntu
  sudo apt update && sudo apt upgrade -y
  sudo apt install -y build-essential git curl python3 python3-pip \
    uuid-dev libssl-dev libffi-dev libglib2.0-dev libpixman-1-dev \
    ninja-build pkg-config gcc-multilib qemu-system-arm qemu-user-static
  
  # macOS  
  brew install automake coreutils curl gmp gnutls libtool libusb make wget qemu
  ```
- [ ] **1.1.3** 配置Rust工具链
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source ~/.cargo/env
  rustup target add aarch64-unknown-linux-gnu
  rustup target add aarch64-unknown-optee-trustzone
  rustup target add armv7-unknown-linux-gnueabihf
  cargo install cargo-make
  ```

#### Task 1.2: Teaclave TrustZone SDK 环境搭建
- [ ] **1.2.1** 克隆SDK及子模块
  ```bash
  git clone --recursive https://github.com/apache/incubator-teaclave-trustzone-sdk.git
  cd incubator-teaclave-trustzone-sdk
  ```
- [ ] **1.2.2** 构建交叉编译工具链
  ```bash
  make toolchains
  # 预计耗时：20-60分钟
  ```
- [ ] **1.2.3** 构建QEMU TEE环境
  ```bash
  make optee-qemuv8
  # 预计耗时：1-2小时
  ```
- [ ] **1.2.4** 验证QEMU环境
  ```bash
  make run-qemuv8
  # 在Normal World终端运行: xtest -l 3
  ```

---

## Phase 1.6: 安全强化与代码质量提升 (基于专家审查报告)

### 优先级分类说明
- 🔴 **P0 - 严重**: 影响安全性的关键问题，必须立即修复
- 🟡 **P1 - 高**: 功能完整性和性能问题，短期内修复  
- 🟢 **P2 - 中**: 代码质量和维护性改进，中期规划
- 🔵 **P3 - 低**: 增强功能和优化，长期规划

---

### Task 1.6.1: 关键安全漏洞修复 🔴 P0

#### 子任务 1.6.1.1: 密钥管理系统重构
- [ ] **实现密钥派生函数(KDF)**
  ```rust
  // 新增: src/security/key_derivation.rs
  pub struct KeyDerivationManager {
      algorithm: KdfAlgorithm, // Argon2id, PBKDF2, scrypt
      salt_size: usize,
      iterations: u32,
  }
  ```
- [ ] **替换直接密钥使用**
  - 修复 `audit.rs:155` 中的直接密钥创建
  - 所有密钥操作通过KDF处理
- [ ] **实现密钥生命周期管理**
  - 密钥生成、轮换、销毁流程
  - 安全密钥存储机制
- [ ] **添加密钥强度验证**
  - 熵质量检查
  - 密钥复杂度验证

#### 子任务 1.6.1.2: 增强随机数生成安全
- [ ] **实现TEE专用熵源**
  ```rust
  // 新增: src/security/entropy.rs
  pub trait EntropySource {
      fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError>;
      fn entropy_estimate(&self) -> f64;
  }
  
  pub struct TEEEntropySource {
      hw_rng: Option<HardwareRng>,
      timing_jitter: TimingJitterCollector,
      physical_noise: NoiseCollector,
  }
  ```
- [ ] **改进栈金丝雀强度**
  - 增加金丝雀位数到128位
  - 实现多层金丝雀保护
- [ ] **添加熵质量监控**
  - 熵池状态检查
  - 随机性统计测试

#### 子任务 1.6.1.3: 审计安全加固
- [ ] **实现防篡改审计日志**
  ```rust
  // 增强: src/security/audit.rs
  pub struct TamperProofAuditLog {
      hmac_key: SecureBytes,
      sequence_number: AtomicU64,
      chain_hash: Mutex<[u8; 32]>,
  }
  ```
- [ ] **敏感信息脱敏**
  - 审计日志中的密钥信息移除
  - 实现结构化脱敏规则
- [ ] **审计日志完整性验证**
  - HMAC链式验证
  - 日志序列号检查

**预计工期**: 2周
**验收标准**: 通过安全渗透测试，无已知密码学漏洞

---

### Task 1.6.2: eth_wallet示例集成与核心功能实现 🟡 P1

#### 子任务 1.6.2.1: 研究和集成eth_wallet代码
- [ ] **分析eth_wallet架构**
  - 研究TA入口点设计模式
  - 理解OP-TEE系统调用使用
  - 学习TEE内存管理机制
- [ ] **提取可重用组件**
  - 密钥生成和存储逻辑
  - 数字签名实现
  - TEE-Normal World通信协议
- [ ] **适配到AirAccount架构**
  - 集成到现有安全框架
  - 保持模块化设计原则

#### 子任务 1.6.2.2: 数字签名功能实现  
- [ ] **ECDSA签名实现**
  ```rust
  // 新增: src/crypto/signature.rs
  pub struct SignatureManager {
      key_store: Arc<KeyStore>,
      audit_logger: Arc<AuditLogger>,
  }
  
  impl SignatureManager {
      pub fn sign_message(&self, key_id: &str, message: &[u8]) -> Result<Signature>;
      pub fn verify_signature(&self, pubkey: &PublicKey, message: &[u8], sig: &Signature) -> bool;
  }
  ```
- [ ] **支持多种签名算法**
  - secp256k1 (以太坊标准)
  - Ed25519 (现代椭圆曲线)
  - secp256r1 (NIST标准)
- [ ] **签名操作审计**
  - 签名请求完整记录
  - 频率限制和异常检测

#### 子任务 1.6.2.3: 密钥存储系统
- [ ] **安全密钥存储**
  ```rust
  // 新增: src/security/key_store.rs
  pub struct SecureKeyStore {
      storage_backend: Box<dyn StorageBackend>,
      encryption_key: SecureBytes,
  }
  ```
- [ ] **密钥备份和恢复**
  - 助记词生成 (BIP39)
  - 分片备份机制
  - 恢复流程实现

**预计工期**: 3周  
**验收标准**: 能够生成密钥、签名交易、通过兼容性测试

---

### Task 1.6.3: 性能优化和效率提升 🟡 P1

#### 子任务 1.6.3.1: 审计日志性能优化
- [ ] **批量日志处理**
  ```rust
  // 优化: src/security/audit.rs  
  pub struct BatchAuditLogger {
      batch_size: usize,
      flush_interval: Duration,
      buffer: Arc<Mutex<Vec<AuditLogEntry>>>,
  }
  ```
- [ ] **异步日志写入**
  - 非阻塞日志记录
  - 后台批量刷盘
- [ ] **日志级别过滤**
  - 运行时日志级别控制
  - 热路径日志优化

#### 子任务 1.6.3.2: 内存管理优化
- [ ] **内存池实现**
  ```rust
  // 新增: src/security/memory_pool.rs
  pub struct SecureMemoryPool {
      pools: Vec<FixedSizePool>,
      large_alloc_threshold: usize,
  }
  ```
- [ ] **减少内存清零开销**
  - 延迟清零策略
  - 批量清零操作
- [ ] **SIMD加速内存操作**
  - AVX2/NEON指令优化
  - 平台特定优化

#### 子任务 1.6.3.3: 加密操作优化
- [ ] **硬件加速支持**
  - AES-NI指令集支持
  - ARM Crypto Extensions
- [ ] **算法性能调优**
  - ChaCha20多线程优化
  - 批量加密操作

**预计工期**: 2周
**验收标准**: 关键操作性能提升50%以上

---

### Task 1.6.4: 代码质量和维护性改进 🟢 P2

#### 子任务 1.6.4.1: 错误处理系统重构
- [ ] **结构化错误类型**
  ```rust
  // 重构: src/lib.rs
  #[derive(Debug, thiserror::Error)]
  pub enum SecurityError {
      #[error("Cryptographic operation failed: {operation} - {details}")]
      CryptoError { 
          operation: String, 
          details: String,
          context: ErrorContext 
      },
      
      #[error("Memory protection violation at {address:#x}")]
      MemoryViolation { 
          address: usize, 
          operation: String,
          stack_trace: Vec<String>
      },
      
      #[error("Key management error: {kind}")]
      KeyManagementError {
          kind: KeyErrorKind,
          key_id: Option<String>
      }
  }
  ```
- [ ] **错误上下文增强**
  - 调用栈信息
  - 操作时间戳
  - 相关组件状态
- [ ] **错误恢复机制**
  - 自动重试逻辑
  - 优雅降级处理

#### 子任务 1.6.4.2: 配置系统增强
- [ ] **运行时配置验证**
  ```rust
  // 新增: src/config/validator.rs
  #[derive(serde::Deserialize, Debug, validator::Validate)]
  pub struct SecurityConfig {
      #[validate(range(min = 1000, max = 100000))]
      pub pbkdf2_iterations: u32,
      
      #[validate(length(min = 32, max = 64))]
      pub salt_size: usize,
      
      #[validate]
      pub entropy_config: EntropyConfig,
  }
  ```
- [ ] **配置热更新**
  - 非关键配置动态更新
  - 配置变更审计
- [ ] **环境适配配置**
  - 开发/测试/生产环境配置
  - 平台特定参数

#### 子任务 1.6.4.3: 测试增强和文档完善
- [ ] **模糊测试集成**
  ```rust
  // 新增: fuzz/fuzz_targets/crypto_operations.rs
  use libfuzzer_sys::fuzz_target;
  
  fuzz_target!(|data: &[u8]| {
      // 测试密码学操作的边界情况
  });
  ```
- [ ] **基准测试扩展**
  - 更全面的性能基准
  - 回归测试自动化
- [ ] **API文档生成**
  - rustdoc完整覆盖
  - 使用示例和最佳实践

**预计工期**: 1.5周
**验收标准**: 代码质量检查通过，文档覆盖率>90%

---

### Task 1.6.5: 监控和运维支持 🔵 P3

#### 子任务 1.6.5.1: 指标收集系统
- [ ] **性能指标**
  ```rust
  // 新增: src/monitoring/metrics.rs
  pub struct SecurityMetrics {
      crypto_ops_total: Counter,
      memory_alloc_histogram: Histogram,
      audit_events_total: Counter,
      error_count_by_type: CounterVec,
  }
  ```
- [ ] **健康检查接口**
- [ ] **资源使用监控**

#### 子任务 1.6.5.2: 调试和诊断工具
- [ ] **内存使用分析器**
- [ ] **性能分析工具**
- [ ] **安全状态检查器**

**预计工期**: 1周
**验收标准**: 运维工具完整，监控指标准确

---

### Phase 2: AirAccount项目结构创建

#### Task 2.1: 项目目录结构初始化
- [ ] **2.1.1** 在项目根目录创建packages结构
  ```bash
  mkdir -p packages/{shared,core-logic,ta-arm-trustzone,client-tauri}/{src,tests}
  mkdir -p third_party scripts docs
  ```
- [ ] **2.1.2** 创建根级Cargo.toml工作空间配置
  ```toml
  [workspace]
  members = [
    "packages/shared",
    "packages/core-logic", 
    "packages/ta-arm-trustzone",
    "packages/client-tauri",
  ]
  ```
- [ ] **2.1.3** 初始化各包的Cargo.toml文件
- [ ] **2.1.4** 设置项目级Makefile和构建脚本

#### Task 2.2: 共享接口定义
- [ ] **2.2.1** 在packages/shared中定义TA UUID
- [ ] **2.2.2** 定义Command枚举（CreateWallet, GetPublicKey, SignTransaction, VerifyFingerprint）
- [ ] **2.2.3** 定义请求/响应数据结构
  - SignTransactionRequest
  - SignTransactionResponse  
  - WalletInfo等
- [ ] **2.2.4** 实现序列化/反序列化支持

### Phase 3: 核心业务逻辑层开发

#### Task 3.1: 硬件无关的核心逻辑 (packages/core-logic)
- [ ] **3.1.1** 实现密码学基础组件
  ```rust
  // crypto.rs - 封装ECDSA, SHA256等算法
  pub struct CryptoProvider;
  impl CryptoProvider {
      fn generate_keypair() -> Result<(PrivateKey, PublicKey)>;
      fn sign_hash(&self, private_key: &PrivateKey, hash: &[u8]) -> Result<Signature>;
      fn verify_signature(&self, public_key: &PublicKey, hash: &[u8], sig: &Signature) -> Result<bool>;
  }
  ```
- [ ] **3.1.2** 实现钱包管理逻辑
  ```rust
  // wallet.rs - BIP32密钥派生、地址生成
  pub struct WalletManager;
  impl WalletManager {
      fn derive_key(&self, master_key: &[u8], path: &str) -> Result<PrivateKey>;
      fn generate_address(&self, public_key: &PublicKey) -> Result<Address>;
  }
  ```
- [ ] **3.1.3** 定义共享类型和错误处理
- [ ] **3.1.4** 编写单元测试（no_std兼容）

### Phase 4: 可信应用(TA)开发

#### Task 4.1: TA基础框架搭建
- [ ] **4.1.1** 创建TA入口点和生命周期函数
  ```rust
  #[ta_create], #[ta_open_session], #[ta_close_session], 
  #[ta_destroy], #[ta_invoke_command]
  ```
- [ ] **4.1.2** 配置TA属性文件 (ta.rs)
  - 设置UUID、标志位、内存大小等
- [ ] **4.1.3** 实现命令路由机制

#### Task 4.2: 安全存储实现
- [ ] **4.2.1** 实现基于OP-TEE安全存储的密钥管理
  ```rust
  struct SecureStorage {
      storage_id: StorageID,
  }
  impl SecureStorage {
      fn store_master_key(&self, key: &[u8]) -> Result<()>;
      fn retrieve_master_key(&self) -> Result<Vec<u8>>;
  }
  ```
- [ ] **4.2.2** 实现密钥生成和派生功能
- [ ] **4.2.3** 实现事务签名核心逻辑

#### Task 4.3: 指纹验证集成
- [ ] **4.3.1** 设计指纹数据验证接口
- [ ] **4.3.2** 实现双重验证机制（指纹+TEE签名）
- [ ] **4.3.3** 添加防重放攻击保护

### Phase 5: 客户端应用(CA)开发

#### Task 5.1: TEE客户端封装
- [ ] **5.1.1** 实现TEEClient结构体
  ```rust
  pub struct TEEClient {
      context: Arc<Mutex<Context>>,
  }
  impl TEEClient {
      async fn create_wallet(&self) -> optee_teec::Result<()>;
      async fn sign_transaction(&self, request: SignTransactionRequest) 
          -> optee_teec::Result<SignTransactionResponse>;
  }
  ```
- [ ] **5.1.2** 实现会话管理和错误处理
- [ ] **5.1.3** 添加连接池和重试机制

#### Task 5.2: Web服务接口开发
- [ ] **5.2.1** 使用Axum构建HTTP API服务
  ```rust
  Router::new()
      .route("/health", get(health_check))
      .route("/wallet/create", post(create_wallet_handler))
      .route("/wallet/sign", post(sign_transaction_handler))
      .route("/wallet/pubkey", get(get_pubkey_handler))
  ```
- [ ] **5.2.2** 实现JSON请求/响应处理
- [ ] **5.2.3** 添加输入验证和错误处理
- [ ] **5.2.4** 实现基础的认证机制

### Phase 6: 测试与调试

#### Task 6.1: 单元测试开发
- [ ] **6.1.1** 为core-logic编写全面的单元测试
- [ ] **6.1.2** 为TA功能编写模拟测试
- [ ] **6.1.3** 为CA接口编写集成测试
- [ ] **6.1.4** 实现测试覆盖率报告

#### Task 6.2: QEMU环境集成测试
- [ ] **6.2.1** 创建构建脚本，将TA和CA部署到QEMU
  ```makefile
  build-and-deploy:
      cargo build --target aarch64-unknown-optee-trustzone --release
      cp target/.../airaccount.ta $(QEMU_SHARED_FOLDER)/
      cp target/.../airaccount $(QEMU_SHARED_FOLDER)/
  ```
- [ ] **6.2.2** 编写端到端测试脚本
- [ ] **6.2.3** 实现自动化测试流水线

#### Task 6.3: 调试工具配置
- [ ] **6.3.1** 配置GDB调试环境
- [ ] **6.3.2** 实现详细的日志记录系统
- [ ] **6.3.3** 添加性能基准测试

### Phase 7: 安全性增强

#### Task 7.1: 故障注入测试
- [ ] **7.1.1** 实现密钥生成过程中的断电模拟
- [ ] **7.1.2** 测试内存错误和异常处理
- [ ] **7.1.3** 验证签名过程的原子性

#### Task 7.2: 侧信道攻击防护
- [ ] **7.2.1** 实现恒定时间算法
- [ ] **7.2.2** 添加随机化延迟
- [ ] **7.2.3** 进行功耗分析基线测试

### Phase 8: 文档与部署准备

#### Task 8.1: 技术文档完善
- [ ] **8.1.1** 编写API文档
- [ ] **8.1.2** 创建部署指南
- [ ] **8.1.3** 完善安全模型说明
- [ ] **8.1.4** 记录已知限制和风险

#### Task 8.2: 下一阶段准备
- [ ] **8.2.1** 整理硬件需求清单（Raspberry Pi 5相关）
- [ ] **8.2.2** 准备V0.2阶段的迁移计划
- [ ] **8.2.3** 建立持续集成管道

### 验收标准

#### 功能验收
- [ ] 在QEMU环境中成功创建钱包
- [ ] 能够生成和检索公钥
- [ ] 能够签署交易并验证签名
- [ ] HTTP API响应时间 < 100ms
- [ ] 单元测试覆盖率 > 90%

#### 安全验收  
- [ ] 私钥从不在Normal World中出现
- [ ] 通过基础侧信道测试
- [ ] 实现防重放攻击保护
- [ ] 通过故障注入测试

#### 性能验收
- [ ] 签名操作 < 50ms
- [ ] 支持并发会话 ≥ 10个
- [ ] 内存使用 < 1MB

**预估完成时间：6-8周**
- Phase 1-2: 1周（环境搭建）  
- Phase 3-4: 2-3周（核心开发）
- Phase 5: 1-2周（客户端开发）
- Phase 6-7: 1-2周（测试与安全）
- Phase 8: 1周（文档与收尾）

---

## Phase 1 专家评审后的改进计划

*基于资深TEE专家评审报告 (`docs/Phase1-Expert-Review.md`)，总体评分 6.8/10*

### Phase 1.5: 安全强化与基础设施完善 (立即执行 - 2-3周)

#### Task 1.5.1: 脚本标准化和配置管理
**优先级**: 🔴 高  
**预估时间**: 2-3天

- [ ] **1.5.1.1** 创建统一脚本库 `scripts/lib/common.sh`
  ```bash
  # 统一错误处理函数
  handle_error() { echo "❌ Error: $1" >&2; exit 1; }
  # 环境检查函数  
  check_docker() { command -v docker >/dev/null || handle_error "Docker not installed"; }
  # 配置加载函数
  load_config() { source "${1:-config/development.conf}"; }
  ```

- [ ] **1.5.1.2** 集中配置管理
  ```bash
  # config/development.conf
  export DOCKER_IMAGE="teaclave/teaclave-trustzone-emulator-nostd-optee-4.5.0-expand-memory:latest"
  export OPTEE_CLIENT_EXPORT="/opt/teaclave/optee/optee_client/export_arm64"
  export TA_DEV_KIT_DIR="/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64"
  ```

- [ ] **1.5.1.3** 重构现有脚本使用新的标准库
- [ ] **1.5.1.4** 添加配置验证和环境一致性检查

#### Task 1.5.2: 安全加固实现
**优先级**: 🔴 高  
**预估时间**: 1-2周

- [ ] **1.5.2.1** 实现常量时间密码学算法
  ```rust
  // src/security/constant_time.rs
  use subtle::{ConstantTimeEq, ConditionallySelectable};
  
  pub fn secure_compare(a: &[u8], b: &[u8]) -> bool {
      if a.len() != b.len() { return false; }
      a.ct_eq(b).into()
  }
  
  pub fn timing_safe_ecdsa_sign(
      private_key: &SecretKey, 
      message_hash: &[u8; 32]
  ) -> Result<Signature> {
      add_random_delay();
      let sig = secp256k1_sign_constant_time(private_key, message_hash)?;
      clear_sensitive_data();
      Ok(sig)
  }
  ```

- [ ] **1.5.2.2** 实现内存安全保护
  ```rust
  impl Drop for SensitiveData {
      fn drop(&mut self) {
          // 安全清零敏感内存
          self.data.iter_mut().for_each(|x| *x = 0);
      }
  }
  ```

- [ ] **1.5.2.3** 添加侧信道攻击防护
  - 实现随机延迟机制
  - 常量时间比较函数
  - 内存访问模式随机化

- [ ] **1.5.2.4** 实现安全审计日志系统

#### Task 1.5.3: 全面测试框架建立
**优先级**: 🔴 高  
**预估时间**: 1-2周

- [ ] **1.5.3.1** 单元测试框架
  ```rust
  #[cfg(test)]
  mod security_tests {
      #[test]
      fn test_constant_time_signature() {
          // 测试签名时间一致性
          let time_diff = measure_signing_time_difference();
          assert!(time_diff < Duration::from_micros(100));
      }
      
      #[test]
      fn test_memory_isolation() {
          // 测试内存隔离效果
          let sensitive = vec![0x42u8; 1024];
          process_sensitive_data(&sensitive);
          assert!(sensitive.iter().all(|&x| x == 0));
      }
  }
  ```

- [ ] **1.5.3.2** 集成测试套件
  - TA-CA通信测试
  - 端到端功能测试
  - 错误处理测试

- [ ] **1.5.3.3** 安全测试框架
  - 侧信道攻击检测
  - 故障注入测试
  - 内存泄露检测

- [ ] **1.5.3.4** 性能基准测试
  ```rust
  #[bench]
  fn bench_signature_performance(b: &mut Bencher) {
      b.iter(|| {
          let sig = sign_transaction(&test_key, &test_hash);
          assert!(sig.is_ok());
      });
  }
  ```

#### Task 1.5.4: Docker环境优化
**优先级**: 🟡 中  
**预估时间**: 2-3天

- [ ] **1.5.4.1** 多阶段Docker构建优化
  ```dockerfile
  FROM teaclave/base AS builder
  WORKDIR /workspace
  COPY . .
  RUN make clean && make all
  
  FROM teaclave/runtime AS runtime
  COPY --from=builder /workspace/out /opt/airaccount/
  ENTRYPOINT ["/opt/airaccount/run.sh"]
  ```

- [ ] **1.5.4.2** 构建缓存优化和镜像大小减少
- [ ] **1.5.4.3** 本地构建fallback选项实现
- [ ] **1.5.4.4** 容器安全配置强化

### Phase 2: CI/CD与监控系统 (3-4周)

#### Task 2.1: 持续集成流水线
**优先级**: 🟡 中  
**预估时间**: 1周

- [ ] **2.1.1** GitHub Actions CI/CD配置
  ```yaml
  # .github/workflows/tee-build-test.yml
  name: TEE Build and Test Pipeline
  on: [push, pull_request]
  jobs:
    build-and-test:
      strategy:
        matrix:
          platform: [qemu, hardware-sim]
      steps:
        - name: Checkout with submodules
          uses: actions/checkout@v4
          with:
            submodules: recursive
        - name: Build and Test
          run: |
            ./scripts/setup_ci_environment.sh
            make build-all TARGET_PLATFORM=${{ matrix.platform }}
            make test-security
            make benchmark
  ```

- [ ] **2.1.2** 自动化安全扫描集成
- [ ] **2.1.3** 构建产物自动化上传和版本管理
- [ ] **2.1.4** 多平台并行构建优化

#### Task 2.2: 监控和可观测性系统
**优先级**: 🟡 中  
**预估时间**: 1-2周

- [ ] **2.2.1** 指标收集系统实现
  ```rust
  use prometheus::{Counter, Histogram, Gauge};
  
  pub struct TeeMetrics {
      pub signature_requests: Counter,
      pub signature_duration: Histogram,
      pub active_sessions: Gauge,
      pub security_events: Counter,
  }
  
  impl TeeMetrics {
      pub fn record_signature(&self, duration: Duration) {
          self.signature_requests.inc();
          self.signature_duration.observe(duration.as_secs_f64());
      }
  }
  ```

- [ ] **2.2.2** 实时监控仪表板
- [ ] **2.2.3** 安全事件告警系统
- [ ] **2.2.4** 性能分析和瓶颈识别工具

#### Task 2.3: 硬件迁移准备
**优先级**: 🟡 中  
**预估时间**: 1周

- [ ] **2.3.1** Raspberry Pi 5 环境配置脚本
- [ ] **2.3.2** 硬件抽象层(HAL)设计实现
  ```rust
  #[async_trait]
  pub trait TeeHardware {
      async fn generate_secure_random(&self, size: usize) -> Result<Vec<u8>>;
      async fn generate_keypair(&self) -> Result<KeyPair>;
      async fn secure_sign(&self, key_id: &str, data: &[u8]) -> Result<Signature>;
      async fn generate_attestation(&self) -> Result<AttestationReport>;
  }
  
  pub struct QemuTeeHardware { /* QEMU实现 */ }
  pub struct RaspberryPiTeeHardware { /* 真实硬件实现 */ }
  ```

- [ ] **2.3.3** 跨平台兼容性测试套件
- [ ] **2.3.4** 性能对比基准建立

### Phase 2.5: 生产就绪性强化 (4-6周)

#### Task 2.5.1: 密钥生命周期管理
**优先级**: 🔴 高  
**预估时间**: 1-2周

- [ ] **2.5.1.1** 密钥轮换机制实现
- [ ] **2.5.1.2** 密钥撤销和恢复策略
- [ ] **2.5.1.3** 密钥使用审计功能
- [ ] **2.5.1.4** 密钥备份和同步机制

#### Task 2.5.2: 故障恢复和灾难恢复
**优先级**: 🟡 中  
**预估时间**: 1-2周

- [ ] **2.5.2.1** 自动故障检测和恢复机制
- [ ] **2.5.2.2** 数据备份和恢复系统
- [ ] **2.5.2.3** 多节点集群支持准备
- [ ] **2.5.2.4** 灾难恢复测试流程

#### Task 2.5.3: 负载测试和容量规划
**优先级**: 🟡 中  
**预估时间**: 1周

- [ ] **2.5.3.1** 压力测试套件开发
- [ ] **2.5.3.2** 并发性能测试
- [ ] **2.5.3.3** 资源使用优化
- [ ] **2.5.3.4** 容量规划和扩展策略

### Phase 3: 安全审计与合规准备 (6-8周)

#### Task 3.1: 安全评估准备
**优先级**: 🔴 高  
**预估时间**: 2-3周

- [ ] **3.1.1** 代码安全审计准备
  - 移除调试信息和开发工具
  - 代码混淆和保护
  - 安全编码标准检查

- [ ] **3.1.2** 渗透测试环境准备
- [ ] **3.1.3** 第三方安全评估对接
- [ ] **3.1.4** 安全文档和流程规范化

#### Task 3.2: 合规性准备
**优先级**: 🟡 中  
**预估时间**: 2-3周

- [ ] **3.2.1** Common Criteria认证准备
- [ ] **3.2.2** FIPS 140-2合规性评估
- [ ] **3.2.3** GlobalPlatform兼容性测试
- [ ] **3.2.4** 监管合规文档准备

#### Task 3.3: 生产部署自动化
**优先级**: 🟡 中  
**预估时间**: 2周

- [ ] **3.3.1** 生产环境部署脚本
- [ ] **3.3.2** 蓝绿部署和金丝雀发布
- [ ] **3.3.3** 配置管理和密钥分发
- [ ] **3.3.4** 运维手册和故障处理指南

### 验收标准更新

#### 安全验收标准
- [ ] 通过侧信道攻击防护测试
- [ ] 实现常量时间密码学算法
- [ ] 通过第三方安全审计 (≥85分)
- [ ] 密钥生命周期管理完整性
- [ ] 安全事件检测和响应 < 5分钟

#### 性能验收标准
- [ ] 签名操作延迟 < 50ms (硬件环境)
- [ ] 支持并发会话 ≥ 100个
- [ ] 内存使用 < 10MB
- [ ] CPU利用率 < 80%
- [ ] 系统可用性 ≥ 99.9%

#### 可维护性验收标准  
- [ ] 自动化测试覆盖率 ≥ 80%
- [ ] API文档完整性 ≥ 95%
- [ ] CI/CD流水线成功率 ≥ 95%
- [ ] 监控和告警覆盖率 ≥ 90%
- [ ] 故障恢复时间 < 5分钟

#### 合规验收标准
- [ ] GlobalPlatform TEE规范兼容
- [ ] 通过安全代码审计
- [ ] 满足Web3安全最佳实践
- [ ] 具备生产级运维能力
- [ ] 完成监管合规评估

### 更新后的时间估算

**Phase 1 + 改进**: 已完成 ✅  
**Phase 1.5**: 2-3周 (安全强化)  
**Phase 2**: 3-4周 (CI/CD与监控)  
**Phase 2.5**: 2-3周 (生产就绪)  
**Phase 3**: 2-3周 (安全审计)  

**总计: 15-17周 (约4个月)** - 从原型到生产级系统

### 里程碑检查点

#### 🎯 Milestone 1.5 (3周后)
- 安全加固实现完成
- 测试框架覆盖率 > 70%
- 脚本和配置标准化

#### 🎯 Milestone 2.0 (7周后)  
- CI/CD流水线运行正常
- 监控系统功能完善
- 硬件迁移准备就绪

#### 🎯 Milestone 2.5 (10周后)
- 生产级特性实现
- 性能达到目标要求
- 故障恢复机制验证

#### 🎯 Milestone 3.0 (15周后)
- 第三方安全审计通过
- 合规认证准备完成
- 生产部署能力具备

**基于专家评审的改进计划将确保AirAccount从实验性原型发展为企业级生产系统。**

---

## 🔍 测试与安全评估报告分析 (2025-08-08)

### 测试报告关键发现

基于最新的《AirAccount 项目综合测试计划与报告》分析，项目在V0.1阶段取得了显著成果：

#### ✅ 测试成果亮点
- **整体测试覆盖率**: 89% (超出目标80%)
- **安全模块测试**: 32/32 通过 (100%)
- **集成测试**: 24/24 通过 (100%)
- **安全测试**: 21/21 通过 (100%)
- **性能基准**: 8/8 达标，常时操作470ns延迟

#### 📊 测试覆盖率详细分析
- **高覆盖率模块 (>90%)**:
  - 安全模块 (95%): 常时算法、内存保护、审计日志
  - Mock 通信 (93%): 协议序列化、错误处理
- **中等覆盖率模块 (70-90%)**:
  - TA 钱包功能 (82%): 基础操作完整，错误处理部分覆盖
  - 构建系统 (76%): 成功路径完整测试
- **待改进模块 (<70%)**:
  - 高级钱包功能 (45%): 多链支持架构完成但测试不足
  - QEMU 运行时 (30%): 构建验证完成，运行时测试待实施

### 安全评估关键发现

基于《AirAccount 项目安全评估报告》的深入分析：

#### 🟢 安全优势 (评级: B+)
- **内存安全**: Rust语言特性 + 额外内存保护机制
- **常时算法**: 优秀的侧信道攻击防护实现  
- **安全架构**: 完整的纵深防御设计
- **审计日志**: 完善的操作审计和完整性验证

#### 🔴 高优先级安全风险 (P0)
1. **输入验证不足**: TEE边界缺乏严格参数验证，存在恶意CA攻击风险
2. **并发安全问题**: 全局静态变量未同步保护，存在数据竞争
3. **密码学实现**: 简化哈希算法不具备生产级密码学强度

#### ⚠️ 中等优先级安全风险 (P1)
1. **会话管理**: 缺乏会话超时和重放攻击防护
2. **权限控制**: 无基于用户身份的权限控制机制
3. **密钥派生**: 不符合BIP32标准的HD钱包实现

### 基于评估的优先级任务规划

#### 🚨 P0 - 关键安全修复 (立即执行)
优先级: 🔴 最高 | 预估时间: 2-3周

1. **输入验证强化**
   ```rust
   // 实现严格的TEE边界验证
   const MAX_COMMAND_BUFFER: usize = 4096;
   fn validate_command_input(cmd: u32, buffer: &[u8]) -> Result<(), SecurityError>
   ```

2. **并发安全修复**
   ```rust
   // 替换全局静态变量为线程安全实现
   use spin::{Mutex, Once};
   static SECURITY_MANAGER: Once<Mutex<SecurityManager>> = Once::new();
   ```

3. **密码学实现标准化**
   ```rust
   // 集成标准密码学库
   use sha3::{Sha3_256, Digest};
   use bip32::{ExtendedPrivateKey, DerivationPath};
   ```

#### 🟡 P1 - 安全强化 (4周内完成)
优先级: 🟡 高 | 预估时间: 3-4周

1. **会话管理系统**
   - 实现会话超时和nonce防重放机制
   - 添加频率限制防护暴力破解
   - 安全会话生命周期管理

2. **权限控制框架**
   - 基于用户身份的操作权限验证
   - 操作审计和权限变更日志
   - 最小权限原则实施

3. **审计日志增强**
   - 防篡改日志存储机制
   - 安全事件实时告警
   - 日志完整性验证

#### 🟢 P2 - 测试完善 (6-8周内完成)  
优先级: 🟢 中 | 预估时间: 4-5周

1. **QEMU环境完整验证**
   - 实现端到端TA运行时测试
   - TA-CA真实TEE环境通信验证
   - 完整钱包功能流程测试

2. **硬件安全集成**
   - OP-TEE硬件随机数生成器
   - 硬件安全模块接口实现
   - 硬件特定优化和验证

3. **压力和稳定性测试**
   - 并发多钱包操作压力测试
   - 长期运行稳定性验证
   - 故障恢复能力测试

#### 🔵 P3 - 生产就绪 (10-12周内完成)
优先级: 🔵 中低 | 预估时间: 3-4周

1. **Raspberry Pi 5硬件部署**
   - 真实硬件环境集成测试
   - 性能基准和优化调整
   - 硬件特定安全特性验证

2. **第三方安全审计准备**
   - 代码安全审计准备
   - 渗透测试和漏洞扫描
   - 安全认证文档准备

### 修订后的开发路线图

#### Phase 1.5 安全加固期 (3-4周)
**目标**: 修复所有P0安全风险，实施P1安全强化

**关键里程碑**:
- [ ] 完成输入验证系统重构
- [ ] 解决所有并发安全问题  
- [ ] 集成标准密码学库
- [ ] 实现会话管理框架
- [ ] 建立权限控制体系

**验收标准**:
- 安全评估等级提升至 A- (≥85分)
- 无P0级安全风险
- P1风险降低至可接受水平
- 测试覆盖率保持 >85%

#### Phase 2.0 测试完善期 (4-5周)
**目标**: 提升测试覆盖率，完成QEMU环境验证

**关键里程碑**:
- [ ] QEMU运行时测试覆盖率 >80%
- [ ] 硬件安全模块集成测试
- [ ] 压力测试和性能优化
- [ ] 自动化测试框架完善

#### Phase 3.0 生产准备期 (3-4周) 
**目标**: 完成硬件部署，准备安全审计

**关键里程碑**:
- [ ] Raspberry Pi 5成功部署
- [ ] 通过内部安全审计
- [ ] 生产级监控和运维
- [ ] 第三方审计准备完成

### 成功指标更新

#### 安全指标
- [ ] 第三方安全审计评级 ≥ A- (85分)
- [ ] 0个P0级安全风险
- [ ] P1风险缓解率 ≥ 90%
- [ ] 安全事件响应时间 < 5分钟

#### 测试指标  
- [ ] 整体测试覆盖率 ≥ 90%
- [ ] QEMU运行时测试覆盖率 ≥ 80%
- [ ] 自动化测试通过率 ≥ 98%
- [ ] 性能回归测试零失败

#### 质量指标
- [ ] 代码质量评级 A级
- [ ] 文档完整性 ≥ 95%
- [ ] 构建成功率 ≥ 99%
- [ ] 部署自动化成功率 ≥ 95%

**基于全面测试和安全评估的改进计划确保AirAccount成为企业级安全的生产系统。**