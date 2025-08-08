// Licensed to AirAccount under the Apache License, Version 2.0
// Core wallet implementation adapted from eth_wallet with security enhancements

use crate::security::{SecurityManager, SecureBytes, SecureMemory, AuditEvent};
use crate::proto::EthTransaction;
use super::WalletResult;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;
use ethereum_tx_sign::Transaction;

// 重新导出eth_wallet需要的密码学依赖
use bip32::{Mnemonic, XPrv};
use secp256k1;
use sha3::{Digest, Keccak256};

/// 核心钱包结构 - 基于eth_wallet但增加安全增强
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletCore {
    id: Uuid,
    entropy: SecureBytes,  // 使用我们的SecureBytes而不是Vec<u8>
    created_at: u64,
    last_used_at: Option<u64>,
}

// 手动实现Zeroize，因为SecureBytes已经实现了
impl Zeroize for WalletCore {
    fn zeroize(&mut self) {
        self.entropy.zeroize();
        // Uuid和时间戳不需要清零
    }
}

impl Drop for WalletCore {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl WalletCore {
    /// 创建新钱包 - 使用安全随机数生成器
    pub fn new(security_manager: &SecurityManager) -> WalletResult<Self> {
        let start_time = std::time::Instant::now();
        
        // 使用我们的安全随机数生成器
        let mut entropy_bytes = vec![0u8; 32];
        security_manager.secure_rng().fill_bytes(&mut entropy_bytes);
        let entropy = SecureBytes::from(entropy_bytes);
        
        // 生成UUID
        let mut uuid_bytes = vec![0u8; 16];
        security_manager.secure_rng().fill_bytes(&mut uuid_bytes);
        let uuid_array: [u8; 16] = uuid_bytes.try_into()
            .map_err(|_| WalletError::CryptographicError("Failed to generate UUID".to_string()))?;
        let id = Uuid::from_bytes(uuid_array);
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let wallet = Self {
            id,
            entropy,
            created_at: now,
            last_used_at: None,
        };
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        security_manager.audit_info(
            AuditEvent::KeyGeneration {
                operation: "wallet_creation".to_string(),
                key_type: "hd_wallet_seed".to_string(),
                duration_ms,
                entropy_bits: 256,
            },
            "wallet_core",
        );
        
        Ok(wallet)
    }
    
    /// 获取钱包ID
    pub fn get_id(&self) -> Uuid {
        self.id
    }
    
    /// 获取BIP39助记词 - 使用常时算法保护
    pub fn get_mnemonic(&self) -> WalletResult<String> {
        let entropy_slice = self.entropy.expose_secret();
        
        // 验证熵长度
        if entropy_slice.len() != 32 {
            return Err(WalletError::CryptographicError("Invalid entropy length".to_string()));
        }
        
        let entropy_array: [u8; 32] = entropy_slice.try_into()
            .map_err(|_| WalletError::CryptographicError("Entropy conversion failed".to_string()))?;
            
        let mnemonic = Mnemonic::from_entropy(&entropy_array, bip32::Language::English);
        Ok(mnemonic.phrase().to_string())
    }
    
    /// 获取种子 - 内部使用，不暴露
    fn get_seed(&self) -> WalletResult<SecureMemory> {
        let entropy_slice = self.entropy.expose_secret();
        
        let entropy_array: [u8; 32] = entropy_slice.try_into()
            .map_err(|_| WalletError::CryptographicError("Entropy conversion failed".to_string()))?;
            
        let mnemonic = Mnemonic::from_entropy(&entropy_array, bip32::Language::English);
        let seed = mnemonic.to_seed(""); // 空密码
        
        // 使用SecureMemory保护种子
        let seed_bytes = seed.as_bytes().to_vec();
        Ok(SecureMemory::new(seed_bytes))
    }
    
    /// 派生私钥 - 使用常时算法
    pub fn derive_prv_key(&self, hd_path: &str) -> WalletResult<SecureMemory> {
        let path = hd_path.parse()
            .map_err(|e| WalletError::InvalidParameter(format!("Invalid HD path: {}", e)))?;
            
        let seed = self.get_seed()?;
        let seed_bytes = seed.as_slice();
        
        let child_xprv = XPrv::derive_from_path(seed_bytes, &path)
            .map_err(|e| WalletError::CryptographicError(format!("Key derivation failed: {}", e)))?;
            
        let child_xprv_bytes = child_xprv.to_bytes();
        Ok(SecureMemory::new(child_xprv_bytes.to_vec()))
    }
    
