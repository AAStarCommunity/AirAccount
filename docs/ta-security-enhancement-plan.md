# TA安全增强计划

*创建时间: 2025-09-29 | 最后更新: 2026-06-07*

---

## 2026-06-07 — MX93 实机测试报告 & 预装 TA 分析

### E2E 测试结果（两次独立运行，结果一致）

**目标**: https://kms.aastar.io（NXP FRDM-IMX93, OP-TEE 4.8, ta_mode=real）

| 分类 | 通过 | 失败 |
|------|------|------|
| api | 25 | 1 |
| consistency | 4 | 0 |
| crypto | 7 | 0 |
| security | 16 | 0 |
| **合计** | **52** | **1** |

唯一失败：`GET /stats` 路由缺失（已在 2026-06-07 同日修复，见下方 GET /stats 设计节）。

**本次修复部署**：`POST /DeleteKey` warp 路由之前要求 header `TrentService.ScheduleKeyDeletion`，
而所有客户端发送的是 `TrentService.DeleteKey`，导致 HTTP 500 无日志。
已修复为同时接受两个值，并在板子上原生编译（aarch64, 3m27s）、systemd 重启，修复生效。

### MX93 预装 TA（27 个 + 我们自己 = 28 个）

板子路径：`/lib/optee_armtz/`（OP-TEE 4.8 Yocto 镜像随附）

#### 可借鉴（7 个，与 AirAccount 直接相关）

| UUID（前8位） | TA 名称 | 对 AirAccount 的价值 |
|-------------|--------|-------------------|
| `528938ce` | **PKCS#11 TA** | 将 KMS 密钥操作暴露为标准 PKCS#11 接口，使 OpenSSL/Nginx/各类 SDK 可直接调用 TEE 私钥签名 |
| `a4c04d50` | PKCS#11 Token TA | PKCS#11 token slot 管理 |
| `11b5c4aa` | **FIDO/WebAuthn TA** | 把 WebAuthn P256 验签从 CA（host）移进 TEE，彻底消除 host 侧信任依赖 |
| `b3091a65` | **Trusted Keys TA** | Linux kernel Trusted Keys：用 TEE seal/unseal 密钥，只有特定固件状态下才能解封 |
| `5ce0c432` | **RPMB TA** | 防回滚存储。`ChangePasskey` 撤销状态存 RPMB，物理攻击无法回滚。**→ 见 [RPMB 防回滚计划](rpmb-anti-rollback-plan.md)** |
| `731e279e` | **Attestation TA** | TEE 远程证明，客户端可验证"签名真的来自合法 TEE"。**→ 见 [远程证明计划](attestation-plan.md)** |
| `fd02c9da` | Provisioning TA | 生产密钥注入，批量出厂时把根密钥安全写入每块板子 TEE |

#### 平台能力（6 个，NXP 专属）

| UUID（前8位） | TA 名称 | 说明 |
|-------------|--------|-----|
| `5c206987` | EdgeLock ELE TA | TRNG、OTP fuse、设备生命周期。我们的 CreateKey 已在用 |
| `b689f2a7` | ELE Crypto TA | NXP CAAM 硬件加速（AES/SHA/ECDSA，比纯软快 10-100×） |
| `380231ac` | IMX Crypto TA | i.MX CAAM 第二通道 |
| `a720ccbb` | SE05x TA | NXP SE050/SE051 Secure Element i2c 桥接 |
| `80a4c275` | Secure Boot TA | 安全启动固件完整性校验 |
| `ffd2bded` | Firmware Verification TA | NXP 固件签名校验 |

#### OP-TEE 测试/示例（11 个）

随 OP-TEE 4.8 发行版附带，主要用于验证 TEE 环境和 crypto 原语是否正常。
包括：Hello World、AES Test、Crypt、Storage、OTP、Secure Storage v2、Benchmark/SHA、
OPTEE Test Supp、GP TEE Internal Core API、TrEE Measurement、Secure Channel。

#### 其他

