# KMS API设计文档

## 📋 基于eth_wallet TA的AWS KMS兼容API设计

### **核心原则**
- **所有密钥操作必须在TA中完成**
- **Host仅处理API请求/响应和协议转换**
- **基于eth_wallet的4个TA命令映射AWS KMS API**

---

## 🔐 **TA核心能力分析**

基于eth_wallet源码分析，TA提供4个原子命令：

### **1. CreateWallet** → **CreateKey API**
```rust
// TA功能：main.rs:68
fn create_wallet(_input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput>

// 输出
struct CreateWalletOutput {
    wallet_id: uuid::Uuid,  // → KeyId
    mnemonic: String        // → 内部安全存储，不暴露
}
```

### **2. RemoveWallet** → **ScheduleKeyDeletion API**
```rust
// TA功能：main.rs:84
fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput>
```

### **3. DeriveAddress** → **GetPublicKey API**
```rust
// TA功能：main.rs:94
fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput>

// 输出
struct DeriveAddressOutput {
    address: [u8; 20],      // → 以太坊地址
    public_key: Vec<u8>     // → 公钥数据
}
```

### **4. SignTransaction** → **Sign API**
```rust
// TA功能：main.rs:111
fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput>

// 输出
struct SignTransactionOutput {
    signature: Vec<u8>      // → 签名数据
}
```

---

## 🌐 **AWS KMS API映射设计**

### **API 1: CreateKey**
```http
POST /
X-Amz-Target: TrentService.CreateKey

请求体:
{
  "Description": "string",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1",
  "Origin": "AWS_KMS"
}

响应:
{
  "KeyMetadata": {
    "KeyId": "uuid-v4",
    "Description": "string",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "KeyState": "Enabled",
    "CreationDate": timestamp
  }
}
```

**实现映射:**
- `CreateKey` → `TA::CreateWallet()`
- `KeyId` ← `wallet_id`
- `mnemonic`在TA中安全存储，不返回

### **API 2: DescribeKey / ListKeys**
```http
POST /
X-Amz-Target: TrentService.DescribeKey

请求体:
{
  "KeyId": "uuid-v4"
}

响应:
{
  "KeyMetadata": {
    "KeyId": "uuid-v4",
    "Description": "string",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "KeyState": "Enabled"
  }
}
```

**实现映射:**
- 基于Host层metadata存储
- TA用于验证KeyId有效性

### **API 3: GetPublicKey**
```http
POST /
X-Amz-Target: TrentService.GetPublicKey

请求体:
{
  "KeyId": "uuid-v4"
}

响应:
{
  "KeyId": "uuid-v4",
  "PublicKey": "base64-encoded-public-key",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1"
}
```

**实现映射:**
- `GetPublicKey` → `TA::DeriveAddress(wallet_id, hd_path)`
- `PublicKey` ← `public_key` (base64编码)
- 默认HD路径: `m/44'/60'/0'/0/0`

### **API 4: Sign**
```http
POST /
X-Amz-Target: TrentService.Sign

请求体:
{
  "KeyId": "uuid-v4",
  "Message": "base64-encoded-message",
  "MessageType": "RAW",
  "SigningAlgorithm": "ECDSA_SHA_256"
}

响应:
{
  "KeyId": "uuid-v4",
  "Signature": "base64-encoded-signature",
  "SigningAlgorithm": "ECDSA_SHA_256"
}
```

**实现映射:**
- `Sign` → `TA::SignTransaction(wallet_id, hd_path, transaction)`
- 将RAW message包装为以太坊交易格式
- `Signature` ← `signature` (base64编码)

### **API 5: ScheduleKeyDeletion**
```http
POST /
X-Amz-Target: TrentService.ScheduleKeyDeletion

请求体:
{
  "KeyId": "uuid-v4",
  "PendingWindowInDays": 7
}

响应:
{
  "KeyId": "uuid-v4",
  "DeletionDate": timestamp
}
```

**实现映射:**
- `ScheduleKeyDeletion` → `TA::RemoveWallet(wallet_id)`
- 支持延迟删除策略

---

## 🏗️ **架构实现要点**

### **三层架构**
```
┌─────────────────────────────────────┐
│       AWS KMS Compatible API       │  ← HTTP/JSON接口
├─────────────────────────────────────┤
│         Host Application           │  ← 协议转换，metadata管理
├─────────────────────────────────────┤
│      TA (eth_wallet based)         │  ← 所有密钥操作
└─────────────────────────────────────┘
```

### **安全边界**
- **TA层**: 密钥生成、存储、签名 (零泄漏)
- **Host层**: API转换、请求验证、日志审计
- **API层**: HTTP处理、认证授权、错误处理

### **数据流**
1. **创建密钥**: `CreateKey` → `create_wallet()` → `uuid + metadata`
2. **获取公钥**: `GetPublicKey` → `derive_address()` → `public_key`
3. **签名数据**: `Sign` → `sign_transaction()` → `signature`
4. **删除密钥**: `ScheduleKeyDeletion` → `remove_wallet()` → `success`

---

## 🔧 **技术实现细节**

### **协议转换**
```rust
// Host层协议转换示例
pub fn kms_create_key_to_ta_create_wallet(
    request: CreateKeyRequest
) -> proto::CreateWalletInput {
    proto::CreateWalletInput {
        // eth_wallet的CreateWallet不需要参数
    }
}

pub fn ta_create_wallet_to_kms_response(
    output: proto::CreateWalletOutput,
    request: CreateKeyRequest
) -> CreateKeyResponse {
    CreateKeyResponse {
        key_metadata: KeyMetadata {
            key_id: output.wallet_id.to_string(),
            description: request.description,
            key_usage: "SIGN_VERIFY".to_string(),
            key_spec: "ECC_SECG_P256K1".to_string(),
            key_state: "Enabled".to_string(),
            creation_date: SystemTime::now(),
        }
    }
}
```

### **错误处理映射**
```rust
// TA错误 → AWS KMS错误码
impl From<TAError> for KMSError {
    fn from(ta_error: TAError) -> Self {
        match ta_error {
            TAError::WalletNotFound => KMSError::NotFoundException,
            TAError::InvalidParameter => KMSError::InvalidRequestException,
            TAError::InternalError => KMSError::KMSInternalException,
        }
    }
}
```

---

## 📊 **支持的密钥规格**

| AWS KMS KeySpec | TA实现 | 支持状态 |
|----------------|--------|----------|
| `ECC_SECG_P256K1` | secp256k1 | ✅ 完全支持 |
| `RSA_2048` | - | ❌ 不支持 |
| `ECC_NIST_P256` | - | ❌ 不支持 |

---

## 🎯 **下一步实现计划**

1. **Phase 1**: 实现基础API映射 (CreateKey, GetPublicKey, Sign)
2. **Phase 2**: 添加密钥管理 (DescribeKey, ListKeys, ScheduleKeyDeletion)
3. **Phase 3**: 错误处理和安全审计
4. **Phase 4**: 性能优化和扩展功能

---

*基于eth_wallet TA能力的KMS API设计 v1.0*