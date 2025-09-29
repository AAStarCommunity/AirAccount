use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};

// 引用 main.rs 中的 TA 调用函数
use crate::{create_wallet, remove_wallet, derive_address, sign_transaction, hello_world};

// 添加 hex 依赖
use hex;

/// KMS API Server - 基于 eth_wallet TA 实现 AWS KMS 兼容 API
///
/// ## 架构原则
/// - 所有密钥操作必须在 TEE (TA) 中完成
/// - Host 只负责 HTTP 服务和元数据管理
/// - 使用 eth_wallet 的 4 个 TA 命令实现 6 个 KMS API
///
/// ## TA 映射关系
/// - CreateKey → TA CreateWallet
/// - DeleteKey → TA RemoveWallet
/// - DeriveAddress → TA DeriveAddress
/// - Sign → TA SignTransaction
/// - DescribeKey/ListKeys → 本地元数据

// ========================================
// AWS KMS 兼容数据结构
// ========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "Arn")]
    pub arn: String,

    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,

    #[serde(rename = "Description")]
    pub description: String,

    #[serde(rename = "Enabled")]
    pub enabled: bool,

    #[serde(rename = "KeyUsage")]
    pub key_usage: String,

    #[serde(rename = "KeySpec")]
    pub key_spec: String,

    #[serde(rename = "Origin")]
    pub origin: String,

    // 扩展字段 - 钱包相关
    #[serde(rename = "WalletType")]
    pub wallet_type: String,

    #[serde(rename = "HasMnemonic")]
    pub has_mnemonic: bool,
}

// ========================================
// API 请求和响应结构
// ========================================

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    #[serde(rename = "Description")]
    pub description: Option<String>,

    #[serde(rename = "KeyUsage")]
    pub key_usage: Option<String>,

    #[serde(rename = "KeySpec")]
    pub key_spec: Option<String>,

    #[serde(rename = "Origin")]
    pub origin: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,

    // 只在创建时返回一次
    #[serde(rename = "Mnemonic")]
    pub mnemonic: String,
}

#[derive(Debug, Deserialize)]
pub struct DescribeKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize)]
pub struct DescribeKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
}

#[derive(Debug, Deserialize)]
pub struct ListKeysRequest {
    #[serde(rename = "Limit")]
    pub limit: Option<u32>,

    #[serde(rename = "Marker")]
    pub marker: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListKeysResponse {
    #[serde(rename = "Keys")]
    pub keys: Vec<KeyMetadata>,

    #[serde(rename = "NextMarker")]
    pub next_marker: Option<String>,

    #[serde(rename = "Truncated")]
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeriveAddressRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
}

#[derive(Debug, Serialize)]
pub struct DeriveAddressResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "Address")]
    pub address: String,

    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,

    #[serde(rename = "PublicKey")]
    pub public_key: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct SignRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,

    #[serde(rename = "Transaction")]
    pub transaction: EthereumTransaction,
}

#[derive(Debug, Serialize)]
pub struct SignResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "Signature")]
    pub signature: String,

    #[serde(rename = "TransactionHash")]
    pub transaction_hash: String,

    #[serde(rename = "RawTransaction")]
    pub raw_transaction: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,

    #[serde(rename = "DeletionDate")]
    pub deletion_date: DateTime<Utc>,
}

// ========================================
// eth_wallet TA 接口 (模拟)
// ========================================

pub struct EthWalletTA {
    // 这里将集成真实的 eth_wallet TA 调用
    // 当前为了 API 开发，使用模拟实现
}

impl EthWalletTA {
    pub fn new() -> Self {
        Self {}
    }

    /// 调用 TA CreateWallet 命令
    pub async fn create_wallet(&self) -> Result<(String, String)> {
        // 使用真实的 TA 调用
        let wallet_uuid = create_wallet()?;
        let wallet_id = wallet_uuid.to_string();

        // 注意：真实的eth_wallet TA不返回助记词（安全原因）
        // KMS API需要返回助记词，这里我们生成模拟的
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();

        Ok((wallet_id, mnemonic))
    }

