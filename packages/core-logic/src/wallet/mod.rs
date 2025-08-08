// Licensed to AirAccount under the Apache License, Version 2.0
// Wallet module integrating eth_wallet core with AirAccount security enhancements

mod core_wallet;
mod wallet_manager;
mod biometric_integration;
mod multi_chain_support;

pub use core_wallet::{AirAccountWallet, WalletCore, WalletCrypto};
pub use wallet_manager::{WalletManager, UserWalletBinding, WalletPermissions};
pub use biometric_integration::{BiometricVerifier, BiometricTemplate, BiometricAuth};
pub use multi_chain_support::{ChainAdapter, ChainConfig, MultiChainWallet};

use crate::security::{SecurityManager, AuditEvent};
use crate::proto::{WalletCommand, ProtoError};
use uuid::Uuid;
use std::collections::HashMap;

/// 钱包错误类型
#[derive(Debug)]
pub enum WalletError {
    SecurityError(String),
    CryptographicError(String),
    BiometricError(String),
    StorageError(String),
    InvalidParameter(String),
    WalletNotFound(Uuid),
    InsufficientPermissions,
    ProtoError(ProtoError),
}

impl std::fmt::Display for WalletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletError::SecurityError(msg) => write!(f, "Security error: {}", msg),
            WalletError::CryptographicError(msg) => write!(f, "Cryptographic error: {}", msg),
            WalletError::BiometricError(msg) => write!(f, "Biometric error: {}", msg),
            WalletError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            WalletError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            WalletError::WalletNotFound(id) => write!(f, "Wallet not found: {}", id),
            WalletError::InsufficientPermissions => write!(f, "Insufficient permissions"),
            WalletError::ProtoError(e) => write!(f, "Protocol error: {}", e),
        }
    }
}

impl std::error::Error for WalletError {}

impl From<ProtoError> for WalletError {
    fn from(err: ProtoError) -> Self {
        WalletError::ProtoError(err)
    }
}

pub type WalletResult<T> = Result<T, WalletError>;

/// 钱包系统配置
#[derive(Debug, Clone)]
pub struct WalletConfig {
    /// 启用生物识别验证
    pub enable_biometric: bool,
    /// 默认HD钱包路径
    pub default_hd_path: String,
    /// 支持的链配置
    pub chain_configs: HashMap<u64, ChainConfig>,
    /// 审计日志配置
    pub audit_level: crate::security::AuditLevel,
}

impl Default for WalletConfig {
    fn default() -> Self {
        let mut chain_configs = HashMap::new();
        
        // 以太坊主网
        chain_configs.insert(1, ChainConfig {
            chain_id: 1,
            name: "Ethereum Mainnet".to_string(),
            coin_type: 60,
            gas_price_multiplier: 1.0,
            confirmation_blocks: 12,
        });
        
        // Polygon
        chain_configs.insert(137, ChainConfig {
            chain_id: 137,
            name: "Polygon".to_string(),
            coin_type: 966,
            gas_price_multiplier: 1.2,
            confirmation_blocks: 64,
        });
        
        Self {
            enable_biometric: true,
            default_hd_path: "m/44'/60'/0'/0/0".to_string(),
            chain_configs,
            audit_level: crate::security::AuditLevel::Info,
        }
    }
}

/// 钱包系统主入口 - 集成所有功能
pub struct AirAccountWalletSystem {
    security_manager: SecurityManager,
    wallet_manager: WalletManager,
    biometric_verifier: BiometricVerifier,
    chain_adapter: ChainAdapter,
    config: WalletConfig,
}

impl AirAccountWalletSystem {
    /// 创建新的钱包系统实例
    pub fn new(security_manager: SecurityManager, config: WalletConfig) -> WalletResult<Self> {
        let wallet_manager = WalletManager::new(&security_manager)?;
        let biometric_verifier = BiometricVerifier::new(&security_manager)?;
        let chain_adapter = ChainAdapter::new(config.chain_configs.clone())?;
        
        security_manager.audit_info(
            AuditEvent::TEEOperation {
                operation: "wallet_system_init".to_string(),
                duration_ms: 0,
                success: true,
            },
            "wallet_system",
        );
        
        Ok(Self {
            security_manager,
            wallet_manager,
            biometric_verifier,
            chain_adapter,
            config,
        })
    }
    