| UUID | TA 名称 |
|------|--------|
| `25497083` | SDP/DRM TA（Secure Data Path，媒体 DRM） |
| `873bcd08` | eCryptfs/IMA TA（文件系统加密密钥管理） |

### 预装 TA 深度分析——每个可借鉴 TA 的技术细节

#### 1. PKCS#11 TA（`528938ce` + `a4c04d50`）

**是什么**：GlobalPlatform 标准密钥库接口，OP-TEE 官方实现（`optee_os/ta/pkcs11/`）。
支持 RSA、AES、ECDSA、HMAC、SHA 等几十种算法，通过标准 PKCS#11 C API 调用。

**AirAccount 可以做什么**：
```
当前：外部系统必须用我们的私有 KMS HTTP API 才能调用 TEE 签名
改后：外部系统用标准 PKCS#11 调用
  → OpenSSL：PKCS11_load_key() + SSL_CTX_use_PrivateKey()
  → Nginx/Apache TLS：engine 自动调用 TEE 私钥完成 TLS 握手
  → Java PKCS11Provider、Python python-pkcs11
  → HSM 替代：把 AirAccount 作为软件 HSM 接入企业 PKI
```

**实现路径**（难度：中）：
1. Host 侧用 `libckteec.so`（NXP Yocto 已有）与 PKCS#11 TA 通信
2. 在 PKCS#11 slot 里注册 KMS 的 secp256k1 私钥（通过 OP-TEE 安全存储桥接）
3. 外部应用通过 `p11-kit` 或直接 `dlopen` 调用

**优先级**：中（v0.23.0 候选）

---

#### 2. FIDO/WebAuthn TA（`11b5c4aa`）

**是什么**：FIDO Alliance 的 UAF（User Authentication Framework）TA。
OP-TEE 侧的 FIDO authenticator 实现，可生成和存储 FIDO credential，处理 FIDO assertion 验证。

**AirAccount 当前的 WebAuthn 信任点问题**：
```
当前流程：
  Client → HTTP POST {authenticatorData, clientDataJSON, signature}
  → CA (host) → 验证 P256 ECDSA（在普通进程里！）
  → TEE → 执行 sign/change_passkey

弱点：CA 侧的 P256 验签在普通进程中执行。
攻击者若能攻破 CA 进程（RCE），可以绕过 passkey 验证直接调用 TEE。
```

**有了 FIDO TA 后**：
```
改进流程：
  Client → assertion
  → CA 直接将 assertion 转发到 FIDO TA
  → FIDO TA 在 TEE Secure World 内验证
  → 验证通过 → KMS TA 执行操作

TEE 内验证：即使 CA 被完全攻破，攻击者也无法伪造 passkey 验证
```

**实现路径**（难度：高）：
- 需要研究 FIDO TA 的 `TEE_InvokeCommand` 接口（未公开文档，需逆向或联系 NXP）
- `kms/ta/src/passkey.rs` 的验签逻辑迁移进 FIDO TA inter-TA invoke
- 这是 AirAccount 从"TEE 保护私钥"升级到"TEE 保护整个认证链"的关键步骤

**优先级**：高（v0.23.0，与 PKCS#11 并行）

---

#### 3. RPMB TA（`5ce0c432`）

详见 [docs/rpmb-anti-rollback-plan.md](rpmb-anti-rollback-plan.md) — GitHub Issue #36，目标版本 v0.21.0。

**核心价值**：eMMC 硬件单调计数器，物理回滚攻击无效。
当前 OP-TEE Secure Storage 可被物理回滚（备份 eMMC → ChangePasskey → 恢复 eMMC = 旧 passkey 复活）。

---

#### 4. Attestation TA（`731e279e`）

详见 [docs/attestation-plan.md](attestation-plan.md) — GitHub Issue #37，目标版本 v0.22.0。

**核心价值**：让用户可以密码学验证"签名来自合法 OP-TEE 环境"，而不需要信任部署方。
基于 OP-TEE 4.8 DICE（Device Identifier Composition Engine）机制。

---

#### 5. Trusted Keys TA（`b3091a65`）

