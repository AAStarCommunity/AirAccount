//! KMS Core Logic
//!
//! Hardware-agnostic key management functionality that can be used
//! across different TEE implementations (ARM TrustZone, Intel SGX, etc.)

#![no_std]
#![cfg_attr(feature = "std", allow(unused_imports))]

extern crate alloc;

pub mod crypto;
pub mod error;
pub mod types;

pub use error::{KmsError, Result};
pub use types::*;