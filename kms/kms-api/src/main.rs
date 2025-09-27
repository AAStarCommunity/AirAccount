// KMS JSON-RPC API Server
mod types;
mod simple_kms;

use axum::{
    extract::{State, Path},
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
use simple_kms::KmsService;

type SharedKmsService = Arc<KmsService>;

async fn handle_aws_kms_action(
    State(kms): State<SharedKmsService>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    // Extract the action from X-Amz-Target header
    let action = headers
        .get("X-Amz-Target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown");

    info!("Handling action: {}", action);

    match action {
        "TrentService.CreateKey" => {
            let request: CreateKeyRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse CreateKey request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.create_key(request).await {
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
                    error!("CreateKey failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "InternalFailureException".to_string(),
                        message: e.to_string(),
                    })))
                }
            }
        }
        "TrentService.Sign" => {
            let request: SignRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse Sign request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.sign(request).await {
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
                    error!("Sign failed: {}", e);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error_type: "KMSInvalidStateException".to_string(),
                        message: e.to_string(),
                    })))
                }
            }
        }
        "TrentService.GetPublicKey" => {
            let request: GetPublicKeyRequest = serde_json::from_value(payload)
                .map_err(|e| {
                    error!("Failed to parse GetPublicKey request: {}", e);
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error_type: "ValidationException".to_string(),
                        message: format!("Invalid request: {}", e),
                    }))
                })?;

            match kms.get_public_key(request).await {
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
                    error!("GetPublicKey failed: {}", e);
                    Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                        error_type: "NotFoundException".to_string(),
                        message: e.to_string(),
                    })))
                }
            }
        }
        _ => {
            error!("Unknown action: {}", action);
            Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error_type: "UnknownOperationException".to_string(),
                message: format!("Unknown operation: {}", action),
            })))
        }
    }
}

async fn list_keys(
    State(kms): State<SharedKmsService>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    match kms.list_keys().await {
        Ok(keys) => Ok(Json(json!({ "Keys": keys }))),
        Err(e) => {
            error!("ListKeys failed: {}", e);
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
        "service": "KMS API",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🔐 Starting KMS API Server");

    // Initialize KMS service
    let kms_service = Arc::new(KmsService::new(
        "us-west-2".to_string(),
        "123456789012".to_string(),
    ));

    // Build the application
    let app = Router::new()
        .route("/", post(handle_aws_kms_action))
        .route("/health", get(health_check))
        .route("/keys", get(list_keys))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
        .with_state(kms_service);

    // Start the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("🚀 KMS API Server listening on http://0.0.0.0:8080");
    info!("📖 API Documentation:");
    info!("   POST / - AWS KMS compatible endpoints");
    info!("   GET  /health - Health check");
    info!("   GET  /keys - List all keys");
    info!("");
    info!("🔧 Example usage:");
    info!("   curl -X POST http://localhost:8080/ \\");
    info!("     -H 'X-Amz-Target: TrentService.CreateKey' \\");
    info!("     -H 'Content-Type: application/x-amz-json-1.1' \\");
    info!("     -d '{{\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\"}}'");

    axum::serve(listener, app).await.unwrap();
}