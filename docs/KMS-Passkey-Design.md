# KMS Passkey 认证设计方案

**最后更新**: 2025-10-02 12:26

---

## 📋 需求背景

### 当前架构问题
- ❌ 无用户认证: 任何人知道 wallet_id 就可以签名
- ❌ 无访问控制: TEE 内部无法验证调用者身份
- ❌ 安全风险: 钱包私钥完全依赖 TEE 隔离,无用户层认证

### 改进目标
- ✅ 添加 **Passkey (FIDO2/WebAuthn)** 认证
- ✅ 用户使用生物识别 (指纹/Face ID) 控制钱包
- ✅ 双因素安全: TEE 存储 + Passkey 认证

---

## 🔐 Passkey 技术规范

### ✅ 已确认技术栈

**Passkey/FIDO2 使用**:
- **曲线**: **secp256r1 (P-256, prime256v1)** - R1 曲线
- **算法**: ECDSA
- **哈希**: SHA-256
- **编码**: DER (Distinguished Encoding Rules)
- **格式**: COSE (CBOR Object Signing and Encryption)

**与 Ethereum 的区别**:
| 项目 | Ethereum | Passkey/FIDO2 |
|------|----------|---------------|
| 曲线 | secp256k1 (K1) | secp256r1 (R1) |
| 用途 | 交易签名 | 用户认证 |

### ✅ P-256 支持验证 (已确认)

**Rust p256 crate 完全支持 OP-TEE 环境**:
- ✅ **no_std 兼容**: p256 crate 支持 bare-metal 环境 (OP-TEE TA 使用 no_std)
- ✅ **ECDSA 验证**: 提供 `VerifyingKey` 和 `Signature` 类型,支持签名验证
- ✅ **纯 Rust 实现**: 无外部依赖,适合 TEE 环境
- ✅ **已有依赖**: KMS TA 已经包含 `ecdsa v0.13.4` crate

**使用示例**:
```rust
use p256::ecdsa::{VerifyingKey, Signature, signature::Verifier};

let verifying_key = VerifyingKey::from_sec1_bytes(&passkey_pubkey)?;
let signature = Signature::from_der(&signature_der)?;
verifying_key.verify(message, &signature)?; // 验证成功
```

**依赖添加** (kms/ta/Cargo.toml):
```toml
[dependencies]
p256 = { version = "0.13", features = ["ecdsa"], default-features = false }
sha2 = { version = "0.10", default-features = false }  # SHA-256 哈希
```

### ⚠️ 实现注意事项

1. **OP-TEE 原生 API 性能问题**:
   - OP-TEE 的 TEE_ALG_ECDSA_P256 性能较差 (1-2 ops/s)
   - 建议使用 Rust p256 crate (纯软件实现,但性能更稳定)

2. **硬件依赖**:
   - **不依赖特定硬件**: p256 crate 是纯软件实现
   - 如果未来需要硬件加速,可以考虑 ARM CryptoCell 或类似 HSM

---

## 🏗️ TEE 存储结构改进

### 当前结构 (KMS 分支)
```rust
pub struct Wallet {
    id: Uuid,                    // wallet_id (主键)
    entropy: Vec<u8>,            // 32字节种子 (生成助记词和私钥)
    next_address_index: u32,     // 地址计数器 (0-99)
    next_account_index: u32,     // 账户计数器 (未使用)
}
```

**存储关系**:
- `wallet_id` → `Wallet { entropy, counter }`
- 私钥不存储,通过 `derive_prv_key(entropy, path)` 临时计算

### 新结构 (支持 Passkey)
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Wallet {
    id: Uuid,
    entropy: Vec<u8>,
    next_address_index: u32,
    next_account_index: u32,

    // 新增: Passkey 认证字段
    passkey_pubkey: Vec<u8>,            // Passkey 公钥 (P-256, 65字节 uncompressed)
    passkey_credential_id: Vec<u8>,     // Credential ID (WebAuthn)
}

