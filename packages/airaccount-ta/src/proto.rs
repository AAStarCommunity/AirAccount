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
            _ => Command::HelloWorld, // Default fallback
        }
    }
}

// Simplified structures for now - will add serialization later
#[derive(Debug)]
pub struct HelloWorldOutput {
    pub message: String,
    pub version: String,
}

#[derive(Debug)]
pub struct EchoInput {
    pub message: String,
}

#[derive(Debug)]
pub struct EchoOutput {
    pub echoed_message: String,
}

#[derive(Debug)]
pub struct GetVersionOutput {
    pub version: String,
    pub build_info: String,
}

// Wallet management commands (compatible with eth_wallet)
#[derive(Debug)]
pub struct CreateWalletInput {
    // Empty for now, could add entropy or other parameters
}

#[derive(Debug)]
pub struct CreateWalletOutput {
    pub wallet_id: uuid::Uuid,
    pub mnemonic: String,
}

#[derive(Debug)]
pub struct RemoveWalletInput {
    pub wallet_id: uuid::Uuid,
}

#[derive(Debug)]
pub struct RemoveWalletOutput {
    // Empty response
}

#[derive(Debug)]
pub struct DeriveAddressInput {
    pub wallet_id: uuid::Uuid,
    pub hd_path: String,
}

#[derive(Debug)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: [u8; 65],
}

#[derive(Debug)]
pub struct GetWalletInfoInput {
    pub wallet_id: uuid::Uuid,
}

#[derive(Debug)]
pub struct GetWalletInfoOutput {
    pub wallet_id: uuid::Uuid,
    pub created_at: u64,
    pub derivations_count: u32,
}

// Ethereum transaction structure (compatible with eth_wallet)
#[derive(Debug)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct SignTransactionInput {
    pub wallet_id: uuid::Uuid,
    pub hd_path: String,
    pub transaction: EthTransaction,
}

#[derive(Debug)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
}