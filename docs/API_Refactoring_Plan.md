# KMS API 重构方案 (API Refactoring Plan)

**文档创建时间:** 2025-10-02 15:45
**最后更新:** 2025-10-02 19:30
**状态:** ⚠️ 发现严重问题,需立即修复
**版本:** v3.0 (全面代码审查后修正)

---

## 🎯 系统定位澄清

### 这不是一个 Passkey Server

**误解澄清:**
- ❌ **不是:** 通用的 WebAuthn/FIDO2 认证服务器
- ✅ **是:** TEE-based KMS,使用 Passkey 作为身份验证层

**Passkey 相关职责划分:**

| 功能 | 由谁负责 | 我们是否实现 |
|-----|---------|------------|
| **Passkey 注册 (Registration)** | 浏览器 `navigator.credentials.create()` | ❌ 不实现 |
| **Passkey 验证 (Authentication)** | 浏览器 `navigator.credentials.get()` | ❌ 不实现 |
| **生物识别** | 操作系统/安全硬件 (Touch ID / Face ID) | ❌ 不实现 |
| **Challenge 生成** | **KMS TA** | ✅ **我们实现** |
| **Challenge 存储和验证** | **KMS TA** | ✅ **我们实现** |
| **Passkey 公钥存储** | **KMS TA SecureDB** | ✅ **我们实现** |
| **WebAuthn 签名验证** | **KMS TA** | ✅ **我们实现** |
| **HD Wallet 管理** | **KMS TA** | ✅ **核心功能** |
| **交易签名 (secp256k1)** | **KMS TA** | ✅ **核心功能** |

**实际流程:**
```
1. 用户访问我们的网站 (https://kms.aastar.io)

2. 首次注册钱包:
   a. 前端调用 GET /GetChallenge → TA 生成 challenge
   b. 前端调用 navigator.credentials.create({challenge}) → 浏览器/OS 创建 Passkey
   c. 前端提交 POST /CreateKey {credential_id, pubkey, challenge_signature}
   d. TA 验证签名 → 存储 (credential_id, pubkey) → 创建 HD Wallet

3. 后续签名交易:
   a. 前端调用 GET /GetChallenge(tx_hash) → TA 生成绑定交易的 challenge
   b. 前端调用 navigator.credentials.get({challenge}) → 浏览器/OS 验证指纹
   c. 前端提交 POST /SignHash {address, hash, passkey_signature}
   d. TA 验证 WebAuthn 签名 → 验证 challenge 绑定 → 用 secp256k1 签署交易
```

**我们 vs 真正的 Passkey Server 区别:**

| 特性 | 真正的 Passkey Server (如 Auth0) | 我们的 KMS |
|-----|----------------------------|-----------|
| 核心功能 | 身份验证 (Authentication) | 密钥管理 + 交易签名 |
| 存储内容 | 用户信息 + Passkey 凭证 | HD Wallet 私钥 + Passkey 公钥 |
| Passkey 用途 | 登录验证 | 触发签名操作的授权 |
| 密钥管理 | 无 | ✅ BIP-32/BIP-39 HD Wallet |
| 签名能力 | 无 | ✅ secp256k1 交易签名 |

---

## ✅ 功能恢复完成 (2025-10-02 20:00)

### 问题根源已查明并修复:

1. **SignHash/SignMessage 功能已恢复** ✅
   - 原因: commit 59b50d1 错误地从旧版KMS分支创建feat分支
   - 导致缺失 `sign_hash()`, `sign_message()`, `derive_address_auto()`
   - **已修复:** 从 KMS 分支完整恢复所有功能
   - **当前状态:** 所有KMS功能 + Passkey功能共存

2. **Challenge 时间戳失效** ❌
   - challenge.rs:74 返回固定的 `0`
   - 导致所有过期检查无效 (challenge 永不过期)
   - 需要集成 OP-TEE time API

3. **Passkey 验证不完整** ❌
   - 只验证简单的 P-256 签名
   - 缺少 WebAuthn 完整流程 (authenticatorData, clientDataJSON)
   - 没有验证 rpId, origin, flags