// 挑战管理 (短期缓存,不持久化)
#[derive(Debug, Clone)]
pub struct Challenge {
    value: [u8; 32],           // 随机挑战值
    wallet_id: Uuid,           // 关联的钱包
    created_at: u64,           // 创建时间戳
    expires_at: u64,           // 过期时间戳 (created_at + 180s)
}
```

---

## 🔄 API 流程设计

### 1. 创建钱包 (带 Passkey 绑定)

#### Step 1: 获取挑战
```bash
POST /GetChallenge
Request: {}

Response: {
  "Challenge": "0xabc123...",    # 32字节随机值 (hex)
  "ExpiresIn": 180               # 3分钟
}
```

**TA 操作**:
1. 生成 32 字节随机挑战
2. 缓存 `(challenge, timestamp)`
3. 返回挑战和过期时间

#### Step 2: dApp 使用 Passkey 签名挑战
```javascript
// 前端 WebAuthn 流程
const challenge = await fetch('/GetChallenge').then(r => r.json())

// 创建 Passkey (如果是新用户)
const credential = await navigator.credentials.create({
  publicKey: {
    challenge: hexToArrayBuffer(challenge.Challenge),
    rp: { name: "AirAccount KMS" },
    user: { id: userIdBytes, name: "user@example.com", displayName: "User" },
    pubKeyCredParams: [{ alg: -7, type: "public-key" }],  // ES256 (P-256)
    authenticatorSelection: { userVerification: "required" }
  }
})

// 使用已有 Passkey 签名
const assertion = await navigator.credentials.get({
  publicKey: {
    challenge: hexToArrayBuffer(challenge.Challenge),
    allowCredentials: [{ type: "public-key", id: credentialId }],
    userVerification: "required"
  }
})

const passkeySignature = arrayBufferToHex(assertion.response.signature)
const passkeyPubkey = extractPubkeyFromAttestation(credential)  // P-256 公钥
```

#### Step 3: 创建钱包
```bash
POST /CreateKey
{
  "Description": "My Wallet",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1",
  "Origin": "AWS_KMS",

  // 新增 Passkey 参数
  "PasskeyPubkey": "0x048f3d...",         # P-256 公钥 (65字节 uncompressed)
  "PasskeyCredentialId": "0x1a2b3c...",   # Credential ID
  "PasskeySignature": "0x9d8e7f...",      # 对挑战的 ECDSA 签名 (DER格式)
  "Challenge": "0xabc123..."              # 之前获取的挑战
}
```

**TA 验证流程**:
1. ✅ 检查挑战是否存在且未过期
2. ✅ 使用 `PasskeyPubkey` 验证 `PasskeySignature` 对 `Challenge` 的签名 (P-256 ECDSA)
3. ✅ 验证通过: 创建钱包,存储 `passkey_pubkey` 和 `passkey_credential_id`
4. ✅ 删除已使用的挑战

---

### 2. 签名交易 (需要 Passkey 认证)

#### Step 1: 获取新挑战
```bash
POST /GetChallenge
Response: {
  "Challenge": "0xdef456...",
  "ExpiresIn": 180
}
```

#### Step 2: dApp 使用 Passkey 签名 (txHash + challenge)
```javascript
const challenge = await fetch('/GetChallenge').then(r => r.json())
const txHash = "0x4e6b3f9f..."  // 交易哈希

// 组合消息: keccak256(txHash + challenge)
const message = keccak256(hexToBytes(txHash + challenge.Challenge))

// 使用 Passkey 签名组合消息
const assertion = await navigator.credentials.get({
  publicKey: {
    challenge: hexToArrayBuffer(message),
    allowCredentials: [{ type: "public-key", id: credentialId }],
    userVerification: "required"
  }
})

