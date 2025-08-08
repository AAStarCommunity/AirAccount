// AirAccount Wallet implementation - simplified initial version
use crate::proto;

type Result<T> = core::result::Result<T, &'static str>;

const DB_NAME: &str = "airaccount_wallet_db";

#[derive(Debug, Clone)]
pub struct Wallet {
    pub id: uuid::Uuid,
    pub created_at: u64,
    pub derivations_count: u32,
    // Simplified for now - will add crypto later
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
    
    pub fn get_mnemonic(&self) -> Result<&'static str> {
        // TODO: Implement mnemonic generation
        Ok("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
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

// Simplified wallet storage functions - will integrate secure_db later
// use std::collections::HashMap;

// 使用固定大小的数组作为临时存储
const MAX_WALLETS: usize = 10;

#[derive(Debug, Clone)]
struct WalletEntry {
    id: uuid::Uuid,
    wallet: Option<Wallet>,
}

static mut WALLET_STORAGE: [WalletEntry; MAX_WALLETS] = [WalletEntry { 
    id: uuid::Uuid::nil(), 
    wallet: None 
}; MAX_WALLETS];

fn get_storage() -> &'static mut [WalletEntry; MAX_WALLETS] {
    unsafe { &mut WALLET_STORAGE }
}

pub fn save_wallet(wallet: &Wallet) -> Result<()> {
    let storage = get_storage();
    
    // 查找空槽位
    for entry in storage.iter_mut() {
        if entry.wallet.is_none() {
            entry.id = wallet.id;
            entry.wallet = Some(wallet.clone());
            return Ok(());
        }
    }
    
    Err("Storage full")
}

pub fn load_wallet(wallet_id: &uuid::Uuid) -> Result<Wallet> {
    let storage = get_storage();
    
    for entry in storage.iter() {
        if &entry.id == wallet_id {
            if let Some(ref wallet) = entry.wallet {
                return Ok(wallet.clone());
            }
        }
    }
    
    Err("Wallet not found")
}

pub fn delete_wallet(wallet_id: &uuid::Uuid) -> Result<()> {
    let storage = get_storage();
    
    for entry in storage.iter_mut() {
        if &entry.id == wallet_id {
            entry.wallet = None;
            entry.id = uuid::Uuid::nil();
            return Ok(());
        }
    }
    
    Err("Wallet not found")
}