4. **缺少 credential_id 管理** ❌
   - Wallet 结构缺少 `passkey_credential_id` 字段
   - 无法按 Passkey 列出钱包
   - 无法实现多钱包管理

5. **缺少 account_index 自增** ❌
   - CreateWallet 不支持自动派生
   - 无法实现 100 钱包/Passkey 限制

---

## 📊 一、现状分析 (v3.0 - 全面审查版)

### 1.1 API 实现状态 (v3.1 - 修正后)

| API 端点 | TA 实现 | ta_client.rs | api_server.rs | 路由注册 | 整体状态 |
|---------|--------|-------------|--------------|---------|---------|
| POST /CreateKey | ✅ main.rs:88 | ✅ line:72 | ✅ 完整 | ✅ | ✅ **可用** |
| POST /DeriveAddress | ✅ main.rs:114 | ✅ line:90 | ✅ 完整 | ✅ | ✅ **可用** |
| POST /Sign | ✅ main.rs:131 | ✅ line:105 | ✅ 完整 | ✅ | ✅ **可用** |
| **POST /SignHash** | ✅ main.rs:261 | ✅ **line:147** | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ **待完成** |
| **POST /SignMessage** | ✅ (TA有) | ✅ **line:126** | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ **待完成** |
| POST /GetChallenge | ✅ main.rs:193 | ✅ line:185 | ✅ line:377 | ✅ | ⚠️ **时间戳bug** |
| POST /SetPasskeyPubkey | ✅ main.rs:221 | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ **待完成** |
| POST /SetPasskeyEnabled | ✅ main.rs:239 | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ 需添加 | ⚠️ **待完成** |
| POST /ListKeys | ✅ (内存) | N/A | ✅ 完整 | ✅ | ⚠️ **仅内存** |
| POST /DeleteKey | ✅ main.rs:104 | ✅ line:80 | ✅ line:355 | ✅ | ✅ **可用** |

**关键发现:**

**SignHash 实现历史 (commit 25a0c1c):**
```rust
// kms/host/src/api_server.rs:367-404
pub async fn sign_hash(&self, req: SignHashRequest) -> Result<SignHashResponse> {
    let wallet_uuid = Uuid::parse_str(&req.key_id)?;

    // Decode 32-byte hash
    let hash_bytes = hex::decode(&req.hash.trim_start_matches("0x"))?;
    let mut hash_array = [0u8; 32];
    hash_array.copy_from_slice(&hash_bytes);

    // 调用 TA SignHash
    let mut ta_client = TaClient::new()?;
    let signature = ta_client.sign_hash(wallet_uuid, &req.derivation_path, &hash_array)?;

    Ok(SignHashResponse {
        signature: hex::encode(&signature),
    })
}

// 路由注册:712-717
let sign_hash = warp::path("SignHash")
    .and(warp::post())
    .and(warp::header::exact("x-amz-target", "TrentService.SignHash"))
    .and(aws_kms_body())
    .and_then(handle_sign_hash);
```

### 1.2 TA 命令实现状态 (修正)

| TA Command | 实现状态 | 文件位置 | 说明 |
|-----------|---------|---------|------|
| CreateWallet | ✅ 已实现 | ta/src/main.rs:88 | 创建钱包,返回助记词 |
| DeriveAddress | ✅ 已实现 | ta/src/main.rs:114 | 派生地址+公钥 |
| SignTransaction | ✅ 已实现 | ta/src/main.rs:131 | 签名 Legacy Transaction |
| **SignHash** | ✅ **已实现** | **ta/src/main.rs:261** | **签名任意哈希,支持 Passkey** |
| GetChallenge | ✅ 已实现 | ta/src/main.rs:193 | 生成挑战 |
| SetPasskeyPubkey | ✅ 已实现 | ta/src/main.rs:221 | 配置 Passkey 公钥 |
| SetPasskeyEnabled | ✅ 已实现 | ta/src/main.rs:239 | 启用/禁用 Passkey |
| ExportPrivateKey | ✅ 已实现 | ta/src/main.rs:173 | 导出私钥 (工具用) |
| RemoveWallet | ✅ 已实现 | ta/src/main.rs:104 | 删除钱包 |
| TestP256Verify | ✅ 已实现 | ta/src/main.rs:144 | 测试 P-256 验证 |
| SignMessage | ❌ 未实现 | - | 未实现 EIP-191/EIP-712 |
| DeriveAddressAuto | ❌ 仅枚举 | proto/lib.rs:33 | 无实现代码 |

