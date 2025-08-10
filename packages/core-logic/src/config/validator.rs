// Licensed to AirAccount under the Apache License, Version 2.0
// Configuration validation system

use super::{EnhancedSecurityConfig, Environment};
use crate::error::{SecurityError, SecurityResult, ConfigErrorKind};

/// Configuration validator for security and consistency checks
pub struct ConfigValidator {
    /// Validation rules
    rules: Vec<Box<dyn ValidationRule>>,
}

/// Validation rule trait
pub trait ValidationRule {
    /// Validate configuration section
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()>;
    
    /// Get rule name
    fn name(&self) -> &str;
    
    /// Get rule description
    fn description(&self) -> &str;
}

impl ConfigValidator {
    /// Create new configuration validator
    pub fn new() -> Self {
        let rules: Vec<Box<dyn ValidationRule>> = vec![
            Box::new(KdfValidationRule),
            Box::new(EntropyValidationRule),
            Box::new(AuditValidationRule),
            Box::new(PerformanceValidationRule),
            Box::new(SecurityValidationRule),
            Box::new(EnvironmentValidationRule),
        ];
        
        Self { rules }
    }
    
    /// Validate entire configuration
    pub fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        for rule in &self.rules {
            if let Err(e) = rule.validate(config) {
                eprintln!("Validation failed for rule '{}': {}", rule.name(), e);
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    /// Add custom validation rule
    pub fn add_rule(&mut self, rule: Box<dyn ValidationRule>) {
        self.rules.push(rule);
    }
    
    /// Get all validation rules
    pub fn rules(&self) -> &[Box<dyn ValidationRule>] {
        &self.rules
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// KDF configuration validation rule
struct KdfValidationRule;

impl ValidationRule for KdfValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        let kdf = &config.kdf_config;
        
        // Validate PBKDF2 iterations (minimum 10,000, maximum 1,000,000)
        if kdf.pbkdf2_iterations < 10_000 || kdf.pbkdf2_iterations > 1_000_000 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "pbkdf2_iterations",
                Some("10,000-1,000,000".to_string()),
                Some(kdf.pbkdf2_iterations.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate Argon2 memory cost (1MB to 1GB)
        if kdf.argon2_memory_cost < 1_024 || kdf.argon2_memory_cost > 1_048_576 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "argon2_memory_cost",
                Some("1,024-1,048,576 KB".to_string()),
                Some(kdf.argon2_memory_cost.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate Argon2 time cost (1-100)
        if kdf.argon2_time_cost == 0 || kdf.argon2_time_cost > 100 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "argon2_time_cost",
                Some("1-100".to_string()),
                Some(kdf.argon2_time_cost.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate Argon2 parallelism (1-16)
        if kdf.argon2_parallelism == 0 || kdf.argon2_parallelism > 16 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "argon2_parallelism",
                Some("1-16".to_string()),
                Some(kdf.argon2_parallelism.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate salt size (16-64 bytes)
        if kdf.salt_size < 16 || kdf.salt_size > 64 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "salt_size",
                Some("16-64 bytes".to_string()),
                Some(kdf.salt_size.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate output length (16-64 bytes)
        if kdf.output_length < 16 || kdf.output_length > 64 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "output_length",
                Some("16-64 bytes".to_string()),
                Some(kdf.output_length.to_string()),
                "kdf_validator"
            ));
        }
        
        // Validate algorithm name
        let valid_algorithms = ["Argon2id", "Argon2i", "Argon2d", "PBKDF2", "scrypt"];
        if !valid_algorithms.contains(&kdf.default_algorithm.as_str()) {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "default_algorithm",
                Some(valid_algorithms.join(", ")),
                Some(kdf.default_algorithm.clone()),
                "kdf_validator"
            ));
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "KDF Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates key derivation function parameters for security compliance"
    }
}

/// Entropy configuration validation rule
struct EntropyValidationRule;

impl ValidationRule for EntropyValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        let entropy = &config.entropy_config;
        
        // At least one entropy source must be enabled
        if !entropy.use_tee_source && !entropy.use_hardware_rng && !entropy.use_timing_jitter && !entropy.use_physical_noise {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "entropy_sources",
                Some("at least one source enabled".to_string()),
                Some("all sources disabled".to_string()),
                "entropy_validator"
            ));
        }
        
