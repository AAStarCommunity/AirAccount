// KMS Service Implementation
use crate::types::*;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Utc;
use secp256k1::{Secp256k1, SecretKey, PublicKey};
use sha3::{Digest, Sha3_256};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// Include wallet functionality
mod hash {
    include!("../../kms-ta-test/src/hash.rs");
}
mod mock_tee {
    include!("../../kms-ta-test/src/mock_tee.rs");
}
mod wallet {
    include!("../../kms-ta-test/src/wallet.rs");
}

use wallet::Wallet;

pub struct KmsService {
    keys: Arc<Mutex<HashMap<String, StoredKey>>>,
    region: String,
    account_id: String,
}

impl KmsService {
    pub fn new(region: String, account_id: String) -> Self {
        Self {
            keys: Arc::new(Mutex::new(HashMap::new())),
            region,
            account_id,
        }
    }

    pub async fn create_key(&self, request: CreateKeyRequest) -> Result<CreateKeyResponse> {
        let key_id = Uuid::new_v4().to_string();
        let arn = format!(
            "arn:aws:kms:{}:{}:key/{}",
            self.region, self.account_id, key_id
        );

        // Generate key using our wallet functionality
        let wallet = Wallet::new()?;
        let hd_path = "m/44'/60'/0'/0/0"; // Standard derivation path
        let (_, public_key_bytes) = wallet.derive_address(hd_path)?;
        let private_key_bytes = wallet.derive_prv_key(hd_path)?;

        let metadata = KeyMetadata {
            key_id: key_id.clone(),
            arn: arn.clone(),
            creation_date: Utc::now(),
            enabled: true,
            description: request.description.unwrap_or_else(|| "KMS generated key".to_string()),
            key_usage: request.key_usage.clone(),
            key_spec: request.key_spec.clone(),
            origin: request.origin.clone(),
        };

        let stored_key = StoredKey {
            id: key_id.clone(),
            arn: arn.clone(),
            private_key: private_key_bytes,
            public_key: public_key_bytes,
            metadata: metadata.clone(),
        };

        let mut keys = self.keys.lock().unwrap();
        keys.insert(key_id, stored_key);

        Ok(CreateKeyResponse {
            key_metadata: metadata,
        })
    }

    pub async fn sign(&self, request: SignRequest) -> Result<SignResponse> {
        let keys = self.keys.lock().unwrap();
        let stored_key = keys.get(&request.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", request.key_id))?;

        // Decode the message
        let message_bytes = BASE64.decode(&request.message)
            .map_err(|e| anyhow!("Invalid base64 message: {}", e))?;

        // Create signature using secp256k1
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&stored_key.private_key[0..32])
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;

        // Hash the message if needed
        let hash = match request.message_type {
            MessageType::Raw => {
                let mut hasher = Sha3_256::new();
                hasher.update(&message_bytes);
                hasher.finalize().to_vec()
            }
            MessageType::Digest => message_bytes,
        };

        // Create ECDSA signature
        let message = secp256k1::Message::from_slice(&hash)
            .map_err(|e| anyhow!("Invalid message hash: {}", e))?;

        let signature = secp.sign_ecdsa(&message, &secret_key);
        let signature_bytes = signature.serialize_compact().to_vec();

        Ok(SignResponse {
            key_id: request.key_id,
            signature: BASE64.encode(&signature_bytes),
            signing_algorithm: request.signing_algorithm,
        })
    }

    pub async fn get_public_key(&self, request: GetPublicKeyRequest) -> Result<GetPublicKeyResponse> {
        let keys = self.keys.lock().unwrap();
        let stored_key = keys.get(&request.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", request.key_id))?;

        // Encode public key as base64
        let public_key_b64 = BASE64.encode(&stored_key.public_key);

        Ok(GetPublicKeyResponse {
            key_id: request.key_id,
            public_key: public_key_b64,
            key_usage: stored_key.metadata.key_usage.clone(),
            key_spec: stored_key.metadata.key_spec.clone(),
            signing_algorithms: vec![SigningAlgorithm::EcdsaSha256],
        })
    }

    pub async fn list_keys(&self) -> Result<Vec<KeyMetadata>> {
        let keys = self.keys.lock().unwrap();
        Ok(keys.values().map(|k| k.metadata.clone()).collect())
    }
}