**是什么**：Linux kernel 的 Trusted Keys 子系统（`Documentation/security/keys/trusted-encrypted.rst`）。
密钥由 TPM 或 TEE seal/unseal，只有在特定 Platform Configuration Register（PCR）状态下才能解封。

**AirAccount 可以做什么**：
```
场景：KMS 服务启动时，需要加载数据库密钥（SQLite 加密密钥）。
当前：密钥存在配置文件或环境变量里（明文，可被 root 读取）

改进：把 SQLite 加密密钥用 Trusted Keys seal 到 TEE + 特定固件哈希
→ 只有在 Secure Boot 验证通过 + 固件未被篡改时才能解封
→ 攻击者即使有 root 权限、即使修改了用户空间，也无法解密数据库
```

**实现路径**（难度：低，直接用 Linux keyctl API）：
```bash
# 在 MX93 上
keyctl add trusted kmsdisk "new 32 keyhandle=<TEE_handle>" @u
keyctl pipe <keyid> > /etc/kms-db.key.enc

# 启动时
keyctl add trusted kmsdisk "load $(cat /etc/kms-db.key.enc)" @u
# 解封失败 → 服务拒绝启动，数据库保持加密
```

**优先级**：中（v0.21.0 附加项，与 RPMB 一起做）

---

#### 6. Provisioning TA（`fd02c9da`）

**是什么**：设备预置 TA，工厂生产流程中把根密钥/证书安全注入每块板子。
NXP EdgeLock 2GO 云服务的板端实现。

**AirAccount 可以做什么**：
```
当前问题：新板子上线时，如何安全地分发 Cloudflare Tunnel 凭证、API key？
  - 现在通过串口 base64 传输（不够安全，需人工操作）

改进：工厂出厂时通过 Provisioning TA 把初始凭证注入 TEE
→ 设备开机自动从 ELE TA 解封凭证（无需人工操作）
→ 凭证加密存在 TEE，root 权限也无法提取
```

**实现路径**（难度：高，需要 NXP EdgeLock 2GO 云服务账号）

---

#### 7. EdgeLock ELE TA（`5c206987`）— 已在用

**是什么**：NXX EdgeLock Enclave，i.MX93 的独立安全处理器（与 Cortex-A55 分离的 Arm Cortex-M33）。
提供：TRNG（真随机数）、OTP fuse 编程、设备生命周期管理（open/closed/locked）、密钥派生。

**AirAccount 当前已用**：`CreateKey` 调用 OP-TEE TRNG（底层走 ELE）生成随机种子。

**可以进一步用**：
- 读取设备唯一 ID（`fuse`）作为钱包 namespace，防止跨设备混用
- 调用 ELE ECDSA 加速（走 CAAM，比软件快 50×）
- 设备生命周期锁定：生产后锁定为 `closed` 状态，防止 JTAG 调试器附着

---

#### 8. NXP CAAM TA（`b689f2a7` + `380231ac`）

**是什么**：CAAM = Cryptographic Acceleration and Assurance Module，i.MX93 上的专用硬件密码引擎。
支持：AES-256-GCM、SHA-512、ECDSA（secp256k1 + P256）、RSA-4096、TRNG、DES。

**AirAccount 可以做什么**：
```
当前：kms/ta/src/ 的 ECDSA 签名用纯软件实现（k256 crate）
改进：通过 CAAM TA 把签名操作卸载到硬件

性能对比（预估）：
  软件 ECDSA-secp256k1 sign：~2ms（Cortex-A55 @ 1.7GHz）
  CAAM ECDSA sign：~0.1ms（专用硬件）

高频签名场景（SuperRelay bundler）20× 性能提升
```

**实现路径**（难度：中）：
- OP-TEE 侧通过 `TEE_AllocateOperation(TEE_ALG_ECDSA_P256, ...)` 自动走 CAAM（OP-TEE 已集成）
- 需要确认 secp256k1（非标准曲线）是否有 CAAM 硬件支持，否则只有 P256 能加速

