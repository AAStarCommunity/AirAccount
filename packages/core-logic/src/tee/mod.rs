// Licensed to AirAccount under the Apache License, Version 2.0
// TEE adapter layer for cross-platform TEE support

mod optee_adapter;
mod sgx_adapter;
mod tee_interface;

pub use optee_adapter::OpTeeAdapter;
pub use sgx_adapter::SgxAdapter;
pub use tee_interface::{TEEInterface, TEEResult, TEESecureStorage, TEERandom};

use std::collections::HashMap;

/// TEE错误类型
#[derive(Debug)]
pub enum TEEError {
    InitializationFailed(String),
    SessionError(String),
    StorageError(String),
    CryptographicError(String),
    HardwareError(String),
    UnsupportedOperation(String),
}

impl std::fmt::Display for TEEError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TEEError::InitializationFailed(msg) => write!(f, "TEE initialization failed: {}", msg),
            TEEError::SessionError(msg) => write!(f, "TEE session error: {}", msg),
            TEEError::StorageError(msg) => write!(f, "TEE storage error: {}", msg),
            TEEError::CryptographicError(msg) => write!(f, "TEE cryptographic error: {}", msg),
            TEEError::HardwareError(msg) => write!(f, "TEE hardware error: {}", msg),
            TEEError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
        }
    }
}

impl std::error::Error for TEEError {}

/// TEE平台类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TEEPlatform {
    OpTEE,      // OP-TEE (ARM TrustZone)
    IntelSGX,   // Intel SGX
    AmdSev,     // AMD SEV
    Simulation, // 仿真模式
}

/// TEE能力集合
#[derive(Debug, Clone)]
pub struct TEECapabilities {
    pub secure_storage: bool,
    pub hardware_random: bool,
    pub secure_display: bool,
    pub biometric_support: bool,
    pub key_derivation: bool,
    pub attestation: bool,
}

impl Default for TEECapabilities {
    fn default() -> Self {
        Self {
            secure_storage: true,
            hardware_random: true,
            secure_display: false,
            biometric_support: false,
            key_derivation: true,
            attestation: false,
        }
    }
}

/// TEE配置
#[derive(Debug, Clone)]
pub struct TEEConfig {
    pub platform: TEEPlatform,
    pub ta_uuid: String,
    pub capabilities: TEECapabilities,
    pub max_sessions: u32,
    pub session_timeout_ms: u32,
}

impl Default for TEEConfig {
    fn default() -> Self {
        Self {
            platform: TEEPlatform::OpTEE,
            ta_uuid: "be2dc9a0-02b4-4b33-ba21-9964dbdf1573".to_string(),
            capabilities: TEECapabilities::default(),
            max_sessions: 10,
            session_timeout_ms: 300_000, // 5分钟
        }
    }
}

/// TEE适配器 - 跨平台TEE支持
pub struct TEEAdapter {
    platform: TEEPlatform,
    config: TEEConfig,
    interface: Box<dyn TEEInterface + Send + Sync>,
    active_sessions: HashMap<u32, SessionInfo>,
}

#[derive(Debug, Clone)]
struct SessionInfo {
    session_id: u32,
    created_at: std::time::Instant,
    last_activity: std::time::Instant,
    user_context: Option<String>,
}

impl TEEAdapter {
    /// 创建新的TEE适配器
    pub fn new(config: TEEConfig) -> Result<Self, TEEError> {
        let interface: Box<dyn TEEInterface + Send + Sync> = match config.platform {
            TEEPlatform::OpTEE => {
                Box::new(OpTeeAdapter::new(&config)?)
            },
            TEEPlatform::IntelSGX => {
                Box::new(SgxAdapter::new(&config)?)
            },
            _ => {
                return Err(TEEError::UnsupportedOperation(
                    format!("Platform {:?} not yet supported", config.platform)
                ));
            }
        };
        
        Ok(Self {
            platform: config.platform,
            config,
            interface,
            active_sessions: HashMap::new(),
        })
    }
    
    /// 初始化TEE环境
    pub async fn initialize(&mut self) -> Result<(), TEEError> {
        self.interface.initialize().await?;
        
        // 验证TEE能力
        self.verify_capabilities().await?;
        
        Ok(())
    }
    
    /// 创建TEE会话
    pub async fn create_session(&mut self, user_context: Option<String>) -> Result<u32, TEEError> {
        let session_id = self.interface.create_session().await?;
        
        let session_info = SessionInfo {
            session_id,
            created_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            user_context,
        };
        
        self.active_sessions.insert(session_id, session_info);
        
        // 清理过期会话
        self.cleanup_expired_sessions().await;
        
        Ok(session_id)
    }
    