---

## 🔴 二、核心问题识别 (修正版)

### 2.1 ✅ SignHash 已实现 (用户测试证实)

**测试结果:**
```bash
curl -X POST https://kms.aastar.io/SignHash \
  -H "x-amz-target: TrentService.SignHash" \
  -d '{"Address": "0xe3bab...", "Hash": "0x4e6b..."}'

# 返回
{"Signature":"76f62571..."}
```

**结论:** SignHash API 和 TA 命令都已完整实现,无需开发。

---

### 2.2 ❌ Passkey 签名验证逻辑不完整 (P0)

**当前实现 (kms/ta/src/main.rs:310-314):**
```rust
// ❌ 只验证 challenge,未按 WebAuthn 规范验证
verify_passkey_signature(
    passkey_pubkey,
    &passkey_sig.challenge,  // 直接验证 challenge 字节
    &passkey_sig.signature_der,
)?;
```

**问题:**
1. 未验证 `authenticatorData` 和 `clientDataJSON`
2. 未验证 `rpId`, `origin`, `flags` (UP, UV)
3. 签名消息构造不符合 WebAuthn 规范

**正确的 WebAuthn 验证流程:**
```rust
// ✅ 正确实现
fn verify_webauthn_signature(
    pubkey: &[u8],
    passkey_sig: &PasskeySignature,
) -> Result<Vec<u8>> {
    // 1. 解析 authenticatorData
    let auth_data = parse_authenticator_data(&passkey_sig.authenticator_data)?;

    // 2. 验证 rpId hash
    if auth_data.rp_id_hash != sha256(b"kms.aastar.io") {
        return Err(anyhow!("rpId mismatch"));
    }

    // 3. 验证 flags (UP, UV)
    if !auth_data.flags.user_present || !auth_data.flags.user_verified {
        return Err(anyhow!("User not verified"));
    }

    // 4. 解析 clientDataJSON
    let client_data: ClientData = serde_json::from_slice(&passkey_sig.client_data_json)?;

    // 5. 验证 type 和 origin
    if client_data.type_ != "webauthn.get" {
        return Err(anyhow!("Invalid type"));
    }
    if client_data.origin != "https://kms.aastar.io" {
        return Err(anyhow!("Origin mismatch"));
    }

    // 6. 构造签名消息 (WebAuthn 规范)
    let client_data_hash = sha256(&passkey_sig.client_data_json);
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&passkey_sig.authenticator_data);
    signed_data.extend_from_slice(&client_data_hash);

    // 7. 验证 P-256 签名
    verify_p256_signature(pubkey, &signed_data, &passkey_sig.signature_der)?;

    // 8. 返回 challenge (供后续验证交易绑定)
    let challenge_bytes = base64_url_decode(&client_data.challenge)?;
    Ok(challenge_bytes)
}
```

---

### 2.3 ❌ Challenge 未绑定交易哈希

**当前实现:**
```rust
// ❌ Challenge 是随机生成,与交易无关
let mut challenge = [0u8; 32];
Random::generate(&mut challenge as _);
```

**用户要求:**
```
Challenge 应该绑定交易哈希,防止重放攻击
```

**正确实现 (选项C - 结构化消息):**
```rust
fn generate_transaction_challenge(tx_hash: &[u8; 32]) -> [u8; 32] {
    let message = format!(
        "AirAccount KMS Sign Transaction\n\
         Transaction Hash: {}\n\
         Nonce: {}\n\
         Timestamp: {}",
        hex::encode(tx_hash),
        hex::encode(&random_nonce()),
        get_timestamp()
    );

    sha256(message.as_bytes())
}
```

---

### 2.4 ❌ 缺少 credential_id 存储和管理