        // Validate quality threshold (0.0-1.0)
        if entropy.min_quality_threshold < 0.0 || entropy.min_quality_threshold > 1.0 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "min_quality_threshold",
                Some("0.0-1.0".to_string()),
                Some(entropy.min_quality_threshold.to_string()),
                "entropy_validator"
            ));
        }
        
        // Validate pool size (minimum 1KB, maximum 1MB)
        if entropy.pool_size < 1024 || entropy.pool_size > 1_048_576 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "pool_size",
                Some("1,024-1,048,576 bytes".to_string()),
                Some(entropy.pool_size.to_string()),
                "entropy_validator"
            ));
        }
        
        // Validate collection timeout (100ms to 60s)
        if entropy.collection_timeout_ms < 100 || entropy.collection_timeout_ms > 60_000 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "collection_timeout_ms",
                Some("100-60,000 ms".to_string()),
                Some(entropy.collection_timeout_ms.to_string()),
                "entropy_validator"
            ));
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Entropy Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates entropy collection parameters for cryptographic security"
    }
}

/// Audit configuration validation rule
struct AuditValidationRule;

impl ValidationRule for AuditValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        let audit = &config.audit_config;
        
        // If audit is enabled, file path must be provided
        if audit.enabled && audit.file_path.is_none() {
            return Err(SecurityError::config_error(
                ConfigErrorKind::MissingRequired,
                "audit_file_path",
                Some("file path when enabled".to_string()),
                None,
                "audit_validator"
            ));
        }
        
        // If secure mode is enabled, encryption key must be provided
        if audit.secure_mode && audit.encryption_key.is_none() {
            return Err(SecurityError::config_error(
                ConfigErrorKind::MissingRequired,
                "audit_encryption_key",
                Some("32-byte key when secure mode enabled".to_string()),
                None,
                "audit_validator"
            ));
        }
        
        // Validate file size limits (1MB to 10GB)
        if audit.max_file_size_mb == 0 || audit.max_file_size_mb > 10_240 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "max_file_size_mb",
                Some("1-10,240 MB".to_string()),
                Some(audit.max_file_size_mb.to_string()),
                "audit_validator"
            ));
        }
        
        // Validate rotation count (1-100)
        if audit.rotation_count == 0 || audit.rotation_count > 100 {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "rotation_count",
                Some("1-100".to_string()),
                Some(audit.rotation_count.to_string()),
                "audit_validator"
            ));
        }
        
        // Validate batch configuration if present
        if let Some(batch_config) = &audit.batch_config {
            if batch_config.batch_size == 0 || batch_config.batch_size > 10_000 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "batch_size",
                    Some("1-10,000".to_string()),
                    Some(batch_config.batch_size.to_string()),
                    "audit_validator"
                ));
            }
            
            if batch_config.flush_interval_ms < 100 || batch_config.flush_interval_ms > 300_000 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "flush_interval_ms",
                    Some("100-300,000 ms".to_string()),
                    Some(batch_config.flush_interval_ms.to_string()),
                    "audit_validator"
                ));
            }
            
            if batch_config.max_queue_size < 100 || batch_config.max_queue_size > 1_000_000 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "max_queue_size",
                    Some("100-1,000,000".to_string()),
                    Some(batch_config.max_queue_size.to_string()),
                    "audit_validator"
                ));
            }
            
            if batch_config.worker_threads == 0 || batch_config.worker_threads > 16 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "worker_threads",
                    Some("1-16".to_string()),
                    Some(batch_config.worker_threads.to_string()),
                    "audit_validator"
                ));
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Audit Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates audit logging configuration for compliance and security"
    }
}

/// Performance configuration validation rule
struct PerformanceValidationRule;

