// Licensed to AirAccount under the Apache License, Version 2.0
// Environment-specific configuration management

use super::{EnhancedSecurityConfig, Environment};
use crate::error::{SecurityError, SecurityResult, ConfigErrorKind};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Environment configuration manager
pub struct EnvironmentManager {
    current_env: Environment,
    env_configs: HashMap<Environment, EnhancedSecurityConfig>,
    env_variables: HashMap<String, String>,
}

impl EnvironmentManager {
    /// Create new environment manager
    pub fn new() -> SecurityResult<Self> {
        let current_env = Self::detect_environment()?;
        let env_variables = Self::load_environment_variables();
        
        let mut env_configs = HashMap::new();
        
        // Load environment-specific configurations
        env_configs.insert(Environment::Development, Self::development_config());
        env_configs.insert(Environment::Testing, Self::testing_config());
        env_configs.insert(Environment::Staging, Self::staging_config());
        env_configs.insert(Environment::Production, Self::production_config());
        
        Ok(Self {
            current_env,
            env_configs,
            env_variables,
        })
    }
    
    /// Detect current environment from environment variables
    fn detect_environment() -> SecurityResult<Environment> {
        // Check various environment variables
        let env_indicators = [
            "AIRACCOUNT_ENV",
            "NODE_ENV",
            "ENVIRONMENT",
            "ENV",
            "DEPLOYMENT_ENV",
        ];
        
        for indicator in &env_indicators {
            if let Ok(env_value) = env::var(indicator) {
                match Environment::from_str(&env_value) {
                    Ok(environment) => return Ok(environment),
                    Err(_) => continue,
                }
            }
        }
        
        // Check for common production indicators
        if env::var("KUBERNETES_SERVICE_HOST").is_ok() ||
           env::var("AWS_EXECUTION_ENV").is_ok() ||
           env::var("DOCKER_CONTAINER").is_ok() {
            return Ok(Environment::Production);
        }
        
        // Check for development indicators
        if cfg!(debug_assertions) ||
           env::var("CARGO_PKG_NAME").is_ok() ||
           PathBuf::from("Cargo.toml").exists() {
            return Ok(Environment::Development);
        }
        
        // Default to development for safety
        Ok(Environment::Development)
    }
    
    /// Load all relevant environment variables
    fn load_environment_variables() -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        
        // System environment variables
        for (key, value) in env::vars() {
            if key.starts_with("AIRACCOUNT_") ||
               key.starts_with("SECURITY_") ||
               key.starts_with("TEE_") ||
               key.starts_with("CRYPTO_") {
                env_vars.insert(key, value);
            }
        }
        
