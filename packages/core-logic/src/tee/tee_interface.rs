// Licensed to AirAccount under the Apache License, Version 2.0
// TEE interface definitions

use super::TEEError;

pub type TEEResult<T> = Result<T, TEEError>;

/// 通用TEE接口
#[async_trait::async_trait]
pub trait TEEInterface {
    async fn initialize(&mut self) -> TEEResult<()>;
    async fn create_session(&mut self) -> TEEResult<u32>;
    async fn close_session(&mut self, session_id: u32) -> TEEResult<()>;
    async fn invoke_command(&mut self, session_id: u32, command_id: u32, input: &[u8]) -> TEEResult<Vec<u8>>;
    async fn test_secure_storage(&self) -> TEEResult<()>;
    async fn generate_random(&self, buffer: &mut [u8]) -> TEEResult<()>;
}

/// TEE安全存储接口
pub trait TEESecureStorage {
    fn store(&self, key: &str, data: &[u8]) -> TEEResult<()>;
    fn load(&self, key: &str) -> TEEResult<Vec<u8>>;
    fn delete(&self, key: &str) -> TEEResult<()>;
}

/// TEE随机数生成器接口
pub trait TEERandom {
    fn generate(&self, buffer: &mut [u8]) -> TEEResult<()>;
}