---

#### 9. SE05x TA（`a720ccbb`）

**是什么**：NXP SE050/SE051 Secure Element 的 i2c 桥接 TA。
SE05x 是独立的硬件安全芯片（类似银行卡芯片），提供：ECC、RSA、AES 存储和运算，抗物理攻击。

**如果接了 SE050 硬件**（目前 MX93 开发板上没有默认接）：
```
升级路线：
  当前：TEE（软件保护，抗软件攻击）
  升级：SE05x（独立芯片，抗物理攻击）

SE050 成本约 $3/片，能把 AirAccount 升级到银行卡级别的物理安全
适合：金融机构、托管服务、高净值用户资产管理
```

---

### GET /stats 设计（2026-06-07 修复）

**问题**：`GET /stats` 返回 500（路由缺失），`GET /` 已有 HTML dashboard，CLAUDE.md 文档写的是 `GET /stats`。

**设计决策**：
- `GET /` — 继续返回 HTML dashboard（人工查看）
- `GET /stats` — 新增，返回**机器可读 JSON**（无需 auth，和 `/health` 同级）

**服务对象**：
- **内部**（主要）：Cloudflare Workers 健康检查、Prometheus scrape、SuperPaymaster/SuperRelay 集成前检查
- **外部**（次要）：运维监控、状态页面（如 `status.aastar.io`）

**不暴露 key_id 列表**（安全）：只暴露聚合计数，防止钱包枚举攻击。

**响应结构**：
```json
{
  "service": "kms-api",
  "version": "0.19.0",
  "ta_mode": "real",
  "keys": { "total": 22, "active": 18, "with_address": 20, "with_passkey": 20 },
  "operations": {
    "total_signs": 1234, "daily_signs": 56,
    "avg_sign_ms": 45.3, "avg_derive_ms": 23.1,
    "errors": 0, "panics": 0, "webauthn": 100
  },
  "queue": { "circuit_breaker": "closed", "consecutive_failures": 0 },
  "api_keys": 3
}
```

**已实现**：`kms/host/src/api_server.rs` `handle_get_stats()` + `GET /stats` 路由，v0.19.1 发布。

---

### 两个高优先级后续工作

1. **RPMB 防回滚**（安全关键）→ 计划文档：[docs/rpmb-anti-rollback-plan.md](rpmb-anti-rollback-plan.md)，GitHub Issue #36
2. **Attestation 远程证明**（信任升级）→ 计划文档：[docs/attestation-plan.md](attestation-plan.md)，GitHub Issue #37

---

## 当前安全状况评估

### 现有安全特性
1. **TEE环境隔离**: 在ARM TrustZone中运行，与Rich OS隔离
2. **标准库使用**: eth_wallet使用std模式，支持复杂crypto操作
3. **密钥生成**: 基于BIP39助记词和HD钱包路径派生
4. **签名算法**: 支持secp256k1椭圆曲线签名

### 识别的安全风险

#### 高优先级风险
1. **std模式攻击面**: std库增加了攻击面相比no-std模式
2. **内存泄露风险**: 复杂crypto库可能导致敏感数据残留
3. **侧信道攻击**: 签名操作的时序分析风险
4. **随机数质量**: 密钥生成的熵源质量

#### 中优先级风险
1. **依赖供应链**: 第三方crypto库的安全性
2. **错误处理泄露**: 错误信息可能泄露内部状态
3. **调试信息**: 开发阶段的调试信息残留
4. **版本管理**: OP-TEE和Rust版本的安全更新

## 分阶段安全增强策略

### 阶段1: 立即改进 (1-2周)

#### 1.1 内存安全加固
```rust
// 敏感数据清零
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(ZeroizeOnDrop)]
struct WalletPrivateData {
    mnemonic: String,
    private_keys: Vec<[u8; 32]>,
}

impl Drop for WalletPrivateData {
    fn drop(&mut self) {
        self.mnemonic.zeroize();
        for key in &mut self.private_keys {
            key.zeroize();
        }
    }
}
```

