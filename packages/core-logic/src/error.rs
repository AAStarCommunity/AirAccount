// Licensed to AirAccount under the Apache License, Version 2.0
// Structured error handling system for enhanced maintainability

// Use backtrace only in nightly builds
#[cfg(feature = "std_backtrace")]
use std::backtrace::Backtrace;

#[cfg(not(feature = "std_backtrace"))]
type Backtrace = String;
use std::time::SystemTime;
// use thiserror::Error; // 保留以备将来使用
use serde::{Deserialize, Serialize};

/// Error context providing detailed information for debugging and auditing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Timestamp when the error occurred
    pub timestamp: SystemTime,
    /// Component where the error originated
    pub component: String,
    /// Thread ID where the error occurred
    pub thread_id: String,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
    /// Operation being performed when error occurred
    pub operation: String,
}

impl ErrorContext {
    /// Create new error context
    pub fn new(component: &str, operation: &str) -> Self {
        Self {
            timestamp: SystemTime::now(),
            component: component.to_string(),
            thread_id: format!("{:?}", std::thread::current().id()),
            session_id: None,
            metadata: std::collections::HashMap::new(),
            operation: operation.to_string(),
        }
    }
    
    /// Add metadata to the error context
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Add session ID to the error context
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Key management error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyErrorKind {
    /// Key generation failed
    GenerationFailed,
    /// Key derivation failed
    DerivationFailed,
    /// Key not found
    NotFound,
    /// Invalid key format
    InvalidFormat,
    /// Key rotation failed
    RotationFailed,
    /// Key encryption failed
    EncryptionFailed,
    /// Key decryption failed
    DecryptionFailed,
}

/// TEE operation error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TEEErrorKind {
    /// TEE initialization failed
    InitializationFailed,
    /// Secure function invocation failed
    InvocationFailed,
    /// Memory allocation failed
    MemoryAllocationFailed,
    /// Communication error
    CommunicationError,
    /// Session management error
    SessionError,
}

/// Network communication error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkErrorKind {
    /// Connection timeout
    ConnectionTimeout,
    /// Connection refused
    ConnectionRefused,
    /// Invalid response
    InvalidResponse,
    /// Authentication failed
    AuthenticationFailed,
    /// Protocol error
    ProtocolError,
}

/// Audit system error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditErrorKind {
    /// Log writing failed
    LogWriteFailed,
    /// Log verification failed
    VerificationFailed,
    /// Log corruption detected
    CorruptionDetected,
    /// Audit sink unavailable
    SinkUnavailable,
}

/// Configuration error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigErrorKind {
    /// Invalid configuration value
    InvalidValue,
    /// Missing required configuration
    MissingRequired,
    /// Configuration parsing failed
    ParsingFailed,
    /// Validation failed
    ValidationFailed,
}

