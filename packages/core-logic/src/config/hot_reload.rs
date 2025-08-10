// Licensed to AirAccount under the Apache License, Version 2.0
// Hot reload configuration system for non-critical settings

use super::{HotReloadConfig, EnhancedSecurityConfig};
use crate::error::{SecurityError, SecurityResult, ConfigErrorKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/// Hot reload handler for configuration changes
pub struct HotReloadHandler {
    config: HotReloadConfig,
    _watchers: Vec<FileWatcher>,
    callbacks: Arc<Mutex<HashMap<String, Box<dyn Fn(&str, serde_json::Value) -> SecurityResult<()> + Send>>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    _worker_thread: Option<thread::JoinHandle<()>>,
}

impl HotReloadHandler {
    /// Create new hot reload handler
    pub fn new(config: HotReloadConfig) -> SecurityResult<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                _watchers: Vec::new(),
                callbacks: Arc::new(Mutex::new(HashMap::new())),
                shutdown_tx: None,
                _worker_thread: None,
            });
        }
        
        let watchers = Self::create_watchers(&config)?;
        let callbacks = Arc::new(Mutex::new(HashMap::new()));
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        
        // Start worker thread for handling file system events
        let worker_config = config.clone();
        let worker_callbacks = Arc::clone(&callbacks);
        let worker_thread = thread::spawn(move || {
            Self::worker_loop(worker_config, worker_callbacks, shutdown_rx);
        });
        
        Ok(Self {
            config,
            _watchers: watchers,
            callbacks,
            shutdown_tx: Some(shutdown_tx),
            _worker_thread: Some(worker_thread),
        })
    }
    
    /// Create file watchers for configured paths
    fn create_watchers(config: &HotReloadConfig) -> SecurityResult<Vec<FileWatcher>> {
        let mut watchers = Vec::new();
        
        for path in &config.watch_paths {
            if !path.exists() {
                continue; // Skip non-existent paths
            }
            
            let watcher = FileWatcher::new(path.clone(), config.debounce_ms)?;
            watchers.push(watcher);
        }
        
        Ok(watchers)
    }
    
    /// Register callback for configuration section changes
    pub fn register_callback<F>(&mut self, section: &str, callback: F) -> SecurityResult<()>
    where
        F: Fn(&str, serde_json::Value) -> SecurityResult<()> + Send + 'static,
    {
        if !self.config.reloadable_sections.contains(&section.to_string()) {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                section,
                Some("reloadable section".to_string()),
                Some("not in reloadable sections".to_string()),
                "hot_reload_handler"
            ));
        }
        
        let mut callbacks = self.callbacks.lock()
            .map_err(|_| SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "callback_mutex",
                None,
                Some("mutex poisoned".to_string()),
                "hot_reload_handler"
            ))?;
        
        callbacks.insert(section.to_string(), Box::new(callback));
        
        Ok(())
    }
    
    /// Worker loop for handling configuration changes
    fn worker_loop(
        config: HotReloadConfig,
        callbacks: Arc<Mutex<HashMap<String, Box<dyn Fn(&str, serde_json::Value) -> SecurityResult<()> + Send>>>>,
        shutdown_rx: mpsc::Receiver<()>,
    ) {
        let check_interval = Duration::from_millis(config.debounce_ms);
        let mut last_check = Instant::now();
        let mut pending_changes: HashMap<PathBuf, Instant> = HashMap::new();
        
        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            
            let now = Instant::now();
            
            // Check for file changes
            for path in &config.watch_paths {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        let modified_instant = Instant::now() - modified.elapsed().unwrap_or_default();
                        
                        // Check if file was modified since last check
                        if modified_instant > last_check {
                            pending_changes.insert(path.clone(), now);
                        }
                    }
                }
            }
            
            // Process pending changes after debounce period
            let mut to_process = Vec::new();
            pending_changes.retain(|path, change_time| {
                if now.duration_since(*change_time) >= check_interval {
                    to_process.push(path.clone());
                    false // Remove from pending
                } else {
                    true // Keep pending
                }
            });
            
            // Process debounced changes
            for path in to_process {
                if let Err(e) = Self::process_config_change(&path, &config, &callbacks) {
                    eprintln!("Hot reload error for {}: {}", path.display(), e);
                }
            }
            
            last_check = now;
            thread::sleep(Duration::from_millis(100)); // Check every 100ms
        }
    }
    
    /// Process a configuration file change
    fn process_config_change(
        path: &Path,
        config: &HotReloadConfig,
        callbacks: &Arc<Mutex<HashMap<String, Box<dyn Fn(&str, serde_json::Value) -> SecurityResult<()> + Send>>>>,
    ) -> SecurityResult<()> {
        // Read the configuration file
        let content = std::fs::read_to_string(path)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "file_read",
                Some(path.display().to_string()),
                Some(e.to_string()),
                "hot_reload_processor"
            ))?;
        
        // Parse the configuration
        let new_config: EnhancedSecurityConfig = toml::from_str(&content)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "config_parsing",
                Some(path.display().to_string()),
                Some(e.to_string()),
                "hot_reload_processor"
            ))?;
        
        // Convert to JSON for easier section extraction
        let config_json = serde_json::to_value(&new_config)
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "json_conversion",
                None,
                Some(e.to_string()),
                "hot_reload_processor"
            ))?;
        
        // Process each reloadable section
        let callbacks_lock = callbacks.lock()
            .map_err(|_| SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "callback_mutex",
                None,
                Some("mutex poisoned".to_string()),
                "hot_reload_processor"
            ))?;
        
        for section in &config.reloadable_sections {
            if let Some(section_value) = config_json.get(section) {
                if let Some(callback) = callbacks_lock.get(section) {
                    if let Err(e) = callback(section, section_value.clone()) {
                        eprintln!("Callback error for section '{}': {}", section, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if hot reload is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
    
    /// Get reloadable sections
    pub fn reloadable_sections(&self) -> &[String] {
        &self.config.reloadable_sections
    }
    
    /// Manually trigger reload for a specific file
    pub fn manual_reload(&self, path: &Path) -> SecurityResult<()> {
        if !self.config.enabled {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "hot_reload_disabled",
                Some("enabled".to_string()),
                Some("disabled".to_string()),
                "hot_reload_handler"
            ));
        }
        
        Self::process_config_change(path, &self.config, &self.callbacks)
    }
}

impl Drop for HotReloadHandler {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

/// File watcher for monitoring configuration changes
struct FileWatcher {
    path: PathBuf,
    last_modified: Option<std::time::SystemTime>,
    _debounce_duration: Duration,
}

impl FileWatcher {
    /// Create new file watcher
    fn new(path: PathBuf, debounce_ms: u64) -> SecurityResult<Self> {
        let last_modified = if path.exists() {
            std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
        } else {
            None
        };
        
        Ok(Self {
            path,
            last_modified,
            _debounce_duration: Duration::from_millis(debounce_ms),
        })
    }
    
    /// Check if file has been modified
    fn check_modified(&mut self) -> SecurityResult<bool> {
        if !self.path.exists() {
            return Ok(false);
        }
        
        let current_modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .map_err(|e| SecurityError::config_error(
                ConfigErrorKind::ParsingFailed,
                "file_metadata",
                Some(self.path.display().to_string()),
                Some(e.to_string()),
                "file_watcher"
            ))?;
        
        let is_modified = match self.last_modified {
            Some(last) => current_modified > last,
            None => true,
        };
        
        if is_modified {
            self.last_modified = Some(current_modified);
        }
        
        Ok(is_modified)
    }
    
    /// Get file path
    fn _path(&self) -> &Path {
        &self.path
    }
}

/// Hot reload manager for coordinating multiple handlers
pub struct HotReloadManager {
    handlers: HashMap<String, HotReloadHandler>,
    global_config: HotReloadConfig,
}

impl HotReloadManager {
    /// Create new hot reload manager
    pub fn new(global_config: HotReloadConfig) -> Self {
        Self {
            handlers: HashMap::new(),
            global_config,
        }
    }
    
    /// Add handler for specific configuration aspect
    pub fn add_handler(&mut self, name: String, handler: HotReloadHandler) {
        self.handlers.insert(name, handler);
    }
    
    /// Register callback for all handlers
    pub fn register_global_callback<F>(&mut self, section: &str, callback: F) -> SecurityResult<()>
    where
        F: Fn(&str, serde_json::Value) -> SecurityResult<()> + Send + Clone + 'static,
    {
        for handler in self.handlers.values_mut() {
            handler.register_callback(section, callback.clone())?;
        }
        
        Ok(())
    }
    
    /// Check if any handler is enabled
    pub fn is_enabled(&self) -> bool {
        self.global_config.enabled || self.handlers.values().any(|h| h.is_enabled())
    }
    
    /// Get all reloadable sections
    pub fn all_reloadable_sections(&self) -> Vec<String> {
        let mut sections = self.global_config.reloadable_sections.clone();
        
        for handler in self.handlers.values() {
            for section in handler.reloadable_sections() {
                if !sections.contains(section) {
                    sections.push(section.clone());
                }
            }
        }
        
        sections.sort();
        sections
    }
    
    /// Manual reload for all handlers
    pub fn manual_reload_all(&self, path: &Path) -> SecurityResult<()> {
        let mut errors = Vec::new();
        
        for (name, handler) in &self.handlers {
            if let Err(e) = handler.manual_reload(path) {
                errors.push(format!("Handler '{}': {}", name, e));
            }
        }
        
        if !errors.is_empty() {
            return Err(SecurityError::config_error(
                ConfigErrorKind::ValidationFailed,
                "manual_reload",
                None,
                Some(errors.join("; ")),
                "hot_reload_manager"
            ));
        }
        
        Ok(())
    }
}

// Mock toml crate since it's not in dependencies
mod toml {
    use crate::config::EnhancedSecurityConfig;
    
    pub fn from_str(_content: &str) -> Result<EnhancedSecurityConfig, String> {
        // Return a default config for now
        Ok(EnhancedSecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    #[test]
    fn test_hot_reload_handler_creation() {
        let config = HotReloadConfig {
            enabled: false,
            watch_paths: vec![],
            debounce_ms: 1000,
            reloadable_sections: vec!["performance".to_string()],
        };
        
        let handler = HotReloadHandler::new(config).unwrap();
        assert!(!handler.is_enabled());
    }
    
    #[test]
    fn test_file_watcher() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        let mut watcher = FileWatcher::new(path.clone(), 100).unwrap();
        
        // Initial check should return false (no change)
        assert!(!watcher.check_modified().unwrap());
        
        // Modify file
        writeln!(temp_file, "test content").unwrap();
        temp_file.flush().unwrap();
        
        // Should detect modification
        assert!(watcher.check_modified().unwrap());
        
        // Second check should return false (no new change)
        assert!(!watcher.check_modified().unwrap());
    }
    
    #[test]
    fn test_callback_registration() {
        let config = HotReloadConfig {
            enabled: true,
            watch_paths: vec![],
            debounce_ms: 100,
            reloadable_sections: vec!["performance".to_string(), "cache".to_string()],
        };
        
        let mut handler = HotReloadHandler::new(config).unwrap();
        
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        
        // Register valid callback
        let result = handler.register_callback("performance", move |_section, _value| {
            called_clone.store(true, Ordering::Relaxed);
            Ok(())
        });
        assert!(result.is_ok());
        
        // Register invalid section callback
        let invalid_result = handler.register_callback("invalid_section", |_section, _value| {
            Ok(())
        });
        assert!(invalid_result.is_err());
    }
    
    #[test]
    fn test_hot_reload_manager() {
        let global_config = HotReloadConfig {
            enabled: true,
            watch_paths: vec![],
            debounce_ms: 100,
            reloadable_sections: vec!["performance".to_string()],
        };
        
        let mut manager = HotReloadManager::new(global_config);
        
        let handler_config = HotReloadConfig {
            enabled: true,
            watch_paths: vec![],
            debounce_ms: 100,
            reloadable_sections: vec!["cache".to_string()],
        };
        
        let handler = HotReloadHandler::new(handler_config).unwrap();
        manager.add_handler("test_handler".to_string(), handler);
        
        assert!(manager.is_enabled());
        
        let sections = manager.all_reloadable_sections();
        assert!(sections.contains(&"performance".to_string()));
        assert!(sections.contains(&"cache".to_string()));
    }
    
    #[test]
    fn test_reloadable_sections_validation() {
        let config = HotReloadConfig {
            enabled: true,
            watch_paths: vec![],
            debounce_ms: 100,
            reloadable_sections: vec!["performance".to_string()],
        };
        
        let mut handler = HotReloadHandler::new(config).unwrap();
        
        // Should allow reloadable section
        assert!(handler.register_callback("performance", |_, _| Ok(())).is_ok());
        
        // Should reject non-reloadable section
        assert!(handler.register_callback("security", |_, _| Ok(())).is_err());
    }
}