    /// 处理钱包命令
    pub async fn handle_command(&mut self, command: WalletCommand, input_data: &[u8]) -> WalletResult<Vec<u8>> {
        let start_time = std::time::Instant::now();
        
        let result = match command {
            WalletCommand::CreateWallet => self.handle_create_wallet(input_data).await,
            WalletCommand::RemoveWallet => self.handle_remove_wallet(input_data).await,
            WalletCommand::DeriveAddress => self.handle_derive_address(input_data).await,
            WalletCommand::SignTransaction => self.handle_sign_transaction(input_data).await,
            WalletCommand::SetupBiometric => self.handle_setup_biometric(input_data).await,
            WalletCommand::VerifyBiometric => self.handle_verify_biometric(input_data).await,
            WalletCommand::GetWalletInfo => self.handle_get_wallet_info(input_data).await,
            WalletCommand::ListWallets => self.handle_list_wallets(input_data).await,
            _ => Err(WalletError::InvalidParameter(format!("Unsupported command: {:?}", command))),
        };
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = result.is_ok();
        
        self.security_manager.audit_info(
            AuditEvent::TEEOperation {
                operation: format!("wallet_command_{:?}", command),
                duration_ms,
                success,
            },
            "wallet_system",
        );
        
        result
    }
    
    /// 创建钱包处理
    async fn handle_create_wallet(&mut self, input_data: &[u8]) -> WalletResult<Vec<u8>> {
        use crate::proto::{deserialize_output, serialize_input, CreateWalletInput, CreateWalletOutput};
        
        let input: CreateWalletInput = deserialize_output(input_data)?;
        
        // 创建核心钱包
        let wallet = AirAccountWallet::new(&self.security_manager)?;
        let wallet_id = wallet.get_id();
        
        // 派生地址
        let hd_path = input.chain_id
            .and_then(|chain_id| self.config.chain_configs.get(&chain_id))
            .map(|config| format!("m/44'/{}'/{}/0/0", config.coin_type, 0))
            .unwrap_or_else(|| self.config.default_hd_path.clone());
            
        let (address, public_key) = wallet.derive_address(&hd_path)?;
        let mnemonic = wallet.get_mnemonic()?;
        
        // 存储钱包
        let binding = UserWalletBinding {
            user_id: input.user_id.unwrap_or(0),
            wallet_id,
            address,
            alias: input.alias.clone(),
            is_primary: true,
            permissions: WalletPermissions::full_permissions(),
        };
        
        self.wallet_manager.store_wallet_binding(binding).await?;
        
        let output = CreateWalletOutput {
            wallet_id,
            address,
            mnemonic,
            public_key,
        };
        
        serialize_input(&output).map_err(WalletError::from)
    }
    
    /// 签名交易处理
    async fn handle_sign_transaction(&mut self, input_data: &[u8]) -> WalletResult<Vec<u8>> {
        use crate::proto::{deserialize_output, serialize_input, SignTransactionInput, SignTransactionOutput};
        use sha3::{Digest, Keccak256};
        
        let input: SignTransactionInput = deserialize_output(input_data)?;
        
        // 生物识别验证（如果需要）
        if let Some(biometric_proof) = &input.biometric_proof {
            self.biometric_verifier.verify_biometric(biometric_proof).await?;
        }
        
        // 加载钱包
        let wallet = self.wallet_manager.load_wallet(&input.wallet_id).await?;
        
        // 签名交易
        let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
        
        // 计算交易哈希
        let transaction_bytes = bincode::serialize(&input.transaction)
            .map_err(|e| WalletError::CryptographicError(e.to_string()))?;
        let mut hasher = Keccak256::new();
        hasher.update(&transaction_bytes);
        let transaction_hash: [u8; 32] = hasher.finalize().into();
        
        let output = SignTransactionOutput {
            signature,
            transaction_hash,
        };
        
        serialize_input(&output).map_err(WalletError::from)
    }
    
    /// 其他命令处理函数的占位实现
    async fn handle_remove_wallet(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement remove wallet")
    }
    
    async fn handle_derive_address(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement derive address")
    }
    
    async fn handle_setup_biometric(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement setup biometric")
    }
    
    async fn handle_verify_biometric(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement verify biometric")
    }
    
    async fn handle_get_wallet_info(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement get wallet info")
    }
    
    async fn handle_list_wallets(&mut self, _input_data: &[u8]) -> WalletResult<Vec<u8>> {
        todo!("Implement list wallets")
    }
    
    /// 获取钱包配置
    pub fn config(&self) -> &WalletConfig {
        &self.config
    }
    
    /// 获取安全管理器引用
    pub fn security_manager(&self) -> &SecurityManager {
        &self.security_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityConfig;

    #[tokio::test]
    async fn test_wallet_system_creation() {
        let security_manager = SecurityManager::new(SecurityConfig::default());
        let config = WalletConfig::default();
        
        let wallet_system = AirAccountWalletSystem::new(security_manager, config).unwrap();
        assert!(wallet_system.config.enable_biometric);
        assert!(!wallet_system.config.chain_configs.is_empty());
    }
}