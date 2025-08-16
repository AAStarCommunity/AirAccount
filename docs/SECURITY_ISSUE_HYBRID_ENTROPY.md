# AirAccount 混合熵源安全问题分析

## 🚨 发现的安全问题

### 1. 核心安全问题

**位置**: `packages/core-logic/src/security/hybrid_entropy/`

**问题**: 混合熵源代码在 Core Logic 层实现，而不是在 TA（Trusted Application）中实现

### 2. 安全风险分析

#### 2.1 硬件私钥暴露风险

```rust
// ❌ 错误：在Core Logic层处理硬件种子
// packages/core-logic/src/security/hybrid_entropy/factory_seed.rs:78-102

#[cfg(feature = "hardware")]
fn load_from_hardware_otp() -> Result<Self> {
    unsafe {
        let otp_base = OTP_BASE_ADDR as *mut u32;
        let mut seed_data = [0u8; 32];
        
        // 直接从OTP读取厂家种子 - 暴露硬件独有私钥
        for i in 0..8 {
            let word = ptr::read_volatile(otp_base.add(FACTORY_SEED_OFFSET / 4 + i));
            // ...
        }
    }
}
```

**风险说明**:
1. **私钥暴露**: 厂家根种子直接暴露在用户态代码中
2. **内存泄漏**: 种子数据在普通内存中处理，可能被恶意程序读取
3. **调试风险**: 开发环境下种子可能被日志、调试器捕获
4. **中间人攻击**: 种子在 Core Logic 到 TA 传输过程中可能被截获

#### 2.2 架构违反TEE原则

```
❌ 当前架构（错误）:
Core Logic (用户态) → 直接访问硬件OTP → 处理厂家种子

✅ 应该的架构:
Core Logic → CA → TA (TEE内) → 硬件OTP → 在TEE内处理种子
```

### 3. 正确的架构设计

#### 3.1 TEE内密钥管理

```rust
// ✅ 正确：在TA内实现混合熵源
// packages/airaccount-ta-simple/src/hybrid_entropy.rs

use optee_utee::*;

// 在TEE内的实现
impl HybridEntropyTA {
    // 厂家种子永远不离开TEE
    fn load_factory_seed_in_tee() -> TeecResult<[u8; 32]> {
        // 在TEE内直接从硬件读取
        let factory_seed = read_hardware_otp_secure()?;
        
        // TEE内验证
        validate_seed_entropy(&factory_seed)?;
        
        Ok(factory_seed)
    }
    
    // 混合熵源派生在TEE内完成
    fn derive_hybrid_key_in_tee(
        user_passkey: &[u8],
        user_email: &str
    ) -> TeecResult<[u8; 32]> {
        // 1. 在TEE内获取厂家种子
        let factory_seed = self.load_factory_seed_in_tee()?;
        
        // 2. 在TEE内生成硬件随机数
        let tee_random = generate_tee_random()?;
        
        // 3. 在TEE内进行HKDF派生
        let derived_key = hkdf_expand_in_tee(
            &factory_seed,
            &tee_random,
            user_passkey,
            user_email.as_bytes()
        )?;
        
        // 派生的密钥永远不离开TEE
        Ok(derived_key)
    }
}
```

#### 3.2 安全的接口设计

```rust
// Core Logic 层的安全接口
pub struct SecureHybridEntropy {
    // 不直接存储任何敏感数据
    // 仅提供与TA通信的接口
}

impl SecureHybridEntropy {
    // ✅ 安全：不处理敏感数据，仅转发请求
    pub fn create_user_account(
        &self,
        user_email: &str,
        passkey_credential: &PasskeyCredential
    ) -> Result<EthereumAddress> {
        // 将用户信息发送到TA，让TA在TEE内处理
        let ta_result = self.tee_client.invoke_command(
            CMD_CREATE_HYBRID_ACCOUNT,
            Some(&user_email.as_bytes()),
            Some(&passkey_credential.public_key),
            None,
            None
        )?;
        
        // 只接收以太坊地址（公开信息）
        Ok(EthereumAddress::from_bytes(&ta_result))
    }
}
```

### 4. 修复方案

#### 4.1 立即删除暴露的代码

