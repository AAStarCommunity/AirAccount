// KMS API Server
// Real TA integration only - requires OP-TEE environment
// Deploy to QEMU for testing, production-ready architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::sync::{RwLock, Semaphore};
use anyhow::{Result, anyhow};
use warp::Filter;
use hex;

// Import from kms library and proto
use kms::ta_client::TaClient;
use kms::address_cache::lookup_address;
use proto;

/// Max concurrent TEE operations (STM32 is single-core, TEE is single-threaded)
const TEE_MAX_CONCURRENCY: usize = 1;

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

enum SignPayload {
    Transaction(proto::EthTransaction),
    Message(Vec<u8>),
}

// ========================================
// KMS API Server
// ========================================

pub struct KmsApiServer {
    metadata_store: Arc<RwLock<HashMap<String, KeyMetadata>>>,
    tee_semaphore: Arc<Semaphore>,
}

impl KmsApiServer {
    pub fn new() -> Self {
        Self {
            metadata_store: Arc::new(RwLock::new(HashMap::new())),
            tee_semaphore: Arc::new(Semaphore::new(TEE_MAX_CONCURRENCY)),
        }
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("📝 KMS CreateKey API called");

        let _permit = self.tee_semaphore.acquire().await
            .map_err(|_| anyhow!("TEE semaphore closed"))?;

        // Only create wallet (fast: entropy generation only).
        // Address derivation is done separately via /DeriveAddress.
        let wallet_id = tokio::task::spawn_blocking(move || -> Result<uuid::Uuid> {
            let mut ta_client = TaClient::new()?;
            Ok(ta_client.create_wallet()?)
        }).await
            .map_err(|e| anyhow!("spawn_blocking join error: {}", e))??;

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

    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("📝 KMS DeriveAddress API called for key: {}", req.key_id);

        let store = self.metadata_store.read().await;
        if !store.contains_key(&req.key_id) {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }
        drop(store);

        let _permit = self.tee_semaphore.acquire().await
            .map_err(|_| anyhow!("TEE semaphore closed"))?;

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        let derivation_path = req.derivation_path.clone();
        let address_bytes = tokio::task::spawn_blocking(move || -> Result<[u8; 20]> {
            let mut ta_client = TaClient::new()?;
            Ok(ta_client.derive_address(wallet_uuid, &derivation_path)?)
        }).await
            .map_err(|e| anyhow!("spawn_blocking join error: {}", e))??;

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

        // Prepare sign payload before acquiring TEE permit
        let sign_payload = if let Some(transaction) = req.transaction {
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
            SignPayload::Transaction(eth_transaction)
        } else if let Some(message) = req.message {
            println!("  📝 Message signing mode");
            let message_bytes = if message.starts_with("0x") {
                hex::decode(&message[2..])?
            } else {
                base64::decode(&message).unwrap_or_else(|_| message.as_bytes().to_vec())
            };
            SignPayload::Message(message_bytes)
        } else {
            return Err(anyhow!("Either Transaction or Message must be provided"));
        };

        let _permit = self.tee_semaphore.acquire().await
            .map_err(|_| anyhow!("TEE semaphore closed"))?;

        let signature = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let mut ta_client = TaClient::new()?;
            match sign_payload {
                SignPayload::Transaction(eth_tx) => {
                    Ok(ta_client.sign_transaction(wallet_uuid, &derivation_path, eth_tx)?)
                }
                SignPayload::Message(msg) => {
                    Ok(ta_client.sign_message(wallet_uuid, &derivation_path, &msg)?)
                }
            }
        }).await
            .map_err(|e| anyhow!("spawn_blocking join error: {}", e))??;

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

        let _permit = self.tee_semaphore.acquire().await
            .map_err(|_| anyhow!("TEE semaphore closed"))?;

        let signature = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let mut ta_client = TaClient::new()?;
            Ok(ta_client.sign_hash(wallet_uuid, &derivation_path, &hash_array)?)
        }).await
            .map_err(|e| anyhow!("spawn_blocking join error: {}", e))??;

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

        let _permit = self.tee_semaphore.acquire().await
            .map_err(|_| anyhow!("TEE semaphore closed"))?;

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut ta_client = TaClient::new()?;
            ta_client.remove_wallet(wallet_uuid)?;
            Ok(())
        }).await
            .map_err(|e| anyhow!("spawn_blocking join error: {}", e))??;

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
            "POST": ["/CreateKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/DeleteKey"],
            "GET": ["/health"]
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

    // Clone server for each route
    let server1 = server.clone();
    let server2 = server.clone();
    let server3 = server.clone();
    let server4 = server.clone();
    let server5 = server.clone();
    let server6 = server.clone();
    let server7 = server.clone();

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

    // DeleteKey API (ScheduleKeyDeletion)
    let delete_key = warp::path("DeleteKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.ScheduleKeyDeletion"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server7.clone()))
        .and_then(handle_delete_key);

    let routes = index
        .or(test_ui)
        .or(health)
        .or(create_key)
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(sign_hash)
        .or(get_public_key)
        .or(delete_key)
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
    println!("   POST /DeleteKey     - Schedule key deletion");
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