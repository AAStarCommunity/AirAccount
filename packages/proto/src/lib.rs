// Licensed to AirAccount under the Apache License, Version 2.0
// Protocol definitions for AirAccount TA-CA communication

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use serde::{Serialize, Deserialize};

/// Main request structure from CA to TA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirAccountRequest {
    pub request_id: String,
    pub command: WalletCommand,
}

/// Main response structure from TA to CA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirAccountResponse {
    pub request_id: String,
    pub response: WalletResponse,
}

/// Wallet commands that can be sent to TA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletCommand {
    /// Create a new wallet
    CreateWallet,
    /// Derive an address from an existing wallet
    DeriveAddress { 
        wallet_id: String,
    },
    /// Sign a transaction
    SignTransaction { 
        wallet_id: String,
        transaction_data: Vec<u8>,
    },
    /// Remove a wallet
    RemoveWallet { 
        wallet_id: String,
    },
}

/// Response types from TA operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletResponse {
    CreateWallet(CreateWalletResponse),
    DeriveAddress(DeriveAddressResponse),
    SignTransaction(SignTransactionResponse),
    RemoveWallet(RemoveWalletResponse),
}

/// Response for wallet creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWalletResponse {
    pub success: bool,
    pub wallet_id: Option<String>,
    pub mnemonic: Option<String>,
    pub error: Option<String>,
}

/// Response for address derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveAddressResponse {
    pub success: bool,
    pub address: Option<String>,
    pub public_key: Option<String>,
    pub error: Option<String>,
}

/// Response for transaction signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignTransactionResponse {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
}

/// Response for wallet removal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveWalletResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Command IDs for TA communication
pub mod command_ids {
    /// Wallet operations command ID
    pub const WALLET_COMMAND: u32 = 0x1000;
    /// Core operations command ID  
    pub const CORE_OPERATION: u32 = 0x2000;
    /// Security operations command ID
    pub const SECURITY_OPERATION: u32 = 0x3000;
}

/// TA UUID for AirAccount
pub const AIRACCOUNT_TA_UUID: &str = "11223344-5566-7788-99AA-BBCCDDEEFF01";