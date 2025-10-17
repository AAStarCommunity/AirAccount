# KMS 钱包地址管理系统设计文档

**创建时间**: 2025-10-01 23:46 (本地时间)
**最后更新**: 2025-10-01 23:50 (本地时间)
**版本**: v2.0 (简化版)
**状态**: 实现阶段

---

## 1. 背景与目标

### 1.1 当前问题
- 用户每次调用 API 都需要提供 `DerivationPath` 参数，使用复杂
- 无法自动管理同一钱包的多个地址（需手动指定不同 path）
- 地址无法作为直接标识符（必须使用 wallet_id + derivation_path）
- 缺少地址恢复机制

### 1.2 设计目标
1. **自动化地址管理**：创建钱包时自动递增地址索引
2. **简化 API**：使用地址作为主要标识符，隐藏 derivation_path 细节
3. **数据恢复能力**：通过 wallet_id 和确定性推导恢复所有地址
4. **向后兼容**：保留旧 API 参数（KeyId + DerivationPath）
5. **安全存储**：关键数据在 TEE Secure Storage，缓存在 Normal World
6. **最小化 TEE 存储**：移除 address_lookup 索引，单钱包仅 56 字节
7. **可配置限制**：单钱包地址数限制（开发阶段 100 个）

---

## 2. 存储架构设计

### 2.1 TEE Secure Storage（核心安全层）

#### 数据结构

```rust
/// 最小化钱包数据（仅存储核心熵和计数器）
#[derive(Serialize, Deserialize)]
pub struct MinimalWallet {
    id: Uuid,                    // wallet_id (16 字节)
    entropy: [u8; 32],           // BIP39 熵 (32 字节)
    next_address_index: u32,     // 下一个可用的 address_index (4 字节)
    next_account_index: u32,     // 下一个可用的 account (4 字节，预留)
}
// 总计：56 字节/钱包

/// TEE 存储结构（简化版 - 移除 address_lookup）
SecureStorage {
    // 唯一索引：wallet_id → MinimalWallet
    wallets: HashMap<Uuid, MinimalWallet>,
}
```

**⚠️ 设计简化说明（v2.0）**：
- ❌ **移除** `AddressIndex` 和 `address_lookup` 反向索引（节省 TEE 存储）
- ✅ **要求用户必须备份 KeyId（wallet_id）**
- ✅ 通过确定性推导恢复所有地址（从 entropy 重新计算）
- ❌ 不支持 "仅通过地址反查 wallet_id"

#### 存储容量分析（简化版）

| 钱包数量 | TEE 存储占用 | OP-TEE 限制 | 支持情况 |
|---------|-------------|------------|---------|
| 1,000 钱包 | 56 KB | 16-64 MB | ✅ |
| 10,000 钱包 | 560 KB | 16-64 MB | ✅ |
| 100,000 钱包 | 5.6 MB | 16-64 MB | ✅ |
| 1,000,000 钱包 | 56 MB | 16-64 MB | ✅ (接近上限) |

**结论**：
- ✅ 单钱包仅 56 字节，极大降低存储压力
- ✅ 可轻松支持数十万钱包（对比之前数千钱包）
- ✅ 每钱包 100 个地址限制不占用额外存储（按需计算）

#### 确定性推导原理

**关键设计**：只存储 `(entropy, counters)`，所有地址可重新计算

```
已知数据：
- entropy (32 字节)
- next_address_index (当前最大索引)

推导过程：
1. entropy → BIP39 mnemonic → BIP32 seed
2. seed + "m/44'/60'/0'/0/0" → 第 0 个地址
3. seed + "m/44'/60'/0'/0/1" → 第 1 个地址
4. ... 循环到 next_address_index - 1

特性：
✅ 确定性：相同 entropy + path = 相同地址（BIP32 标准保证）
✅ 无需存储历史地址数据（可按需重新派生）
✅ 恢复依赖 wallet_id（用户必须备份）
```

#### 地址数量限制

