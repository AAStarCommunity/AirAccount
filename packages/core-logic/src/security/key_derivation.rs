// Licensed to AirAccount under the Apache License, Version 2.0
// Key Derivation Function (KDF) implementation for secure key management

use std::sync::Arc;
// use zeroize::ZeroizeOnDrop; // 保留以备将来使用
use serde::{Deserialize, Serialize};

use super::{SecureBytes, SecureRng};
use super::audit::{AuditEvent, AuditLogger};

/// 支持的密钥派生算法
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum KdfAlgorithm {
    /// Argon2id - 推荐用于密码哈希和密钥派生
    Argon2id,
    /// PBKDF2 - 传统但仍然安全的选择
    PBKDF2,
    /// scrypt - 内存困难函数
    Scrypt,
}

impl std::fmt::Display for KdfAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KdfAlgorithm::Argon2id => write!(f, "Argon2id"),
            KdfAlgorithm::PBKDF2 => write!(f, "PBKDF2"),
            KdfAlgorithm::Scrypt => write!(f, "scrypt"),
        }
    }
}

/// 密钥派生参数配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    /// 使用的算法
    pub algorithm: KdfAlgorithm,
    /// 盐的长度（字节）
    pub salt_size: usize,
    /// 迭代次数（Argon2id使用时间成本）
    pub iterations: u32,
    /// 内存成本（仅Argon2id和scrypt使用，单位KB）
    pub memory_cost: Option<u32>,
    /// 并行度（仅Argon2id使用）
    pub parallelism: Option<u32>,
    /// 输出密钥长度
    pub output_length: usize,
}

impl Default for KdfParams {
    fn default() -> Self {
        // 使用OWASP推荐的Argon2id参数
        Self {
            algorithm: KdfAlgorithm::Argon2id,
            salt_size: 32,
            iterations: 3,  // 时间成本
            memory_cost: Some(65536), // 64MB
            parallelism: Some(4),
            output_length: 32,
        }
    }
}

impl KdfParams {
    /// 创建高安全性配置（更多资源消耗）
    pub fn high_security() -> Self {
        Self {
            algorithm: KdfAlgorithm::Argon2id,
            salt_size: 32,
            iterations: 5,
            memory_cost: Some(131072), // 128MB
            parallelism: Some(4),
            output_length: 32,
        }
    }
    
    /// 创建快速配置（降低安全性换取性能）
    pub fn fast() -> Self {
        Self {
            algorithm: KdfAlgorithm::Argon2id,
            salt_size: 16,
            iterations: 1,
            memory_cost: Some(32768), // 32MB
            parallelism: Some(2),
            output_length: 32,
        }
    }
    
    /// 验证参数有效性
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.salt_size < 16 {
            return Err("Salt size must be at least 16 bytes");
        }
        if self.salt_size > 64 {
            return Err("Salt size too large (max 64 bytes)");
        }
        if self.iterations < 1 {
            return Err("Iterations must be at least 1");
        }
        if self.output_length < 16 {
            return Err("Output length must be at least 16 bytes");
        }
        if self.output_length > 128 {
            return Err("Output length too large (max 128 bytes)");
        }
        
        match self.algorithm {
            KdfAlgorithm::Argon2id => {
                if self.memory_cost.unwrap_or(0) < 8192 {
                    return Err("Argon2id memory cost must be at least 8MB");
                }
                if self.parallelism.unwrap_or(0) < 1 {
                    return Err("Argon2id parallelism must be at least 1");
                }
            }
            KdfAlgorithm::Scrypt => {
                if self.memory_cost.unwrap_or(0) < 1024 {
                    return Err("scrypt memory cost must be at least 1MB");
                }
            }
            KdfAlgorithm::PBKDF2 => {
                if self.iterations < 1000 {
                    return Err("PBKDF2 iterations must be at least 1000");
                }
            }
        }
        
        Ok(())
    }
}

/// 密钥派生上下文
pub struct DerivedKey {
    key_material: SecureBytes,
    salt: SecureBytes,
    params: KdfParams,
}