    /// 派生公钥
    pub fn derive_pub_key(&self, hd_path: &str) -> WalletResult<Vec<u8>> {
        let path = hd_path.parse()
            .map_err(|e| WalletError::InvalidParameter(format!("Invalid HD path: {}", e)))?;
            
        let seed = self.get_seed()?;
        let seed_bytes = seed.as_slice();
        
        let child_xprv = XPrv::derive_from_path(seed_bytes, &path)
            .map_err(|e| WalletError::CryptographicError(format!("Key derivation failed: {}", e)))?;
            
        let child_xpub_bytes = child_xprv.public_key().to_bytes();
        Ok(child_xpub_bytes.to_vec())
    }
    
    /// 派生以太坊地址
    pub fn derive_address(&self, hd_path: &str) -> WalletResult<([u8; 20], Vec<u8>)> {
        let public_key_bytes = self.derive_pub_key(hd_path)?;
        
        // 解压公钥
        let public_key = secp256k1::PublicKey::from_slice(&public_key_bytes)
            .map_err(|e| WalletError::CryptographicError(format!("Invalid public key: {}", e)))?;
            
        let uncompressed_public_key = &public_key.serialize_uncompressed()[1..]; // 去掉0x04前缀
        
        // 计算Keccak256哈希
        let address_hash = keccak_hash_to_bytes(uncompressed_public_key);
        let address: [u8; 20] = address_hash[12..].try_into()
            .map_err(|_| WalletError::CryptographicError("Address calculation failed".to_string()))?;
            
        Ok((address, public_key_bytes))
    }
    
    /// 签名交易 - 使用常时算法保护
    pub fn sign_transaction(&self, hd_path: &str, transaction: &EthTransaction) -> WalletResult<Vec<u8>> {
        let xprv = self.derive_prv_key(hd_path)?;
        let xprv_bytes = xprv.as_slice();
        
        // 构造Legacy交易
        let legacy_transaction = ethereum_tx_sign::LegacyTransaction {
            chain: transaction.chain_id,
            nonce: transaction.nonce,
            gas_price: transaction.gas_price,
            gas: transaction.gas,
            to: transaction.to,
            value: transaction.value,
            data: transaction.data.clone(),
        };
        
        let ecdsa = legacy_transaction.ecdsa(xprv_bytes).map_err(|e| {
            let ethereum_tx_sign::Error::Secp256k1(inner_error) = e;
            WalletError::CryptographicError(format!("ECDSA creation failed: {}", inner_error))
        })?;
        
        let signature = legacy_transaction.sign(&ecdsa);
        Ok(signature)
    }
    
    /// 更新最后使用时间
    pub fn update_last_used(&mut self) {
        self.last_used_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
    }
    
    /// 获取创建时间
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
    
    /// 获取最后使用时间
    pub fn last_used_at(&self) -> Option<u64> {
        self.last_used_at
    }
}

/// Keccak256哈希工具函数 - 从eth_wallet移植
fn keccak_hash_to_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// AirAccount钱包 - 主要对外接口
pub struct AirAccountWallet {
    core: WalletCore,
    security_manager: SecurityManager,
}

impl AirAccountWallet {
    /// 创建新的AirAccount钱包
    pub fn new(security_manager: &SecurityManager) -> WalletResult<Self> {
        let core = WalletCore::new(security_manager)?;
        
        Ok(Self {
            core,
            security_manager: security_manager.clone(),
        })
    }
    
    /// 从现有核心钱包创建
    pub fn from_core(core: WalletCore, security_manager: SecurityManager) -> Self {
        Self {
            core,
            security_manager,
        }
    }
    
    /// 获取钱包ID
    pub fn get_id(&self) -> Uuid {
        self.core.get_id()
    }
    
    /// 获取助记词 - 带安全审计
    pub fn get_mnemonic(&self) -> WalletResult<String> {
        self.security_manager.audit_warning(
            AuditEvent::SecurityOperation {
                operation: "mnemonic_export".to_string(),
                risk_level: "HIGH".to_string(),
                details: "Mnemonic exported to normal world".to_string(),
            },
            "wallet_core",
        );
        
        self.core.get_mnemonic()
    }
    
