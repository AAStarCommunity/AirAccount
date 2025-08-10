// Licensed to AirAccount under the Apache License, Version 2.0
// TEE Wallet Manager - Secure wallet operations within TrustZone

use alloc::{string::String, vec::Vec};
use optee_utee::{trace_println, Error, ErrorKind, Result as TeeResult};
use airaccount_proto::{WalletResponse, CreateWalletResponse, DeriveAddressResponse, 
                      SignTransactionResponse, RemoveWalletResponse};
use airaccount_core_logic::wallet::WalletCore;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::secure_storage::SecureStorage;

#[derive(ZeroizeOnDrop)]
pub struct TeeWalletManager {
    storage: SecureStorage,
}

impl TeeWalletManager {
    pub fn new() -> Self {
        trace_println!("[+] Creating TEE Wallet Manager");
        Self {
            storage: SecureStorage::new(),
        }
    }
    
    pub fn create_wallet(&self) -> WalletResponse {
        trace_println!("[+] TEE: Creating new wallet");
        
        // Generate new wallet ID
        let wallet_id = Uuid::new_v4().to_string();
        
        // Generate secure entropy within TEE
        let mut entropy = [0u8; 32];
        if let Err(_) = self.generate_secure_entropy(&mut entropy) {
            trace_println!("[-] Failed to generate entropy");
            return WalletResponse::CreateWallet(CreateWalletResponse {
                success: false,
                wallet_id: None,
                mnemonic: None,
                error: Some("Failed to generate secure entropy".into()),
            });
        }
        
        // Create wallet using core logic
        match WalletCore::new_from_entropy(&entropy) {
            Ok(wallet) => {
                // Store wallet securely
                if let Err(_) = self.storage.store_wallet(&wallet_id, &wallet) {
                    trace_println!("[-] Failed to store wallet");
                    return WalletResponse::CreateWallet(CreateWalletResponse {
                        success: false,
                        wallet_id: None,
                        mnemonic: None,
                        error: Some("Failed to store wallet".into()),
                    });
                }
                
                // Generate mnemonic for backup (should be shown on secure display)
                let mnemonic = wallet.get_mnemonic_phrase().unwrap_or_default();
                
                trace_println!("[+] TEE: Wallet created successfully: {}", wallet_id);
                
                WalletResponse::CreateWallet(CreateWalletResponse {
                    success: true,
                    wallet_id: Some(wallet_id),
                    mnemonic: Some(mnemonic),
                    error: None,
                })
            }
            Err(e) => {
                trace_println!("[-] Failed to create wallet: {:?}", e);
                WalletResponse::CreateWallet(CreateWalletResponse {
                    success: false,
                    wallet_id: None,
                    mnemonic: None,
                    error: Some(format!("Wallet creation failed: {:?}", e)),
                })
            }
        }
    }
    
    pub fn derive_address(&self, wallet_id: &str) -> WalletResponse {
        trace_println!("[+] TEE: Deriving address for wallet: {}", wallet_id);
        
        // Load wallet from secure storage
        match self.storage.load_wallet(wallet_id) {
            Ok(wallet) => {
                // Derive Ethereum address
                match wallet.derive_address(0) {
                    Ok(address) => {
                        let public_key = wallet.derive_public_key(0).unwrap_or_default();
                        
                        trace_println!("[+] TEE: Address derived successfully");
                        
                        WalletResponse::DeriveAddress(DeriveAddressResponse {
                            success: true,
                            address: Some(format!("0x{}", hex::encode(address))),
                            public_key: Some(hex::encode(public_key)),
                            error: None,
                        })
                    }
                    Err(e) => {
                        trace_println!("[-] Failed to derive address: {:?}", e);
                        WalletResponse::DeriveAddress(DeriveAddressResponse {
                            success: false,
                            address: None,
                            public_key: None,
                            error: Some(format!("Address derivation failed: {:?}", e)),
                        })
                    }
                }
            }
            Err(_) => {
                trace_println!("[-] Wallet not found: {}", wallet_id);
                WalletResponse::DeriveAddress(DeriveAddressResponse {
                    success: false,
                    address: None,
                    public_key: None,
                    error: Some("Wallet not found".into()),
                })
            }
        }
    }
    
    pub fn sign_transaction(&self, wallet_id: &str, transaction_data: &[u8]) -> WalletResponse {
        trace_println!("[+] TEE: Signing transaction for wallet: {}", wallet_id);
        
        // Load wallet from secure storage
        match self.storage.load_wallet(wallet_id) {
            Ok(wallet) => {
                // Sign transaction within TEE
                match wallet.sign_transaction(transaction_data) {
                    Ok(signature) => {
                        trace_println!("[+] TEE: Transaction signed successfully");
                        
                        WalletResponse::SignTransaction(SignTransactionResponse {
                            success: true,
                            signature: Some(hex::encode(signature)),
                            error: None,
                        })
                    }
                    Err(e) => {
                        trace_println!("[-] Failed to sign transaction: {:?}", e);
                        WalletResponse::SignTransaction(SignTransactionResponse {
                            success: false,
                            signature: None,
                            error: Some(format!("Transaction signing failed: {:?}", e)),
                        })
                    }
                }
            }
            Err(_) => {
                trace_println!("[-] Wallet not found: {}", wallet_id);
                WalletResponse::SignTransaction(SignTransactionResponse {
                    success: false,
                    signature: None,
                    error: Some("Wallet not found".into()),
                })
            }
        }
    }
    
    pub fn remove_wallet(&self, wallet_id: &str) -> WalletResponse {
        trace_println!("[+] TEE: Removing wallet: {}", wallet_id);
        
        match self.storage.remove_wallet(wallet_id) {
            Ok(_) => {
                trace_println!("[+] TEE: Wallet removed successfully");
                
                WalletResponse::RemoveWallet(RemoveWalletResponse {
                    success: true,
                    error: None,
                })
            }
            Err(_) => {
                trace_println!("[-] Failed to remove wallet");
                
                WalletResponse::RemoveWallet(RemoveWalletResponse {
                    success: false,
                    error: Some("Failed to remove wallet".into()),
                })
            }
        }
    }
    
    /// Generate cryptographically secure entropy within TEE
    fn generate_secure_entropy(&self, entropy: &mut [u8]) -> TeeResult<()> {
        trace_println!("[+] Generating secure entropy in TEE");
        
        // Use OP-TEE's secure random number generator
        optee_utee::Random::generate(entropy)?;
        
        Ok(())
    }
}

impl Drop for TeeWalletManager {
    fn drop(&mut self) {
        trace_println!("[+] TEE Wallet Manager dropped");
    }
}