/// Structured error types for AirAccount security system
#[derive(Debug, Serialize)]
#[serde(tag = "error_type", content = "details")]
pub enum SecurityError {
    /// Cryptographic operation failed
    /// Cryptographic operation failed
    CryptoError { 
        operation: String, 
        details: String,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Memory protection violation
    /// Memory protection violation
    MemoryViolation { 
        address: usize, 
        operation: String,
        stack_trace: Vec<String>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Key management error
    KeyManagementError {
        kind: KeyErrorKind,
        key_id: Option<String>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// TEE operation error
    TEEError {
        kind: TEEErrorKind,
        details: String,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Network communication error
    NetworkError {
        kind: NetworkErrorKind,
        endpoint: String,
        status_code: Option<u16>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Validation error
    ValidationError {
        field: String,
        message: String,
        value: Option<String>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Authentication/Authorization error
    AuthError {
        operation: String,
        reason: String,
        user_id: Option<String>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Audit system error
    AuditError {
        kind: AuditErrorKind,
        details: String,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Configuration error
    ConfigError {
        kind: ConfigErrorKind,
        parameter: String,
        expected: Option<String>,
        actual: Option<String>,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
    
    /// Internal system error (catch-all)
    InternalError {
        component: String,
        message: String,
        context: ErrorContext,
        #[serde(skip)]
        backtrace: Backtrace,
    },
}

impl SecurityError {
    /// Create a crypto error with context
    pub fn crypto_error(operation: &str, details: &str, component: &str) -> Self {
        Self::CryptoError {
            operation: operation.to_string(),
            details: details.to_string(),
            context: ErrorContext::new(component, operation),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create a memory violation error with context
    pub fn memory_violation(address: usize, operation: &str, component: &str) -> Self {
        Self::MemoryViolation {
            address,
            operation: operation.to_string(),
            stack_trace: vec![], // Could be populated with actual stack trace
            context: ErrorContext::new(component, operation),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create a key management error with context
    pub fn key_management_error(kind: KeyErrorKind, key_id: Option<String>, component: &str) -> Self {
        Self::KeyManagementError {
            kind,
            key_id,
            context: ErrorContext::new(component, "key_management"),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create a validation error with context
    pub fn validation_error(field: &str, message: &str, value: Option<String>, component: &str) -> Self {
        Self::ValidationError {
            field: field.to_string(),
            message: message.to_string(),
            value,
            context: ErrorContext::new(component, "validation"),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create an authentication error with context
    pub fn auth_error(operation: &str, reason: &str, user_id: Option<String>, component: &str) -> Self {
        Self::AuthError {
            operation: operation.to_string(),
            reason: reason.to_string(),
            user_id,
            context: ErrorContext::new(component, operation),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create a TEE error with context
    pub fn tee_error(kind: TEEErrorKind, details: &str, component: &str) -> Self {
        Self::TEEError {
            kind,
            details: details.to_string(),
            context: ErrorContext::new(component, "tee_operation"),
            backtrace: create_backtrace(),
        }
    }
    
    /// Create a configuration error with context
    pub fn config_error(
        kind: ConfigErrorKind, 
        parameter: &str, 
        expected: Option<String>, 
        actual: Option<String>, 
        component: &str
    ) -> Self {
        Self::ConfigError {
            kind,
            parameter: parameter.to_string(),
            expected,
            actual,
            context: ErrorContext::new(component, "configuration"),
            backtrace: create_backtrace(),
        }
    }
    
    /// Get the error context
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::CryptoError { context, .. } => context,
            Self::MemoryViolation { context, .. } => context,
            Self::KeyManagementError { context, .. } => context,
            Self::TEEError { context, .. } => context,
            Self::NetworkError { context, .. } => context,
            Self::ValidationError { context, .. } => context,
            Self::AuthError { context, .. } => context,
            Self::AuditError { context, .. } => context,
            Self::ConfigError { context, .. } => context,
            Self::InternalError { context, .. } => context,
        }
    }
    
    /// Check if error is security-critical
    pub fn is_security_critical(&self) -> bool {
        matches!(self, 
            Self::CryptoError { .. } |
            Self::MemoryViolation { .. } |
            Self::KeyManagementError { .. } |
            Self::AuthError { .. }
        )
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::CryptoError { .. } => ErrorSeverity::Critical,
            Self::MemoryViolation { .. } => ErrorSeverity::Critical,
            Self::KeyManagementError { .. } => ErrorSeverity::High,
            Self::TEEError { .. } => ErrorSeverity::High,
            Self::AuthError { .. } => ErrorSeverity::High,
            Self::NetworkError { .. } => ErrorSeverity::Medium,
            Self::ValidationError { .. } => ErrorSeverity::Medium,
            Self::AuditError { .. } => ErrorSeverity::Medium,
            Self::ConfigError { .. } => ErrorSeverity::Low,
            Self::InternalError { .. } => ErrorSeverity::High,
        }
    }
    
    /// Convert error to audit event
    pub fn to_audit_event(&self) -> crate::security::AuditEvent {
        use crate::security::AuditEvent;
        
        match self {
            Self::CryptoError { operation, details, .. } => {
                AuditEvent::SecurityViolation {
                    violation_type: format!("crypto_error_{}", operation),
                    details: details.clone(),
                }
            },
            Self::KeyManagementError { kind, key_id, .. } => {
                AuditEvent::SecurityViolation {
                    violation_type: format!("key_management_{:?}", kind),
                    details: format!("Key ID: {:?}", key_id),
                }
            },
            Self::AuthError { operation, reason: _, user_id, .. } => { // reason 保留以备将来使用
                AuditEvent::Authentication {
                    user_id: user_id.clone().unwrap_or_default(),
                    success: false,
                    method: operation.clone(),
                }
            },
            _ => AuditEvent::SecurityViolation {
                violation_type: "general_error".to_string(),
                details: self.to_string(),
            },
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Result type alias for security operations
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Create backtrace based on feature availability
#[cfg(feature = "std_backtrace")]
fn create_backtrace() -> Backtrace {
    Backtrace::capture()
}

#[cfg(not(feature = "std_backtrace"))]
fn create_backtrace() -> Backtrace {
    "Backtrace not available (compile with std_backtrace feature)".to_string()
}

/// Error handler for centralized error processing
pub struct ErrorHandler {
    audit_logger: Option<std::sync::Arc<crate::security::AuditLogger>>,
}

impl ErrorHandler {
    /// Create new error handler
    pub fn new(audit_logger: Option<std::sync::Arc<crate::security::AuditLogger>>) -> Self {
        Self { audit_logger }
    }
    
    /// Handle error with logging and optional audit
    pub fn handle_error(&self, error: &SecurityError) {
        // Log error details
        if error.is_security_critical() {
            eprintln!("CRITICAL SECURITY ERROR: {}", error);
            
            // Audit security-critical errors
            if let Some(logger) = &self.audit_logger {
                logger.log_critical(error.to_audit_event(), &error.context().component);
            }
        } else {
            eprintln!("ERROR: {}", error);
            
            // Audit non-critical errors at warning level
            if let Some(logger) = &self.audit_logger {
                logger.log_warning(error.to_audit_event(), &error.context().component);
            }
        }
        
        // Print backtrace in debug mode
        #[cfg(debug_assertions)]
        {
            eprintln!("Backtrace: {}", error.context().component);
        }
    }
}

/// Error recovery strategies
pub trait ErrorRecovery {
    /// Attempt to recover from the error
    fn recover(&self, error: &SecurityError) -> SecurityResult<()>;
    
    /// Check if error is recoverable
    fn is_recoverable(&self, error: &SecurityError) -> bool;
}

/// Default error recovery implementation
pub struct DefaultErrorRecovery;

impl ErrorRecovery for DefaultErrorRecovery {
    fn recover(&self, _error: &SecurityError) -> SecurityResult<()> {
        // Basic recovery: log and continue
        Ok(())
    }
    
    fn is_recoverable(&self, error: &SecurityError) -> bool {
        // Critical security errors are generally not recoverable
        !error.is_security_critical()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_context_creation() {
        let context = ErrorContext::new("test_component", "test_operation")
            .with_metadata("key", "value")
            .with_session_id("session_123".to_string());
        
        assert_eq!(context.component, "test_component");
        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.metadata.get("key"), Some(&"value".to_string()));
        assert_eq!(context.session_id, Some("session_123".to_string()));
    }
    
    #[test]
    fn test_crypto_error_creation() {
        let error = SecurityError::crypto_error(
            "signature_verification", 
            "Invalid signature format", 
            "crypto_module"
        );
        
        assert!(error.is_security_critical());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context().component, "crypto_module");
    }
    
    #[test]
    fn test_key_management_error() {
        let error = SecurityError::key_management_error(
            KeyErrorKind::NotFound,
            Some("key_123".to_string()),
            "key_manager"
        );
        
        assert!(error.is_security_critical());
        assert_eq!(error.severity(), ErrorSeverity::High);
    }
    
    #[test]
    fn test_validation_error() {
        let error = SecurityError::validation_error(
            "password_length",
            "Password must be at least 8 characters",
            Some("123".to_string()),
            "auth_module"
        );
        
        assert!(!error.is_security_critical());
        assert_eq!(error.severity(), ErrorSeverity::Medium);
    }
    
    #[test]
    fn test_error_serialization() {
        let error = SecurityError::crypto_error(
            "test_operation",
            "test_details",
            "test_component"
        );
        
        // Test that error can be serialized (backtrace is skipped)
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(serialized.contains("CryptoError"));
        assert!(serialized.contains("test_operation"));
    }
    
    #[test]
    fn test_error_recovery() {
        let recovery = DefaultErrorRecovery;
        let critical_error = SecurityError::crypto_error(
            "test", "test", "test"
        );
        let non_critical_error = SecurityError::config_error(
            ConfigErrorKind::InvalidValue,
            "test_param",
            None,
            None,
            "config"
        );
        
        assert!(!recovery.is_recoverable(&critical_error));
        assert!(recovery.is_recoverable(&non_critical_error));
    }
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::CryptoError { operation, details, .. } => {
                write!(f, "Cryptographic operation failed: {} - {}", operation, details)
            },
            SecurityError::MemoryViolation { address, operation, .. } => {
                write!(f, "Memory protection violation at {:#x} during {}", address, operation)
            },
            SecurityError::KeyManagementError { kind, key_id, .. } => {
                match key_id {
                    Some(id) => write!(f, "Key management error: {:?} for key {}", kind, id),
                    None => write!(f, "Key management error: {:?}", kind),
                }
            },
            SecurityError::TEEError { kind, details, .. } => {
                write!(f, "TEE operation error: {:?} - {}", kind, details)
            },
            SecurityError::NetworkError { kind, endpoint, status_code, .. } => {
                match status_code {
                    Some(code) => write!(f, "Network error: {:?} - {} (status: {})", kind, endpoint, code),
                    None => write!(f, "Network error: {:?} - {}", kind, endpoint),
                }
            },
            SecurityError::ValidationError { field, message, .. } => {
                write!(f, "Validation failed for {}: {}", field, message)
            },
            SecurityError::AuthError { operation, reason, user_id, .. } => {
                match user_id {
                    Some(id) => write!(f, "Authentication error for user {}: {} - {}", id, operation, reason),
                    None => write!(f, "Authentication error: {} - {}", operation, reason),
                }
            },
            SecurityError::AuditError { kind, details, .. } => {
                write!(f, "Audit system error: {:?} - {}", kind, details)
            },
            SecurityError::ConfigError { kind, parameter, expected, actual, .. } => {
                match (expected, actual) {
                    (Some(exp), Some(act)) => write!(f, "Configuration error: {:?} - {} (expected: {}, actual: {})", kind, parameter, exp, act),
                    (Some(exp), None) => write!(f, "Configuration error: {:?} - {} (expected: {})", kind, parameter, exp),
                    _ => write!(f, "Configuration error: {:?} - {}", kind, parameter),
                }
            },
            SecurityError::InternalError { component, message, .. } => {
                write!(f, "Internal error in {}: {}", component, message)
            },
        }
    }
}

impl std::error::Error for SecurityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // We could implement chaining here if needed
    }
}