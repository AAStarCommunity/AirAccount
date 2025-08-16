/**
 * WebAuthn完整错误处理
 * 涵盖所有可能的WebAuthn认证失败场景
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebAuthnError {
    #[error("用户不存在: {user_id}")]
    UserNotFound { user_id: String },
    
    #[error("用户 {user_id} 没有注册任何设备")]
    NoDevicesRegistered { user_id: String },
    
    #[error("Challenge已过期或无效")]
    InvalidChallenge,
    
    #[error("Challenge不匹配: 期望={expected}, 实际={actual}")]
    ChallengeMismatch { expected: String, actual: String },
    
    #[error("注册失败: {reason}")]
    RegistrationFailed { reason: String },
    
    #[error("认证失败: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("检测到计数器回滚 - 可能的重放攻击")]
    CounterRollback,
    
    #[error("设备不识别: credential_id={credential_id}")]
    UnknownDevice { credential_id: String },
    
    #[error("用户验证失败: {reason}")]
    UserVerificationFailed { reason: String },
    
    #[error("原点验证失败: 期望={expected}, 实际={actual}")]
    OriginMismatch { expected: String, actual: String },
    
    #[error("RP ID验证失败: 期望={expected}, 实际={actual}")]
    RpIdMismatch { expected: String, actual: String },
    
    #[error("签名验证失败")]
    SignatureVerificationFailed,
    
    #[error("认证器数据格式错误: {reason}")]
    InvalidAuthenticatorData { reason: String },
    
    #[error("客户端数据格式错误: {reason}")]
    InvalidClientData { reason: String },
    
    #[error("注册状态不匹配: 当前步骤={current_step}, 期望步骤={expected_step}")]
    InvalidRegistrationState { 
        current_step: String, 
        expected_step: String 
    },
    
    #[error("认证状态不匹配: 当前步骤={current_step}, 期望步骤={expected_step}")]
    InvalidAuthenticationState { 
        current_step: String, 
        expected_step: String 
    },
    
    #[error("Passkey序列化失败: {reason}")]
    PasskeySerializationError { reason: String },
    
    #[error("Passkey反序列化失败: {reason}")]
    PasskeyDeserializationError { reason: String },
    
    #[error("状态存储失败: {reason}")]
    StateStorageError { reason: String },
    
    #[error("数据库操作失败: {0}")]
    DatabaseError(#[from] anyhow::Error),
    
    #[error("WebAuthn库错误: {message}")]
    WebAuthnLibError { message: String },
    
    #[error("配置错误: {message}")]
    ConfigurationError { message: String },
    
    #[error("网络错误: {message}")]
    NetworkError { message: String },
    
    #[error("超时错误: 操作={operation}, 超时时间={timeout_secs}秒")]
    TimeoutError { operation: String, timeout_secs: u64 },
    
    #[error("并发错误: {message}")]
    ConcurrencyError { message: String },
    
    #[error("权限不足: {message}")]
    PermissionDenied { message: String },
    
    #[error("资源不足: {message}")]
    ResourceExhausted { message: String },
    
    #[error("内部错误: {message}")]
    InternalError { message: String },
}

impl WebAuthnError {
    // 错误分类辅助方法
    pub fn is_user_error(&self) -> bool {
        matches!(self, 
            WebAuthnError::UserNotFound { .. } |
            WebAuthnError::NoDevicesRegistered { .. } |
            WebAuthnError::InvalidChallenge |
            WebAuthnError::ChallengeMismatch { .. } |
            WebAuthnError::UnknownDevice { .. } |
            WebAuthnError::UserVerificationFailed { .. }
        )
    }
    
    pub fn is_security_error(&self) -> bool {
        matches!(self,
            WebAuthnError::CounterRollback |
            WebAuthnError::OriginMismatch { .. } |
            WebAuthnError::RpIdMismatch { .. } |
            WebAuthnError::SignatureVerificationFailed
        )
    }
    
    pub fn is_system_error(&self) -> bool {
        matches!(self,
            WebAuthnError::DatabaseError(_) |
            WebAuthnError::StateStorageError { .. } |
            WebAuthnError::InternalError { .. } |
            WebAuthnError::ResourceExhausted { .. }
        )
    }
    
    // 获取用户友好的错误消息
    pub fn user_message(&self) -> String {
        match self {
            WebAuthnError::UserNotFound { .. } => "用户不存在，请先注册".to_string(),
            WebAuthnError::NoDevicesRegistered { .. } => "您还没有注册任何认证设备，请先注册".to_string(),
            WebAuthnError::InvalidChallenge => "认证请求已过期，请重新开始".to_string(),
            WebAuthnError::UnknownDevice { .. } => "设备不识别，请使用已注册的设备".to_string(),
            WebAuthnError::CounterRollback => "检测到安全异常，认证被拒绝".to_string(),
            WebAuthnError::SignatureVerificationFailed => "认证失败，请重试".to_string(),
            _ => "认证过程中出现错误，请稍后重试".to_string(),
        }
    }
    
    // 获取错误代码用于日志和监控
    pub fn error_code(&self) -> &'static str {
        match self {
            WebAuthnError::UserNotFound { .. } => "USER_NOT_FOUND",
            WebAuthnError::NoDevicesRegistered { .. } => "NO_DEVICES_REGISTERED",
            WebAuthnError::InvalidChallenge => "INVALID_CHALLENGE",
            WebAuthnError::ChallengeMismatch { .. } => "CHALLENGE_MISMATCH",
            WebAuthnError::RegistrationFailed { .. } => "REGISTRATION_FAILED",
            WebAuthnError::AuthenticationFailed { .. } => "AUTHENTICATION_FAILED",
            WebAuthnError::CounterRollback => "COUNTER_ROLLBACK",
            WebAuthnError::UnknownDevice { .. } => "UNKNOWN_DEVICE",
            WebAuthnError::UserVerificationFailed { .. } => "USER_VERIFICATION_FAILED",
            WebAuthnError::OriginMismatch { .. } => "ORIGIN_MISMATCH",
            WebAuthnError::RpIdMismatch { .. } => "RP_ID_MISMATCH",
            WebAuthnError::SignatureVerificationFailed => "SIGNATURE_VERIFICATION_FAILED",
            WebAuthnError::InvalidAuthenticatorData { .. } => "INVALID_AUTHENTICATOR_DATA",
            WebAuthnError::InvalidClientData { .. } => "INVALID_CLIENT_DATA",
            WebAuthnError::InvalidRegistrationState { .. } => "INVALID_REGISTRATION_STATE",
            WebAuthnError::InvalidAuthenticationState { .. } => "INVALID_AUTHENTICATION_STATE",
            WebAuthnError::PasskeySerializationError { .. } => "PASSKEY_SERIALIZATION_ERROR",
            WebAuthnError::PasskeyDeserializationError { .. } => "PASSKEY_DESERIALIZATION_ERROR",
            WebAuthnError::StateStorageError { .. } => "STATE_STORAGE_ERROR",
            WebAuthnError::DatabaseError(_) => "DATABASE_ERROR",
            WebAuthnError::WebAuthnLibError { .. } => "WEBAUTHN_LIB_ERROR",
            WebAuthnError::ConfigurationError { .. } => "CONFIGURATION_ERROR",
            WebAuthnError::NetworkError { .. } => "NETWORK_ERROR",
            WebAuthnError::TimeoutError { .. } => "TIMEOUT_ERROR",
            WebAuthnError::ConcurrencyError { .. } => "CONCURRENCY_ERROR",
            WebAuthnError::PermissionDenied { .. } => "PERMISSION_DENIED",
            WebAuthnError::ResourceExhausted { .. } => "RESOURCE_EXHAUSTED",
            WebAuthnError::InternalError { .. } => "INTERNAL_ERROR",
        }
    }
}

// 结果类型别名
pub type WebAuthnResult<T> = Result<T, WebAuthnError>;

// 从webauthn-rs错误转换
// 从 webauthn-rs错误转换
// 注意：webauthn-rs 0.5版本的错误类型位置可能不同
// 这里使用通用的字符串转换
// impl From<webauthn_rs::error::WebauthnError> for WebAuthnError {
//     fn from(err: webauthn_rs::error::WebauthnError) -> Self {
//         WebAuthnError::WebAuthnLibError {
//             message: err.to_string(),
//         }
//     }
// }

// 从serde_json错误转换  
impl From<serde_json::Error> for WebAuthnError {
    fn from(err: serde_json::Error) -> Self {
        if err.to_string().contains("serialize") {
            WebAuthnError::PasskeySerializationError {
                reason: err.to_string(),
            }
        } else {
            WebAuthnError::PasskeyDeserializationError {
                reason: err.to_string(),
            }
        }
    }
}

// 错误处理工具函数
pub fn log_webauthn_error(error: &WebAuthnError, context: &str) {
    println!("❌ WebAuthn错误 [{}]: {} (代码: {})", 
             context, error, error.error_code());
    
    if error.is_security_error() {
        println!("🚨 安全警告: 检测到潜在的安全威胁");
    }
}