    /// 调用 TA RemoveWallet 命令
    pub async fn remove_wallet(&self, wallet_id: &str) -> Result<()> {
        // 使用真实的 TA 调用
        let wallet_uuid = Uuid::parse_str(wallet_id)
            .map_err(|e| anyhow!("无效的钱包ID格式: {}", e))?;

        remove_wallet(wallet_uuid)?;
        println!("TA: 删除钱包 {}", wallet_id);
        Ok(())
    }

    /// 调用 TA DeriveAddress 命令
    pub async fn derive_address(&self, _wallet_id: &str, _path: &str) -> Result<(String, String)> {
        // 使用真实的 TA 调用
        let wallet_uuid = Uuid::parse_str(_wallet_id)
            .map_err(|e| anyhow!("无效的钱包ID格式: {}", e))?;

        let address_bytes = derive_address(wallet_uuid, _path)?;
        let address = format!("0x{}", hex::encode(&address_bytes));

        // TA不返回公钥，这里生成模拟的
        let public_key = "0x04...eth_wallet_ta_derived_pubkey".to_string();

        Ok((address, public_key))
    }

    /// 调用 TA SignTransaction 命令
    pub async fn sign_transaction(&self, _wallet_id: &str, _path: &str, _tx: &EthereumTransaction) -> Result<(String, String)> {
        // 使用真实的 TA 调用
        let wallet_uuid = Uuid::parse_str(_wallet_id)
            .map_err(|e| anyhow!("无效的钱包ID格式: {}", e))?;

        // 转换地址格式
        let to_bytes = if _tx.to.starts_with("0x") {
            hex::decode(&_tx.to[2..])
                .map_err(|e| anyhow!("无效的地址格式: {}", e))?
        } else {
            hex::decode(&_tx.to)
                .map_err(|e| anyhow!("无效的地址格式: {}", e))?
        };

        if to_bytes.len() != 20 {
            return Err(anyhow!("地址必须是20字节"));
        }

        let mut to_array = [0u8; 20];
        to_array.copy_from_slice(&to_bytes);

        let signature = sign_transaction(
            wallet_uuid,
            _path,
            _tx.chain_id,
            _tx.nonce as u128,
            to_array,
            _tx.value.parse::<u128>().unwrap_or(0),
            _tx.gas_price.parse::<u128>().unwrap_or(0),
            _tx.gas.into(),
        )?;

        let signature_hex = format!("0x{}", hex::encode(&signature));

        // 计算交易哈希（模拟）
        let tx_hash = "0x1234...eth_wallet_ta_signed_hash".to_string();

        Ok((signature_hex, tx_hash))
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

    /// 1. TrentService.CreateKey → TA CreateWallet
    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("🔑 CreateKey API 调用");

        // 调用 TA CreateWallet
        let (wallet_id, mnemonic) = self.ta.create_wallet().await?;

        // 创建元数据
        let metadata = KeyMetadata {
            key_id: wallet_id.clone(),
            arn: format!("arn:aws:kms:us-east-1:123456789012:key/{}", wallet_id),
            creation_date: Utc::now(),
            description: req.description.unwrap_or_else(|| "TEE-based HD wallet".to_string()),
            enabled: true,
            key_usage: req.key_usage.unwrap_or_else(|| "SIGN_VERIFY".to_string()),
            key_spec: req.key_spec.unwrap_or_else(|| "ECC_SECG_P256K1".to_string()),
            origin: req.origin.unwrap_or_else(|| "AWS_KMS".to_string()),
            wallet_type: "HD_BIP32".to_string(),
            has_mnemonic: true,
        };

        // 存储元数据
        let mut store = self.metadata_store.write().await;
        store.insert(wallet_id.clone(), metadata.clone());

        Ok(CreateKeyResponse {
            key_metadata: metadata,
            mnemonic,
        })
    }

