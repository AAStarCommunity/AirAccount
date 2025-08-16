/**
 * AirAccount CA Extended - HTTP API服务器
 * 基于现有airaccount-ca的扩展，添加WebAuthn和HTTP API支持
 */

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::{info, error, warn};

mod tee_client;
mod webauthn;
mod webauthn_real;

use tee_client::{TeeClient, TeeAccountResult, TeeTransferResult};
use webauthn::{SimpleWebAuthnManager, RegistrationOptions, RegistrationChallenge, AuthenticationChallenge, AuthenticationResponse};
use webauthn_real::{RealWebAuthnService, WebAuthnConfig};

// API请求/响应结构
#[derive(Debug, Deserialize)]
struct CreateAccountRequest {
    email: String,
    passkey_credential_id: String,
    passkey_public_key_base64: String,
}

#[derive(Debug, Serialize)]
struct CreateAccountResponse {
    success: bool,
    wallet_id: u32,
    ethereum_address: String,
    tee_device_id: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct TransferRequest {
    wallet_id: u32,
    to_address: String,
    amount: String,
    gas_price: Option<String>,
}

#[derive(Debug, Serialize)]
struct TransferResponse {
    success: bool,
    transaction_hash: String,
    signature: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct BalanceRequest {
    wallet_id: u32,
}

#[derive(Debug, Serialize)]
struct BalanceResponse {
    success: bool,
    wallet_id: u32,
    ethereum_address: String,
    balance_wei: String,
    balance_eth: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    tee_connected: bool,
    timestamp: String,
}

// 应用状态
#[derive(Clone)]
struct AppState {
    tee_client: Arc<Mutex<TeeClient>>,
    webauthn_manager: Arc<SimpleWebAuthnManager>,
    real_webauthn_service: Arc<RealWebAuthnService>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("airaccount_ca_extended=info,tower_http=debug")
        .init();

    info!("🚀 Starting AirAccount CA Extended Server...");

    // 初始化TEE客户端
    let tee_client = TeeClient::new().map_err(|e| {
        error!("Failed to initialize TEE client: {}", e);
        e
    })?;

    info!("✅ TEE client initialized successfully");

    // 初始化WebAuthn管理器
    let webauthn_manager = SimpleWebAuthnManager::new(
        "localhost".to_string(), // 生产环境中应该从配置读取
        "AirAccount".to_string(),
    );

    // 初始化真实WebAuthn服务
    let webauthn_config = WebAuthnConfig {
        rp_name: "AirAccount".to_string(),
        rp_id: "localhost".to_string(),
        rp_origin: webauthn_rs::prelude::Url::parse("http://localhost:3001")?,
    };
    let real_webauthn_service = Arc::new(
        RealWebAuthnService::new(webauthn_config, "sqlite:webauthn.db").await?
    );
    info!("✅ Real WebAuthn service initialized");

    // 创建应用状态
    let app_state = AppState {
        tee_client: Arc::new(Mutex::new(tee_client)),
        webauthn_manager: Arc::new(webauthn_manager),
        real_webauthn_service,
    };

    // 测试TEE连接
    {
        let mut client = app_state.tee_client.lock().unwrap();
        match client.test_connection() {
            Ok(response) => info!("✅ TEE connection test: {}", response),
            Err(e) => {
                error!("❌ TEE connection test failed: {}", e);
                return Err(e);
            }
        }
    }

    // 构建路由
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/account/create", post(create_account))
        .route("/api/account/balance", post(get_balance))
        .route("/api/transaction/transfer", post(transfer))
        .route("/api/wallet/list", get(list_wallets))
        // WebAuthn路由
        .route("/api/webauthn/register/begin", post(webauthn_register_begin))
        .route("/api/webauthn/authenticate/begin", post(webauthn_authenticate_begin))
        .route("/api/webauthn/authenticate/finish", post(webauthn_authenticate_finish))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .unwrap();

