// AirAccount Wallet implementation - simplified initial version
use crate::proto;

type Result<T> = core::result::Result<T, &'static str>;

const DB_NAME: &str = "airaccount_wallet_db";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub id: uuid::Uuid,
    pub created_at: u64,
    pub derivations_count: u32,
    // Will add cryptographic fields later
}

impl Wallet {
    pub fn new() -> Result<Self> {
        // Simplified wallet creation - will add crypto later
        let wallet = Wallet {
            id: uuid::Uuid::new_v4(),
            created_at: get_current_timestamp(),
            derivations_count: 0,
        };
        
        Ok(wallet)
    }
    
    pub fn get_id(&self) -> uuid::Uuid {
        self.id
    }
    
    pub fn get_mnemonic(&self) -> Result<String> {
        // TODO: Implement real mnemonic generation with BIP39
        Ok("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string())
    }
    
    pub fn derive_address(&mut self, _hd_path: &str) -> Result<([u8; 20], [u8; 65])> {
        // TODO: Implement real address derivation
        self.derivations_count += 1;
        
        // Mock address and public key for now
        let address = [0u8; 20];
        let public_key = [4u8; 65]; // 0x04 prefix + 32 bytes x + 32 bytes y
        
        Ok((address, public_key))
    }
    
    pub fn sign_transaction(&mut self, _hd_path: &str, _transaction: &proto::EthTransaction) -> Result<Vec<u8>> {
        // TODO: Implement real transaction signing
        Ok(vec![0u8; 65]) // Mock signature
    }
}

// Helper functions
fn get_current_timestamp() -> u64 {
    // TODO: Get proper timestamp in TEE environment
    42 // Mock timestamp for now
}

// 生产级安全存储 - 使用 secure_db 替代不安全的内存存储
use secure_db::{SecureStorageClient, SecureStorageError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// 使用 secure_db 作为安全后端存储
static mut SECURE_CLIENT: Option<SecureStorageClient> = None;

fn get_secure_client() -> &'static mut SecureStorageClient {
    unsafe {
        if SECURE_CLIENT.is_none() {
            SECURE_CLIENT = Some(SecureStorageClient::new(DB_NAME));
        }
        SECURE_CLIENT.as_mut().unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct WalletMetadata {
    wallet_ids: Vec<uuid::Uuid>,
    created_count: u32,
}

impl Default for WalletMetadata {
    fn default() -> Self {
        WalletMetadata {
            wallet_ids: Vec::new(),
            created_count: 0,
        }
    }
}

const WALLET_METADATA_KEY: &str = "wallet_metadata";

fn load_metadata() -> Result<WalletMetadata> {
    let client = get_secure_client();
    match client.get::<WalletMetadata>(WALLET_METADATA_KEY) {
        Ok(Some(metadata)) => Ok(metadata),
        Ok(None) => Ok(WalletMetadata::default()),
        Err(_) => Ok(WalletMetadata::default()),
    }
}

fn save_metadata(metadata: &WalletMetadata) -> Result<()> {
    let client = get_secure_client();
    client.set(WALLET_METADATA_KEY, metadata)
        .map_err(|_| "Failed to save metadata")?;
    Ok(())
}

pub fn save_wallet(wallet: &Wallet) -> Result<()> {
    let client = get_secure_client();
    let wallet_key = format!("wallet_{}", wallet.id);
    
    // 保存钱包数据
    client.set(&wallet_key, wallet)
        .map_err(|_| "Failed to save wallet")?;
    
    // 更新元数据
    let mut metadata = load_metadata()?;
    if !metadata.wallet_ids.contains(&wallet.id) {
        metadata.wallet_ids.push(wallet.id);
        metadata.created_count += 1;
        save_metadata(&metadata)?;
    }
    
    Ok(())
}

pub fn load_wallet(wallet_id: &uuid::Uuid) -> Result<Wallet> {
    let client = get_secure_client();
    let wallet_key = format!("wallet_{}", wallet_id);
    
    match client.get::<Wallet>(&wallet_key) {
        Ok(Some(wallet)) => Ok(wallet),
        Ok(None) => Err("Wallet not found"),
        Err(_) => Err("Failed to load wallet"),
    }
}

pub fn delete_wallet(wallet_id: &uuid::Uuid) -> Result<()> {
    let client = get_secure_client();
    let wallet_key = format!("wallet_{}", wallet_id);
    
    // 删除钱包数据
    client.remove(&wallet_key)
        .map_err(|_| "Failed to delete wallet")?;
    
    // 更新元数据
    let mut metadata = load_metadata()?;
    metadata.wallet_ids.retain(|id| id != wallet_id);
    save_metadata(&metadata)?;
    
    Ok(())
}

pub fn list_wallets() -> Result<Vec<uuid::Uuid>> {
    let metadata = load_metadata()?;
    Ok(metadata.wallet_ids)
}

pub fn get_wallet_count() -> Result<u32> {
    let metadata = load_metadata()?;
    Ok(metadata.created_count)
}