#### 1.2 错误处理安全化
```rust
// 统一错误处理，避免信息泄露
#[derive(Debug)]
pub enum TAError {
    InternalError,          // 不暴露具体错误
    InvalidParameter,       // 通用参数错误
    KeyNotFound,           // 密钥不存在
    SignatureFailure,      // 签名失败
}

// 日志过滤敏感信息
fn log_safe(operation: &str, success: bool) {
    // 只记录操作类型和成功/失败状态
    // 不记录任何密钥材料或具体错误细节
}
```

#### 1.3 输入验证强化
```rust
fn validate_hd_path(path: &str) -> Result<(), TAError> {
    // 严格验证HD路径格式
    // 防止路径注入攻击
    const MAX_PATH_LENGTH: usize = 100;
    const ALLOWED_PATTERN: &str = r"^m(/[0-9]+'?)*$";

    if path.len() > MAX_PATH_LENGTH {
        return Err(TAError::InvalidParameter);
    }
    // ... 更多验证
}
```

### 阶段2: 中期加固 (3-4周)

#### 2.1 侧信道防护
```rust
// 常时间比较
use subtle::ConstantTimeEq;

fn secure_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

// 固定时间签名操作
fn sign_with_constant_time(message: &[u8], key: &[u8; 32]) -> Signature {
    // 使用抗时序攻击的签名实现
    // 添加随机延迟混淆
}
```

#### 2.2 安全随机数生成
```rust
// 集成硬件随机数生成器
use optee_utee::random::RngCore;

struct SecureRng {
    hardware_rng: optee_utee::random::Rng,
}

impl SecureRng {
    fn generate_entropy(&mut self, buffer: &mut [u8]) -> Result<(), TAError> {
        // 混合多个熵源
        // 硬件随机数 + 时间戳 + 系统事件
        self.hardware_rng.fill_bytes(buffer);
        // 添加熵源混合和白化
        Ok(())
    }
}
```

#### 2.3 密钥派生增强
```rust
// 使用PBKDF2增强密钥派生
use pbkdf2::pbkdf2;
use hmac::Hmac;
use sha2::Sha256;

fn derive_key_secure(mnemonic: &str, path: &str, salt: &[u8]) -> Result<[u8; 32], TAError> {
    const ITERATIONS: u32 = 100_000; // 高迭代次数
    let mut key = [0u8; 32];

    pbkdf2::<Hmac<Sha256>>(
        mnemonic.as_bytes(),
        salt,
        ITERATIONS,
        &mut key
    ).map_err(|_| TAError::InternalError)?;

    Ok(key)
}
```

### 阶段3: 高级安全特性 (5-8周)

#### 3.1 硬件安全模块集成
```rust
// 集成OP-TEE安全存储
use optee_utee::storage::{ObjectHandle, ObjectInfo, StorageId};

struct SecureKeyStorage {
    storage_id: StorageId,
}

impl SecureKeyStorage {
    fn store_key_secure(&self, key_id: &str, key_data: &[u8]) -> Result<(), TAError> {
        // 使用OP-TEE安全存储
        // 硬件加密和完整性保护
        let object = ObjectHandle::create(
            self.storage_id,
            key_id,
            key_data,
        )?;
        Ok(())
    }
}
```

#### 3.2 多因子认证
```rust
// 生物识别 + PIN认证
struct MultiFactorAuth {
    biometric_template: Option<Vec<u8>>,
    pin_hash: [u8; 32],
    failure_count: u32,
}

impl MultiFactorAuth {
    fn authenticate(&mut self, biometric: &[u8], pin: &str) -> Result<(), TAError> {
        // 防暴力破解：失败计数和时间延迟
        if self.failure_count >= 3 {
            return Err(TAError::TooManyFailures);
        }

        // 双因子验证
        let biometric_ok = self.verify_biometric(biometric)?;
        let pin_ok = self.verify_pin(pin)?;

        if biometric_ok && pin_ok {
            self.failure_count = 0;
            Ok(())
        } else {
            self.failure_count += 1;
            Err(TAError::AuthenticationFailure)
        }
    }
}
```