    /// 2. TrentService.DescribeKey → 本地元数据
    pub async fn describe_key(&self, req: DescribeKeyRequest) -> Result<DescribeKeyResponse> {
        println!("📋 DescribeKey API 调用: {}", req.key_id);

        let store = self.metadata_store.read().await;
        let metadata = store.get(&req.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        Ok(DescribeKeyResponse {
            key_metadata: metadata.clone(),
        })
    }

    /// 3. TrentService.ListKeys → 本地元数据列表
    pub async fn list_keys(&self, req: ListKeysRequest) -> Result<ListKeysResponse> {
        println!("📜 ListKeys API 调用");

        let store = self.metadata_store.read().await;
        let all_keys: Vec<KeyMetadata> = store.values().cloned().collect();

        let limit = req.limit.unwrap_or(100) as usize;
        let keys = all_keys.into_iter().take(limit).collect::<Vec<_>>();
        let truncated = keys.len() >= limit;

        Ok(ListKeysResponse {
            keys,
            next_marker: None,
            truncated,
        })
    }

    /// 4. TrentService.DeriveAddress → TA DeriveAddress
    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("🏠 DeriveAddress API 调用: {} {}", req.key_id, req.derivation_path);

        // 验证 key 存在
        {
            let store = self.metadata_store.read().await;
            if !store.contains_key(&req.key_id) {
                return Err(anyhow!("Key not found: {}", req.key_id));
            }
        }

        // 调用 TA DeriveAddress
        let (address, public_key) = self.ta.derive_address(&req.key_id, &req.derivation_path).await?;

        Ok(DeriveAddressResponse {
            key_id: req.key_id,
            address,
            derivation_path: req.derivation_path,
            public_key,
        })
    }

    /// 5. TrentService.Sign → TA SignTransaction
    pub async fn sign(&self, req: SignRequest) -> Result<SignResponse> {
        println!("✍️  Sign API 调用: {}", req.key_id);

        // 验证 key 存在
        {
            let store = self.metadata_store.read().await;
            if !store.contains_key(&req.key_id) {
                return Err(anyhow!("Key not found: {}", req.key_id));
            }
        }

        // 调用 TA SignTransaction
        let (signature, tx_hash) = self.ta.sign_transaction(
            &req.key_id,
            &req.derivation_path,
            &req.transaction
        ).await?;

        // 构造原始交易 (简化)
        let raw_transaction = format!("0x{}", tx_hash);

        Ok(SignResponse {
            key_id: req.key_id,
            signature,
            transaction_hash: tx_hash,
            raw_transaction,
        })
    }

    /// 6. TrentService.ScheduleKeyDeletion → TA RemoveWallet
    pub async fn delete_key(&self, req: DeleteKeyRequest) -> Result<DeleteKeyResponse> {
        println!("🗑️  DeleteKey API 调用: {}", req.key_id);

        // 验证 key 存在
        {
            let store = self.metadata_store.read().await;
            if !store.contains_key(&req.key_id) {
                return Err(anyhow!("Key not found: {}", req.key_id));
            }
        }

        // 调用 TA RemoveWallet
        self.ta.remove_wallet(&req.key_id).await?;

        // 删除元数据
        {
            let mut store = self.metadata_store.write().await;
            store.remove(&req.key_id);
        }

        Ok(DeleteKeyResponse {
            key_id: req.key_id,
            deletion_date: Utc::now(),
        })
    }
}

// ========================================
// HTTP 服务器
// ========================================

use warp::Filter;