impl ValidationRule for PerformanceValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        let perf = &config.performance_config;
        
        // Validate worker thread count (1 to CPU count * 4)
        let max_threads = num_cpus::get() * 4;
        if perf.worker_threads == 0 || perf.worker_threads > max_threads {
            return Err(SecurityError::config_error(
                ConfigErrorKind::InvalidValue,
                "worker_threads",
                Some(format!("1-{}", max_threads)),
                Some(perf.worker_threads.to_string()),
                "performance_validator"
            ));
        }
        
        // Validate memory pool configuration
        if perf.memory_pool.enabled {
            if perf.memory_pool.pool_configs.is_empty() {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::ValidationFailed,
                    "pool_configs",
                    Some("non-empty when memory pool enabled".to_string()),
                    Some("empty".to_string()),
                    "performance_validator"
                ));
            }
            
            // Validate each pool configuration
            for (size, max_blocks) in &perf.memory_pool.pool_configs {
                if *size == 0 || *size > 1_048_576 {  // Max 1MB
                    return Err(SecurityError::config_error(
                        ConfigErrorKind::InvalidValue,
                        "pool_size",
                        Some("1-1,048,576 bytes".to_string()),
                        Some(size.to_string()),
                        "performance_validator"
                    ));
                }
                
                if *max_blocks == 0 || *max_blocks > 100_000 {
                    return Err(SecurityError::config_error(
                        ConfigErrorKind::InvalidValue,
                        "max_blocks",
                        Some("1-100,000".to_string()),
                        Some(max_blocks.to_string()),
                        "performance_validator"
                    ));
                }
            }
        }
        
        // Validate cache configuration
        if perf.cache.enabled {
            if perf.cache.max_size_mb == 0 || perf.cache.max_size_mb > 10_240 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "cache_max_size_mb",
                    Some("1-10,240 MB".to_string()),
                    Some(perf.cache.max_size_mb.to_string()),
                    "performance_validator"
                ));
            }
            
            if perf.cache.ttl_seconds == 0 || perf.cache.ttl_seconds > 86_400 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "cache_ttl_seconds",
                    Some("1-86,400 seconds".to_string()),
                    Some(perf.cache.ttl_seconds.to_string()),
                    "performance_validator"
                ));
            }
            
            let valid_policies = ["LRU", "LFU", "FIFO"];
            if !valid_policies.contains(&perf.cache.eviction_policy.as_str()) {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "eviction_policy",
                    Some(valid_policies.join(", ")),
                    Some(perf.cache.eviction_policy.clone()),
                    "performance_validator"
                ));
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Performance Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates performance settings for optimal and safe operation"
    }
}

/// Security configuration validation rule
struct SecurityValidationRule;

impl ValidationRule for SecurityValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        // In production, both constant time and memory protection must be enabled
        if config.environment == Environment::Production {
            if !config.enable_constant_time {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::ValidationFailed,
                    "enable_constant_time",
                    Some("true in production".to_string()),
                    Some("false".to_string()),
                    "security_validator"
                ));
            }
            
            if !config.enable_memory_protection {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::ValidationFailed,
                    "enable_memory_protection",
                    Some("true in production".to_string()),
                    Some("false".to_string()),
                    "security_validator"
                ));
            }
            
            // Audit must be enabled in production
            if !config.audit_config.enabled {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::ValidationFailed,
                    "audit_enabled",
                    Some("true in production".to_string()),
                    Some("false".to_string()),
                    "security_validator"
                ));
            }
            
            // Secure audit mode recommended for production
            if !config.audit_config.secure_mode {
                eprintln!("WARNING: Secure audit mode is recommended for production environments");
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Security Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates security settings for environment-appropriate protection"
    }
}

/// Environment configuration validation rule
struct EnvironmentValidationRule;