const passkeySignature = arrayBufferToHex(assertion.response.signature)
```

#### Step 3: 签名交易
```bash
POST /SignHash
{
  "Address": "0x7586...",           # 地址 (用于查找 wallet_id)
  "Hash": "0x4e6b...",              # 交易哈希

  // 新增 Passkey 认证参数
  "Challenge": "0xdef456...",        # 当前挑战
  "PasskeySignature": "0x1c2d3e..."  # 对 keccak256(txHash + challenge) 的签名
}
```

**TA 验证流程**:
1. ✅ 从 Address 查找 `wallet_id` 和 `passkey_pubkey`
2. ✅ 检查挑战是否存在且未过期
3. ✅ 计算 `message = keccak256(Hash + Challenge)`
4. ✅ 使用 `passkey_pubkey` (P-256) 验证 `PasskeySignature`
5. ✅ 验证通过: 使用 TEE 私钥签名交易 (secp256k1)
6. ✅ 删除已使用的挑战
7. ✅ 返回交易签名

---

## 🔧 改造 export_key 工具

### 当前用法
```bash
./export_key <wallet_id> <derivation_path>
# 例: ./export_key 0ea30231-c431-46b5-b092-157a026b8303 "m/44'/60'/0'/0/0"
```

### 改进为支持 Address
```bash
./export_key <wallet_id> <address>
# 例: ./export_key 0ea30231-c431-46b5-b092-157a026b8303 0x7586f1a7f97bdacca13415296aedc76f585712e6
```

**实现**:
1. ✅ 读取 Address Cache: `lookup_address(address) → (wallet_id, derivation_path)`
2. ✅ 调用 TA: `export_private_key(wallet_id, derivation_path)`

---

## 📦 实施计划

### Phase 1: P-256 签名验证支持
**目标**: TA 内支持 secp256r1 (P-256) ECDSA 签名验证

**任务**:
1. ✅ 调研 OP-TEE/Rust 中的 P-256 库
   - 检查 `p256` crate 是否支持 `no_std`
   - 或使用 OP-TEE 内置的 P-256 实现
2. ✅ 添加 P-256 依赖到 `kms/ta/Cargo.toml`
3. ✅ 实现 `verify_p256_signature(pubkey, message, signature)` 函数
4. ✅ 单元测试: 使用已知测试向量验证

**预期输出**:
```rust
// kms/ta/src/crypto.rs
pub fn verify_p256_signature(
    pubkey: &[u8],      // 65字节 uncompressed P-256 公钥
    message: &[u8],     // 消息
    signature: &[u8]    // DER 编码的 ECDSA 签名
) -> Result<bool>
```

---

### Phase 2: 挑战管理系统
**目标**: 实现挑战的生成、存储、验证、过期

**任务**:
1. ✅ 创建 `ChallengeManager` (TA 内存中,非持久化)
   ```rust
   pub struct ChallengeManager {
       challenges: HashMap<[u8; 32], Challenge>,
   }
   ```
2. ✅ 实现 `GetChallenge` TA 命令
3. ✅ 实现挑战验证和过期检查
4. ✅ 添加 `GetChallenge` API 端点

**API**:
```rust
// proto/src/lib.rs
pub enum Command {
    CreateWallet,
    // ...
    GetChallenge,        // 新增
    VerifyChallenge,     // 新增 (内部使用)
}
```

---

### Phase 3: Wallet 结构升级
**目标**: 支持 Passkey 公钥存储

**任务**:
1. ✅ 修改 `Wallet` 结构添加 Passkey 字段
   ```rust
   pub struct Wallet {
       id: Uuid,
       entropy: Vec<u8>,
       next_address_index: u32,
       next_account_index: u32,
       passkey_pubkey: Vec<u8>,         // 新增
       passkey_credential_id: Vec<u8>,  // 新增
   }
   ```
2. ✅ 更新 `CreateWallet` 输入参数
3. ✅ 数据库迁移脚本 (如果需要)

---

### Phase 4: CreateKey 集成 Passkey
**目标**: CreateKey API 支持 Passkey 认证

**任务**:
1. ✅ 修改 `CreateKeyRequest` 添加 Passkey 参数
2. ✅ 实现 TA 内的 Passkey 验证逻辑
3. ✅ 集成 P-256 签名验证
4. ✅ API 端点更新

**验证流程**:
```
CreateKey Request
  ↓
验证挑战未过期
  ↓
P-256 验证 PasskeySignature(Challenge)
  ↓
创建 Wallet (存储 passkey_pubkey)
  ↓
删除挑战
  ↓