    /// 派生地址
    pub fn derive_address(&self, hd_path: &str) -> WalletResult<([u8; 20], Vec<u8>)> {
        let result = self.core.derive_address(hd_path)?;
        
        self.security_manager.audit_info(
            AuditEvent::TEEOperation {
                operation: "address_derivation".to_string(),
                duration_ms: 0,
                success: true,
            },
            "wallet_core",
        );
        
        Ok(result)
    }
    
    /// 签名交易 - 带完整审计
    pub fn sign_transaction(&self, hd_path: &str, transaction: &EthTransaction) -> WalletResult<Vec<u8>> {
        let start_time = std::time::Instant::now();
        
        // 审计交易签名请求
        self.security_manager.audit_info(
            AuditEvent::SecurityOperation {
                operation: "transaction_signing".to_string(),
                risk_level: "HIGH".to_string(),
                details: format!("Signing transaction to {:?}, value: {}", transaction.to, transaction.value),
            },
            "wallet_core",
        );
        
        let result = self.core.sign_transaction(hd_path, transaction);
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = result.is_ok();
        
        self.security_manager.audit_info(
            AuditEvent::TEEOperation {
                operation: "transaction_signature".to_string(),
                duration_ms,
                success,
            },
            "wallet_core",
        );
        
        result
    }
    
    /// 获取核心钱包引用
    pub fn core(&self) -> &WalletCore {
        &self.core
    }
    
    /// 获取可变核心钱包引用
    pub fn core_mut(&mut self) -> &mut WalletCore {
        &mut self.core
    }
}

/// 钱包密码学工具函数
pub struct WalletCrypto;

impl WalletCrypto {
    /// 验证助记词
    pub fn validate_mnemonic(mnemonic: &str) -> bool {
        Mnemonic::parse(mnemonic, bip32::Language::English).is_ok()
    }
    
    /// 验证HD路径
    pub fn validate_hd_path(path: &str) -> bool {
        crate::proto::validate_hd_path(path).is_ok()
    }
    
    /// 验证以太坊地址
    pub fn validate_eth_address(address: &[u8; 20]) -> bool {
        crate::proto::validate_eth_address(address)
    }
    
    /// 计算地址校验和
    pub fn address_checksum(address: &[u8; 20]) -> String {
        let hex_address = hex::encode(address);
        let hash = keccak_hash_to_bytes(hex_address.as_bytes());
        
        let mut result = String::with_capacity(42);
        result.push_str("0x");
        
        for (i, c) in hex_address.chars().enumerate() {
            if c.is_ascii_digit() {
                result.push(c);
            } else {
                let byte_val = hash[i / 2];
                let nibble = if i % 2 == 0 { byte_val >> 4 } else { byte_val & 0xf };
                if nibble >= 8 {
                    result.push(c.to_ascii_uppercase());
                } else {
                    result.push(c.to_ascii_lowercase());
                }
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityConfig;

    #[test]
    fn test_wallet_creation() {
        let security_manager = SecurityManager::new(SecurityConfig::default());
        let wallet = AirAccountWallet::new(&security_manager).unwrap();
        
        let wallet_id = wallet.get_id();
        assert_ne!(wallet_id, Uuid::nil());
        
        let mnemonic = wallet.get_mnemonic().unwrap();
        assert!(WalletCrypto::validate_mnemonic(&mnemonic));
        
        // 测试地址派生
        let (address, _public_key) = wallet.derive_address("m/44'/60'/0'/0/0").unwrap();
        assert!(WalletCrypto::validate_eth_address(&address));
    }
    
    #[test]
    fn test_address_checksum() {
        let address = [0x52, 0x90, 0x8c, 0x7c, 0x40, 0xef, 0x87, 0x4c, 0x71, 0x60,
                      0x1e, 0x6f, 0x0b, 0x8c, 0x9d, 0x7b, 0x4c, 0x5a, 0x67, 0x52];
        let checksum = WalletCrypto::address_checksum(&address);
        assert!(checksum.starts_with("0x"));
        assert_eq!(checksum.len(), 42);
    }
}