/**
 * AirAccount CA Extended - 库模块
 */

pub mod tee_client;
pub mod webauthn;

pub use tee_client::{TeeClient, TeeAccountResult, TeeTransferResult};
pub use webauthn::{SimpleWebAuthnManager, RegistrationOptions, RegistrationChallenge, AuthenticationChallenge, AuthenticationResponse};

// 重新导出常用类型
pub use anyhow::{Result, Error};
pub use serde::{Deserialize, Serialize};

// 版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// CA 扩展错误类型
#[derive(Debug, thiserror::Error)]
pub enum CaExtendedError {
    #[error("TEE communication error: {0}")]
    TeeError(#[from] anyhow::Error),
    
    #[error("WebAuthn error: {0}")]
    WebAuthnError(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
}