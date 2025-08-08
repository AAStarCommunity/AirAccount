// Licensed to AirAccount under the Apache License, Version 2.0
// Protocol definitions adapted from eth_wallet for AirAccount integration

use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 钱包命令枚举 - 基于eth_wallet扩展
#[derive(FromPrimitive, IntoPrimitive, Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum WalletCommand {
    // 基础eth_wallet命令
    CreateWallet,
    RemoveWallet,
    DeriveAddress,
    SignTransaction,
    
    // AirAccount扩展命令
    SetupBiometric,        // 设置生物识别
    VerifyBiometric,       // 验证生物识别
    CreateMultiSigWallet,  // 创建多签钱包
    SocialRecovery,        // 社交恢复
    GetWalletInfo,         // 获取钱包信息
    ListWallets,           // 列出用户钱包
    
    #[default]
    Unknown,
}

/// 以太坊交易结构 - 直接从eth_wallet移植
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

/// 钱包信息结构 - AirAccount扩展
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletInfo {
    pub wallet_id: Uuid,
    pub address: Option<[u8; 20]>,
    pub chain_id: Option<u64>,
    pub alias: Option<String>,
    pub is_primary: bool,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

/// 生物识别配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BiometricConfig {
    pub biometric_type: BiometricType,
    pub template_hash: [u8; 32],
    pub threshold: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BiometricType {
    Fingerprint,
    Face,
    Voice,
}

// ===========================================
// 输入输出结构定义 - 基于eth_wallet扩展
// ===========================================

/// 创建钱包输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateWalletInput {
    pub user_id: Option<u64>,           // AirAccount扩展：用户ID
    pub alias: Option<String>,          // 钱包别名
    pub chain_id: Option<u64>,          // 目标链ID
}

/// 创建钱包输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateWalletOutput {
    pub wallet_id: Uuid,
    pub address: [u8; 20],
    pub mnemonic: String,               // 注意：生产环境需要安全处理
    pub public_key: Vec<u8>,
}

/// 删除钱包输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveWalletInput {
    pub wallet_id: Uuid,
    pub confirmation_signature: Option<Vec<u8>>, // AirAccount扩展：确认签名
}

/// 删除钱包输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveWalletOutput {
    pub success: bool,
}

/// 地址派生输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeriveAddressInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub chain_id: Option<u64>,          // AirAccount扩展：多链支持
}

/// 地址派生输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
    pub hd_path: String,                // 返回使用的路径
}

/// 交易签名输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignTransactionInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub transaction: EthTransaction,
    pub biometric_proof: Option<BiometricProof>, // AirAccount扩展：生物识别证明
}

/// 交易签名输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
    pub transaction_hash: [u8; 32],     // AirAccount扩展：返回交易哈希
}

/// 生物识别证明
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BiometricProof {
    pub biometric_type: BiometricType,
    pub template_hash: [u8; 32],
    pub fresh_sample_hash: [u8; 32],
    pub timestamp: u64,
}

/// 设置生物识别输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupBiometricInput {
    pub user_id: u64,
    pub biometric_config: BiometricConfig,
    pub device_info: String,
}

/// 设置生物识别输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetupBiometricOutput {
    pub template_id: Uuid,
    pub success: bool,
}

/// 获取钱包信息输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetWalletInfoInput {
    pub wallet_id: Uuid,
    pub include_address: bool,
}

/// 获取钱包信息输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetWalletInfoOutput {
    pub wallet_info: WalletInfo,
}

/// 列出钱包输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListWalletsInput {
    pub user_id: u64,
    pub chain_id: Option<u64>,          // 筛选特定链的钱包
}

/// 列出钱包输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListWalletsOutput {
    pub wallets: Vec<WalletInfo>,
    pub total_count: usize,
}

// ===========================================
// 多签钱包扩展
// ===========================================

