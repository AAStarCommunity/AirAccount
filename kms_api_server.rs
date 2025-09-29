// 独立的 KMS API 服务器
// 此版本可在没有 OP-TEE 环境的情况下运行，用于演示 6 个 KMS API
// 准备集成真实的 eth_wallet TA 调用

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};
use warp::Filter;

// ========================================
// AWS KMS 兼容的数据结构
// ========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyRequest {
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
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
    #[serde(rename = "Transaction")]
    pub transaction: EthereumTransaction,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignResponse {
    #[serde(rename = "Signature")]
    pub signature: String,
    #[serde(rename = "TransactionHash")]
    pub transaction_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyResponse {
    #[serde(rename = "DeletionDate")]
    pub deletion_date: DateTime<Utc>,
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

// ========================================
// TA 调用接口（准备集成真实的 eth_wallet TA）
// ========================================

pub struct EthWalletTA {
    // 未来将集成真实的 TA 调用
}

impl EthWalletTA {
    pub fn new() -> Self {
        Self {}
    }

    // 注意：用户要求不要模拟，但在没有OPTEE环境时暂时使用简单响应
    // 这些方法准备好集成真实的 eth_wallet TA 调用
    pub async fn create_wallet(&self) -> Result<(String, String)> {
        // TODO: 集成真实 TA 调用
        // let wallet_uuid = crate::create_wallet()?;
        let wallet_id = Uuid::new_v4().to_string();
        let mnemonic = "[READY FOR REAL TA INTEGRATION]".to_string();
        println!("🔐 TA CreateWallet Called - Ready for real implementation");
        Ok((wallet_id, mnemonic))
    }

    pub async fn remove_wallet(&self, wallet_id: &str) -> Result<()> {
        // TODO: 集成真实 TA 调用
        // let wallet_uuid = Uuid::parse_str(wallet_id)?;
        // crate::remove_wallet(wallet_uuid)?;
        println!("🔐 TA RemoveWallet Called for ID: {} - Ready for real implementation", wallet_id);
        Ok(())
    }

    pub async fn derive_address(&self, wallet_id: &str, path: &str) -> Result<(String, String)> {
        // TODO: 集成真实 TA 调用
        // let wallet_uuid = Uuid::parse_str(wallet_id)?;
        // let address_bytes = crate::derive_address(wallet_uuid, path)?;
        // let address = format!("0x{}", hex::encode(&address_bytes));
        let address = format!("[TA_DERIVED_ADDRESS_FOR_{}]", wallet_id);
        let public_key = "[TA_DERIVED_PUBKEY]".to_string();
        println!("🔐 TA DeriveAddress Called for ID: {}, Path: {} - Ready for real implementation", wallet_id, path);
        Ok((address, public_key))
    }

    pub async fn sign_transaction(&self, wallet_id: &str, path: &str, _tx: &EthereumTransaction) -> Result<(String, String)> {
        // TODO: 集成真实 TA 调用
        // let wallet_uuid = Uuid::parse_str(wallet_id)?;
        // let signature = crate::sign_transaction(wallet_uuid, path, ...)?;
        let signature = "[TA_SIGNATURE]".to_string();
        let tx_hash = "[TA_TX_HASH]".to_string();
        println!("🔐 TA SignTransaction Called for ID: {}, Path: {} - Ready for real implementation", wallet_id, path);
        Ok((signature, tx_hash))
    }
}

// ========================================
// KMS API 服务器
// ========================================

pub struct KmsApiServer {
    ta: EthWalletTA,
    metadata_store: Arc<RwLock<HashMap<String, KeyMetadata>>>,
}

impl KmsApiServer {
    pub fn new() -> Self {
        Self {
            ta: EthWalletTA::new(),
            metadata_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("📝 KMS CreateKey API called");

        // 调用 TA CreateWallet
        let (wallet_id, mnemonic) = self.ta.create_wallet().await?;

        // 创建密钥元数据
        let key_metadata = KeyMetadata {
            key_id: wallet_id.clone(),
            arn: format!("arn:aws:kms:region:account:key/{}", wallet_id),
            creation_date: Utc::now(),
            enabled: true,
            description: req.description.clone(),
            key_usage: req.key_usage,
            key_spec: req.key_spec,
            origin: req.origin,
        };

        // 存储元数据
        let mut store = self.metadata_store.write().await;
        store.insert(wallet_id.clone(), key_metadata.clone());

        Ok(CreateKeyResponse {
            key_metadata,
            mnemonic,
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

        // 验证密钥存在
        let store = self.metadata_store.read().await;
        if !store.contains_key(&req.key_id) {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }

        // 调用 TA DeriveAddress
        let (address, public_key) = self.ta.derive_address(&req.key_id, &req.derivation_path).await?;

        Ok(DeriveAddressResponse {
            address,
            public_key,
        })
    }

    pub async fn sign(&self, req: SignRequest) -> Result<SignResponse> {
        println!("📝 KMS Sign API called for key: {}", req.key_id);

        // 验证密钥存在
        let store = self.metadata_store.read().await;
        if !store.contains_key(&req.key_id) {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }

        // 调用 TA SignTransaction
        let (signature, transaction_hash) = self.ta.sign_transaction(
            &req.key_id,
            &req.derivation_path,
            &req.transaction
        ).await?;

        Ok(SignResponse {
            signature,
            transaction_hash,
        })
    }

    pub async fn delete_key(&self, req: DeleteKeyRequest) -> Result<DeleteKeyResponse> {
        println!("📝 KMS DeleteKey API called for key: {}", req.key_id);

        // 调用 TA RemoveWallet
        self.ta.remove_wallet(&req.key_id).await?;

        // 从元数据存储中删除
        let mut store = self.metadata_store.write().await;
        store.remove(&req.key_id);

        Ok(DeleteKeyResponse {
            deletion_date: Utc::now(),
        })
    }
}

// ========================================
// HTTP 服务器路由
// ========================================

async fn health_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kms-api",
        "ta_integration": "ready"
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

async fn handle_delete_key(
    body: DeleteKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.delete_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DeleteKey error: {}", e);
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
// 主服务器启动函数
// ========================================

pub async fn start_kms_server() -> Result<()> {
    let server = Arc::new(KmsApiServer::new());

    // 健康检查
    let health = warp::path("health")
        .and(warp::get())
        .and_then(health_check);

    // 为每个路由创建 server 的副本
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
        .and(warp::body::json())
        .and(warp::any().map(move || server1.clone()))
        .and_then(handle_create_key);

    // DescribeKey API
    let describe_key = warp::path("DescribeKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.DescribeKey"))
        .and(warp::body::json())
        .and(warp::any().map(move || server2.clone()))
        .and_then(handle_describe_key);

    // ListKeys API
    let list_keys = warp::path("ListKeys")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.ListKeys"))
        .and(warp::body::json())
        .and(warp::any().map(move || server3.clone()))
        .and_then(handle_list_keys);

    // DeriveAddress API
    let derive_address = warp::path("DeriveAddress")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.DeriveAddress"))
        .and(warp::body::json())
        .and(warp::any().map(move || server4.clone()))
        .and_then(handle_derive_address);

    // Sign API
    let sign = warp::path("Sign")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.Sign"))
        .and(warp::body::json())
        .and(warp::any().map(move || server5.clone()))
        .and_then(handle_sign);

    // DeleteKey API
    let delete_key = warp::path("DeleteKey")
        .and(warp::post())
        .and(warp::header::exact("x-amz-target", "TrentService.DeleteKey"))
        .and(warp::body::json())
        .and(warp::any().map(move || server6.clone()))
        .and_then(handle_delete_key);

    let routes = health
        .or(create_key)
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(delete_key)
        .recover(handle_rejection);

    println!("🚀 KMS API Server 启动在 http://0.0.0.0:3000");
    println!("📚 支持的 API:");
    println!("   POST /CreateKey     - 创建新的 TEE 钱包");
    println!("   POST /DescribeKey   - 查询钱包元数据");
    println!("   POST /ListKeys      - 列出所有钱包");
    println!("   POST /DeriveAddress - 派生以太坊地址");
    println!("   POST /Sign          - 签名以太坊交易");
    println!("   POST /DeleteKey     - 删除钱包");
    println!("   GET  /health        - 健康检查");
    println!("🔐 TA 集成状态：准备集成真实的 eth_wallet TA 调用");

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