**当前 Wallet 结构:**
```rust
pub struct Wallet {
    id: Uuid,
    entropy: Vec<u8>,
    passkey_pubkey: Option<Vec<u8>>,  // ✅ 有
    passkey_enabled: bool,             // ✅ 有
    // ❌ 缺少:
    // passkey_credential_id: Option<Vec<u8>>,
    // account_index: u32,
    // created_at: u64,
}
```

**需要添加:**
- `credential_id`: Passkey 标识符 (16-128 bytes)
- `account_index`: BIP-44 账户索引 (0-99)
- `created_at`: 创建时间戳

---

### 2.5 ❌ 缺少自动地址派生

**当前 CreateWallet:**
```rust
// kms/ta/src/main.rs:88-102
fn create_wallet(_input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    let wallet = Wallet::new()?;
    // ❌ 未实现: 自动派生地址
    // ❌ 未实现: account_index 管理
    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,  // 只返回这两项
    })
}
```

**用户确认:** "create携带wallet-id自动自增" - 但检查代码后**确认未实现**

**DeriveAddressAuto:** 仅枚举定义,无实现代码

---

## 🏗️ 三、重构方案 (最终版)

### 3.1 最终 API 清单 (统一命名)

| API 端点 | HTTP 方法 | 用途 | TA 命令 | 状态 |
|---------|----------|------|---------|------|
| **/CreateKey** | POST | 创建钱包(无 Passkey) | CreateWallet | ✅ 保留 |
| **/CreateKeyWithPasskey** | POST | 创建钱包(绑定 Passkey,自动派生) | CreateWalletWithPasskey | ⚠️ 新增 |
| **/ListKeys** | POST | 列出所有钱包 (按 credential_id 过滤) | ListWallets | ⚠️ 改造 |
| **/Sign** | POST | 签名交易 | SignTransaction | ✅ 保留,增强 |
| **/SignHash** | POST | 签名任意哈希 | SignHash | ✅ 保留,增强 |
| **/GetChallenge** | POST | 获取挑战 (绑定交易哈希) | GetChallenge | ✅ 保留,改造 |
| **/health** | GET | 健康检查 | - | ✅ 保留 |

**移除的 API:**
- ❌ /DeriveAddress - 功能由 CreateKeyWithPasskey 自动完成
- ❌ /GetPublicKey - 公钥已在 CreateKey/ListKeys 返回
- ❌ /DescribeKey - 功能由 ListKeys 替代
- ❌ /DeleteKey - 不对外暴露 (保留 TA 命令用于内部)

**命名规则:**
- 全部采用 AWS KMS 风格: `/CreateKey`, `/SignHash`
- 不使用 `/wallet/create` 等 REST 风格

---

### 3.2 PasskeySignature 数据结构升级

```rust
// kms/proto/src/in_out.rs

/// WebAuthn 标准的 Passkey 签名数据
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasskeySignature {
    pub credential_id: Vec<u8>,           // Passkey 标识符 (16-128 bytes)
    pub authenticator_data: Vec<u8>,      // WebAuthn authenticatorData
    pub client_data_json: Vec<u8>,        // WebAuthn clientDataJSON
    pub signature_der: Vec<u8>,           // P-256 DER signature
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientData {
    #[serde(rename = "type")]
    pub type_: String,                    // "webauthn.get"
    pub challenge: String,                // base64url challenge
    pub origin: String,                   // "https://kms.aastar.io"
}

pub struct AuthenticatorData {
    pub rp_id_hash: [u8; 32],             // SHA-256(rpId)
    pub flags: AuthenticatorFlags,
    pub sign_count: u32,
}

pub struct AuthenticatorFlags {
    pub user_present: bool,               // UP flag
    pub user_verified: bool,              // UV flag (生物识别)
}
```

---

### 3.3 Wallet 结构升级

