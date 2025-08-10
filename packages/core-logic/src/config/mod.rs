// Licensed to AirAccount under the Apache License, Version 2.0
// Enhanced configuration system with validation and hot-reload support

pub mod validator;
pub mod environment;
pub mod hot_reload;

pub use validator::*;
pub use environment::*;
pub use hot_reload::*;

use crate::error::{SecurityError, SecurityResult, ConfigErrorKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Enhanced security configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSecurityConfig {
    /// Constant time operations enabled
    pub enable_constant_time: bool,
    
    /// Memory protection enabled
    pub enable_memory_protection: bool,
    
    /// Audit logging configuration
    pub audit_config: AuditConfig,
    
    /// Entropy configuration
    pub entropy_config: EntropyConfig,
    
    /// Key derivation configuration
    pub kdf_config: KdfConfig,
    
    /// Performance configuration
    pub performance_config: PerformanceConfig,
    
    /// Environment-specific settings
    pub environment: Environment,
    
    /// Hot-reload configuration
    pub hot_reload: HotReloadConfig,
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging
    pub enabled: bool,
    
    /// Audit file path
    pub file_path: Option<String>,
    
    /// Enable secure audit (encrypted)
    pub secure_mode: bool,
    
    /// Audit encryption key
    #[serde(skip_serializing)]
    pub encryption_key: Option<[u8; 32]>,
    
    /// Maximum audit file size (MB)
    pub max_file_size_mb: u32,
    
    /// Audit log rotation count
    pub rotation_count: u8,
    
    /// Batch audit configuration
    pub batch_config: Option<BatchAuditConfigData>,
}

/// Batch audit configuration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAuditConfigData {
    /// Batch size
    pub batch_size: usize,
    
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
    
    /// Maximum queue size
    pub max_queue_size: usize,
    
    /// Worker threads count
    pub worker_threads: usize,
}

/// Entropy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyConfig {
    /// Enable TEE entropy source
    pub use_tee_source: bool,
    
    /// Enable hardware RNG if available
    pub use_hardware_rng: bool,
    
    /// Enable timing jitter collection
    pub use_timing_jitter: bool,
    
    /// Enable physical noise collection
    pub use_physical_noise: bool,
    
    /// Minimum entropy quality threshold (0.0-1.0)
    pub min_quality_threshold: f64,
    
    /// Entropy pool size in bytes
    pub pool_size: usize,
    
    /// Collection timeout in milliseconds
    pub collection_timeout_ms: u64,
}

/// Key derivation function configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfConfig {
    /// Default KDF algorithm
    pub default_algorithm: String,
    
    /// PBKDF2 iterations (10,000-1,000,000)
    pub pbkdf2_iterations: u32,
    
    /// Argon2 memory cost in KB (1,024-1,048,576)
    pub argon2_memory_cost: u32,
    
    /// Argon2 time cost (1-100)
    pub argon2_time_cost: u32,
    
    /// Argon2 parallelism (1-16)
    pub argon2_parallelism: u32,
    
    /// Salt size in bytes (16-64)
    pub salt_size: usize,
    
    /// Output key length in bytes (16-64)
    pub output_length: usize,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable batch operations
    pub enable_batch_operations: bool,
    
    /// Enable memory pooling
    pub enable_memory_pool: bool,
    
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    
    /// Worker thread pool size
    pub worker_threads: usize,
    
    /// Memory pool configuration
    pub memory_pool: MemoryPoolConfig,
    
    /// Cache configuration
    pub cache: CacheConfig,
}

/// Memory pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    /// Enable memory pool
    pub enabled: bool,
    
    /// Pool sizes and max blocks
    pub pool_configs: Vec<(usize, usize)>, // (size, max_blocks)
    
    /// Large allocation threshold
    pub large_alloc_threshold: usize,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    
    /// Maximum cache size in MB
    pub max_size_mb: u32,
    
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    
    /// Cache eviction policy
    pub eviction_policy: String,
}

/// Environment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

/// Hot reload configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotReloadConfig {
    /// Enable hot reload
    pub enabled: bool,
    
    /// Configuration file watch paths
    pub watch_paths: Vec<PathBuf>,
    
    /// Reload debounce time in milliseconds
    pub debounce_ms: u64,
    
    /// Hot-reloadable sections
    pub reloadable_sections: Vec<String>,
}

impl Default for EnhancedSecurityConfig {
    fn default() -> Self {
        Self {
            enable_constant_time: true,
            enable_memory_protection: true,
            audit_config: AuditConfig::default(),
            entropy_config: EntropyConfig::default(),
            kdf_config: KdfConfig::default(),
            performance_config: PerformanceConfig::default(),
            environment: Environment::Development,
            hot_reload: HotReloadConfig::default(),
        }
    }
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            file_path: Some("audit.log".to_string()),
            secure_mode: false,
            encryption_key: None,
            max_file_size_mb: 100,
            rotation_count: 5,
            batch_config: Some(BatchAuditConfigData {
                batch_size: 100,
                flush_interval_ms: 1000,
                max_queue_size: 10000,
                worker_threads: 2,
            }),
        }
    }
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self {
            use_tee_source: true,
            use_hardware_rng: true,
            use_timing_jitter: true,
            use_physical_noise: false,
            min_quality_threshold: 0.8,
            pool_size: 4096,
            collection_timeout_ms: 5000,
        }
    }
}

