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