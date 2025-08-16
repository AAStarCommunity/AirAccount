//! 安全接口层 - 替代混合熵源的安全实现
//! 
//! 重要：此模块不处理任何敏感数据，仅提供与TA通信的安全接口
//! 所有密钥操作必须在TEE内完成

use crate::error::{SecurityError, Result};

/// 安全混合熵源接口
/// 注意：此接口不直接处理密钥，只与TA通信
pub struct SecureHybridEntropyInterface {
    // 不存储任何敏感数据
    initialized: bool,
}

impl SecureHybridEntropyInterface {
    /// 创建安全接口
    pub fn new() -> Result<Self> {
        Ok(Self {
            initialized: false,
        })
    }

    /// 初始化接口（不涉及敏感操作）
    pub fn initialize(&mut self) -> Result<()> {
        // 仅标记初始化状态，不处理敏感数据
        self.initialized = true;
        Ok(())
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// 用户账户信息（仅公开信息）
#[derive(Debug, Clone)]
pub struct PublicAccountInfo {
    /// 用户邮箱
    pub user_email: String,
    
    /// 以太坊地址（公开信息）
    pub ethereum_address: String,
    
    /// Passkey凭据ID（公开信息）
    pub passkey_credential_id: Vec<u8>,
    
    /// 创建时间
    pub created_at: std::time::SystemTime,
    
    /// 账户版本
    pub account_version: u32,
}

/// Passkey凭据（仅公开部分）
#[derive(Debug, Clone)]
pub struct PublicPasskeyCredential {
    pub id: Vec<u8>,
    pub public_key: Vec<u8>,  // 公钥可以暴露
    pub user_handle: Option<Vec<u8>>,
}

/// 验证邮箱地址格式（安全的工具函数）
pub fn is_valid_email(email: &str) -> bool {
    email.contains('@') && email.len() > 3 && email.len() < 255
}

/// SHA256哈希计算（安全的工具函数）
pub fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Keccak256哈希计算（安全的工具函数）
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    use sha3::{Keccak256, Digest};
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_interface_creation() {
        let interface = SecureHybridEntropyInterface::new();
        assert!(interface.is_ok());
        
        let mut interface = interface.unwrap();
        assert!(!interface.is_initialized());
        
        interface.initialize().unwrap();
        assert!(interface.is_initialized());
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("test@example.com"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email(""));
    }

    #[test]
    fn test_hash_functions() {
        let data = b"test data";
        let sha_hash = sha256(data);
        let keccak_hash = keccak256(data);
        
        assert_eq!(sha_hash.len(), 32);
        assert_eq!(keccak_hash.len(), 32);
        assert_ne!(sha_hash, keccak_hash); // 不同算法应产生不同结果
    }
}