    /// 关闭TEE会话
    pub async fn close_session(&mut self, session_id: u32) -> Result<(), TEEError> {
        self.interface.close_session(session_id).await?;
        self.active_sessions.remove(&session_id);
        Ok(())
    }
    
    /// 调用TEE命令
    pub async fn invoke_command(
        &mut self,
        session_id: u32,
        command_id: u32,
        input: &[u8]
    ) -> Result<Vec<u8>, TEEError> {
        // 验证会话
        if let Some(session) = self.active_sessions.get_mut(&session_id) {
            session.last_activity = std::time::Instant::now();
        } else {
            return Err(TEEError::SessionError("Invalid session ID".to_string()));
        }
        
        // 调用底层TEE接口
        self.interface.invoke_command(session_id, command_id, input).await
    }
    
    /// 获取平台信息
    pub fn platform(&self) -> TEEPlatform {
        self.platform
    }
    
    /// 获取配置
    pub fn config(&self) -> &TEEConfig {
        &self.config
    }
    
    /// 验证TEE能力
    async fn verify_capabilities(&self) -> Result<(), TEEError> {
        let required_caps = &self.config.capabilities;
        
        // 检查安全存储
        if required_caps.secure_storage {
            self.interface.test_secure_storage().await?;
        }
        
        // 检查硬件随机数
        if required_caps.hardware_random {
            let mut test_bytes = vec![0u8; 32];
            self.interface.generate_random(&mut test_bytes).await?;
            
            // 简单的随机性检查
            if test_bytes.iter().all(|&x| x == 0) {
                return Err(TEEError::HardwareError("Hardware random generator failed".to_string()));
            }
        }
        
        Ok(())
    }
    
    /// 清理过期会话
    async fn cleanup_expired_sessions(&mut self) {
        let timeout = std::time::Duration::from_millis(self.config.session_timeout_ms as u64);
        let now = std::time::Instant::now();
        
        let expired_sessions: Vec<u32> = self.active_sessions
            .iter()
            .filter(|(_, info)| now.duration_since(info.last_activity) > timeout)
            .map(|(&id, _)| id)
            .collect();
            
        for session_id in expired_sessions {
            if let Err(e) = self.interface.close_session(session_id).await {
                eprintln!("Failed to close expired session {}: {}", session_id, e);
            }
            self.active_sessions.remove(&session_id);
        }
    }
    
    /// 获取会话统计信息
    pub fn session_stats(&self) -> (usize, usize) {
        (self.active_sessions.len(), self.config.max_sessions as usize)
    }
}

/// 用于测试的模拟TEE适配器
#[cfg(test)]
pub struct MockTEEAdapter {
    sessions: HashMap<u32, SessionInfo>,
    next_session_id: u32,
}

#[cfg(test)]
impl MockTEEAdapter {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_session_id: 1,
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl TEEInterface for MockTEEAdapter {
    async fn initialize(&mut self) -> TEEResult<()> {
        Ok(())
    }
    
    async fn create_session(&mut self) -> TEEResult<u32> {
        let session_id = self.next_session_id;
        self.next_session_id += 1;
        
        let session_info = SessionInfo {
            session_id,
            created_at: std::time::Instant::now(),
            last_activity: std::time::Instant::now(),
            user_context: None,
        };
        
        self.sessions.insert(session_id, session_info);
        Ok(session_id)
    }
    
    async fn close_session(&mut self, session_id: u32) -> TEEResult<()> {
        self.sessions.remove(&session_id);
        Ok(())
    }
    
    async fn invoke_command(&mut self, _session_id: u32, _command_id: u32, input: &[u8]) -> TEEResult<Vec<u8>> {
        // 简单回显输入
        Ok(input.to_vec())
    }
    
    async fn test_secure_storage(&self) -> TEEResult<()> {
        Ok(())
    }
    
    async fn generate_random(&self, buffer: &mut [u8]) -> TEEResult<()> {
        // 模拟随机数生成
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tee_adapter_creation() {
        let config = TEEConfig {
            platform: TEEPlatform::Simulation,
            ..TEEConfig::default()
        };
        
        // 注意：在实际环境中需要实现Simulation平台
        // 这里只测试配置创建
        assert_eq!(config.platform, TEEPlatform::Simulation);
        assert!(config.capabilities.secure_storage);
    }
    
    #[test]
    fn test_tee_error_display() {
        let error = TEEError::InitializationFailed("Test error".to_string());
        let display = format!("{}", error);
        assert!(display.contains("initialization failed"));
        assert!(display.contains("Test error"));
    }
}