```rust
// kms/ta/src/config.rs
pub const MAX_ADDRESSES_PER_WALLET: u32 = 100;  // 开发阶段限制

// 限制检查
impl MinimalWallet {
    pub fn can_create_address(&self) -> Result<()> {
        if self.next_address_index >= MAX_ADDRESSES_PER_WALLET {
            return Err(anyhow!(
                "Wallet address limit reached ({}/{})",
                self.next_address_index,
                MAX_ADDRESSES_PER_WALLET
            ));
        }
        Ok(())
    }
}
```

**配置说明**：
- 开发阶段：100 个地址/钱包
- 生产阶段：根据实际需求调整（编译时常量）
- 未来扩展：支持运行时配置
```

### 2.2 Normal World 缓存（性能优化层）

#### 数据结构（address_map.json）

```json
{
    "0x1234567890abcdef1234567890abcdef12345678": {
        "wallet_id": "550e8400-e29b-41d4-a716-446655440000",
        "derivation_path": "m/44'/60'/0'/0/0",
        "public_key": "0x04...",
        "created_at": 1696118400
    },
    "0xabcdefabcdefabcdefabcdefabcdefabcdef": {
        "wallet_id": "550e8400-e29b-41d4-a716-446655440000",
        "derivation_path": "m/44'/60'/0'/0/1",
        "public_key": "0x04...",
        "created_at": 1696118500
    }
}
```

#### 特性
- ❌ **可损坏可重建**：丢失后可通过 `kms-recovery-cli` 从 TEE 恢复（需 wallet_id）
- ✅ **快速查询**：Address → (wallet_id, derivation_path) O(1) 查询
- ✅ **开发友好**：JSON 格式便于调试和手动验证
- 🔄 **未来升级**：生产环境可迁移到 SQLite
- ⚠️ **依赖性**：Sign API 的 Address 参数依赖此缓存可用

---

## 3. API 设计改进

### 3.1 CreateKey API（改进版）

#### Request

```json
{
    "KeyId": "550e8400-e29b-41d4-a716-446655440000",  // 可选
    "Description": "User wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS",
    "AutoIncrement": true  // 默认 true，自动递增 address_index
}
```

#### Response

```json
{
    "KeyMetadata": {
        "KeyId": "550e8400-e29b-41d4-a716-446655440000",
        "Address": "0x1234567890abcdef1234567890abcdef12345678",  // ✅ 新增
        "PublicKey": "0x04...",                                    // ✅ 新增
        "DerivationPath": "m/44'/60'/0'/0/0",                      // ✅ 新增
        "Description": "User wallet",
        "KeyState": "Enabled",
        "KeyUsage": "SIGN_VERIFY",
        "KeySpec": "ECC_SECG_P256K1",
        "Origin": "AWS_KMS"
        // ❌ 不返回 Mnemonic（安全原因）
    }
}
```

#### 内部逻辑

```rust
fn create_key(req: CreateKeyRequest) -> Result<CreateKeyResponse> {
    let mut ta_client = TaClient::new()?;

    let (wallet_id, index) = if let Some(key_id) = req.key_id {
        // 已存在钱包，递增 address_index
        let wallet = load_wallet(&key_id)?;
        let index = wallet.next_address_index;
        wallet.next_address_index += 1;
        save_wallet(&wallet)?;
        (key_id, index)
    } else {
        // 创建新钱包
        let wallet = MinimalWallet::new()?;
        save_wallet(&wallet)?;
        (wallet.id, 0)
    };

    // 派生地址
    let path = format!("m/44'/60'/0'/0/{}", index);
    let (address, pubkey) = ta_client.derive_address(wallet_id, &path)?;

    // 更新 TEE address_lookup
    save_address_index(AddressIndex {
        address,
        wallet_id,
        derivation_path: path.clone(),
    })?;

    // 更新 Normal World 缓存
    update_address_map(address, wallet_id, &path, &pubkey)?;

    Ok(CreateKeyResponse {
        key_metadata: KeyMetadata {
            key_id: wallet_id,
            address: hex::encode(address),
            public_key: hex::encode(pubkey),
            derivation_path: path,
            ...
        }
    })
}
```

### 3.2 Sign API（简化版）

#### Request（新增 Address 参数）

```json
{
    "Address": "0x1234567890abcdef1234567890abcdef12345678",  // ✅ 优先使用
    "Message": "SGVsbG8gV29ybGQ=",

    // 或使用旧方式（向后兼容）
    "KeyId": "550e8400-e29b-41d4-a716-446655440000",
    "DerivationPath": "m/44'/60'/0'/0/0"
}
```

#### 查询逻辑

```rust
fn sign(req: SignRequest) -> Result<SignResponse> {
    let (wallet_id, path) = if let Some(address) = req.address {
        // 方式 1：通过地址查询（优先）
        resolve_address(&address)?
    } else if let (Some(key_id), Some(path)) = (req.key_id, req.derivation_path) {
        // 方式 2：直接使用 KeyId + DerivationPath（兼容）
        (key_id, path)
    } else {
        return Err("Must provide either Address or (KeyId + DerivationPath)");
    };

    let mut ta_client = TaClient::new()?;
    let signature = ta_client.sign_message(wallet_id, &path, &req.message)?;

    Ok(SignResponse { signature })
}

