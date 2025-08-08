pub mod constant_time;
pub mod memory_protection;
pub mod audit;

pub use constant_time::{SecureBytes, ConstantTimeOps, SecureRng};
pub use memory_protection::{SecureMemory, StackCanary, MemoryGuard, SecureString};
pub use audit::{AuditLogger, AuditEvent, AuditLevel, AuditLogEntry, audit_log, init_global_audit_logger};

use std::sync::Arc;

#[derive(Clone)]
pub struct SecurityConfig {
    pub enable_constant_time: bool,
    pub enable_memory_protection: bool,
    pub enable_audit_logging: bool,
    pub audit_file_path: Option<String>,
    pub enable_secure_audit: bool,
    pub audit_encryption_key: Option<[u8; 32]>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_constant_time: true,
            enable_memory_protection: true,
            enable_audit_logging: true,
            audit_file_path: Some("audit.log".to_string()),
            enable_secure_audit: false,
            audit_encryption_key: None,
        }
    }
}

#[derive(Clone)]
pub struct SecurityManager {
    config: SecurityConfig,
    audit_logger: Option<Arc<std::sync::Mutex<AuditLogger>>>,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Self {
        let audit_logger = if config.enable_audit_logging {
            Some(init_global_audit_logger())
        } else {
            None
        };
        
        if config.enable_memory_protection {
            MemoryGuard::enable_protection();
        } else {
            MemoryGuard::disable_protection();
        }
        
        Self {
            config,
            audit_logger,
        }
    }
    
    pub fn is_constant_time_enabled(&self) -> bool {
        self.config.enable_constant_time
    }
    
    pub fn is_memory_protection_enabled(&self) -> bool {
        self.config.enable_memory_protection
    }
    
    pub fn is_audit_logging_enabled(&self) -> bool {
        self.config.enable_audit_logging
    }
    
    pub fn audit_security_event(&self, event: AuditEvent, component: &str) {
        if let Some(logger) = &self.audit_logger {
            if let Ok(logger) = logger.lock() {
                logger.log_security(event, component);
            }
        }
    }
    
    pub fn audit_info(&self, event: AuditEvent, component: &str) {
        if let Some(logger) = &self.audit_logger {
            if let Ok(logger) = logger.lock() {
                logger.log_info(event, component);
            }
        }
    }
    
    pub fn audit_warning(&self, event: AuditEvent, component: &str) {
        if let Some(logger) = &self.audit_logger {
            if let Ok(logger) = logger.lock() {
                logger.log_warning(event, component);
            }
        }
    }
    
    pub fn audit_error(&self, event: AuditEvent, component: &str) {
        if let Some(logger) = &self.audit_logger {
            if let Ok(logger) = logger.lock() {
                logger.log_error(event, component);
            }
        }
    }
    
    pub fn create_secure_memory(&self, size: usize) -> Result<SecureMemory, &'static str> {
        let memory = SecureMemory::new(size)?;
        
        if self.config.enable_audit_logging {
            self.audit_info(
                AuditEvent::MemoryAllocation { 
                    size, 
                    secure: true 
                },
                "security_manager"
            );
        }
        
        Ok(memory)
    }
    
    pub fn create_secure_rng(&self) -> Result<SecureRng, &'static str> {
        let rng = SecureRng::new()?;
        
        if self.config.enable_audit_logging {
            self.audit_security_event(
                AuditEvent::TEEOperation {
                    operation: "secure_rng_init".to_string(),
                    duration_ms: 0,
                    success: true,
                },
                "security_manager"
            );
        }
        
        Ok(rng)
    }
    
    pub fn validate_security_invariants(&self) -> Result<(), &'static str> {
        if self.config.enable_memory_protection && !MemoryGuard::is_protection_enabled() {
            self.audit_error(
                AuditEvent::SecurityViolation {
                    violation_type: "memory_protection_disabled".to_string(),
                    details: "Memory protection should be enabled but is disabled".to_string(),
                },
                "security_manager"
            );
            return Err("Memory protection invariant violated");
        }
        
        Ok(())
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_manager_creation() {
        let config = SecurityConfig::default();
        let manager = SecurityManager::new(config);
        
        assert!(manager.is_constant_time_enabled());
        assert!(manager.is_memory_protection_enabled());
        assert!(manager.is_audit_logging_enabled());
    }
    
    #[test]
    fn test_security_manager_memory_allocation() {
        let manager = SecurityManager::default();
        let memory = manager.create_secure_memory(1024).unwrap();
        
        assert_eq!(memory.size(), 1024);
    }
    
    #[test]
    fn test_security_invariants() {
        let manager = SecurityManager::default();
        assert!(manager.validate_security_invariants().is_ok());
    }
}