返回 Address + PublicKey
```

---

### Phase 5: SignHash 集成 Passkey
**目标**: 签名需要 Passkey 认证

**任务**:
1. ✅ 修改 `SignHashRequest` 添加挑战和签名参数
2. ✅ 实现验证逻辑: `verify_p256(passkey_pubkey, keccak256(hash+challenge), signature)`
3. ✅ 只有验证通过才执行 secp256k1 签名
4. ✅ API 端点更新

**验证流程**:
```
SignHash Request (Address, Hash, Challenge, PasskeySignature)
  ↓
查找 wallet_id 和 passkey_pubkey (from Address)
  ↓
验证挑战未过期
  ↓
计算 message = keccak256(Hash + Challenge)
  ↓
P-256 验证 PasskeySignature(message)
  ↓
secp256k1 签名 Hash (TEE 私钥)
  ↓
删除挑战
  ↓
返回 Signature
```

---

### Phase 6: export_key 改进
**目标**: 支持 Address 参数

**任务**:
1. ✅ 修改 `export_key.rs` 支持两种输入:
   - `<wallet_id> <derivation_path>` (旧)
   - `<wallet_id> <address>` (新)
2. ✅ 从 Address Cache 查找 derivation_path
3. ✅ 调用 TA 导出私钥

---

## 🧪 测试计划

### 单元测试
1. ✅ P-256 签名验证 (使用标准测试向量)
2. ✅ 挑战生成和过期检查
3. ✅ WebAuthn 签名格式解析

### 集成测试
1. ✅ 完整 CreateKey 流程 (GetChallenge → Sign → CreateKey)
2. ✅ 完整 SignHash 流程 (GetChallenge → Sign → SignHash)
3. ✅ 挑战过期拒绝
4. ✅ 错误签名拒绝

### 端到端测试
1. ✅ 浏览器 WebAuthn 集成
2. ✅ 不同设备 Passkey 测试
3. ✅ 生物识别认证流程

---

## 📊 技术依赖

### Rust Crates (TA)
```toml
[dependencies]
p256 = { version = "0.13", default-features = false, features = ["ecdsa"] }
# 或使用 OP-TEE 内置的 P-256
```

### 前端库
```javascript
// WebAuthn 封装
import { create, get } from '@github/webauthn-json'
```

---

## 🔒 安全考虑

### 优势
1. ✅ **双因素认证**: TEE 存储 + Passkey 生物识别
2. ✅ **防重放**: 挑战一次性,3分钟过期
3. ✅ **防钓鱼**: WebAuthn origin 绑定
4. ✅ **无密码**: 纯生物识别,无法暴力破解

### 风险缓解
1. ✅ **挑战泄露**: 短期过期 (3分钟)
2. ✅ **设备丢失**: 可通过助记词恢复 + 重新绑定 Passkey
3. ✅ **中间人攻击**: HTTPS + WebAuthn origin 验证

---

## 🎯 里程碑

| Phase | 功能 | 预计时间 | 状态 |
|-------|------|---------|------|
| Phase 1 | P-256 签名验证 | 2天 | ⏳ 待开始 |
| Phase 2 | 挑战管理系统 | 1天 | ⏳ 待开始 |
| Phase 3 | Wallet 升级 | 1天 | ⏳ 待开始 |
| Phase 4 | CreateKey 集成 | 2天 | ⏳ 待开始 |
| Phase 5 | SignHash 集成 | 2天 | ⏳ 待开始 |
| Phase 6 | export_key 改进 | 0.5天 | ⏳ 待开始 |
| **总计** | | **8.5天** | |

---

## 📚 参考资料

- [WebAuthn Spec](https://www.w3.org/TR/webauthn-3/)
- [FIDO2 Spec](https://fidoalliance.org/specs/fido-v2.0-id-20180227/fido-registry-v2.0-id-20180227.html)
- [COSE Algorithms](https://www.iana.org/assignments/cose/cose.xhtml)
- [P-256 Curve (secp256r1)](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-4.pdf)
- [EIP-7951: Precompile for secp256r1](https://eips.ethereum.org/EIPS/eip-7951)

---

**最后更新**: 2025-10-02 12:20