```bash
# 删除Core Logic中的敏感实现
rm -rf packages/core-logic/src/security/hybrid_entropy/factory_seed.rs
rm -rf packages/core-logic/src/security/hybrid_entropy/tee_random.rs
```

#### 4.2 在TA中重新实现

```rust
// packages/airaccount-ta-simple/src/commands/hybrid_entropy.rs

// 新的TA命令
const CMD_INIT_HYBRID_ENTROPY: u32 = 30;
const CMD_CREATE_HYBRID_ACCOUNT: u32 = 31;
const CMD_DERIVE_HYBRID_KEY: u32 = 32;

pub fn handle_hybrid_entropy_command(
    cmd_id: u32,
    param_types: u32,
    params: &mut [Parameter; 4]
) -> TeecResult<()> {
    match cmd_id {
        CMD_INIT_HYBRID_ENTROPY => {
            // 在TEE内初始化混合熵源
            init_hybrid_entropy_tee()
        },
        CMD_CREATE_HYBRID_ACCOUNT => {
            // 在TEE内创建混合熵账户
            create_hybrid_account_tee(params)
        },
        CMD_DERIVE_HYBRID_KEY => {
            // 在TEE内派生混合密钥
            derive_hybrid_key_tee(params)
        },
        _ => Err(TeecErrorCode::BadParameters)
    }
}
```

#### 4.3 更新Core Logic接口

```rust
// packages/core-logic/src/security/hybrid_entropy/mod.rs

// ✅ 安全的接口（不处理敏感数据）
pub struct HybridEntropyInterface {
    tee_client: TeecClient,
}

impl HybridEntropyInterface {
    pub fn new() -> Result<Self> {
        Ok(Self {
            tee_client: TeecClient::new()?,
        })
    }
    
    // 只提供高级接口，具体实现在TA中
    pub fn create_user_wallet(
        &self,
        user_email: &str,
        passkey_public_key: &[u8]
    ) -> Result<WalletInfo> {
        // 调用TA命令，敏感操作在TEE内完成
        self.tee_client.create_hybrid_wallet(user_email, passkey_public_key)
    }
}
```

### 5. 验证修复效果

#### 5.1 安全检查清单

- [ ] ✅ 厂家种子永远不出现在用户态代码中
- [ ] ✅ 混合熵源派生完全在TEE内执行
- [ ] ✅ Core Logic层只处理公开信息
- [ ] ✅ 私钥生成和存储完全隔离在TEE中
- [ ] ✅ 调试和日志不会泄露敏感信息

#### 5.2 测试验证

```bash
# 验证Core Logic中没有硬件访问代码
grep -r "OTP_BASE_ADDR\|ptr::read_volatile" packages/core-logic/ 
# 应该返回空结果

# 验证TA中有完整的混合熵实现
grep -r "CMD_.*HYBRID" packages/airaccount-ta-simple/
# 应该找到相关命令定义

# 验证核心数据流不暴露敏感信息
grep -r "factory_seed\|root_seed" packages/core-logic/
# 应该只有接口定义，没有具体实现
```

### 6. 改进建议

#### 6.1 分层安全原则

```
用户应用
    ↓ (公开API)
Core Logic (用户态)
    ↓ (TEEC调用)
CA Service (用户态)
    ↓ (TA调用)
TA (TEE内) ← 所有敏感操作在这里
    ↓ (硬件访问)
硬件OTP/Random
```

#### 6.2 最小权限原则

- **Core Logic**: 只能访问公开信息和接口
- **CA Service**: 只能转发命令，不处理敏感数据
- **TA**: 拥有硬件访问权限，处理所有敏感操作

#### 6.3 审计和监控

```rust
// 在TA中添加安全审计
fn audit_sensitive_operation(operation: &str, user_id_hash: &[u8; 32]) {
    // 记录敏感操作（不记录实际密钥）
    trace_println!("AUDIT: {} for user {}", operation, hex::encode(user_id_hash));
}
```

## 总结

当前的混合熵源实现存在严重的安全漏洞，违反了TEE的基本安全原则。必须立即修复，将所有敏感操作迁移到TA中执行，确保硬件独有私钥永远不暴露在用户态。

---

**紧急程度**: 🔴 高危  
**影响范围**: 核心安全架构  
**修复优先级**: P0  
**估计修复时间**: 2-3天  