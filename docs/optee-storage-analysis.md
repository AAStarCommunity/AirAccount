# OP-TEE Secure Storage 深度分析

*创建时间: 2025-09-30*

## 📋 目录

1. [OP-TEE存储架构](#optee存储架构)
2. [QEMU vs 真实硬件对比](#qemu-vs-真实硬件对比)
3. [eth_wallet存储实现](#eth_wallet存储实现)
4. [安全性分析](#安全性分析)
5. [KMS部署建议](#kms部署建议)

---

## OP-TEE存储架构

### 三层存储架构

```
┌────────────────────────────────────────────────────────┐
│         TA Application Code                            │
│  ┌──────────────────────────────────────────────────┐  │
│  │  SecureStorageDb (Rust Wrapper)                  │  │
│  │  - put(key, value)                               │  │
│  │  - get(key) -> value                             │  │
│  │  - delete(key)                                   │  │
│  └──────────────────────────────────────────────────┘  │
│                     ↓ bincode serialize                 │
│  ┌──────────────────────────────────────────────────┐  │
│  │  OP-TEE API (optee-utee crate)                   │  │
│  │  - PersistentObject::create()                    │  │
│  │  - PersistentObject::open()                      │  │
│  │  - ObjectStorageConstants::Private               │  │
│  └──────────────────────────────────────────────────┘  │
│                     ↓ TEE Internal Storage API          │
│  ┌──────────────────────────────────────────────────┐  │
│  │  OP-TEE OS Storage Layer                         │  │
│  │  - TEE_CreatePersistentObject()                  │  │
│  │  - TEE_OpenPersistentObject()                    │  │
│  │  - Encryption: AES-GCM with TA-specific key      │  │
│  └──────────────────────────────────────────────────┘  │
│                     ↓ Platform-specific                 │
│  ┌──────────────────────────────────────────────────┐  │
│  │  Storage Backend                                  │  │
│  │  ┌────────────┬────────────┬──────────────────┐  │  │
│  │  │ REE FS     │ RPMB       │ Secure Flash     │  │  │
│  │  │ (QEMU)     │ (Hardware) │ (Production)     │  │  │
│  │  └────────────┴────────────┴──────────────────┘  │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

### 核心API详解

#### 1. **PersistentObject::create()**

```rust
// 位置: optee-utee/src/object.rs
pub fn create(
    storage_id: ObjectStorageConstants,  // Private = TA专属存储
    object_id: &[u8],                    // 对象ID (例如: "Wallet:uuid")
    flags: DataFlag,                     // 访问权限
    attrs: Option<&Attributes>,          // 可选属性
    data: &[u8],                         // 实际数据
) -> Result<()>
```

**内部流程**:
```
1. 调用 TEE_CreatePersistentObject()
2. OP-TEE OS生成TA-specific加密密钥
3. 使用AES-GCM加密数据
4. 添加HMAC完整性保护
5. 写入后端存储 (REE FS / RPMB)
```

#### 2. **ObjectStorageConstants::Private**

```rust
pub enum ObjectStorageConstants {
    Private = 0x00000001,  // TA专属,其他TA无法访问
    Illegal = 0x7FFFFFFF,
}
```

**关键特性**:
- **隔离性**: 每个TA有独立的存储命名空间
- **自动加密**: 使用TA UUID派生的加密密钥
- **完整性保护**: HMAC验证防篡改

---

## QEMU vs 真实硬件对比

### 存储后端实现

| 特性 | QEMU (开发环境) | 真实硬件 (生产环境) |
|------|-----------------|---------------------|
| **物理存储** | REE文件系统 (`/data/tee/`) | RPMB / Secure Flash |
| **加密** | ✅ AES-GCM (软件) | ✅ AES-GCM (硬件加速) |
| **密钥派生** | ✅ TA UUID + HUK | ✅ TA UUID + HUK (OTP) |
| **完整性保护** | ✅ HMAC-SHA256 | ✅ HMAC-SHA256 + RPMB Counter |
| **防回滚** | ❌ 无硬件支持 | ✅ RPMB单调计数器 |
| **根密钥** | ⚠️ 软件HUK (固定) | ✅ 硬件OTP (唯一) |
| **物理安全** | ❌ 文件可被root删除 | ✅ RPMB需要认证密钥 |
| **API兼容性** | ✅ 100%兼容 | ✅ 相同API |

### QEMU的REE FS存储

#### 存储位置 (Guest VM内)

```bash
# QEMU中的实际存储位置
/data/tee/
├── <TA-UUID>/                    # TA专属目录
│   ├── dirf.db                   # 目录索引
│   ├── dirh.db.hash              # 目录哈希
│   ├── 00000000000000000000...  # 对象文件 (加密)
│   └── 00000000000000000000...hash  # 对象哈希
```

**示例**: eth_wallet的钱包存储
```bash
/data/tee/be2dc9a0-02b4-4b33-ba21-9964dbdf1573/
├── eth_wallet_db                 # 密钥列表索引
├── eth_wallet_db.hash
├── Wallet:aa5798a1-...           # 钱包1数据 (加密)
├── Wallet:aa5798a1-....hash      # 完整性校验
└── ...
```

#### 加密流程

```
原始数据 (Wallet struct)
    ↓ bincode序列化
明文字节流
    ↓ 派生TA密钥: HKDF(HUK, TA_UUID)
    ↓ AES-GCM-256加密
密文 + Tag
    ↓ HMAC-SHA256(密文)
    ↓ 写入REE FS
加密文件 + .hash文件
```

### 真实硬件的RPMB存储

#### RPMB (Replay Protected Memory Block)

**特性**:
```
1. eMMC标准的安全分区
2. 硬件强制认证写入 (HMAC)
3. 单调计数器 (防回滚攻击)
4. 独立于主存储
5. 需要认证密钥才能访问
```

**工作原理**:
```
写入请求:
1. TA请求写入
2. OP-TEE生成RPMB帧 + HMAC(写入计数器)
3. eMMC验证HMAC
4. 验证通过 → 写入 + 计数器+1
5. 验证失败 → 拒绝

读取请求:
1. TA请求读取
2. eMMC返回数据 + HMAC(读取计数器)
3. OP-TEE验证HMAC
4. 验证防止replay attack
```

---

## eth_wallet存储实现

### 代码分析

#### 1. SecureDB包装层

```rust
// crates/secure_db/src/db.rs

pub struct SecureStorageDb {
    name: String,                    // "eth_wallet_db"
    key_list: HashSet<String>,       // 内存缓存的密钥列表
}

impl SecureStorageDb {
    pub fn open(name: String) -> Result<Self> {
        // 1. 尝试从Secure Storage加载key_list
        match load_from_secure_storage(name.as_bytes())? {
            Some(data) => {
                let key_list = bincode::deserialize(&data)?;
                Ok(Self { name, key_list })
            }
            None => {
                // 2. 不存在则创建新DB
                Ok(Self {
                    name,
                    key_list: HashSet::new(),
                })
            }
        }
    }

    pub fn put(&mut self, key: String, value: Vec<u8>) -> Result<()> {
        // 1. 保存实际数据到Secure Storage
        save_in_secure_storage(key.as_bytes(), &value)?;

        // 2. 更新key_list
        self.key_list.insert(key);

        // 3. 持久化key_list
        self.store_key_list()?;
        Ok(())
    }
}
```

#### 2. OP-TEE后端

```rust
// crates/secure_db/src/backend.rs

pub fn save_in_secure_storage(obj_id: &[u8], data: &[u8]) -> Result<()> {
    PersistentObject::create(
        ObjectStorageConstants::Private,  // TA专属存储
        obj_id,                           // "Wallet:uuid"
        DataFlag::ACCESS_READ | DataFlag::ACCESS_WRITE | DataFlag::OVERWRITE,
        None,
        data,                             // bincode序列化的Wallet结构
    )?;
    Ok(())
}

pub fn load_from_secure_storage(obj_id: &[u8]) -> Result<Option<Vec<u8>>> {
    match PersistentObject::open(
        ObjectStorageConstants::Private,
        obj_id,
        DataFlag::ACCESS_READ,
    ) {
        Ok(object) => {
            let obj_info = object.info()?;
            let mut buf = vec![0u8; obj_info.data_size()];
            object.read(&mut buf)?;
            Ok(Some(buf))
        }
        Err(e) => match e.kind() {
            ErrorKind::ItemNotFound => Ok(None),
            _ => Err(e.into()),
        }
    }
}
```

#### 3. eth_wallet钱包存储

```rust
// projects/web3/eth_wallet/ta/src/wallet.rs

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Wallet {
    id: Uuid,              // 16字节UUID
    entropy: Vec<u8>,      // 32字节熵 (主密钥材料)
}

impl Wallet {
    pub fn new() -> Result<Self> {
        // 1. 生成32字节随机熵 (硬件RNG)
        let mut entropy = vec![0u8; 32];
        Random::generate(entropy.as_mut())?;

        // 2. 生成UUID
        let id = Uuid::new_v4();

        Ok(Self { id, entropy })
    }

    pub fn save(&self, db: &mut SecureStorageDb) -> Result<()> {
        // 序列化Wallet结构
        let data = bincode::serialize(self)?;

        // 保存到Secure Storage: key="Wallet:{uuid}", value=序列化数据
        db.put(format!("Wallet:{}", self.id), data)?;
        Ok(())
    }
}
```

### 存储数据流

```
Wallet { id, entropy }
    ↓ bincode::serialize
[UUID(16字节) | 熵长度(3字节) | 熵(32字节)] = ~51字节
    ↓ SecureStorageDb::put("Wallet:uuid", data)
    ↓ save_in_secure_storage(b"Wallet:uuid", data)
    ↓ PersistentObject::create(Private, id, flags, data)
    ↓ TEE_CreatePersistentObject()
    ↓ HKDF(HUK, TA_UUID) -> 加密密钥
    ↓ AES-GCM-256加密
    ↓ HMAC-SHA256完整性保护
    ↓ 写入REE FS: /data/tee/be2dc9a0.../Wallet:uuid
```

---

## 安全性分析

### QEMU环境安全性

#### ✅ 已保护的威胁

| 威胁 | 防护机制 | 有效性 |
|------|---------|--------|
| **其他TA读取** | TA UUID隔离 + 独立密钥 | ✅ 完全防护 |
| **内存嗅探** | TEE内存隔离 | ✅ 完全防护 |
| **数据篡改** | HMAC完整性校验 | ✅ 检测篡改 |
| **密钥泄露** | 密钥永不离开TEE | ✅ 完全防护 |
| **被动监听** | AES-GCM加密 | ✅ 无法读取 |

#### ⚠️ 未防护的威胁 (QEMU特有)

| 威胁 | QEMU影响 | 真实硬件防护 |
|------|---------|-------------|
| **Root删除文件** | ❌ 可删除 `/data/tee/` | ✅ RPMB需要认证密钥 |
| **Replay攻击** | ❌ 可恢复旧备份 | ✅ RPMB单调计数器 |
| **HUK固定** | ⚠️ 所有QEMU实例共享 | ✅ OTP唯一密钥 |
| **冷启动攻击** | ⚠️ 内存未清零 | ✅ 硬件安全启动 |

### 真实硬件增强安全性

#### 1. **Hardware Unique Key (HUK)**

```
QEMU:
  HUK = 固定值 (所有QEMU实例相同)

真实硬件:
  HUK = OTP熔丝烧录的唯一密钥
  - 设备唯一
  - 不可读取
  - 不可修改
```

#### 2. **RPMB防回滚**

```
攻击场景:
  1. 用户创建钱包A
  2. 攻击者备份 /data/tee/
  3. 用户删除钱包A,创建钱包B
  4. 攻击者恢复备份 → 钱包A复活 ❌

QEMU: 无防护
RPMB: 写入计数器+1,旧备份被拒绝 ✅
```

#### 3. **Secure Boot验证链**

```
Boot ROM (不可变)
    ↓ 验证签名
TF-A (BL2)
    ↓ 验证签名
OP-TEE OS
    ↓ 验证签名
TA (.ta文件)
    ↓ 验证UUID + 签名
运行TA代码

任何环节验证失败 → 启动中止
```

---

## KMS部署建议

### 开发阶段 (当前 - QEMU)

**配置**: 使用REE FS存储

**优点**:
- ✅ API完全兼容真实硬件
- ✅ 快速开发迭代
- ✅ 容易调试
- ✅ 无需特殊硬件

**注意事项**:
```bash
# 备份QEMU存储数据
docker exec teaclave_dev_env bash -c \
  "tar -czf /opt/teaclave/shared/tee-backup.tar.gz /data/tee/"

# 清理测试数据
docker exec teaclave_dev_env bash -c \
  "rm -rf /data/tee/be2dc9a0-*"
```

### 测试阶段 (Raspberry Pi 5)

**配置**: 评估eMMC RPMB支持

**检查RPMB可用性**:
```bash
# 在真实硬件上检查
ls /dev/mmcblk0rpmb
dmesg | grep -i rpmb

# 如果RPMB不可用,降级到REE FS
# (仍然有加密和完整性保护)
```

**OP-TEE配置**:
```makefile
# optee_os/mk/config.mk
CFG_RPMB_FS ?= y              # 启用RPMB
CFG_RPMB_TESTKEY ?= n         # 生产环境禁用测试密钥
CFG_REE_FS ?= n               # 禁用REE FS (可选)
```

### 生产阶段

**硬件要求**:
```
必需:
  ✅ ARMv8-A with TrustZone
  ✅ OP-TEE OS支持
  ✅ Secure Boot

推荐:
  ✅ eMMC with RPMB
  ✅ Hardware Crypto Accelerator
  ✅ Secure Display (可选,用于助记词显示)
  ✅ OTP/eFuse for HUK
```

**密钥备份策略**:
```
问题: RPMB损坏 = 密钥永久丢失

方案1: 助记词备份
  - 创建钱包时显示助记词 (Trusted UI)
  - 用户抄写在纸上
  - ⚠️ 需要用户信任

方案2: 多设备分片
  - Shamir秘密共享 (3-of-5)
  - 分片存储在不同设备
  - 需要多数分片才能恢复

方案3: HSM托管
  - 企业级HSM备份主密钥
  - KMS设备存储派生密钥
  - 💰 成本较高
```

### 是否需要修改eth_wallet存储?

**结论**: ✅ **无需修改,当前实现已是最佳实践**

**理由**:

1. **API兼容性** ✅
   - 使用标准OP-TEE API
   - QEMU和真实硬件完全兼容
   - 代码无需改动即可部署

2. **自动平台适配** ✅
   ```
   QEMU:    OP-TEE自动使用REE FS
   Hardware: OP-TEE自动使用RPMB (如果可用)
   配置:    编译时选择,无需改代码
   ```

3. **安全性充分** ✅
   - TA级别隔离
   - 自动加密 (AES-GCM)
   - 完整性保护 (HMAC)
   - 硬件HUK派生密钥

4. **生产就绪** ✅
   - eth_wallet已被Apache验证
   - 在真实硬件上经过测试
   - 成熟的参考实现

**唯一建议**: 添加备份功能

```rust
// kms-ta/src/backup.rs

/// 导出加密的密钥备份
pub fn export_encrypted_backup(
    master_password: &[u8]
) -> Result<Vec<u8>> {
    // 1. 从Secure Storage读取所有密钥
    let keys = db.list_entries_with_prefix("Key:")?;

    // 2. 使用master_password派生加密密钥
    let key = derive_key(master_password)?;

    // 3. 加密导出
    let backup = encrypt(key, &keys)?;

    Ok(backup)
}

/// 导入加密的备份
pub fn import_encrypted_backup(
    backup: &[u8],
    master_password: &[u8]
) -> Result<()> {
    // 恢复流程...
}
```

---

## 总结

### QEMU存储现状

| 项目 | 评估 |
|------|------|
| **API兼容性** | ✅ 100%兼容真实硬件 |
| **开发效率** | ✅ 快速迭代,易调试 |
| **安全性** | ⚠️ 软件级别,足够开发使用 |
| **生产可用** | ❌ 需要真实硬件 |

### 迁移到真实硬件

**无需代码修改** ✅
- 重新编译OP-TEE (启用RPMB)
- 相同的TA代码
- 相同的API调用

**自动获得增强安全性** ✅
- Hardware HUK
- RPMB防回滚
- Secure Boot
- 物理防护

### 最终建议

1. ✅ **继续使用eth_wallet存储实现**
2. ✅ **QEMU阶段专注功能开发**
3. ✅ **真实硬件阶段仅需重新编译**
4. 🆕 **添加备份/恢复功能** (新增特性)

---

*最后更新: 2025-09-30*