impl ValidationRule for EnvironmentValidationRule {
    fn validate(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        // Hot reload should be disabled in production
        if config.environment == Environment::Production && config.hot_reload.enabled {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "hot_reload_enabled",
                Some("false in production".to_string()),
                Some("true".to_string()),
                "environment_validator"
            ));
        }
        
        // Validate hot reload configuration if enabled
        if config.hot_reload.enabled {
            if config.hot_reload.debounce_ms < 100 || config.hot_reload.debounce_ms > 60_000 {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::InvalidValue,
                    "hot_reload_debounce_ms",
                    Some("100-60,000 ms".to_string()),
                    Some(config.hot_reload.debounce_ms.to_string()),
                    "environment_validator"
                ));
            }
            
            if config.hot_reload.watch_paths.is_empty() {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::MissingRequired,
                    "hot_reload_watch_paths",
                    Some("at least one path when enabled".to_string()),
                    None,
                    "environment_validator"
                ));
            }
            
            if config.hot_reload.reloadable_sections.is_empty() {
                return Err(SecurityError::config_error(
                    ConfigErrorKind::MissingRequired,
                    "hot_reload_sections",
                    Some("at least one section when enabled".to_string()),
                    None,
                    "environment_validator"
                ));
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "Environment Configuration"
    }
    
    fn description(&self) -> &str {
        "Validates environment-specific settings for security and operational compliance"
    }
}

// Helper function to get CPU count (we'll use a simple fallback since num_cpus isn't added yet)
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4) // Fallback to 4 cores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EnhancedSecurityConfig, Environment};
    
    #[test]
    fn test_kdf_validation() {
        let rule = KdfValidationRule;
        let mut config = EnhancedSecurityConfig::default();
        
        // Valid configuration should pass
        assert!(rule.validate(&config).is_ok());
        
        // Invalid PBKDF2 iterations should fail
        config.kdf_config.pbkdf2_iterations = 5000;
        assert!(rule.validate(&config).is_err());
        
        config.kdf_config.pbkdf2_iterations = 2_000_000;
        assert!(rule.validate(&config).is_err());
        
        // Reset to valid value
        config.kdf_config.pbkdf2_iterations = 100_000;
        assert!(rule.validate(&config).is_ok());
        
        // Invalid algorithm should fail
        config.kdf_config.default_algorithm = "MD5".to_string();
        assert!(rule.validate(&config).is_err());
    }
    
    #[test]
    fn test_entropy_validation() {
        let rule = EntropyValidationRule;
        let mut config = EnhancedSecurityConfig::default();
        
        // Valid configuration should pass
        assert!(rule.validate(&config).is_ok());
        
        // No entropy sources enabled should fail
        config.entropy_config.use_tee_source = false;
        config.entropy_config.use_hardware_rng = false;
        config.entropy_config.use_timing_jitter = false;
        config.entropy_config.use_physical_noise = false;
        assert!(rule.validate(&config).is_err());
        
        // Invalid quality threshold should fail
        config.entropy_config.use_tee_source = true;
        config.entropy_config.min_quality_threshold = 1.5;
        assert!(rule.validate(&config).is_err());
    }
    
    #[test]
    fn test_security_validation_production() {
        let rule = SecurityValidationRule;
        let mut config = EnhancedSecurityConfig::default();
        config.environment = Environment::Production;
        
        // Production with security disabled should fail
        config.enable_constant_time = false;
        assert!(rule.validate(&config).is_err());
        
        config.enable_constant_time = true;
        config.enable_memory_protection = false;
        assert!(rule.validate(&config).is_err());
        
        config.enable_memory_protection = true;
        config.audit_config.enabled = false;
        assert!(rule.validate(&config).is_err());
        
        // All security enabled should pass
        config.audit_config.enabled = true;
        assert!(rule.validate(&config).is_ok());
    }
    
    #[test]
    fn test_full_config_validation() {
        let validator = ConfigValidator::new();
        let config = EnhancedSecurityConfig::default();
        
        assert!(validator.validate(&config).is_ok());
        
        let mut prod_config = config.clone();
        prod_config.environment = Environment::Production;
        prod_config.hot_reload.enabled = true; // Should fail
        
        assert!(validator.validate(&prod_config).is_err());
    }
}