```rust
// kms/ta/src/wallet.rs

pub struct Wallet {
    id: Uuid,
    entropy: Vec<u8>,

    // Passkey 绑定
    passkey_pubkey: Option<Vec<u8>>,         // ✅ 已有
    passkey_credential_id: Option<Vec<u8>>,  // ⚠️ 新增
    passkey_enabled: bool,                    // ✅ 已有

    // 账户管理
    account_index: u32,                       // ⚠️ 新增 (0-99)
    created_at: u64,                          // ⚠️ 新增
}

impl Wallet {
    pub fn set_passkey_credential_id(&mut self, credential_id: Vec<u8>) -> Result<()> {
        if credential_id.len() > 128 {
            return Err(anyhow!("Credential ID too long (max 128 bytes)"));
        }
        self.passkey_credential_id = Some(credential_id);
        Ok(())
    }

    pub fn get_passkey_credential_id(&self) -> Option<&[u8]> {
        self.passkey_credential_id.as_deref()
    }

    pub fn set_account_index(&mut self, index: u32) {
        self.account_index = index;
    }

    pub fn get_account_index(&self) -> u32 {
        self.account_index
    }
}
```

---

## 📋 四、实施计划 (v3.0 - 紧急修复优先)

### ⚠️ Phase 0: 紧急Bug修复 (P0+++ 🔥 - 立即执行)

**目标:** 修复当前代码中的致命缺陷

**工作内容:**

1. **恢复 SignHash API 路由** (15 分钟)
   ```rust
   // kms/host/src/ta_client.rs - 添加缺失的方法
   impl TaClient {
       pub fn sign_hash(&mut self, wallet_id: uuid::Uuid, hd_path: &str, hash: &[u8; 32]) -> Result<Vec<u8>> {
           let input = proto::SignHashInput {
               wallet_id,
               hd_path: hd_path.to_string(),
               hash: hash.to_vec(),
               passkey_signature: None,  // 暂时可选
           };
           let serialized_input = bincode::serialize(&input)?;
           let serialized_output = self.invoke_command(proto::Command::SignHash, &serialized_input)?;
           let output: proto::SignHashOutput = bincode::deserialize(&serialized_output)?;
           Ok(output.signature)
       }
   }
   ```

2. **添加 API Server 的 SignHash 处理** (from commit 25a0c1c) (20 分钟)
   ```rust
   // kms/host/src/api_server.rs

   #[derive(Debug, Serialize, Deserialize)]
   pub struct SignHashRequest {
       #[serde(rename = "KeyId")]
       pub key_id: String,
       #[serde(rename = "DerivationPath")]
       pub derivation_path: String,
       #[serde(rename = "Hash")]
       pub hash: String,
   }

   impl KmsApiServer {
       pub async fn sign_hash(&self, req: SignHashRequest) -> Result<SignHashResponse> {
           let wallet_uuid = Uuid::parse_str(&req.key_id)?;
           let hash_bytes = hex::decode(&req.hash.trim_start_matches("0x"))?;
           let mut hash_array = [0u8; 32];
           hash_array.copy_from_slice(&hash_bytes);

           let mut ta_client = TaClient::new()?;
           let signature = ta_client.sign_hash(wallet_uuid, &req.derivation_path, &hash_array)?;

           Ok(SignHashResponse {
               signature: hex::encode(&signature),
           })
       }
   }

   // 添加路由 (在 pub async fn run() 中)
   let sign_hash = warp::path("SignHash")
       .and(warp::post())
       .and(warp::header::exact("x-amz-target", "TrentService.SignHash"))
       .and(aws_kms_body())
       .and_then(handle_sign_hash);
   ```

3. **修复 Challenge 时间戳** (10 分钟)
   ```rust
   // kms/ta/src/challenge.rs:69-75
   fn get_current_time() -> u64 {
       // 使用 OP-TEE time API
       use optee_utee::time::SystemTime;
       match SystemTime::now() {
           Ok(time) => time.seconds() as u64,
           Err(_) => {
               // Fallback: 使用全局计数器
               static COUNTER: AtomicU64 = AtomicU64::new(0);
               COUNTER.fetch_add(1, Ordering::Relaxed)
           }
       }
   }
   ```

4. **添加 SetPasskeyPubkey/SetPasskeyEnabled API** (20 分钟)
   - 在 ta_client.rs 添加对应方法
   - 在 api_server.rs 添加路由处理

**预计时间:** 1小时

**验证:**
```bash
# 测试 SignHash API
curl -X POST https://kms.aastar.io/SignHash \
  -H "x-amz-target: TrentService.SignHash" \
  -d '{"KeyId": "<wallet_id>", "Hash": "0x4e6b...", "DerivationPath": "m/44'"'"'/60'"'"'/0'"'"'/0/0"}'
```