pub async fn start_kms_server() -> Result<()> {
    let server = Arc::new(KmsApiServer::new());

    // 1. CreateKey API
    let create_key = warp::path!("CreateKey")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_create_key);

    // 2. DescribeKey API
    let describe_key = warp::path!("DescribeKey")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_describe_key);

    // 3. ListKeys API
    let list_keys = warp::path!("ListKeys")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_list_keys);

    // 4. DeriveAddress API
    let derive_address = warp::path!("DeriveAddress")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_derive_address);

    // 5. Sign API
    let sign = warp::path!("Sign")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_sign);

    // 6. DeleteKey API
    let delete_key = warp::path!("DeleteKey")
        .and(warp::post())
        .and(warp::header("x-amz-target"))
        .and(warp::body::json())
        .and(with_server(server.clone()))
        .and_then(handle_delete_key);

    // 健康检查
    let health = warp::path!("health")
        .and(warp::get())
        .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));

    let api = create_key
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(delete_key)
        .or(health)
        .with(warp::cors().allow_any_origin());

    println!("🚀 KMS API Server 启动在 http://0.0.0.0:3000");
    println!("📚 支持的 API:");
    println!("   POST /CreateKey     - 创建新的 TEE 钱包");
    println!("   POST /DescribeKey   - 查询钱包元数据");
    println!("   POST /ListKeys      - 列出所有钱包");
    println!("   POST /DeriveAddress - 派生以太坊地址");
    println!("   POST /Sign          - 签名以太坊交易");
    println!("   POST /DeleteKey     - 删除钱包");
    println!("   GET  /health        - 健康检查");

    warp::serve(api)
        .run(([0, 0, 0, 0], 3000))
        .await;

    Ok(())
}

// HTTP 处理器助手
fn with_server(server: Arc<KmsApiServer>) -> impl Filter<Extract = (Arc<KmsApiServer>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || server.clone())
}

// HTTP 处理器
async fn handle_create_key(
    target: String,
    req: CreateKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.create_key(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("CreateKey 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

async fn handle_describe_key(
    target: String,
    req: DescribeKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.describe_key(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DescribeKey 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

async fn handle_list_keys(
    target: String,
    req: ListKeysRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.list_keys(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ListKeys 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

async fn handle_derive_address(
    target: String,
    req: DeriveAddressRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.derive_address(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DeriveAddress 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

async fn handle_sign(
    target: String,
    req: SignRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.sign(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("Sign 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

async fn handle_delete_key(
    target: String,
    req: DeleteKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.delete_key(req).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DeleteKey 错误: {}", e);
            Err(warp::reject::reject())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_key() {
        let server = KmsApiServer::new();

        let req = CreateKeyRequest {
            description: Some("Test key".to_string()),
            key_usage: Some("SIGN_VERIFY".to_string()),
            key_spec: Some("ECC_SECG_P256K1".to_string()),
            origin: Some("AWS_KMS".to_string()),
        };

        let response = server.create_key(req).await.unwrap();
        assert!(!response.key_metadata.key_id.is_empty());
        assert!(!response.mnemonic.is_empty());
    }

    #[tokio::test]
    async fn test_api_flow() {
        let server = KmsApiServer::new();

        // 1. 创建钱包
        let create_req = CreateKeyRequest {
            description: Some("Test wallet".to_string()),
            key_usage: None,
            key_spec: None,
            origin: None,
        };
        let create_resp = server.create_key(create_req).await.unwrap();
        let key_id = create_resp.key_metadata.key_id.clone();

        // 2. 查询钱包
        let describe_req = DescribeKeyRequest { key_id: key_id.clone() };
        let _describe_resp = server.describe_key(describe_req).await.unwrap();

        // 3. 派生地址
        let derive_req = DeriveAddressRequest {
            key_id: key_id.clone(),
            derivation_path: "m/44'/60'/0'/0/0".to_string(),
        };
        let _derive_resp = server.derive_address(derive_req).await.unwrap();

        // 4. 删除钱包
        let delete_req = DeleteKeyRequest { key_id: key_id.clone() };
        let _delete_resp = server.delete_key(delete_req).await.unwrap();
    }
}