// Licensed to AirAccount under the Apache License, Version 2.0
// Secure Storage Manager for TEE environment

use alloc::{string::String, vec::Vec, format};
use optee_utee::{
    trace_println, Error, ErrorKind, Result as TeeResult,
    object, PersistentObject, StorageId,
};
use airaccount_core_logic::wallet::WalletCore;
use serde::{Serialize, Deserialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct SecureWalletData {
    /// Encrypted wallet data
    encrypted_data: Vec<u8>,
    /// Metadata (non-sensitive)
    created_at: u64,
    /// Version for future compatibility
    version: u32,
}

pub struct SecureStorage {
    storage_id: StorageId,
}

impl SecureStorage {
    pub fn new() -> Self {
        trace_println!("[+] Initializing secure storage");
        Self {
            storage_id: StorageId::Private,
        }
    }
    
    pub fn store_wallet(&self, wallet_id: &str, wallet: &WalletCore) -> TeeResult<()> {
        trace_println!("[+] Storing wallet securely: {}", wallet_id);
        
        // Serialize wallet data
        let wallet_data = serde_json::to_vec(wallet)
            .map_err(|_| Error::new(ErrorKind::BadFormat))?;
        
        // Encrypt wallet data (for now using basic approach, should use TEE encryption)
        let encrypted_data = self.encrypt_data(&wallet_data)?;
        
        // Create secure wallet structure
        let secure_data = SecureWalletData {
            encrypted_data,
            created_at: self.get_current_time()?,
            version: 1,
        };
        
        // Serialize secure data
        let serialized_data = serde_json::to_vec(&secure_data)
            .map_err(|_| Error::new(ErrorKind::BadFormat))?;
        
        // Generate object ID from wallet ID
        let object_id = self.generate_object_id(wallet_id);
        
        // Store in persistent storage
        let mut obj = PersistentObject::create(
            self.storage_id,
            &object_id,
            object::Flags::DATA_ONLY,
            None,
            &serialized_data,
        )?;
        
        obj.close()?;
        trace_println!("[+] Wallet stored successfully");
        
        Ok(())
    }
    
    pub fn load_wallet(&self, wallet_id: &str) -> TeeResult<WalletCore> {
        trace_println!("[+] Loading wallet from secure storage: {}", wallet_id);
        
        // Generate object ID from wallet ID
        let object_id = self.generate_object_id(wallet_id);
        
        // Load from persistent storage
        let mut obj = PersistentObject::open(self.storage_id, &object_id, object::Flags::DATA_ONLY)?;
        
        // Read data
        let mut buffer = Vec::new();
        obj.read_to_end(&mut buffer)?;
        obj.close()?;
        
        // Deserialize secure data
        let secure_data: SecureWalletData = serde_json::from_slice(&buffer)
            .map_err(|_| Error::new(ErrorKind::BadFormat))?;
        
        // Decrypt wallet data
        let wallet_data = self.decrypt_data(&secure_data.encrypted_data)?;
        
        // Deserialize wallet
        let wallet: WalletCore = serde_json::from_slice(&wallet_data)
            .map_err(|_| Error::new(ErrorKind::BadFormat))?;
        
        trace_println!("[+] Wallet loaded successfully");
        Ok(wallet)
    }
    
    pub fn remove_wallet(&self, wallet_id: &str) -> TeeResult<()> {
        trace_println!("[+] Removing wallet from secure storage: {}", wallet_id);
        
        // Generate object ID from wallet ID
        let object_id = self.generate_object_id(wallet_id);
        
        // Delete from persistent storage
        PersistentObject::delete(self.storage_id, &object_id)?;
        
        trace_println!("[+] Wallet removed successfully");
        Ok(())
    }
    
    /// Generate object ID for wallet storage
    fn generate_object_id(&self, wallet_id: &str) -> object::ObjectId {
        // Create a deterministic object ID from wallet ID
        // In production, this should be more secure
        let mut id_bytes = [0u8; 32];
        let wallet_bytes = wallet_id.as_bytes();
        let copy_len = core::cmp::min(wallet_bytes.len(), 32);
        id_bytes[..copy_len].copy_from_slice(&wallet_bytes[..copy_len]);
        
        object::ObjectId::from_bytes(&id_bytes)
    }
    
    /// Encrypt data using TEE capabilities
    fn encrypt_data(&self, data: &[u8]) -> TeeResult<Vec<u8>> {
        trace_println!("[+] Encrypting wallet data");
        
        // TODO: Implement proper encryption using OP-TEE crypto operations
        // For now, return as-is (this should be replaced with proper encryption)
        Ok(data.to_vec())
    }
    
    /// Decrypt data using TEE capabilities
    fn decrypt_data(&self, encrypted_data: &[u8]) -> TeeResult<Vec<u8>> {
        trace_println!("[+] Decrypting wallet data");
        
        // TODO: Implement proper decryption using OP-TEE crypto operations
        // For now, return as-is (this should be replaced with proper decryption)
        Ok(encrypted_data.to_vec())
    }
    
    /// Get current time from TEE
    fn get_current_time(&self) -> TeeResult<u64> {
        // TODO: Implement proper time retrieval from TEE
        // For now, return a placeholder
        Ok(0)
    }
}

impl Drop for SecureStorage {
    fn drop(&mut self) {
        trace_println!("[+] Secure storage dropped");
    }
}