fn resolve_address(address: &str) -> Result<(Uuid, String)> {
    // 仅查 Normal World 缓存
    let address_map = load_address_map()?;

    address_map.get(address)
        .map(|m| {
            // 验证缓存一致性（防止污染攻击）
            let (derived_addr, _) = derive_address(m.wallet_id, &m.derivation_path)?;
            if derived_addr == hex::decode(address)? {
                Ok((m.wallet_id, m.derivation_path.clone()))
            } else {
                Err(anyhow!("Cache verification failed"))
            }
        })
        .ok_or_else(|| anyhow!(
            "Address not found in cache. \
             Use 'kms-recovery-cli rebuild-cache --wallet-id <id>' to recover."
        ))??
}
```

### 3.3 ListAddresses API（新增，低优先级）

#### Request

```json
{
    "KeyId": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### Response

```json
{
    "Addresses": [
        {
            "Address": "0x1234...",
            "DerivationPath": "m/44'/60'/0'/0/0",
            "PublicKey": "0x04..."
        },
        {
            "Address": "0x5678...",
            "DerivationPath": "m/44'/60'/0'/0/1",
            "PublicKey": "0x04..."
        }
    ],
    "NextAddressIndex": 2
}
```

---

## 4. 恢复机制设计

### 4.1 场景 1：Normal World 缓存损坏

**症状**：address_map.json 丢失或损坏，TEE Secure Storage 完好

**恢复流程**：

```bash
# 恢复脚本示例
curl -X POST https://kms.aastar.io/admin/RebuildCache
```

```bash
# 使用独立 CLI 工具恢复（需提供 wallet_id）
kms-recovery-cli rebuild-cache --wallet-id 550e8400-e29b-41d4-a716-446655440000
```

```rust
fn rebuild_cache(wallet_id: Uuid) -> Result<()> {
    let mut ta_client = TaClient::new()?;
    let wallet = ta_client.load_wallet(wallet_id)?;

    println!("Rebuilding cache for wallet: {}", wallet_id);
    println!("Total addresses: {}", wallet.next_address_index);

    let mut address_map = load_address_map().unwrap_or_default();

    // 根据 next_address_index 重新派生所有地址
    for i in 0..wallet.next_address_index {
        let path = format!("m/44'/60'/0'/0/{}", i);
        let (address, pubkey) = ta_client.derive_address(wallet_id, &path)?;

        address_map.insert(hex::encode(&address), AddressMetadata {
            wallet_id,
            derivation_path: path.clone(),
            public_key: hex::encode(&pubkey),
            created_at: current_timestamp(),
        });

        println!("  [{}] {} ({})", i, hex::encode(&address), path);
    }

    // 保存缓存
    save_address_map(&address_map)?;

    println!("✅ Cache rebuilt successfully");
    Ok(())
}
```

**性能**：
- 10 地址：<100ms
- 100 地址：<1 秒

### 4.2 场景 2：已知 wallet_id，恢复所有地址

**使用场景**：用户备份了 wallet_id，重新部署系统后恢复

**恢复流程**：

```rust
fn recover_wallet_addresses(wallet_id: Uuid) -> Result<Vec<Address>> {
    let mut ta_client = TaClient::new()?;
    let wallet = ta_client.load_wallet(wallet_id)?;

    let mut addresses = Vec::new();

    // 根据 next_address_index 重新派生所有地址
    for i in 0..wallet.next_address_index {
        let path = format!("m/44'/60'/0'/0/{}", i);
        let (address, pubkey) = ta_client.derive_address(wallet_id, &path)?;

        addresses.push(Address {
            address: hex::encode(address),
            derivation_path: path,
            public_key: hex::encode(pubkey),
        });
    }

    Ok(addresses)
}
```

### 4.3 场景 3：仅记得地址，遗忘 wallet_id

**症状**：用户只知道 `0x1234...`，不知道对应的 wallet_id

**v2.0 设计决策**：
❌ **不支持此场景**（移除 address_lookup 索引）
✅ **要求用户必须备份 KeyId（wallet_id）**

**替代方案**：
- 用户必须妥善保管 wallet_id（类似 AWS KMS 的 Key ARN）
- 如完全遗忘，参考场景 4（管理员辅助恢复）

### 4.4 场景 4：完全遗忘所有信息（管理员辅助）

**恢复流程**：

```rust
fn list_all_wallets() -> Result<Vec<WalletSummary>> {
    let mut ta_client = TaClient::new()?;
    let all_wallets = ta_client.list_all_wallet_ids()?;

    let mut summaries = Vec::new();
    for wallet_id in all_wallets {
        let wallet = ta_client.load_wallet(wallet_id)?;
        let first_address = ta_client.derive_address(
            wallet_id,
            "m/44'/60'/0'/0/0"
        )?;

        summaries.push(WalletSummary {
            wallet_id,
            first_address: hex::encode(first_address.0),
            total_addresses: wallet.next_address_index,
        });
    }

    Ok(summaries)
}
```

**用户操作**：
1. 调用 `ListAllWallets` 查看所有钱包的第一个地址
2. 识别自己的地址
3. 通过 wallet_id 恢复完整钱包

---

## 5. 实现优先级

### Phase 1（核心功能 - v2.0）
- [ ] 修改 `Wallet` 结构为 `MinimalWallet`，添加 `next_address_index`/`next_account_index`
- [ ] 添加 `MAX_ADDRESSES_PER_WALLET` 配置常量（100）
- [ ] 实现 `can_create_address()` 限制检查
- [ ] 修改 `CreateKey` API：自动递增 + 返回 Address/PublicKey/DerivationPath
- [ ] 实现 Normal World 缓存（address_map.json）
- [ ] 修改 `Sign` API：支持 `Address` 参数（仅查缓存）

### Phase 2（恢复工具）
- [ ] 创建 `kms-recovery-cli` 项目
- [ ] 实现 `rebuild-cache` 命令（从 wallet_id 恢复）
- [ ] 实现 `list-addresses` 命令（列出钱包所有地址）
- [ ] 实现 `verify-cache` 命令（验证缓存一致性）

### Phase 3（未来扩展）
- [ ] 支持 Account 自增（`m/44'/60'/{M}'/0/0`）
- [ ] 支持自定义 DerivationPath
- [ ] 迁移 Normal World 缓存到 SQLite
- [ ] 运行时可配置地址限制

---

## 6. 安全考量

### 6.1 Mnemonic 导出控制
**决策**：完全禁用 `ExportMnemonic` API

**原因**：
- Mnemonic 可从 entropy 实时计算，无需持久化
- 导出助记词增加泄露风险
- 用户应备份 wallet_id 而非 mnemonic（更安全）

**实现**：
```rust
// 移除或注释掉 get_mnemonic() 的公开接口
// pub fn get_mnemonic(&self) -> Result<String> { ... }

// API 层完全禁用
// GET /ExportMnemonic → 返回 403 Forbidden
```

### 6.2 TEE 数据完整性
**保护措施**：
1. 所有 TEE 写操作使用事务机制（原子性）
2. 定期验证 `address_lookup` 与 `wallets` 的一致性
3. 每次派生地址后立即更新索引（避免计数器漂移）

### 6.3 Normal World 缓存污染
**风险**：恶意篡改 address_map.json 导致签名到错误地址

**缓解措施**：
```rust
fn resolve_address(address: &str) -> Result<(Uuid, String)> {
    let metadata = load_from_address_map(address)?;

    // 关键验证：从 TEE 重新派生地址，确保一致性
    let (derived_address, _) = derive_address(
        metadata.wallet_id,
        &metadata.derivation_path
    )?;

    if derived_address != hex::decode(address)? {
        // 检测到缓存污染，从 TEE 恢复正确数据
        warn!("Cache poisoning detected for address {}", address);
        rebuild_cache_entry(address)?;
        return Err("Cache verification failed, please retry");
    }

    Ok((metadata.wallet_id, metadata.derivation_path))
}
```

---

## 7. 性能优化

### 7.1 缓存命中率优化
**目标**：95% 以上请求通过 Normal World 缓存响应

**监控指标**：
```rust
struct CacheMetrics {
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    tee_queries: AtomicU64,
}

fn report_cache_hit_rate() -> f64 {
    let hits = METRICS.cache_hits.load(Ordering::Relaxed);
    let misses = METRICS.cache_misses.load(Ordering::Relaxed);
    hits as f64 / (hits + misses) as f64
}
```

### 7.2 TEE 查询优化
**当前**：每次 `resolve_address()` 缓存未命中需调用 TEE

**优化**：批量预热缓存
```rust
// 系统启动时
fn warm_up_cache() -> Result<()> {
    let all_addresses = query_tee_all_addresses()?;

    for (address, index) in all_addresses {
        update_address_map(address, index.wallet_id, &index.derivation_path, ...)?;
    }

    Ok(())
}
```

---

## 8. 向后兼容性

### 8.1 API 参数兼容
**策略**：保留旧参数，优先使用新参数

```rust
#[derive(Deserialize)]
pub struct SignRequest {
    // 新方式（优先）
    #[serde(rename = "Address")]
    pub address: Option<String>,

    // 旧方式（兼容）
    #[serde(rename = "KeyId")]
    pub key_id: Option<Uuid>,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: Option<String>,

    #[serde(rename = "Message")]
    pub message: String,
}
```

### 8.2 数据迁移
**场景**：旧系统只有 `Wallet { id, entropy }`

**迁移脚本**：
```rust
fn migrate_old_wallets() -> Result<()> {
    for wallet in load_old_wallets()? {
        let new_wallet = MinimalWallet {
            id: wallet.id,
            entropy: wallet.entropy,
            next_address_index: 1,  // 假设已派生 1 个地址
            next_account_index: 0,
        };
        save_wallet(&new_wallet)?;
    }
    Ok(())
}
```

---

## 9. 测试计划

### 9.1 单元测试
- [ ] 测试 `MinimalWallet` 序列化/反序列化
- [ ] 测试地址派生确定性（相同 entropy + path = 相同地址）
- [ ] 测试计数器递增逻辑

### 9.2 集成测试
- [ ] 测试 `CreateKey` 自动递增流程
- [ ] 测试 `Sign` 的 Address 查询流程
- [ ] 测试缓存未命中后的 TEE 回退逻辑

### 9.3 恢复测试
- [ ] 删除 address_map.json，验证 `RebuildCache` 恢复
- [ ] 仅提供 wallet_id，验证完整地址列表恢复
- [ ] 仅提供 address，验证 wallet_id 反向查询

---

## 10. 开放问题

### Q1: TEE 存储容量限制
**当前假设**：Raspberry Pi 5 OP-TEE 支持 16-64 MB
**验证方法**：实际硬件测试
**风险缓解**：如超限，实现 address_lookup 的 LRU 淘汰机制

### Q2: 并发写入冲突
**场景**：多个请求同时创建同一 wallet_id 的新地址
**解决方案**：TEE 内部实现 Mutex 保护计数器更新

### Q3: 地址枚举攻击
**风险**：攻击者调用 `ListAllWallets` 枚举所有地址
**缓解**：生产环境禁用或添加认证（Admin API）

---

## 11. 参考资料

- [BIP32: Hierarchical Deterministic Wallets](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
- [BIP39: Mnemonic code for generating deterministic keys](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [BIP44: Multi-Account Hierarchy for Deterministic Wallets](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
- [OP-TEE Secure Storage Documentation](https://optee.readthedocs.io/en/latest/architecture/secure_storage.html)

---

**文档维护者**: Claude Code
**最后更新**: 2025-10-01 23:46
**审核状态**: 待用户确认