impl Default for KdfConfig {
    fn default() -> Self {
        Self {
            default_algorithm: "Argon2id".to_string(),
            pbkdf2_iterations: 100_000,
            argon2_memory_cost: 65_536, // 64MB
            argon2_time_cost: 3,
            argon2_parallelism: 4,
            salt_size: 32,
            output_length: 32,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_batch_operations: true,
            enable_memory_pool: true,
            enable_simd: true,
            worker_threads: num_cpus_fallback(),
            memory_pool: MemoryPoolConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pool_configs: vec![
                (32, 1000),
                (64, 800),
                (128, 600),
                (256, 400),
                (512, 200),
                (1024, 100),
                (2048, 50),
                (4096, 25),
            ],
            large_alloc_threshold: 8192,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_mb: 64,
            ttl_seconds: 3600,
            eviction_policy: "LRU".to_string(),
        }
    }
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for security
            watch_paths: vec![],
            debounce_ms: 1000,
            reloadable_sections: vec![
                "performance".to_string(),
                "cache".to_string(),
                "audit".to_string(),
            ],
        }
    }
}

impl Environment {
    /// Check if environment is production
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }
    
    /// Check if environment is development/testing
    pub fn is_development(&self) -> bool {
        matches!(self, Environment::Development | Environment::Testing)
    }
    
    /// Get environment from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "testing" | "test" => Ok(Environment::Testing),
            "staging" | "stage" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

/// Configuration manager for centralized config handling
pub struct ConfigManager {
    config: EnhancedSecurityConfig,
    validator: ConfigValidator,
    _hot_reload_handler: Option<HotReloadHandler>,
}

impl ConfigManager {
    /// Create new configuration manager
    pub fn new(config: EnhancedSecurityConfig) -> SecurityResult<Self> {
        let validator = ConfigValidator::new();
        
        // Validate initial configuration
        validator.validate(&config)?;
        
        let hot_reload_handler = if config.hot_reload.enabled {
            Some(HotReloadHandler::new(config.hot_reload.clone())?)
        } else {
            None
        };
        
        Ok(Self {
            config,
            validator,
            _hot_reload_handler: hot_reload_handler,
        })
    }
    
    /// Load configuration from file
    pub fn load_from_file(path: &std::path::Path) -> SecurityResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "config_file",
                Some(path.display().to_string()),
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        let config: EnhancedSecurityConfig = serde_json::from_str(&content)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "toml_parsing",
                None,
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        Self::new(config)
    }
    
    /// Get current configuration
    pub fn config(&self) -> &EnhancedSecurityConfig {
        &self.config
    }
    
    /// Update configuration section
    pub fn update_section<T: serde::Serialize>(
        &mut self, 
        section: &str, 
        value: T
    ) -> SecurityResult<()> {
        // Check if section is hot-reloadable
        if !self.config.hot_reload.reloadable_sections.contains(&section.to_string()) {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                section,
                Some("hot-reloadable".to_string()),
                Some("not allowed".to_string()),
                "config_manager"
            ));
        }
        
        // Serialize new section
        let section_value = serde_json::to_value(value)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                section,
                None,
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        // Create a temporary config with updated section
        let mut config_value = serde_json::to_value(&self.config)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "full_config",
                None,
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        if let Some(config_obj) = config_value.as_object_mut() {
            config_obj.insert(section.to_string(), section_value);
        }
        
        let updated_config: EnhancedSecurityConfig = serde_json::from_value(config_value)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "updated_config",
                None,
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        // Validate updated configuration
        self.validator.validate(&updated_config)?;
        
        // Apply the update
        self.config = updated_config;
        
        Ok(())
    }
    
    /// Save configuration to file
    pub fn save_to_file(&self, path: &std::path::Path) -> SecurityResult<()> {
        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "toml_serialization",
                None,
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        std::fs::write(path, content)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "file_write",
                Some(path.display().to_string()),
                Some(e.to_string()),
                "config_manager"
            ))?;
        
        Ok(())
    }
    
    /// Get environment-specific configuration
    pub fn get_environment_config(&self) -> HashMap<String, String> {
        let mut env_config = HashMap::new();
        
        match self.config.environment {
            Environment::Development => {
                env_config.insert("LOG_LEVEL".to_string(), "DEBUG".to_string());
                env_config.insert("ENABLE_TRACING".to_string(), "true".to_string());
            },
            Environment::Testing => {
                env_config.insert("LOG_LEVEL".to_string(), "INFO".to_string());
                env_config.insert("ENABLE_MOCK_TEE".to_string(), "true".to_string());
            },
            Environment::Staging => {
                env_config.insert("LOG_LEVEL".to_string(), "WARN".to_string());
                env_config.insert("ENABLE_MONITORING".to_string(), "true".to_string());
            },
            Environment::Production => {
                env_config.insert("LOG_LEVEL".to_string(), "ERROR".to_string());
                env_config.insert("ENABLE_SECURITY_HARDENING".to_string(), "true".to_string());
            },
        }
        
        env_config
    }
}

/// Helper function to get CPU count without external dependencies
fn num_cpus_fallback() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4) // Fallback to 4 cores
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config_creation() {
        let config = EnhancedSecurityConfig::default();
        
        assert!(config.enable_constant_time);
        assert!(config.enable_memory_protection);
        assert!(config.audit_config.enabled);
        assert_eq!(config.environment, Environment::Development);
    }
    
    #[test]
    fn test_environment_detection() {
        assert!(Environment::Development.is_development());
        assert!(Environment::Testing.is_development());
        assert!(!Environment::Production.is_development());
        assert!(Environment::Production.is_production());
    }
    
    #[test]
    fn test_environment_from_string() {
        assert_eq!(Environment::from_str("dev").unwrap(), Environment::Development);
        assert_eq!(Environment::from_str("production").unwrap(), Environment::Production);
        assert!(Environment::from_str("invalid").is_err());
    }
    
    #[test]
    fn test_config_validation() {
        let config = EnhancedSecurityConfig::default();
        let validator = ConfigValidator::new();
        
        assert!(validator.validate(&config).is_ok());
    }
}