        env_vars
    }
    
    /// Get configuration for current environment
    pub fn get_config(&self) -> SecurityResult<EnhancedSecurityConfig> {
        let mut config = self.env_configs.get(&self.current_env)
            .ok_or_else(|| SecurityError::config_error(
                ConfigErrorKind::MissingRequired,
                "environment_config",
                Some(format!("{:?}", self.current_env)),
                None,
                "environment_manager"
            ))?
            .clone();
        
        // Apply environment variable overrides
        self.apply_env_overrides(&mut config)?;
        
        Ok(config)
    }
    
    /// Get current environment
    pub fn current_environment(&self) -> Environment {
        self.current_env
    }
    
    /// Get environment variables
    pub fn environment_variables(&self) -> &HashMap<String, String> {
        &self.env_variables
    }
    
    /// Apply environment variable overrides to configuration
    fn apply_env_overrides(&self, config: &mut EnhancedSecurityConfig) -> SecurityResult<()> {
        // Security settings
        if let Some(value) = self.get_env_bool("AIRACCOUNT_ENABLE_CONSTANT_TIME") {
            config.enable_constant_time = value;
        }
        
        if let Some(value) = self.get_env_bool("AIRACCOUNT_ENABLE_MEMORY_PROTECTION") {
            config.enable_memory_protection = value;
        }
        
        // Audit configuration
        if let Some(path) = self.env_variables.get("AIRACCOUNT_AUDIT_FILE") {
            config.audit_config.file_path = Some(path.clone());
        }
        
        if let Some(value) = self.get_env_bool("AIRACCOUNT_AUDIT_SECURE_MODE") {
            config.audit_config.secure_mode = value;
        }
        
        if let Some(size) = self.get_env_u32("AIRACCOUNT_AUDIT_MAX_SIZE_MB") {
            config.audit_config.max_file_size_mb = size;
        }
        
        // KDF configuration
        if let Some(iterations) = self.get_env_u32("AIRACCOUNT_PBKDF2_ITERATIONS") {
            config.kdf_config.pbkdf2_iterations = iterations;
        }
        
        if let Some(memory) = self.get_env_u32("AIRACCOUNT_ARGON2_MEMORY") {
            config.kdf_config.argon2_memory_cost = memory;
        }
        
        if let Some(time) = self.get_env_u32("AIRACCOUNT_ARGON2_TIME") {
            config.kdf_config.argon2_time_cost = time;
        }
        
        // Performance configuration
        if let Some(threads) = self.get_env_usize("AIRACCOUNT_WORKER_THREADS") {
            config.performance_config.worker_threads = threads;
        }
        
        if let Some(value) = self.get_env_bool("AIRACCOUNT_ENABLE_MEMORY_POOL") {
            config.performance_config.memory_pool.enabled = value;
        }
        
        if let Some(value) = self.get_env_bool("AIRACCOUNT_ENABLE_SIMD") {
            config.performance_config.enable_simd = value;
        }
        
        // Entropy configuration
        if let Some(threshold) = self.get_env_f64("AIRACCOUNT_ENTROPY_QUALITY_THRESHOLD") {
            config.entropy_config.min_quality_threshold = threshold;
        }
        
        if let Some(size) = self.get_env_usize("AIRACCOUNT_ENTROPY_POOL_SIZE") {
            config.entropy_config.pool_size = size;
        }
        
        Ok(())
    }
    
    /// Get boolean environment variable
    fn get_env_bool(&self, key: &str) -> Option<bool> {
        self.env_variables.get(key).and_then(|v| {
            match v.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            }
        })
    }
    
    /// Get u32 environment variable
    fn get_env_u32(&self, key: &str) -> Option<u32> {
        self.env_variables.get(key).and_then(|v| v.parse().ok())
    }
    
    /// Get usize environment variable
    fn get_env_usize(&self, key: &str) -> Option<usize> {
        self.env_variables.get(key).and_then(|v| v.parse().ok())
    }
    
    /// Get f64 environment variable
    fn get_env_f64(&self, key: &str) -> Option<f64> {
        self.env_variables.get(key).and_then(|v| v.parse().ok())
    }
    
    /// Development environment configuration
    fn development_config() -> EnhancedSecurityConfig {
        let mut config = EnhancedSecurityConfig::default();
        
        config.environment = Environment::Development;
        
        // More relaxed settings for development
        config.kdf_config.pbkdf2_iterations = 10_000;
        config.kdf_config.argon2_memory_cost = 32_768; // 32MB
        config.kdf_config.argon2_time_cost = 2;
        
        // Enable hot reload in development
        config.hot_reload.enabled = true;
        config.hot_reload.watch_paths = vec![
            PathBuf::from("config.toml"),
            PathBuf::from("config/"),
        ];
        
        // Audit with less strict requirements
        config.audit_config.max_file_size_mb = 10;
        config.audit_config.rotation_count = 3;
        
        // Performance settings for development
        config.performance_config.worker_threads = 2;
        config.performance_config.cache.max_size_mb = 16;
        config.performance_config.cache.ttl_seconds = 300;
        
        config
    }
    
    /// Testing environment configuration
    fn testing_config() -> EnhancedSecurityConfig {
        let mut config = EnhancedSecurityConfig::default();
        
        config.environment = Environment::Testing;
        
        // Faster settings for testing
        config.kdf_config.pbkdf2_iterations = 10_000;
        config.kdf_config.argon2_memory_cost = 16_384; // 16MB
        config.kdf_config.argon2_time_cost = 1;
        
        // Disable hot reload in testing
        config.hot_reload.enabled = false;
        
        // Minimal audit for testing
        config.audit_config.enabled = true;
        config.audit_config.file_path = Some("/tmp/airaccount_test_audit.log".to_string());
        config.audit_config.max_file_size_mb = 5;
        config.audit_config.rotation_count = 2;
        
        // Lower entropy requirements for testing
        config.entropy_config.min_quality_threshold = 0.5;
        config.entropy_config.collection_timeout_ms = 1000;
        
        // Performance settings for testing
        config.performance_config.worker_threads = 2;
        config.performance_config.cache.max_size_mb = 8;
        config.performance_config.cache.ttl_seconds = 60;
        
        config
    }
    
    /// Staging environment configuration
    fn staging_config() -> EnhancedSecurityConfig {
        let mut config = EnhancedSecurityConfig::default();
        
        config.environment = Environment::Staging;
        
        // Production-like settings
        config.kdf_config.pbkdf2_iterations = 150_000;
        config.kdf_config.argon2_memory_cost = 131_072; // 128MB
        config.kdf_config.argon2_time_cost = 4;
        
        // Disable hot reload
        config.hot_reload.enabled = false;
        
        // Production-like audit
        config.audit_config.enabled = true;
        config.audit_config.secure_mode = true;
        config.audit_config.max_file_size_mb = 100;
        config.audit_config.rotation_count = 7;
        
        // Higher entropy requirements
        config.entropy_config.min_quality_threshold = 0.9;
        
        // Performance settings for staging
        config.performance_config.worker_threads = 4;
        config.performance_config.cache.max_size_mb = 32;
        config.performance_config.cache.ttl_seconds = 1800;
        
        config
    }
    
    /// Production environment configuration
    fn production_config() -> EnhancedSecurityConfig {
        let mut config = EnhancedSecurityConfig::default();
        
        config.environment = Environment::Production;
        
        // Maximum security settings
        config.enable_constant_time = true;
        config.enable_memory_protection = true;
        
        // Strong KDF settings
        config.kdf_config.pbkdf2_iterations = 200_000;
        config.kdf_config.argon2_memory_cost = 262_144; // 256MB
        config.kdf_config.argon2_time_cost = 5;
        config.kdf_config.argon2_parallelism = 8;
        
        // Disable hot reload for security
        config.hot_reload.enabled = false;
        
        // Secure audit configuration
        config.audit_config.enabled = true;
        config.audit_config.secure_mode = true;
        config.audit_config.max_file_size_mb = 500;
        config.audit_config.rotation_count = 30;
        
        // Maximum entropy requirements
        config.entropy_config.min_quality_threshold = 0.95;
        config.entropy_config.use_tee_source = true;
        config.entropy_config.use_hardware_rng = true;
        config.entropy_config.use_timing_jitter = true;
        
        // Production performance settings
        config.performance_config.worker_threads = num_cpus::get();
        config.performance_config.cache.max_size_mb = 128;
        config.performance_config.cache.ttl_seconds = 3600;
        config.performance_config.enable_batch_operations = true;
        config.performance_config.enable_memory_pool = true;
        config.performance_config.enable_simd = true;
        
        config
    }
    
    /// Generate environment-specific configuration file
    pub fn generate_config_file(&self, environment: Environment, path: &std::path::Path) -> SecurityResult<()> {
        let config = self.env_configs.get(&environment)
            .ok_or_else(|| SecurityError::config_error(
                ConfigErrorKind::MissingRequired,
                "environment_config",
                Some(format!("{:?}", environment)),
                None,
                "environment_manager"
            ))?;
        
        let toml_content = serde_json::to_string_pretty(config)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "toml_serialization",
                None,
                Some(e.to_string()),
                "environment_manager"
            ))?;
        
        // Add environment-specific comments
        let commented_content = self.add_environment_comments(&toml_content, environment);
        
        std::fs::write(path, commented_content)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "file_write",
                Some(path.display().to_string()),
                Some(e.to_string()),
                "environment_manager"
            ))?;
        
        Ok(())
    }
    
    /// Add environment-specific comments to configuration
    fn add_environment_comments(&self, content: &str, environment: Environment) -> String {
        let mut commented = String::new();
        
        commented.push_str(&format!("# AirAccount Configuration - {:?} Environment\n", environment));
        commented.push_str("# Generated automatically - modify with caution\n");
        commented.push_str(&format!("# Generated at: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        commented.push_str("\n");
        
        match environment {
            Environment::Development => {
                commented.push_str("# Development environment settings\n");
                commented.push_str("# - Relaxed security for easier debugging\n");
                commented.push_str("# - Hot reload enabled\n");
                commented.push_str("# - Lower resource usage\n");
            },
            Environment::Testing => {
                commented.push_str("# Testing environment settings\n");
                commented.push_str("# - Fast execution for CI/CD\n");
                commented.push_str("# - Minimal audit logging\n");
                commented.push_str("# - Lower entropy requirements\n");
            },
            Environment::Staging => {
                commented.push_str("# Staging environment settings\n");
                commented.push_str("# - Production-like security\n");
                commented.push_str("# - Full audit logging\n");
                commented.push_str("# - Performance monitoring\n");
            },
            Environment::Production => {
                commented.push_str("# Production environment settings\n");
                commented.push_str("# - Maximum security settings\n");
                commented.push_str("# - Encrypted audit logging\n");
                commented.push_str("# - High entropy requirements\n");
                commented.push_str("# - Performance optimizations enabled\n");
            },
        }
        
        commented.push_str("\n");
        commented.push_str(content);
        
        commented
    }
    
    /// Validate environment consistency
    pub fn validate_environment(&self, config: &EnhancedSecurityConfig) -> SecurityResult<()> {
        if config.environment != self.current_env {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "environment_mismatch",
                Some(format!("{:?}", self.current_env)),
                Some(format!("{:?}", config.environment)),
                "environment_manager"
            ));
        }
        
        Ok(())
    }
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new().expect("Failed to create environment manager")
    }
}