---

### Phase 10: WebAuthn 完整验证 (P0 🔴 - 最高优先级)

**目标:** 修复 Passkey 签名验证逻辑

**工作内容:**

1. **升级 PasskeySignature 数据结构**
   - 添加 `authenticator_data`, `client_data_json`
   - 添加 `ClientData`, `AuthenticatorData` 解析结构

2. **实现完整 WebAuthn 验证**
   - 实现 `verify_webauthn_signature()` (按规范)
   - 实现 `parse_authenticator_data()`
   - 验证 rpId, origin, flags

3. **修改 sign_hash() 使用新验证**
   - 替换当前的简单验证
   - 添加 challenge 交易绑定验证

**预计时间:** 3-4小时

---

### Phase 11: Wallet 结构和账户索引 (P1 🟠)

**目标:** 实现 credential_id 绑定和自动账户管理

**工作内容:**

1. **升级 Wallet 结构**
   - 添加 `credential_id`, `account_index`, `created_at`

2. **实现 CreateWalletWithPasskey**
   ```rust
   fn create_wallet_with_passkey(
       input: &proto::CreateWalletWithPasskeyInput
   ) -> Result<proto::CreateWalletWithPasskeyOutput> {
       // 查找已有钱包数量
       let existing = find_wallets_by_credential_id(&input.credential_id)?;
       let account_index = existing.len() as u32;

       if account_index >= 100 {
           return Err(anyhow!("Maximum 100 wallets per Passkey"));
       }

       // 创建钱包
       let mut wallet = Wallet::new()?;
       wallet.set_passkey_credential_id(input.credential_id.clone())?;
       wallet.set_passkey_pubkey(input.passkey_pubkey.clone())?;
       wallet.set_account_index(account_index);
       wallet.set_passkey_enabled(true)?;

       // 自动派生地址: m/44'/60'/<account_index>'/0/0
       let hd_path = format!("m/44'/60'/{}'/0/0", account_index);
       let (address, public_key) = wallet.derive_address(&hd_path)?;

       // 保存
       let db_client = SecureStorageClient::open(DB_NAME)?;
       db_client.put(&wallet)?;

       Ok(proto::CreateWalletWithPasskeyOutput {
           wallet_id: wallet.get_id(),
           address,
           public_key,
           account_index,
           hd_path,
       })
   }
   ```

3. **实现 ListWalletsByCredentialId**
   ```rust
   fn list_wallets(
       credential_id: Option<Vec<u8>>
   ) -> Result<proto::ListWalletsOutput> {
       let db_client = SecureStorageClient::open(DB_NAME)?;
       let all_wallets = db_client.list_all::<Wallet>()?;

       let filtered = if let Some(cred_id) = credential_id {
           all_wallets.into_iter()
               .filter(|w| w.get_passkey_credential_id() == Some(&cred_id))
               .collect()
       } else {
           all_wallets
       };

       // ... 构造返回
   }
   ```

**预计时间:** 3-4小时

---

### Phase 12: Challenge 交易绑定 (P1 🟠)

**目标:** 实现 challenge 与交易哈希绑定

**工作内容:**

1. **升级 Challenge 结构**
   ```rust
   pub struct Challenge {
       pub challenge: [u8; 32],
       pub created_at: u64,
       pub used: bool,
       pub bound_tx_hash: Option<[u8; 32]>,  // 新增
   }
   ```

2. **实现结构化消息生成**
   ```rust
   fn generate_transaction_challenge(tx_hash: &[u8; 32]) -> [u8; 32] {
       let nonce = random_nonce();
       let message = format!(
           "AirAccount KMS Sign Transaction\n\
            Transaction Hash: {}\n\
            Nonce: {}\n\
            Timestamp: {}",
           hex::encode(tx_hash),
           hex::encode(&nonce),
           get_timestamp()
       );

       sha256(message.as_bytes())
   }
   ```

