// TA-Only KMS API Server - All key operations MUST be done in TA
mod types;
mod ta_client;

use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::Json,
    routing::{post, get},
    Router,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, error};
use tracing_subscriber;

use types::*;
use ta_client::TAKmsService;

type SharedTAKmsService = Arc<TAKmsService>;

async fn handle_aws_kms_action(
    State(kms): State<SharedTAKmsService>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let action = headers
        .get("X-Amz-Target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown");

    info!("🔒 TA-Only KMS Action: {}", action);

    match action {
        // ===== TA-Only APIs (eth_wallet TA based) =====

        "TrentService.CreateAccount" => {
            let request: CreateAccountRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse CreateAccount request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.create_account(request).await {
                Ok(response) => {
                    let json_response = serde_json::to_value(response)
                        .map_err(|e| {
                            error!("Failed to serialize response: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error_type: "InternalFailureException".to_string(),
                                message: "Failed to serialize response".to_string(),
                            }))
                        })?;
                    Ok(Json(json_response))
                }
                Err(e) => {
                    error!("CreateAccount TA call failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "TAException".to_string(),
                        message: format!("TA error: {}", e),
                    })))
                }
            }
        }

        "TrentService.DescribeAccount" => {
            let request: DescribeAccountRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse DescribeAccount request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.describe_account(request).await {
                Ok(response) => {
                    let json_response = serde_json::to_value(response)
                        .map_err(|e| {
                            error!("Failed to serialize response: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error_type: "InternalFailureException".to_string(),
                                message: "Failed to serialize response".to_string(),
                            }))
                        })?;
                    Ok(Json(json_response))
                }
                Err(e) => {
                    error!("DescribeAccount failed: {}", e);
                    Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                        error_type: "NotFoundException".to_string(),
                        message: e.to_string(),
                    })))
                }
            }
        }

        "TrentService.ListAccounts" => {
            match kms.list_accounts().await {
                Ok(accounts) => {
                    let response = json!({
                        "Accounts": accounts
                    });
                    Ok(Json(response))
                }
                Err(e) => {
                    error!("ListAccounts failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "InternalFailureException".to_string(),
                        message: e.to_string(),
                    })))
                }
            }
        }

        "TrentService.DeriveAddress" => {
            let request: DeriveAddressRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse DeriveAddress request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.derive_address(request).await {
                Ok(response) => {
                    let json_response = serde_json::to_value(response)
                        .map_err(|e| {
                            error!("Failed to serialize response: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error_type: "InternalFailureException".to_string(),
                                message: "Failed to serialize response".to_string(),
                            }))
                        })?;
                    Ok(Json(json_response))
                }
                Err(e) => {
                    error!("DeriveAddress TA call failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "TAException".to_string(),
                        message: format!("TA error: {}", e),
                    })))
                }
            }
        }

        "TrentService.SignTransaction" => {
            let request: SignTransactionRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse SignTransaction request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.sign_transaction(request).await {
                Ok(response) => {
                    let json_response = serde_json::to_value(response)
                        .map_err(|e| {
                            error!("Failed to serialize response: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error_type: "InternalFailureException".to_string(),
                                message: "Failed to serialize response".to_string(),
                            }))
                        })?;
                    Ok(Json(json_response))
                }
                Err(e) => {
                    error!("SignTransaction TA call failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "TAException".to_string(),
                        message: format!("TA error: {}", e),
                    })))
                }
            }
        }

        "TrentService.RemoveAccount" => {
            let request: RemoveAccountRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse RemoveAccount request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.remove_account(request).await {
                Ok(response) => {
                    let json_response = serde_json::to_value(response)
                        .map_err(|e| {
                            error!("Failed to serialize response: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error_type: "InternalFailureException".to_string(),
                                message: "Failed to serialize response".to_string(),
                            }))
                        })?;
                    Ok(Json(json_response))
                }
                Err(e) => {
                    error!("RemoveAccount TA call failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "TAException".to_string(),
                        message: format!("TA error: {}", e),
                    })))
                }
            }
        }

        _ => {
            error!("🚫 Unsupported operation (TA-only mode): {}", action);
            Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error_type: "UnsupportedOperationException".to_string(),
                message: format!("Operation not supported in TA-only mode: {}", action),
            })))
        }
    }
}

async fn list_accounts(
    State(kms): State<SharedTAKmsService>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    match kms.list_accounts().await {
        Ok(accounts) => Ok(Json(json!({ "Accounts": accounts }))),
        Err(e) => {
            error!("ListAccounts failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error_type: "InternalFailureException".to_string(),
                message: e.to_string(),
            })))
        }
    }
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "TA-Only KMS API",
        "version": "2.1.0-ta-only",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "mode": "TEE_ONLY",
        "supported_operations": [
            "TrentService.CreateAccount",
            "TrentService.DescribeAccount",
            "TrentService.ListAccounts",
            "TrentService.DeriveAddress",
            "TrentService.SignTransaction",
            "TrentService.RemoveAccount"
        ]
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    info!("🔒 Starting TA-Only KMS API Server");
    info!("🛡️  ALL key operations performed in eth_wallet TA");

    // Initialize TA KMS service
    let kms_service = Arc::new(TAKmsService::new(
        "us-west-2".to_string(),
        "123456789012".to_string(),
    ));

    let app = Router::new()
        .route("/", post(handle_aws_kms_action))
        .route("/health", get(health_check))
        .route("/accounts", get(list_accounts))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
        .with_state(kms_service);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await.unwrap();
    info!("🚀 TA-Only KMS API Server listening on http://0.0.0.0:8081");
    info!("📖 Supported TA-based APIs:");
    info!("   POST / - AWS KMS compatible TA-only endpoints");
    info!("   GET  /health - Health check");
    info!("   GET  /accounts - List all accounts");
    info!("");
    info!("🔧 Example usage (TA-based CreateAccount):");
    info!("   curl -X POST http://localhost:8081/ \\");
    info!("     -H 'X-Amz-Target: TrentService.CreateAccount' \\");
    info!("     -H 'Content-Type: application/x-amz-json-1.1' \\");
    info!("     -d '{{\"Description\":\"My TA Account\"}}'");
    info!("");
    info!("🔒 Security: All private keys remain in eth_wallet TA");

    axum::serve(listener, app).await.unwrap();
}