/// Helper function to get CPU count
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

// Mock chrono for this example
mod chrono {
    pub struct Utc;
    
    impl Utc {
        pub fn now() -> Self {
            Self
        }
        
        pub fn format(&self, _format: &str) -> String {
            "2024-01-17 12:00:00 UTC".to_string()
        }
    }
}

// Mock toml since it's not added to dependencies yet
mod toml {
    use serde::Serialize;
    
    pub fn _to_string_pretty<T: Serialize>(value: &T) -> Result<String, String> {
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_environment_detection() {
        // Test should work even without setting env vars
        let manager = EnvironmentManager::new().unwrap();
        
        // Should default to development in test environment
        assert!(matches!(
            manager.current_environment(), 
            Environment::Development | Environment::Testing
        ));
    }
    
    #[test]
    fn test_environment_configs() {
        let manager = EnvironmentManager::new().unwrap();
        
        // Test all environment configs exist
        assert!(manager.env_configs.contains_key(&Environment::Development));
        assert!(manager.env_configs.contains_key(&Environment::Testing));
        assert!(manager.env_configs.contains_key(&Environment::Staging));
        assert!(manager.env_configs.contains_key(&Environment::Production));
    }
    
    #[test]
    fn test_production_security_settings() {
        let prod_config = EnvironmentManager::production_config();
        
        assert_eq!(prod_config.environment, Environment::Production);
        assert!(prod_config.enable_constant_time);
        assert!(prod_config.enable_memory_protection);
        assert!(prod_config.audit_config.enabled);
        assert!(!prod_config.hot_reload.enabled);
        assert!(prod_config.entropy_config.min_quality_threshold > 0.9);
    }
    
    #[test]
    fn test_development_settings() {
        let dev_config = EnvironmentManager::development_config();
        
        assert_eq!(dev_config.environment, Environment::Development);
        assert!(dev_config.hot_reload.enabled);
        assert!(dev_config.kdf_config.pbkdf2_iterations < 50_000); // Faster for dev
    }
    
    #[test]
    fn test_env_override_parsing() {
        let _manager = EnvironmentManager::new().unwrap(); // 保留以备测试扩展
        
        // Test boolean parsing
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_BOOL".to_string(), "true".to_string());
        
        let test_manager = EnvironmentManager {
            current_env: Environment::Development,
            env_configs: HashMap::new(),
            env_variables: env_vars,
        };
        
        assert_eq!(test_manager.get_env_bool("TEST_BOOL"), Some(true));
        assert_eq!(test_manager.get_env_bool("NONEXISTENT"), None);
    }
}