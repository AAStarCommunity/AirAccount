//! KMS Trusted Application
//!
//! This module will contain the OP-TEE Trusted Application implementation
//! based on the eth_wallet example. The actual OP-TEE integration code
//! will be added after user confirmation to copy from eth_wallet.

#![no_std]
#![no_main]

extern crate alloc;

use kms_core::{KmsOperation, KmsResponse};

// Placeholder TA entry point
// The actual implementation will be added from eth_wallet after confirmation
pub fn handle_kms_operation(_operation: KmsOperation) -> KmsResponse {
    // TODO: Implement actual KMS operations in TEE
    KmsResponse::Error {
        message: "Not implemented yet".into(),
    }
}