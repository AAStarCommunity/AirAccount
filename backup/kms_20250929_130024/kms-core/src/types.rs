use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Key identifier type
pub type KeyId = [u8; 32];

/// Public key representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub key_data: Vec<u8>,
    pub algorithm: KeyAlgorithm,
}

/// Private key representation (never leaves TEE)
#[derive(Debug, Clone)]
pub struct PrivateKey {
    pub key_data: Vec<u8>,
    pub algorithm: KeyAlgorithm,
}

/// Supported key algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAlgorithm {
    Secp256k1,
    Ed25519,
}

/// Key management operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KmsOperation {
    GenerateKey {
        key_id: KeyId,
        algorithm: KeyAlgorithm,
    },
    GetPublicKey {
        key_id: KeyId,
    },
    Sign {
        key_id: KeyId,
        message_hash: Vec<u8>,
    },
    DeleteKey {
        key_id: KeyId,
    },
}

/// KMS operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KmsResponse {
    KeyGenerated {
        key_id: KeyId,
        public_key: PublicKey,
    },
    PublicKey {
        public_key: PublicKey,
    },
    Signature {
        signature: Vec<u8>,
    },
    KeyDeleted {
        key_id: KeyId,
    },
    Error {
        message: alloc::string::String,
    },
}