pub mod security;
pub mod wallet;  // 新增：集成eth_wallet功能
pub mod proto;   // 新增：协议定义层
pub mod tee;     // 新增：TEE适配层

pub use security::{
    SecurityManager, SecurityConfig,
    SecureBytes, ConstantTimeOps, SecureRng,
    SecureMemory, StackCanary, MemoryGuard, SecureString,
    AuditLogger, AuditEvent, AuditLevel, AuditLogEntry,
};

// 导出eth_wallet集成模块
pub use wallet::{AirAccountWallet, WalletManager, WalletError};
pub use proto::{WalletCommand, EthTransaction, WalletInfo};
pub use tee::{TEEAdapter, TEEError};

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum CoreError {
    SecurityError(String),
    MemoryError(String),
    CryptographicError(String),
    ConfigurationError(String),
    ValidationError(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::SecurityError(msg) => write!(f, "Security error: {}", msg),
            CoreError::MemoryError(msg) => write!(f, "Memory error: {}", msg),
            CoreError::CryptographicError(msg) => write!(f, "Cryptographic error: {}", msg),
            CoreError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            CoreError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl Error for CoreError {}

pub type Result<T> = std::result::Result<T, CoreError>;

pub struct CoreContext {
    security_manager: SecurityManager,
    initialized: bool,
}

impl CoreContext {
    pub fn new(security_config: SecurityConfig) -> Self {
        let security_manager = SecurityManager::new(security_config);
        
        Self {
            security_manager,
            initialized: true,
        }
    }
    
    pub fn security_manager(&self) -> &SecurityManager {
        &self.security_manager
    }
    
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    pub fn validate(&self) -> Result<()> {
        if !self.initialized {
            return Err(CoreError::ValidationError("Context not initialized".to_string()));
        }
        
        self.security_manager
            .validate_security_invariants()
            .map_err(|e| CoreError::SecurityError(e.to_string()))?;
        
        Ok(())
    }
}

impl Default for CoreContext {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

pub fn init_with_security_config(config: SecurityConfig) -> Result<CoreContext> {
    let context = CoreContext::new(config);
    context.validate()?;
    
    context.security_manager().audit_info(
        AuditEvent::TEEOperation {
            operation: "core_init".to_string(),
            duration_ms: 0,
            success: true,
        },
        "core_context"
    );
    
    Ok(context)
}

pub fn init_default() -> Result<CoreContext> {
    init_with_security_config(SecurityConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_context_creation() {
        let context = CoreContext::default();
        assert!(context.is_initialized());
        assert!(context.validate().is_ok());
    }
    
    #[test]
    fn test_init_functions() {
        let context = init_default().unwrap();
        assert!(context.is_initialized());
        
        let custom_config = SecurityConfig {
            enable_constant_time: false,
            ..SecurityConfig::default()
        };
        
        let context = init_with_security_config(custom_config).unwrap();
        assert!(context.is_initialized());
        assert!(!context.security_manager().is_constant_time_enabled());
    }
}