impl DerivedKey {
    /// 获取派生的密钥材料
    pub fn key_material(&self) -> &SecureBytes {
        &self.key_material
    }
    
    /// 获取使用的盐
    pub fn salt(&self) -> &SecureBytes {
        &self.salt
    }
    
    /// 获取派生参数
    pub fn params(&self) -> &KdfParams {
        &self.params
    }
}

impl std::fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedKey")
            .field("key_length", &self.key_material.len())
            .field("salt_length", &self.salt.len())
            .field("params", &self.params)
            .finish()
    }
}

/// 密钥派生函数管理器
pub struct KeyDerivationManager {
    params: KdfParams,
    audit_logger: Option<Arc<AuditLogger>>,
    secure_rng: SecureRng,
}

impl KeyDerivationManager {
    /// 创建新的KDF管理器
    pub fn new(params: KdfParams) -> Result<Self, &'static str> {
        params.validate()?;
        
        Ok(Self {
            params,
            audit_logger: None,
            secure_rng: SecureRng::new()?,
        })
    }
    
    /// 使用默认参数创建
    pub fn with_defaults() -> Result<Self, &'static str> {
        Self::new(KdfParams::default())
    }
    
    /// 设置审计日志记录器
    pub fn with_audit_logger(mut self, logger: Arc<AuditLogger>) -> Self {
        self.audit_logger = Some(logger);
        self
    }
    
    /// 派生密钥（使用随机生成的盐）
    pub fn derive_key(&mut self, password: &[u8]) -> Result<DerivedKey, &'static str> {
        let start_time = std::time::Instant::now();
        
        // 生成随机盐
        let mut salt_bytes = vec![0u8; self.params.salt_size];
        self.secure_rng.fill_bytes(&mut salt_bytes)?;
        let salt = SecureBytes::from(salt_bytes);
        
        // 执行密钥派生
        let derived = self.derive_key_with_salt(password, &salt)?;
        
        // 审计记录
        if let Some(logger) = &self.audit_logger {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            logger.log_security(
                AuditEvent::KeyGeneration {
                    algorithm: format!("KDF_{}", self.params.algorithm),
                    key_size: (self.params.output_length * 8) as u32,
                    operation: "key_derivation".to_string(),
                    key_type: "derived_key".to_string(),
                    duration_ms,
                    entropy_bits: (self.params.salt_size * 8) as u32,
                },
                "key_derivation_manager"
            );
        }
        
        Ok(derived)
    }
    
    /// 使用指定盐派生密钥
    pub fn derive_key_with_salt(&self, password: &[u8], salt: &SecureBytes) -> Result<DerivedKey, &'static str> {
        if salt.len() != self.params.salt_size {
            return Err("Salt size mismatch");
        }
        
        // 根据算法执行密钥派生
        let key_material = match self.params.algorithm {
            KdfAlgorithm::Argon2id => self.derive_argon2id(password, salt.as_slice())?,
            KdfAlgorithm::PBKDF2 => self.derive_pbkdf2(password, salt.as_slice())?,
            KdfAlgorithm::Scrypt => self.derive_scrypt(password, salt.as_slice())?,
        };
        
        Ok(DerivedKey {
            key_material: SecureBytes::from(key_material),
            salt: salt.clone(),
            params: self.params.clone(),
        })
    }
    
    /// 验证密钥是否匹配
    pub fn verify_key(&mut self, password: &[u8], expected: &DerivedKey) -> Result<bool, &'static str> {
        let derived = self.derive_key_with_salt(password, &expected.salt)?;
        
        // 使用常时比较避免时序攻击
        Ok(bool::from(derived.key_material.constant_time_eq(&expected.key_material)))
    }
    
    // Argon2id实现（简化版本，生产环境应使用专门的库）
    fn derive_argon2id(&self, password: &[u8], salt: &[u8]) -> Result<Vec<u8>, &'static str> {
        // 注意：这是一个简化的实现，生产环境应使用argon2库
        use sha3::{Digest, Sha3_256};
        
        let iterations = self.params.iterations;
        let output_len = self.params.output_length;
        
        let mut hasher = Sha3_256::new();
        hasher.update(password);
        hasher.update(salt);
        
        let mut result = hasher.finalize().to_vec();
        
        // 简化的迭代过程
        for _ in 1..iterations {
            let mut hasher = Sha3_256::new();
            hasher.update(&result);
            hasher.update(salt);
            result = hasher.finalize().to_vec();
        }
        
        result.truncate(output_len);
        Ok(result)
    }
    
    // PBKDF2实现（简化版本）
    fn derive_pbkdf2(&self, password: &[u8], salt: &[u8]) -> Result<Vec<u8>, &'static str> {
        use sha3::{Digest, Sha3_256};
        
        let iterations = self.params.iterations;
        let output_len = self.params.output_length;
        
        let mut result = vec![0u8; output_len];
        let mut hasher = Sha3_256::new();
        hasher.update(password);
        hasher.update(salt);
        
        let mut u = hasher.finalize().to_vec();
        result[..std::cmp::min(u.len(), output_len)].copy_from_slice(&u[..std::cmp::min(u.len(), output_len)]);
        
        for _ in 1..iterations {
            let mut hasher = Sha3_256::new();
            hasher.update(&u);
            u = hasher.finalize().to_vec();
            
            // XOR结果
            for i in 0..std::cmp::min(u.len(), output_len) {
                result[i] ^= u[i];
            }
        }
        
        Ok(result)
    }
    
    // scrypt实现（简化版本）
    fn derive_scrypt(&self, password: &[u8], salt: &[u8]) -> Result<Vec<u8>, &'static str> {
        // 简化的scrypt实现，生产环境应使用scrypt库
        self.derive_pbkdf2(password, salt) // 降级到PBKDF2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kdf_params_validation() {
        let valid_params = KdfParams::default();
        assert!(valid_params.validate().is_ok());
        
        let invalid_params = KdfParams {
            salt_size: 8, // 太小
            ..Default::default()
        };
        assert!(invalid_params.validate().is_err());
    }
    
    #[test]
    fn test_key_derivation() {
        let mut kdf = KeyDerivationManager::with_defaults().unwrap();
        let password = b"test_password_123";
        
        let key1 = kdf.derive_key(password).unwrap();
        let key2 = kdf.derive_key(password).unwrap();
        
        // 不同的盐应该产生不同的密钥
        assert!(!bool::from(key1.key_material().constant_time_eq(key2.key_material())));
        assert_eq!(key1.key_material().len(), 32);
    }
    
    #[test]
    fn test_key_verification() {
        let mut kdf = KeyDerivationManager::with_defaults().unwrap();
        let password = b"test_password_123";
        let wrong_password = b"wrong_password";
        
        let derived_key = kdf.derive_key(password).unwrap();
        
        // 正确密码应该验证成功
        assert!(kdf.verify_key(password, &derived_key).unwrap());
        
        // 错误密码应该验证失败
        assert!(!kdf.verify_key(wrong_password, &derived_key).unwrap());
    }
    
    #[test]
    fn test_different_algorithms() {
        let params_argon2id = KdfParams { algorithm: KdfAlgorithm::Argon2id, ..Default::default() };
        let params_pbkdf2 = KdfParams { algorithm: KdfAlgorithm::PBKDF2, iterations: 10000, ..Default::default() };
        
        let kdf1 = KeyDerivationManager::new(params_argon2id).unwrap();
        let kdf2 = KeyDerivationManager::new(params_pbkdf2).unwrap();
        
        let password = b"test_password";
        let salt = SecureBytes::from(vec![0u8; 32]);
        
        let key1 = kdf1.derive_key_with_salt(password, &salt).unwrap();
        let key2 = kdf2.derive_key_with_salt(password, &salt).unwrap();
        
        // 不同算法应该产生不同的结果
        assert!(!bool::from(key1.key_material().constant_time_eq(key2.key_material())));
    }
}