#### 3.3 安全审计和监控
```rust
// 安全事件记录
struct SecurityAuditLog {
    events: Vec<SecurityEvent>,
}

#[derive(Debug)]
struct SecurityEvent {
    timestamp: u64,
    event_type: EventType,
    severity: Severity,
    // 不包含敏感数据
}

impl SecurityAuditLog {
    fn log_key_operation(&mut self, operation: KeyOperation, success: bool) {
        let event = SecurityEvent {
            timestamp: get_secure_timestamp(),
            event_type: EventType::KeyOperation(operation),
            severity: if success { Severity::Info } else { Severity::Warning },
        };
        self.events.push(event);
    }
}
```

### 阶段4: 企业级安全 (9-12周)

#### 4.1 密钥托管和恢复
```rust
// 密钥分片和门限签名
use threshold_crypto::{SecretKeySet, PublicKeySet};

struct KeyRecoverySystem {
    threshold: usize,
    total_shares: usize,
    recovery_shares: Vec<SecretKeyShare>,
}

impl KeyRecoverySystem {
    fn create_recovery_shares(&self, master_key: &[u8; 32]) -> Vec<KeyShare> {
        // 使用Shamir秘密分享
        // 需要threshold个份额才能恢复
        // 支持企业级密钥托管需求
    }
}
```

#### 4.2 零知识证明
```rust
// 零知识身份验证
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};

struct ZKProofAuth {
    generators: BulletproofGens,
    pedersen_gens: PedersenGens,
}

impl ZKProofAuth {
    fn verify_without_revealing(&self, proof: &RangeProof, commitment: &[u8]) -> bool {
        // 验证用户知道私钥，但不泄露私钥
        // 支持隐私保护的身份验证
    }
}
```

## 安全测试策略

### 自动化安全测试
```bash
# 1. 内存安全测试
cargo miri test --target aarch64-unknown-optee

# 2. 模糊测试
cargo +nightly fuzz run fuzz_key_operations

# 3. 侧信道分析
dudect --target aarch64-unknown-optee --test constant_time_ops

# 4. 静态安全分析
cargo audit && cargo clippy -- -W clippy::all
```

### 渗透测试计划
1. **物理攻击模拟**: 侧信道、故障注入
2. **软件攻击测试**: 输入验证、内存安全
3. **协议安全审计**: API接口、错误处理
4. **加密安全评估**: 随机数质量、密钥管理

## 合规性和认证

### 目标认证标准
- **Common Criteria EAL4+**: 高等级安全认证
- **FIPS 140-2 Level 3**: 加密模块安全标准
- **ISO 27001**: 信息安全管理体系
- **SOC 2 Type II**: 服务组织控制

### 合规检查清单
- [ ] 密钥生命周期管理
- [ ] 访问控制和身份验证
- [ ] 数据加密和完整性保护
- [ ] 安全审计和日志记录
- [ ] 事件响应和灾难恢复
- [ ] 定期安全评估和更新

## 实施路线图

### 第1个月: 基础加固
- 内存安全和错误处理
- 输入验证和日志安全
- 基础安全测试

### 第2个月: 防护升级
- 侧信道防护
- 安全随机数生成
- 密钥派生增强

### 第3个月: 高级特性
- 硬件安全模块集成
- 多因子认证
- 安全审计系统

### 第4个月: 企业特性
- 密钥托管和恢复
- 零知识证明
- 合规性认证准备

## 持续安全维护

### 定期安全活动
1. **月度安全审查**: 代码审计、漏洞扫描
2. **季度渗透测试**: 第三方安全评估
3. **年度认证更新**: 合规性检查和认证续期
4. **实时威胁监控**: 安全事件检测和响应

### 安全文档维护
- 安全架构文档
- 威胁模型和风险评估
- 事件响应手册
- 安全操作流程

这个安全增强计划将确保我们的TA实现达到企业级安全标准，为Phase 8的生产部署提供坚实的安全基础。