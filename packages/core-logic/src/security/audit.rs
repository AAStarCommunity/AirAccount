use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::io::Write;
use serde::{Deserialize, Serialize};
use zeroize::ZeroizeOnDrop;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditLevel {
    Info,
    Warning, 
    Error,
    Critical,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    KeyGeneration { 
        algorithm: String, 
        key_size: u32,
        operation: String,
        key_type: String,
        duration_ms: u64,
        entropy_bits: u32,
    },
    SignOperation { message_hash: String, success: bool },
    MemoryAllocation { size: usize, secure: bool },
    SecurityViolation { violation_type: String, details: String },
    SecurityOperation { 
        operation: String, 
        details: String, 
        success: bool,
        risk_level: String,
    },
    Authentication { user_id: String, success: bool, method: String },
    ConfigChange { parameter: String, old_value: String, new_value: String },
    TEEOperation { operation: String, duration_ms: u64, success: bool },
    NetworkAccess { endpoint: String, method: String, status_code: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub level: AuditLevel,
    pub event: AuditEvent,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub component: String,
    pub thread_id: String,
    pub metadata: HashMap<String, String>,
}

impl AuditLogEntry {
    pub fn new(level: AuditLevel, event: AuditEvent, component: &str) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            level,
            event,
            session_id: None,
            user_id: None,
            component: component.to_string(),
            thread_id: format!("{:?}", std::thread::current().id()),
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
    
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    pub fn is_security_relevant(&self) -> bool {
        matches!(self.level, AuditLevel::Security | AuditLevel::Critical) ||
        matches!(self.event, 
            AuditEvent::SecurityViolation { .. } | 
            AuditEvent::Authentication { .. } |
            AuditEvent::KeyGeneration { .. } |
            AuditEvent::SignOperation { .. }
        )
    }
}

pub trait AuditSink: Send + Sync {
    fn log_entry(&self, entry: &AuditLogEntry) -> Result<(), Box<dyn std::error::Error>>;
    fn flush(&self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct ConsoleAuditSink;

impl AuditSink for ConsoleAuditSink {
    fn log_entry(&self, entry: &AuditLogEntry) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(entry)?;
        println!("[AUDIT] {}", json);
        Ok(())
    }
    
    fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        std::io::stdout().flush().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

pub struct FileAuditSink {
    file_path: String,
}

impl FileAuditSink {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }
}

impl AuditSink for FileAuditSink {
    fn log_entry(&self, entry: &AuditLogEntry) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::OpenOptions;
        use std::io::Write;
        
        let json = serde_json::to_string(entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        
        writeln!(file, "{}", json)?;
        Ok(())
    }
    
    fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[derive(ZeroizeOnDrop)]
pub struct SecureAuditSink {
    encrypted_file_path: String,
    #[zeroize(skip)]
    encryption_key: [u8; 32],
}

impl SecureAuditSink {
    pub fn new(file_path: String, encryption_key: [u8; 32]) -> Self {
        Self {
            encrypted_file_path: file_path,
            encryption_key,
        }
    }
    
    fn encrypt_entry(&self, entry: &AuditLogEntry) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let json = serde_json::to_string(entry)?;
        
        use aes_gcm::{Aes256Gcm, Nonce, KeyInit};
        use aes_gcm::aead::Aead;
        use rand::RngCore;
        
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.encryption_key);
        let cipher = Aes256Gcm::new(key);
        
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("Encryption failed: {:?}", e))?;
        
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
}

impl AuditSink for SecureAuditSink {
    fn log_entry(&self, entry: &AuditLogEntry) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::OpenOptions;
        use std::io::Write;
        
        let encrypted_data = self.encrypt_entry(entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.encrypted_file_path)?;
        
        let length = encrypted_data.len() as u32;
        file.write_all(&length.to_le_bytes())?;
        file.write_all(&encrypted_data)?;
        
        Ok(())
    }
    
    fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub struct AuditLogger {
    sinks: Vec<Arc<dyn AuditSink>>,
    buffer: Arc<Mutex<Vec<AuditLogEntry>>>,
    max_buffer_size: usize,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            sinks: Vec::new(),
            buffer: Arc::new(Mutex::new(Vec::new())),
            max_buffer_size: 1000,
        }
    }
    
    pub fn add_sink(&mut self, sink: Arc<dyn AuditSink>) {
        self.sinks.push(sink);
    }
    
    pub fn set_max_buffer_size(&mut self, size: usize) {
        self.max_buffer_size = size;
    }
    
    pub fn log(&self, entry: AuditLogEntry) {
        for sink in &self.sinks {
            if let Err(e) = sink.log_entry(&entry) {
                eprintln!("Audit sink error: {}", e);
            }
        }
        
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(entry);
            
            if buffer.len() > self.max_buffer_size {
                let excess = buffer.len() - self.max_buffer_size;
                buffer.drain(0..excess);
            }
        }
    }
    
    pub fn log_info(&self, event: AuditEvent, component: &str) {
        let entry = AuditLogEntry::new(AuditLevel::Info, event, component);
        self.log(entry);
    }
    
    pub fn log_warning(&self, event: AuditEvent, component: &str) {
        let entry = AuditLogEntry::new(AuditLevel::Warning, event, component);
        self.log(entry);
    }
    
    pub fn log_error(&self, event: AuditEvent, component: &str) {
        let entry = AuditLogEntry::new(AuditLevel::Error, event, component);
        self.log(entry);
    }
    
    pub fn log_critical(&self, event: AuditEvent, component: &str) {
        let entry = AuditLogEntry::new(AuditLevel::Critical, event, component);
        self.log(entry);
    }
    
    pub fn log_security(&self, event: AuditEvent, component: &str) {
        let entry = AuditLogEntry::new(AuditLevel::Security, event, component);
        self.log(entry);
    }
    
    pub fn flush_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        for sink in &self.sinks {
            sink.flush()?;
        }
        Ok(())
    }
    
    pub fn get_security_events(&self, since_timestamp: Option<u64>) -> Vec<AuditLogEntry> {
        if let Ok(buffer) = self.buffer.lock() {
            buffer.iter()
                .filter(|entry| entry.is_security_relevant())
                .filter(|entry| {
                    since_timestamp.map_or(true, |ts| entry.timestamp >= ts)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn get_events_by_component(&self, component: &str, limit: Option<usize>) -> Vec<AuditLogEntry> {
        if let Ok(buffer) = self.buffer.lock() {
            let mut events: Vec<_> = buffer.iter()
                .filter(|entry| entry.component == component)
                .cloned()
                .collect();
            
            events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            
            if let Some(limit) = limit {
                events.truncate(limit);
            }
            
            events
        } else {
            Vec::new()
        }
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

pub static GLOBAL_AUDIT_LOGGER: std::sync::OnceLock<Arc<Mutex<AuditLogger>>> = std::sync::OnceLock::new();

pub fn init_global_audit_logger() -> Arc<Mutex<AuditLogger>> {
    GLOBAL_AUDIT_LOGGER.get_or_init(|| {
        let mut logger = AuditLogger::new();
        logger.add_sink(Arc::new(ConsoleAuditSink));
        Arc::new(Mutex::new(logger))
    }).clone()
}

pub fn audit_log(level: AuditLevel, event: AuditEvent, component: &str) {
    let logger = init_global_audit_logger();
    if let Ok(logger) = logger.lock() {
        let entry = AuditLogEntry::new(level, event, component);
        logger.log(entry);
    };
}

#[macro_export]
macro_rules! audit_info {
    ($event:expr, $component:expr) => {
        $crate::security::audit::audit_log(
            crate::security::audit::AuditLevel::Info,
            $event,
            $component
        )
    };
}

#[macro_export]
macro_rules! audit_security {
    ($event:expr, $component:expr) => {
        $crate::security::audit::audit_log(
            crate::security::audit::AuditLevel::Security,
            $event,
            $component
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_entry() {
        let event = AuditEvent::KeyGeneration {
            algorithm: "RSA".to_string(),
            key_size: 2048,
            operation: "test_operation".to_string(),
            key_type: "test_key".to_string(),
            duration_ms: 100,
            entropy_bits: 256,
        };
        
        let entry = AuditLogEntry::new(AuditLevel::Security, event, "crypto")
            .with_user_id("test_user".to_string())
            .with_metadata("key_id".to_string(), "12345".to_string());
        
        assert!(entry.is_security_relevant());
        assert_eq!(entry.component, "crypto");
        assert_eq!(entry.user_id, Some("test_user".to_string()));
    }
    
    #[test]
    fn test_audit_logger() {
        let mut logger = AuditLogger::new();
        logger.add_sink(Arc::new(ConsoleAuditSink));
        
        let event = AuditEvent::SecurityViolation {
            violation_type: "test".to_string(),
            details: "test violation".to_string(),
        };
        
        logger.log_security(event, "test_component");
        
        let security_events = logger.get_security_events(None);
        assert!(!security_events.is_empty());
    }
}