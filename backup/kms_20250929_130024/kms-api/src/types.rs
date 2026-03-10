// TA-Only KMS API Types
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ===== AWS KMS-Compatible Request/Response Types =====

// Account Management (based on eth_wallet TA)
#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    #[serde(rename = "Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateAccountResponse {
    #[serde(rename = "AccountMetadata")]
    pub account_metadata: AccountMetadata,
    #[serde(rename = "Mnemonic")]
    pub mnemonic: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountMetadata {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "Arn")]
    pub arn: String,
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "WalletType")]
    pub wallet_type: String,
    #[serde(rename = "HasMnemonic")]
    pub has_mnemonic: bool,
}

#[derive(Debug, Deserialize)]
pub struct DescribeAccountRequest {
    #[serde(rename = "AccountId")]
    pub account_id: String,
}

#[derive(Debug, Serialize)]
pub struct DescribeAccountResponse {
    #[serde(rename = "AccountMetadata")]
    pub account_metadata: AccountMetadata,
}

#[derive(Debug, Serialize)]
pub struct ListAccountsResponse {
    #[serde(rename = "Accounts")]
    pub accounts: Vec<AccountMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct DeriveAddressRequest {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
}

#[derive(Debug, Serialize)]
pub struct DeriveAddressResponse {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
}

#[derive(Debug, Deserialize)]
pub struct EthereumTransaction {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "nonce")]
    pub nonce: u128,
    #[serde(rename = "to")]
    pub to: Option<String>,
    #[serde(rename = "value")]
    pub value: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    #[serde(rename = "gas")]
    pub gas: u128,
    #[serde(rename = "data", default)]
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct SignTransactionRequest {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
    #[serde(rename = "Transaction")]
    pub transaction: EthereumTransaction,
}

#[derive(Debug, Serialize)]
pub struct SignTransactionResponse {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "Signature")]
    pub signature: String,
    #[serde(rename = "TransactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "RawTransaction")]
    pub raw_transaction: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveAccountRequest {
    #[serde(rename = "AccountId")]
    pub account_id: String,
}

#[derive(Debug, Serialize)]
pub struct RemoveAccountResponse {
    #[serde(rename = "AccountId")]
    pub account_id: String,
    #[serde(rename = "Removed")]
    pub removed: bool,
}

// Error Response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    #[serde(rename = "__type")]
    pub error_type: String,
    pub message: String,
}

// ===== Internal TA Integration Types =====

// eth_wallet TA Command mappings
#[derive(Debug, Clone, Copy)]
pub enum TACommand {
    CreateWallet = 0,
    RemoveWallet = 1,
    DeriveAddress = 2,
    SignTransaction = 3,
}

// eth_wallet TA Input types (must match proto/src/in_out.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TACreateWalletInput {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TACreateWalletOutput {
    pub wallet_id: Uuid,
    pub mnemonic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TARemoveWalletInput {
    pub wallet_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TARemoveWalletOutput {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TADeriveAddressInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TADeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TAEthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TASignTransactionInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub transaction: TAEthTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TASignTransactionOutput {
    pub signature: Vec<u8>,
}

// Internal storage (non-sensitive metadata only)
#[derive(Debug, Clone)]
pub struct StoredAccount {
    pub wallet_id: Uuid,
    pub metadata: AccountMetadata,
}

// Conversion helpers
impl From<EthereumTransaction> for TAEthTransaction {
    fn from(tx: EthereumTransaction) -> Self {
        // Convert string addresses to [u8; 20]
        let to = tx.to.as_ref().and_then(|addr| {
            if addr.starts_with("0x") {
                hex::decode(&addr[2..]).ok().and_then(|bytes| {
                    if bytes.len() == 20 {
                        let mut array = [0u8; 20];
                        array.copy_from_slice(&bytes);
                        Some(array)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        });

        // Convert string values to u128
        let value = tx.value.parse().unwrap_or(0);
        let gas_price = tx.gas_price.parse().unwrap_or(0);

        // Convert hex data to bytes
        let data = if tx.data.starts_with("0x") {
            hex::decode(&tx.data[2..]).unwrap_or_default()
        } else {
            tx.data.into_bytes()
        };

        TAEthTransaction {
            chain_id: tx.chain_id,
            nonce: tx.nonce,
            to,
            value,
            gas_price,
            gas: tx.gas,
            data,
        }
    }
}