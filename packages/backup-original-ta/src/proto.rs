// Protocol definitions compatible with eth_wallet - simplified version
// TA UUID for AirAccount
pub const UUID: &str = "11223344-5566-7788-99aa-bbccddeeff00";

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum Command {
    // Basic commands
    HelloWorld = 0,
    Echo = 1,
    GetVersion = 2,
    
    // Wallet management commands (compatible with eth_wallet)
    CreateWallet = 10,
    RemoveWallet = 11,
    DeriveAddress = 12,
    SignTransaction = 13,
    GetWalletInfo = 14,
    
    // P0安全修复：混合熵源命令
    CreateHybridAccount = 20,
    SignWithHybridKey = 21,
    VerifySecurityState = 22,
}

impl From<u32> for Command {
    fn from(value: u32) -> Self {
        match value {
            0 => Command::HelloWorld,
            1 => Command::Echo,
            2 => Command::GetVersion,
            10 => Command::CreateWallet,
            11 => Command::RemoveWallet,
            12 => Command::DeriveAddress,
            13 => Command::SignTransaction,
            14 => Command::GetWalletInfo,
            20 => Command::CreateHybridAccount,
            21 => Command::SignWithHybridKey,
            22 => Command::VerifySecurityState,
            _ => Command::HelloWorld, // Default fallback
        }
    }
}

// 生产级序列化结构 - 完整的 serde 支持
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloWorldOutput {
    pub message: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoInput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoOutput {
    pub echoed_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetVersionOutput {
    pub version: String,
    pub build_info: String,
}

// Wallet management commands (compatible with eth_wallet)
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWalletInput {
    // Empty for now, could add entropy or other parameters
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWalletOutput {
    pub wallet_id: uuid::Uuid,
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveWalletInput {
    pub wallet_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveWalletOutput {
    // Empty response - could add confirmation message
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressInput {
    pub wallet_id: uuid::Uuid,
    pub hd_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: [u8; 65],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWalletInfoInput {
    pub wallet_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWalletInfoOutput {
    pub wallet_id: uuid::Uuid,
    pub created_at: u64,
    pub derivations_count: u32,
}

// Ethereum transaction structure (compatible with eth_wallet)
#[derive(Debug, Serialize, Deserialize)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTransactionInput {
    pub wallet_id: uuid::Uuid,
    pub hd_path: String,
    pub transaction: EthTransaction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
}

// P0安全修复：混合熵源协议定义
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateHybridAccountInput {
    pub user_email: String,
    pub passkey_public_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateHybridAccountOutput {
    pub account_id: uuid::Uuid,
    pub ethereum_address: [u8; 20],
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignWithHybridKeyInput {
    pub account_id: uuid::Uuid,
    pub transaction_hash: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignWithHybridKeyOutput {
    pub signature: [u8; 65],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifySecurityStateInput {
    // 空输入
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifySecurityStateOutput {
    pub security_verified: bool,
    pub status_message: String,
}