    info!("🌐 Server listening on http://0.0.0.0:3001");
    info!("📚 API Endpoints:");
    info!("  GET  /health - 健康检查");
    info!("  POST /api/account/create - 创建账户");
    info!("  POST /api/account/balance - 查询余额");
    info!("  POST /api/transaction/transfer - 转账");
    info!("  GET  /api/wallet/list - 列出钱包");
    info!("  POST /api/webauthn/register/begin - 开始WebAuthn注册");
    info!("  POST /api/webauthn/authenticate/begin - 开始WebAuthn认证");
    info!("  POST /api/webauthn/authenticate/finish - 完成WebAuthn认证");

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

// 健康检查
async fn health_check(State(state): State<AppState>) -> Result<ResponseJson<HealthResponse>, StatusCode> {
    let tee_connected = {
        let mut client = state.tee_client.lock().unwrap();
        client.test_connection().is_ok()
    };

    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tee_connected,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Ok(ResponseJson(response))
}

// 创建账户
async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<ResponseJson<CreateAccountResponse>, StatusCode> {
    info!("📧 Creating account for email: {}", req.email);

    // 解码base64公钥
    let passkey_public_key = base64::decode(&req.passkey_public_key_base64)
        .map_err(|e| {
            error!("Failed to decode base64 public key: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // 调用TEE创建账户
    let result = {
        let mut client = state.tee_client.lock().unwrap();
        client.create_account_with_passkey(
            &req.email,
            &req.passkey_credential_id,
            &passkey_public_key,
        ).map_err(|e| {
            error!("TEE account creation failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let response = CreateAccountResponse {
        success: true,
        wallet_id: result.wallet_id,
        ethereum_address: result.ethereum_address,
        tee_device_id: result.tee_device_id,
        message: "Account created successfully".to_string(),
    };

    info!("✅ Account created: wallet_id={}, address={}", 
          response.wallet_id, response.ethereum_address);

    Ok(ResponseJson(response))
}

// 查询余额
async fn get_balance(
    State(state): State<AppState>,
    Json(req): Json<BalanceRequest>,
) -> Result<ResponseJson<BalanceResponse>, StatusCode> {
    info!("💰 Getting balance for wallet: {}", req.wallet_id);

    // 获取钱包信息
    let wallet_info = {
        let mut client = state.tee_client.lock().unwrap();
        client.get_wallet_info(req.wallet_id).map_err(|e| {
            error!("Failed to get wallet info: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // 获取地址
    let address_response = {
        let mut client = state.tee_client.lock().unwrap();
        client.derive_address(req.wallet_id).map_err(|e| {
            error!("Failed to derive address: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // 解析地址
    let ethereum_address = if let Some(address_hex) = address_response.strip_prefix("address:") {
        if address_hex.len() >= 40 {
            format!("0x{}", &address_hex[..40])
        } else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    // 模拟余额（实际应用中需要查询区块链）
    let balance_wei = "1000000000000000000"; // 1 ETH in wei
    let balance_eth = "1.0";

    let response = BalanceResponse {
        success: true,
        wallet_id: req.wallet_id,
        ethereum_address,
        balance_wei: balance_wei.to_string(),
        balance_eth: balance_eth.to_string(),
        message: "Balance retrieved successfully".to_string(),
    };

    info!("✅ Balance retrieved for wallet {}: {} ETH", req.wallet_id, balance_eth);

    Ok(ResponseJson(response))
}

// 转账
async fn transfer(
    State(state): State<AppState>,
    Json(req): Json<TransferRequest>,
) -> Result<ResponseJson<TransferResponse>, StatusCode> {
    info!("💸 Processing transfer from wallet {} to {} (amount: {})", 
          req.wallet_id, req.to_address, req.amount);

    // 构建交易数据
    let transaction_data = format!(
        "to:{},amount:{},gas_price:{}",
        req.to_address,
        req.amount,
        req.gas_price.unwrap_or_else(|| "20000000000".to_string())
    );

    // 调用TEE签名交易
    let result = {
        let mut client = state.tee_client.lock().unwrap();
        client.sign_transaction(req.wallet_id, &transaction_data).map_err(|e| {
            error!("TEE transaction signing failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let response = TransferResponse {
        success: true,
        transaction_hash: result.transaction_hash,
        signature: result.signature,
        message: "Transaction signed successfully".to_string(),
    };

    info!("✅ Transaction signed: hash={}, wallet_id={}", 
          response.transaction_hash, req.wallet_id);

    Ok(ResponseJson(response))
}

// 列出钱包
async fn list_wallets(
    State(state): State<AppState>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    info!("📋 Listing all wallets");

    let wallets_info = {
        let mut client = state.tee_client.lock().unwrap();
        client.list_wallets().map_err(|e| {
            error!("Failed to list wallets: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let response = serde_json::json!({
        "success": true,
        "wallets": wallets_info,
        "message": "Wallets listed successfully"
    });

    Ok(ResponseJson(response))
}

// WebAuthn 注册开始
async fn webauthn_register_begin(
    State(state): State<AppState>,
    Json(req): Json<RegistrationOptions>,
) -> Result<ResponseJson<RegistrationChallenge>, StatusCode> {
    info!("🔐 Starting WebAuthn registration for user: {}", req.user_name);

    let challenge = state.webauthn_manager
        .generate_registration_challenge(req)
        .map_err(|e| {
            error!("Failed to generate registration challenge: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("✅ Registration challenge generated");
    Ok(ResponseJson(challenge))
}

// WebAuthn 认证开始
async fn webauthn_authenticate_begin(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<ResponseJson<AuthenticationChallenge>, StatusCode> {
    let user_id = req.get("user_id")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    info!("🔓 Starting WebAuthn authentication for user: {}", user_id);

    let challenge = state.webauthn_manager
        .generate_authentication_challenge(user_id)
        .map_err(|e| {
            error!("Failed to generate authentication challenge: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("✅ Authentication challenge generated");
    Ok(ResponseJson(challenge))
}

// WebAuthn 认证完成
async fn webauthn_authenticate_finish(
    State(state): State<AppState>,
    Json(req): Json<AuthenticationResponse>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    info!("🔍 Verifying WebAuthn authentication response");

    let is_valid = state.webauthn_manager
        .verify_authentication(req)
        .map_err(|e| {
            error!("Failed to verify authentication: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if is_valid {
        info!("✅ WebAuthn authentication successful");
        Ok(ResponseJson(serde_json::json!({
            "success": true,
            "message": "Authentication successful"
        })))
    } else {
        error!("❌ WebAuthn authentication failed");
        Ok(ResponseJson(serde_json::json!({
            "success": false,
            "message": "Authentication failed"
        })))
    }
}