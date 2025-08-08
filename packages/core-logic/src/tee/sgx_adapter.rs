// Licensed to AirAccount under the Apache License, Version 2.0
// Intel SGX adapter implementation

use super::{TEEConfig, TEEError, TEEInterface, TEEResult};

pub struct SgxAdapter {
    _config: TEEConfig,
}

impl SgxAdapter {
    pub fn new(config: &TEEConfig) -> Result<Self, TEEError> {
        Ok(Self {
            _config: config.clone(),
        })
    }
}

#[async_trait::async_trait]
impl TEEInterface for SgxAdapter {
    async fn initialize(&mut self) -> TEEResult<()> {
        // TODO: Initialize SGX enclave
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
    
    async fn create_session(&mut self) -> TEEResult<u32> {
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
    
    async fn close_session(&mut self, _session_id: u32) -> TEEResult<()> {
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
    
    async fn invoke_command(&mut self, _session_id: u32, _command_id: u32, _input: &[u8]) -> TEEResult<Vec<u8>> {
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
    
    async fn test_secure_storage(&self) -> TEEResult<()> {
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
    
    async fn generate_random(&self, _buffer: &mut [u8]) -> TEEResult<()> {
        Err(TEEError::UnsupportedOperation("SGX not yet implemented".to_string()))
    }
}