3. **验证 challenge 绑定**
   ```rust
   fn verify_challenge_binds_tx_hash(
       challenge: &[u8; 32],
       tx_hash: &[u8; 32]
   ) -> Result<()> {
       let stored_challenge = get_challenge(challenge)?;

       if let Some(bound_hash) = stored_challenge.bound_tx_hash {
           if &bound_hash != tx_hash {
               return Err(anyhow!("Transaction hash mismatch"));
           }
       }

       Ok(())
   }
   ```

**预计时间:** 2小时

---

### Phase 13: API 层清理 (P2 🟡)

**目标:** 移除重复/未实现的 API

**工作内容:**

1. **移除 API:**
   - /DeriveAddress (功能由自动派生替代)
   - /GetPublicKey (返回占位符,无实际用途)
   - /DescribeKey (无持久化,由 ListKeys 替代)
   - /DeleteKey (不对外暴露)

2. **移除 TA 命令:**
   - DeriveAddressAuto (仅枚举,无实现)

3. **更新文档和测试**

**预计时间:** 1-2小时

---

## ⏱️ 总体时间估算 (v3.0)

| Phase | 内容 | 预计时间 | 优先级 |
|-------|------|---------|--------|
| **Phase 0** | **紧急Bug修复** | **1h** | **P0+++ 🔥** |
| Phase 10 | WebAuthn 完整验证 | 3-4h | P0 🔴 |
| Phase 11 | Wallet 结构和索引 | 3-4h | P1 🟠 |
| Phase 12 | Challenge 绑定 | 2h | P1 🟠 |
| Phase 13 | API 清理 | 1-2h | P2 🟡 |
| **总计** | | **10-13h** | |

---

## 📝 关键结论 (Code Review Summary)

### 🚨 发现的严重问题:

1. **SignHash API 不可用**
   - TA 实现完整 (main.rs:261)
   - ❌ ta_client.rs 缺失方法
   - ❌ api_server.rs 缺失路由
   - 用户报告可用的版本在 commit 25a0c1c,后续被覆盖

2. **Challenge 时间戳永远为 0**
   - challenge.rs:74 `return 0` (硬编码)
   - 所有 challenge 永不过期 (安全漏洞)

3. **Passkey 验证流程不完整**
   - 只验证 P-256 签名,没有 WebAuthn 标准流程
   - 缺少 rpId, origin, flags 验证

4. **缺少核心字段**
   - Wallet 无 `credential_id` → 无法按 Passkey 查询
   - Wallet 无 `account_index` → 无法自动派生
   - Challenge 无 `bound_tx_hash` → 无法绑定交易

### ✅ 实施优先级调整:

**必须先执行 Phase 0** (1小时紧急修复),否则系统不可用。

---

## ✅ 用户确认清单

- [x] SignHash API: ⚠️ **TA层已实现,但API层缺失** (需 Phase 0 修复)
- [x] Passkey 签名格式: 选项C (结构化消息)
- [x] Credential ID: 不建索引 (1万用户内可接受)
- [x] 账户索引: 自动递增 (0-99)
- [x] 删除机制: 仅设计,不实施
- [x] 工具函数: bin/export_key 已实现
- [x] 自动派生: **确认未实现**,需开发
- [x] API 命名: 统一使用 AWS KMS 风格
- [x] **代码审查: 发现5个严重bug** (需 Phase 0 修复)

---

## 🚀 立即行动计划

**优先级 1:** Phase 0 - 紧急修复 SignHash API (1小时)
**优先级 2:** Phase 10 - WebAuthn 完整验证 (3-4小时)
**优先级 3:** Phase 11-13 - 功能增强

### 当前状态总结:

| 组件 | 状态 | 问题 |
|-----|------|-----|
| TA (Trusted App) | ✅ 完整 | 功能正常 |
| TaClient 库 | ⚠️ 不完整 | 缺 sign_hash, set_passkey_* |
| API Server | ⚠️ 不完整 | 缺 SignHash 路由 |
| Challenge 管理 | ❌ 有bug | 时间戳失效 |
| Passkey 验证 | ❌ 不完整 | 非标准流程 |

**建议:** 先执行 Phase 0 恢复基本功能,再讨论是否继续 Phase 10+

**文档结束**
