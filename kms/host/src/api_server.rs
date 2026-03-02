// KMS API Server
// Real TA integration only - requires OP-TEE environment
// Deploy to QEMU for testing, production-ready architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};
use warp::Filter;
use hex;

// Import from kms library and proto
use kms::ta_client::TeeHandle;
use kms::address_cache::{update_address_entry, lookup_address};
use proto;

/// Estimated seconds per TEE operation with persistent session
const TEE_OP_ESTIMATE_SECS: u64 = 1;

// ========================================
// AWS KMS 兼容的数据结构
// ========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
    #[serde(rename = "Origin")]
    pub origin: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
    #[serde(rename = "Mnemonic")]
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DescribeKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DescribeKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListKeysRequest {
    #[serde(rename = "Limit", skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(rename = "Marker", skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListKeysResponse {
    #[serde(rename = "Keys")]
    pub keys: Vec<KeyListEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyListEntry {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "KeyArn")]
    pub key_arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "PublicKey", skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none")]
    pub derivation_path: Option<String>,
    #[serde(rename = "Arn")]
    pub arn: String,
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
    #[serde(rename = "Origin")]
    pub origin: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressResponse {
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignRequest {
    // New: Address-based lookup (priority)
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none", default)]
    pub address: Option<String>,
    // Old: KeyId + DerivationPath (backward compatibility)
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none", default)]
    pub derivation_path: Option<String>,
    // Transaction signing mode (original)
    #[serde(rename = "Transaction", skip_serializing_if = "Option::is_none", default)]
    pub transaction: Option<EthereumTransaction>,
    // Message signing mode (new)
    #[serde(rename = "Message", skip_serializing_if = "Option::is_none", default)]
    pub message: Option<String>,
    #[serde(rename = "SigningAlgorithm", skip_serializing_if = "Option::is_none", default)]
    pub signing_algorithm: Option<String>,
    /// Optional PassKey assertion for user verification
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignResponse {
    #[serde(rename = "Signature")]
    pub signature: String,
    #[serde(rename = "TransactionHash")]
    pub transaction_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignHashRequest {
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none", default)]
    pub address: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none", default)]
    pub derivation_path: Option<String>,
    #[serde(rename = "Hash")]
    pub hash: String,
    #[serde(rename = "SigningAlgorithm", skip_serializing_if = "Option::is_none", default)]
    pub signing_algorithm: Option<String>,
    /// Optional PassKey assertion for user verification
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignHashResponse {
    #[serde(rename = "Signature")]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "PendingWindowInDays", skip_serializing_if = "Option::is_none")]
    pub pending_window_in_days: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "DeletionDate")]
    pub deletion_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthereumTransaction {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    pub nonce: u64,
    pub to: String,
    pub value: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    pub gas: u64,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyStatusResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Status")]
    pub status: String,  // "creating" | "deriving" | "ready" | "error"
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "PublicKey", skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none")]
    pub derivation_path: Option<String>,
    #[serde(rename = "Error", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueStatusResponse {
    pub queue_depth: usize,
    pub estimated_wait_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WarmupCacheRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WarmupCacheResponse {
    #[serde(rename = "Cached")]
    pub cached: bool,
    #[serde(rename = "CacheSize")]
    pub cache_size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterPasskeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    /// P-256 public key in uncompressed hex (0x04...)
    #[serde(rename = "PasskeyPublicKey")]
    pub passkey_public_key: String,
    /// Optional credential ID from WebAuthn
    #[serde(rename = "CredentialId", skip_serializing_if = "Option::is_none", default)]
    pub credential_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterPasskeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Registered")]
    pub registered: bool,
}

/// WebAuthn assertion data attached to Sign/SignHash requests
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PasskeyAssertion {
    /// authenticatorData in hex
    #[serde(rename = "AuthenticatorData")]
    pub authenticator_data: String,
    /// SHA-256(clientDataJSON) in hex
    #[serde(rename = "ClientDataHash")]
    pub client_data_hash: String,
    /// ECDSA signature in hex (DER or r||s 64 bytes)
    #[serde(rename = "Signature")]
    pub signature: String,
}

/// Parse DER-encoded ECDSA signature into (r, s) 32-byte arrays
fn parse_der_signature(der: &[u8]) -> Result<([u8; 32], [u8; 32])> {
    if der.len() < 8 || der[0] != 0x30 {
        return Err(anyhow!("Invalid DER signature"));
    }
    let mut pos = 2; // skip 0x30 + total_len
    if der[pos] != 0x02 {
        return Err(anyhow!("Expected INTEGER tag for r"));
    }
    pos += 1;
    let r_len = der[pos] as usize;
    pos += 1;
    let r_raw = &der[pos..pos + r_len];
    pos += r_len;
    if der[pos] != 0x02 {
        return Err(anyhow!("Expected INTEGER tag for s"));
    }
    pos += 1;
    let s_len = der[pos] as usize;
    pos += 1;
    let s_raw = &der[pos..pos + s_len];

    // Pad/trim to 32 bytes (DER integers may have leading zero for sign)
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    if r_raw.len() > 32 {
        r.copy_from_slice(&r_raw[r_raw.len() - 32..]);
    } else {
        r[32 - r_raw.len()..].copy_from_slice(r_raw);
    }
    if s_raw.len() > 32 {
        s.copy_from_slice(&s_raw[s_raw.len() - 32..]);
    } else {
        s[32 - s_raw.len()..].copy_from_slice(s_raw);
    }
    Ok((r, s))
}

// ========================================
// KMS API Server
// ========================================

pub struct KmsApiServer {
    metadata_store: Arc<RwLock<HashMap<String, KeyMetadata>>>,
    tee: TeeHandle,
    /// Track key derivation status: key_id -> "deriving" | "ready" | "error:msg"
    key_status: Arc<RwLock<HashMap<String, String>>>,
    /// PassKey public keys: key_id -> passkey_pubkey_bytes (uncompressed P-256, 65 bytes)
    passkey_store: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl KmsApiServer {
    pub fn new() -> Self {
        Self {
            metadata_store: Arc::new(RwLock::new(HashMap::new())),
            tee: TeeHandle::new(),
            key_status: Arc::new(RwLock::new(HashMap::new())),
            passkey_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("📝 KMS CreateKey API called");

        let wallet_id = self.tee.create_wallet().await?;

        let key_metadata = KeyMetadata {
            key_id: wallet_id.to_string(),
            address: None,
            public_key: None,
            derivation_path: None,
            arn: format!("arn:aws:kms:region:account:key/{}", wallet_id),
            creation_date: Utc::now(),
            enabled: true,
            description: req.description.clone(),
            key_usage: req.key_usage,
            key_spec: req.key_spec,
            origin: req.origin,
        };

        let mut store = self.metadata_store.write().await;
        store.insert(wallet_id.to_string(), key_metadata.clone());
        drop(store);

        // Mark as deriving and spawn background address derivation
        {
            let mut status = self.key_status.write().await;
            status.insert(wallet_id.to_string(), "deriving".to_string());
        }
        let metadata_store = self.metadata_store.clone();
        let key_status = self.key_status.clone();
        let tee = self.tee.clone();
        tokio::spawn(async move {
            let derivation_result = tee.derive_address_auto(Some(wallet_id)).await;

            match derivation_result {
                Ok((_wid, address_bytes, public_key, derivation_path)) => {
                    let address_hex = format!("0x{}", hex::encode(&address_bytes));
                    let pubkey_hex = format!("0x{}", hex::encode(&public_key));
                    println!("✅ Background derivation done for {}: {}", wallet_id, address_hex);

                    let mut store = metadata_store.write().await;
                    if let Some(meta) = store.get_mut(&wallet_id.to_string()) {
                        meta.address = Some(address_hex.clone());
                        meta.public_key = Some(pubkey_hex.clone());
                        meta.derivation_path = Some(derivation_path.clone());
                    }
                    drop(store);

                    let _ = update_address_entry(&address_hex, wallet_id, &derivation_path, &pubkey_hex);

                    let mut status = key_status.write().await;
                    status.insert(wallet_id.to_string(), "ready".to_string());
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    eprintln!("❌ Background derivation failed for {}: {}", wallet_id, err_msg);
                    let mut status = key_status.write().await;
                    status.insert(wallet_id.to_string(), format!("error:{}", err_msg));
                }
            }
        });

        Ok(CreateKeyResponse {
            key_metadata,
            mnemonic: "[MNEMONIC_IN_SECURE_WORLD]".to_string(),
        })
    }

    pub async fn describe_key(&self, req: DescribeKeyRequest) -> Result<DescribeKeyResponse> {
        println!("📝 KMS DescribeKey API called for key: {}", req.key_id);

        let store = self.metadata_store.read().await;
        let key_metadata = store.get(&req.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?
            .clone();

        Ok(DescribeKeyResponse { key_metadata })
    }

    pub async fn list_keys(&self, _req: ListKeysRequest) -> Result<ListKeysResponse> {
        println!("📝 KMS ListKeys API called");

        let store = self.metadata_store.read().await;
        let keys: Vec<KeyListEntry> = store.iter()
            .map(|(key_id, metadata)| KeyListEntry {
                key_id: key_id.clone(),
                key_arn: metadata.arn.clone(),
            })
            .collect();

        Ok(ListKeysResponse { keys })
    }

    pub async fn key_status(&self, key_id: &str) -> Result<KeyStatusResponse> {
        let status_store = self.key_status.read().await;
        let status_str = status_store.get(key_id).cloned();
        drop(status_store);

        let store = self.metadata_store.read().await;
        let metadata = store.get(key_id).cloned();
        drop(store);

        let metadata = metadata.ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

        let (status, address, public_key, derivation_path, error) = match status_str.as_deref() {
            Some("ready") => (
                "ready",
                metadata.address.clone(),
                metadata.public_key.clone(),
                metadata.derivation_path.clone(),
                None,
            ),
            Some(s) if s.starts_with("error:") => (
                "error", None, None, None, Some(s[6..].to_string()),
            ),
            Some("deriving") => ("deriving", None, None, None, None),
            _ => {
                if metadata.address.is_some() {
                    ("ready", metadata.address.clone(), metadata.public_key.clone(), metadata.derivation_path.clone(), None)
                } else {
                    ("creating", None, None, None, None)
                }
            }
        };

        Ok(KeyStatusResponse {
            key_id: key_id.to_string(),
            status: status.to_string(),
            address,
            public_key,
            derivation_path,
            error,
        })
    }

    pub fn queue_status(&self) -> QueueStatusResponse {
        let depth = self.tee.pending_count();
        QueueStatusResponse {
            queue_depth: depth,
            estimated_wait_seconds: depth as u64 * TEE_OP_ESTIMATE_SECS,
        }
    }

    pub async fn warmup_cache(&self, req: WarmupCacheRequest) -> Result<WarmupCacheResponse> {
        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        let cache_size = self.tee.warmup_cache(wallet_uuid).await?;
        println!("🔥 Warmup cache for {}: cache_size={}", req.key_id, cache_size);
        Ok(WarmupCacheResponse {
            cached: true,
            cache_size,
        })
    }

    pub async fn register_passkey(&self, req: RegisterPasskeyRequest) -> Result<RegisterPasskeyResponse> {
        println!("📝 KMS RegisterPasskey API called for key: {}", req.key_id);

        // Verify key exists
        let store = self.metadata_store.read().await;
        if !store.contains_key(&req.key_id) {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }
        drop(store);

        // Decode public key from hex
        let pubkey_hex = req.passkey_public_key.trim_start_matches("0x");
        let pubkey_bytes = hex::decode(pubkey_hex)
            .map_err(|e| anyhow!("Invalid passkey public key hex: {}", e))?;

        if pubkey_bytes.len() != 65 || pubkey_bytes[0] != 0x04 {
            return Err(anyhow!(
                "PassKey public key must be 65 bytes uncompressed (0x04 || x || y), got {} bytes",
                pubkey_bytes.len()
            ));
        }

        let mut pk_store = self.passkey_store.write().await;
        pk_store.insert(req.key_id.clone(), pubkey_bytes);

        Ok(RegisterPasskeyResponse {
            key_id: req.key_id,
            registered: true,
        })
    }

    /// Verify passkey assertion in TEE before allowing sign operations.
    /// Returns Ok(()) if no passkey registered (backward compatible) or verification passes.
    async fn verify_passkey_guard(
        &self,
        key_id: &str,
        wallet_uuid: uuid::Uuid,
        passkey: Option<&PasskeyAssertion>,
    ) -> Result<()> {
        let pk_store = self.passkey_store.read().await;
        let pubkey_bytes = pk_store.get(key_id).cloned();
        drop(pk_store);

        // No passkey registered: skip verification (backward compatible)
        let pubkey = match pubkey_bytes {
            Some(pk) => pk,
            None => return Ok(()),
        };

        // Passkey registered but no assertion provided: reject
        let assertion = passkey.ok_or_else(|| anyhow!(
            "This key requires PassKey verification. Provide Passkey assertion in request."
        ))?;

        // Parse assertion fields
        let auth_data = hex::decode(assertion.authenticator_data.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid authenticator_data hex: {}", e))?;
        let cdh_bytes = hex::decode(assertion.client_data_hash.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid client_data_hash hex: {}", e))?;
        if cdh_bytes.len() != 32 {
            return Err(anyhow!("client_data_hash must be 32 bytes"));
        }
        let mut client_data_hash = [0u8; 32];
        client_data_hash.copy_from_slice(&cdh_bytes);

        // Parse signature: try raw r||s (64 bytes) first, then DER
        let sig_bytes = hex::decode(assertion.signature.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid signature hex: {}", e))?;

        let (sig_r, sig_s) = if sig_bytes.len() == 64 {
            // Raw r || s format
            let mut r = [0u8; 32];
            let mut s = [0u8; 32];
            r.copy_from_slice(&sig_bytes[..32]);
            s.copy_from_slice(&sig_bytes[32..]);
            (r, s)
        } else {
            // DER format: 0x30 len 0x02 r_len r 0x02 s_len s
            parse_der_signature(&sig_bytes)?
        };

        // Verify in TEE
        let valid = self.tee.verify_passkey(
            wallet_uuid, &pubkey, &auth_data, &client_data_hash, &sig_r, &sig_s,
        ).await?;
        if !valid {
            return Err(anyhow!("PassKey verification failed"));
        }

        Ok(())
    }

    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("📝 KMS DeriveAddress API called for key: {}", req.key_id);

        let store = self.metadata_store.read().await;
        if !store.contains_key(&req.key_id) {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }
        drop(store);

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        let address_bytes = self.tee.derive_address(wallet_uuid, &req.derivation_path).await?;

        let address = format!("0x{}", hex::encode(&address_bytes));

        Ok(DeriveAddressResponse {
            address,
            public_key: "[PUBKEY_FROM_TA]".to_string(),
        })
    }

    pub async fn sign(&self, req: SignRequest) -> Result<SignResponse> {
        // Resolve wallet_id and derivation_path (support both Address and KeyId modes)
        let (wallet_uuid, derivation_path) = if let Some(ref address) = req.address {
            println!("📝 KMS Sign API called with Address: {}", address);

            let metadata = lookup_address(address)?
                .ok_or_else(|| anyhow!(
                    "Address not found in cache: {}. \
                     Use 'kms-recovery-cli rebuild-cache --wallet-id <id>' to recover, \
                     or provide KeyId + DerivationPath directly.",
                    address
                ))?;

            (metadata.wallet_id, metadata.derivation_path.clone())
        } else if let (Some(ref key_id), Some(ref path)) = (req.key_id.as_ref(), req.derivation_path.as_ref()) {
            println!("📝 KMS Sign API called with KeyId: {}, Path: {}", key_id, path);

            let store = self.metadata_store.read().await;
            if !store.contains_key(&key_id.to_string()) {
                return Err(anyhow!("Key not found: {}", key_id));
            }
            drop(store);

            (Uuid::parse_str(key_id)?, path.to_string())
        } else {
            return Err(anyhow!(
                "Must provide either Address or (KeyId + DerivationPath)"
            ));
        };

        // PassKey verification guard (if registered for this key)
        self.verify_passkey_guard(
            &wallet_uuid.to_string(), wallet_uuid, req.passkey.as_ref(),
        ).await?;

        // Prepare sign payload
        let signature = if let Some(transaction) = req.transaction {
            println!("  📝 Transaction signing mode");
            let to_bytes = if transaction.to.starts_with("0x") {
                hex::decode(&transaction.to[2..])
            } else {
                hex::decode(&transaction.to)
            }?;
            let mut to_array = [0u8; 20];
            to_array.copy_from_slice(&to_bytes[..20]);

            let data = if transaction.data.is_empty() {
                vec![]
            } else {
                hex::decode(&transaction.data.trim_start_matches("0x"))?
            };

            let eth_transaction = proto::EthTransaction {
                chain_id: transaction.chain_id,
                nonce: transaction.nonce as u128,
                to: Some(to_array),
                value: u128::from_str_radix(&transaction.value.trim_start_matches("0x"), 16)?,
                gas_price: u128::from_str_radix(&transaction.gas_price.trim_start_matches("0x"), 16)?,
                gas: transaction.gas as u128,
                data,
            };
            self.tee.sign_transaction(wallet_uuid, &derivation_path, eth_transaction).await?
        } else if let Some(message) = req.message {
            println!("  📝 Message signing mode");
            let message_bytes = if message.starts_with("0x") {
                hex::decode(&message[2..])?
            } else {
                base64::decode(&message).unwrap_or_else(|_| message.as_bytes().to_vec())
            };
            self.tee.sign_message(wallet_uuid, &derivation_path, &message_bytes).await?
        } else {
            return Err(anyhow!("Either Transaction or Message must be provided"));
        };

        Ok(SignResponse {
            signature: hex::encode(&signature),
            transaction_hash: "[TX_HASH_OR_MESSAGE_HASH]".to_string(),
        })
    }

    pub async fn sign_hash(&self, req: SignHashRequest) -> Result<SignHashResponse> {
        // 支持三种方式:
        // 1. Address (优先级最高,从缓存查找)
        // 2. KeyId + DerivationPath (手动指定路径)
        // 3. KeyId only (自动使用默认路径)
        let (wallet_uuid, derivation_path) = if let Some(address) = &req.address {
            println!("📝 KMS SignHash API called with Address: {}", address);

            // 从缓存查找 Address → (wallet_id, path)
            let metadata = lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found in cache: {}", address))?;

            (metadata.wallet_id, metadata.derivation_path)
        } else if let Some(key_id) = &req.key_id {
            println!("📝 KMS SignHash API called with KeyId: {}", key_id);

            // 读取 metadata_store 获取默认路径
            let store = self.metadata_store.read().await;
            let metadata = store.get(key_id)
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            // 使用提供的路径,或者使用默认路径
            let derivation_path = req.derivation_path
                .or_else(|| metadata.derivation_path.clone())
                .ok_or_else(|| anyhow!("No derivation path available for this key"))?;

            drop(store);

            (Uuid::parse_str(key_id)?, derivation_path)
        } else {
            return Err(anyhow!("Either KeyId or Address must be provided"));
        };

        // PassKey verification guard (if registered for this key)
        self.verify_passkey_guard(
            &wallet_uuid.to_string(), wallet_uuid, req.passkey.as_ref(),
        ).await?;

        let hash_bytes = if req.hash.starts_with("0x") {
            hex::decode(&req.hash[2..])?
        } else {
            hex::decode(&req.hash)?
        };

        if hash_bytes.len() != 32 {
            return Err(anyhow!("Hash must be exactly 32 bytes, got {} bytes", hash_bytes.len()));
        }

        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(&hash_bytes);

        let signature = self.tee.sign_hash(wallet_uuid, &derivation_path, &hash_array).await?;

        Ok(SignHashResponse {
            signature: hex::encode(&signature),
        })
    }

    pub async fn get_public_key(&self, req: GetPublicKeyRequest) -> Result<GetPublicKeyResponse> {
        println!("📝 KMS GetPublicKey API called for key: {}", req.key_id);

        // 验证密钥存在并获取元数据
        let store = self.metadata_store.read().await;
        let metadata = store.get(&req.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        let key_usage = metadata.key_usage.clone();
        let key_spec = metadata.key_spec.clone();
        drop(store);

        // 调用 TaClient GetPublicKey (目前返回占位符)
        // TODO: 实现从TA获取真实公钥
        let public_key = "[PUBLIC_KEY_BASE64_ENCODED]".to_string();

        Ok(GetPublicKeyResponse {
            key_id: req.key_id,
            public_key,
            key_usage,
            key_spec,
        })
    }

    pub async fn delete_key(&self, req: DeleteKeyRequest) -> Result<DeleteKeyResponse> {
        println!("📝 KMS ScheduleKeyDeletion API called for key: {}", req.key_id);

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        self.tee.remove_wallet(wallet_uuid).await?;

        let mut store = self.metadata_store.write().await;
        store.remove(&req.key_id);

        let days = req.pending_window_in_days.unwrap_or(7);
        let deletion_date = Utc::now() + chrono::Duration::days(days as i64);

        Ok(DeleteKeyResponse {
            key_id: req.key_id,
            deletion_date,
        })
    }
}

// ========================================
// HTTP Server Routes
// ========================================

async fn health_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kms-api",
        "version": "0.1.0",
        "ta_mode": "real",
        "endpoints": {
            "POST": ["/CreateKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/RegisterPasskey", "/WarmupCache"],
            "GET": ["/health", "/KeyStatus?KeyId=xxx", "/QueueStatus"]
        }
    })))
}

async fn handle_create_key(
    body: CreateKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.create_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("CreateKey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_describe_key(
    body: DescribeKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.describe_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DescribeKey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_list_keys(
    body: ListKeysRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.list_keys(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ListKeys error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_derive_address(
    body: DeriveAddressRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.derive_address(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DeriveAddress error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign(
    body: SignRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.sign(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("Sign error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_hash(
    body: SignHashRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.sign_hash(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("SignHash error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_get_public_key(
    body: GetPublicKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.get_public_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("GetPublicKey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_delete_key(
    body: DeleteKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.delete_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ScheduleKeyDeletion error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_register_passkey(
    body: RegisterPasskeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.register_passkey(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("RegisterPasskey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_warmup_cache(
    body: WarmupCacheRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.warmup_cache(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("WarmupCache error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_key_status(
    key_id: String,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.key_status(&key_id).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("KeyStatus error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_queue_status(
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&server.queue_status()))
}

#[derive(Debug)]
struct ApiError(String);

impl warp::reject::Reject for ApiError {}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    if let Some(api_error) = err.find::<ApiError>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": api_error.0
            })),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "Internal server error"
            })),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

// ========================================
// Custom body filter for AWS KMS content-type
// ========================================

fn aws_kms_body<T: serde::de::DeserializeOwned + Send>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::bytes().and_then(|bytes: bytes::Bytes| async move {
        serde_json::from_slice(&bytes)
            .map_err(|e| {
                eprintln!("JSON parse error: {}", e);
                warp::reject::custom(ApiError(format!("Invalid JSON: {}", e)))
            })
    })
}

// ========================================
// Main Server Startup
// ========================================

pub async fn start_kms_server() -> Result<()> {
    let server = Arc::new(KmsApiServer::new());

    // Root path - serve simple welcome message
    let index = warp::path::end()
        .and(warp::get())
        .map(|| {
            warp::reply::html(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>KMS API</title></head>
<body style="font-family: system-ui; max-width: 800px; margin: 50px auto; padding: 20px;">
<h1>🔐 AirAccount KMS API</h1>
<p>Welcome to the KMS API Server. This server provides AWS KMS-compatible APIs powered by OP-TEE.</p>
<h2>Endpoints:</h2>
<ul>
<li>POST /CreateKey - Create new wallet</li>
<li>POST /DescribeKey - Query wallet metadata</li>
<li>POST /ListKeys - List all wallets</li>
<li>POST /DeriveAddress - Derive Ethereum address</li>
<li>POST /Sign - Sign message</li>
<li>POST /GetPublicKey - Get public key</li>
<li>POST /DeleteKey - Schedule key deletion</li>
<li>GET /health - Health check</li>
</ul>
<p>For interactive testing, visit: <a href="/test">Test UI</a></p>
<p>API is running on OP-TEE Secure World with TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd</p>
</body>
</html>"#)
        });

    // Test UI page
    let test_ui = warp::path("test")
        .and(warp::get())
        .map(|| {
            match std::fs::read_to_string("/root/shared/kms-test-page.html") {
                Ok(html) => warp::reply::html(html),
                Err(_) => warp::reply::html("<html><body><h1>Test UI not available</h1><p>Please deploy kms-test-page.html to /root/shared/</p></body></html>".to_string())
            }
        });

    // Health check
    let health = warp::path("health")
        .and(warp::get())
        .and_then(health_check);

    // KeyStatus - GET /KeyStatus?KeyId=xxx
    let server_ks = server.clone();
    let key_status = warp::path("KeyStatus")
        .and(warp::get())
        .and(warp::query::raw().map(|q: String| {
            // Parse KeyId from query string
            q.split('&')
                .find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some("KeyId"), Some(v)) => Some(v.to_string()),
                        _ => None,
                    }
                })
                .unwrap_or_default()
        }))
        .and(warp::any().map(move || server_ks.clone()))
        .and_then(handle_key_status);

    // QueueStatus - GET /QueueStatus
    let server_qs = server.clone();
    let queue_status = warp::path("QueueStatus")
        .and(warp::get())
        .and(warp::any().map(move || server_qs.clone()))
        .and_then(handle_queue_status);

    // RegisterPasskey API
    let server_rp = server.clone();
    let register_passkey = warp::path("RegisterPasskey")
        .and(warp::post())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_rp.clone()))
        .and_then(handle_register_passkey);

    // WarmupCache API
    let server_wc = server.clone();
    let warmup_cache = warp::path("WarmupCache")
        .and(warp::post())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_wc.clone()))
        .and_then(handle_warmup_cache);

    // Clone server for each route
    let server1 = server.clone();
    let server2 = server.clone();
    let server3 = server.clone();
    let server4 = server.clone();
    let server5 = server.clone();
    let server6 = server.clone();

    // CreateKey API
    let create_key = warp::path("CreateKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.CreateKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server1.clone()))
        .and_then(handle_create_key);

    // DescribeKey API
    let describe_key = warp::path("DescribeKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.DescribeKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server2.clone()))
        .and_then(handle_describe_key);

    // ListKeys API
    let list_keys = warp::path("ListKeys")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.ListKeys"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server3.clone()))
        .and_then(handle_list_keys);

    // DeriveAddress API
    let derive_address = warp::path("DeriveAddress")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.DeriveAddress"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server4.clone()))
        .and_then(handle_derive_address);

    // Sign API
    let sign = warp::path("Sign")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.Sign"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server5.clone()))
        .and_then(handle_sign);

    // SignHash API
    let server6_clone = Arc::clone(&server);
    let sign_hash = warp::path("SignHash")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.SignHash"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6_clone.clone()))
        .and_then(handle_sign_hash);

    // GetPublicKey API
    let get_public_key = warp::path("GetPublicKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.GetPublicKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6.clone()))
        .and_then(handle_get_public_key);

    // DeleteKey removed from public API — use CLI on Mac Mini instead:
    //   ssh root@192.168.7.2 ./kms remove-wallet -w <wallet-id>

    let routes = index
        .or(test_ui)
        .or(health)
        .or(key_status)
        .or(queue_status)
        .or(register_passkey)
        .or(warmup_cache)
        .or(create_key)
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(sign_hash)
        .or(get_public_key)
        .recover(handle_rejection);

    println!("🚀 KMS API Server starting on http://0.0.0.0:3000");
    println!("📚 Supported APIs:");
    println!("   GET  /              - Welcome page");
    println!("   GET  /test          - Interactive test UI");
    println!("   POST /CreateKey     - Create new TEE wallet");
    println!("   POST /DescribeKey   - Query wallet metadata");
    println!("   POST /ListKeys      - List all wallets");
    println!("   POST /DeriveAddress - Derive Ethereum address");
    println!("   POST /Sign          - Sign Ethereum transaction or message");
    println!("   POST /SignHash      - Sign 32-byte hash directly");
    println!("   POST /GetPublicKey  - Get public key");
    println!("   ---- /DeleteKey     - CLI only (removed from public API)");
    println!("   POST /RegisterPasskey - Register PassKey public key");
    println!("   POST /WarmupCache   - Pre-load wallet into TA LRU cache");
    println!("   GET  /KeyStatus     - Key derivation status (polling)");
    println!("   GET  /QueueStatus   - TEE queue depth");
    println!("   GET  /health        - Health check");
    println!("🔐 TA Mode: ✅ Real TA (OP-TEE Secure World required)");
    println!("🆔 TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd");
    println!("🌐 Public URL: https://kms.aastar.io");

    warp::serve(routes)
        .run(([0, 0, 0, 0], 3000))
        .await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    start_kms_server().await
}