/// 多签钱包配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultiSigConfig {
    pub threshold: u8,
    pub owners: Vec<u64>,               // 所有者用户ID列表
    pub daily_limit: Option<u128>,      // 日限额
}

/// 创建多签钱包输入
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateMultiSigWalletInput {
    pub config: MultiSigConfig,
    pub alias: Option<String>,
}

/// 创建多签钱包输出
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateMultiSigWalletOutput {
    pub wallet_id: Uuid,
    pub contract_address: [u8; 20],     // 多签合约地址
    pub deployment_tx_hash: [u8; 32],
}

// ===========================================
// 错误处理
// ===========================================

/// 协议错误类型
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtoError {
    SerializationError(String),
    InvalidParameter(String),
    UnsupportedCommand(u32),
    BiometricVerificationFailed,
    WalletNotFound(Uuid),
    InsufficientPermissions,
    InvalidSignature,
}

impl std::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ProtoError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            ProtoError::UnsupportedCommand(cmd) => write!(f, "Unsupported command: {}", cmd),
            ProtoError::BiometricVerificationFailed => write!(f, "Biometric verification failed"),
            ProtoError::WalletNotFound(id) => write!(f, "Wallet not found: {}", id),
            ProtoError::InsufficientPermissions => write!(f, "Insufficient permissions"),
            ProtoError::InvalidSignature => write!(f, "Invalid signature"),
        }
    }
}

impl std::error::Error for ProtoError {}

// ===========================================
// 工具函数
// ===========================================

/// 序列化命令输入
pub fn serialize_input<T: Serialize>(input: &T) -> Result<Vec<u8>, ProtoError> {
    bincode::serialize(input)
        .map_err(|e| ProtoError::SerializationError(e.to_string()))
}

/// 反序列化命令输出
pub fn deserialize_output<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Result<T, ProtoError> {
    bincode::deserialize(data)
        .map_err(|e| ProtoError::SerializationError(e.to_string()))
}

/// 验证HD钱包路径格式
pub fn validate_hd_path(path: &str) -> Result<(), ProtoError> {
    // 基本格式验证: m/purpose'/coin_type'/account'/change/address_index
    if !path.starts_with("m/") {
        return Err(ProtoError::InvalidParameter("HD path must start with 'm/'".to_string()));
    }
    
    let parts: Vec<&str> = path[2..].split('/').collect();
    if parts.len() < 3 {
        return Err(ProtoError::InvalidParameter("HD path must have at least 3 components".to_string()));
    }
    
    Ok(())
}

/// 验证以太坊地址格式
pub fn validate_eth_address(address: &[u8; 20]) -> bool {
    // 基本长度检查 - 更复杂的验证可以加入checksum
    !address.iter().all(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_command_conversion() {
        let cmd = WalletCommand::CreateWallet;
        let cmd_u32: u32 = cmd.into();
        let cmd_back: WalletCommand = cmd_u32.into();
        assert_eq!(cmd, cmd_back);
    }

    #[test]
    fn test_hd_path_validation() {
        assert!(validate_hd_path("m/44'/60'/0'/0/0").is_ok());
        assert!(validate_hd_path("m/44'/60'/0'").is_ok());
        assert!(validate_hd_path("44'/60'/0'").is_err());
        assert!(validate_hd_path("m/44'").is_err());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let input = CreateWalletInput {
            user_id: Some(123),
            alias: Some("Test Wallet".to_string()),
            chain_id: Some(1),
        };

        let serialized = serialize_input(&input).unwrap();
        let deserialized: CreateWalletInput = deserialize_output(&serialized).unwrap();
        
        assert_eq!(input.user_id, deserialized.user_id);
        assert_eq!(input.alias, deserialized.alias);
        assert_eq!(input.chain_id, deserialized.chain_id);
    }

    #[test]
    fn test_eth_address_validation() {
        let valid_address = [1u8; 20];
        let zero_address = [0u8; 20];
        
        assert!(validate_eth_address(&valid_address));
        assert!(!validate_eth_address(&zero_address));
    }
}