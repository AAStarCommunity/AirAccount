// Licensed to AirAccount under the Apache License, Version 2.0
// Mock TEE implementation for development and testing

use anyhow::{Result, Context as AnyhowContext};
use log::{debug, info, warn};
use airaccount_proto::{WalletResponse, CreateWalletResponse, command_ids};
use uuid::Uuid as StdUuid;
use std::fmt;

// Mock UUID type
#[derive(Debug, Clone)]
pub struct Uuid {
    inner: StdUuid,
}

impl Uuid {
    pub fn parse_str(s: &str) -> Result<Self> {
        let inner = StdUuid::parse_str(s).context("Failed to parse UUID")?;
        Ok(Uuid { inner })
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

// Mock TEE Context
pub struct Context {
    _mock: bool,
}

impl Context {
    pub fn new() -> Result<Self> {
        info!("🔧 Creating mock TEE context");
        Ok(Context { _mock: true })
    }

    pub fn open_session(&self, uuid: Uuid) -> Result<Session> {
        info!("🔧 Mock: Opening session with TA UUID: {}", uuid);
        Ok(Session { 
            _uuid: uuid,
            _mock: true,
        })
    }
}

// Mock Session
pub struct Session {
    _uuid: Uuid,
    _mock: bool,
}

impl Session {
    pub fn invoke_command(&mut self, cmd_id: u32, operation: &mut Operation) -> Result<()> {
        debug!("🔧 Mock: Invoking command 0x{:x}", cmd_id);

        // 模拟处理不同的命令
        match cmd_id {
            cmd if cmd == command_ids::WALLET_COMMAND => {
                info!("🔧 Mock: Processing wallet command");
                
                // 创建模拟响应
                let mock_response = airaccount_proto::AirAccountResponse {
                    request_id: StdUuid::new_v4().to_string(),
                    response: WalletResponse::CreateWallet(CreateWalletResponse {
                        success: true,
                        wallet_id: Some("mock-wallet-123".to_string()),
                        mnemonic: Some("mock twelve word mnemonic phrase for testing purposes only development".to_string()),
                        error: None,
                    }),
                };

                // 序列化响应到操作缓冲区
                let response_data = serde_json::to_vec(&mock_response)
                    .context("Failed to serialize mock response")?;

                // 将响应写入操作的缓冲区
                if let Some(buffer) = operation.get_buffer_mut() {
                    let copy_len = std::cmp::min(response_data.len(), buffer.len());
                    buffer[..copy_len].copy_from_slice(&response_data[..copy_len]);
                    info!("🔧 Mock: Wrote {} bytes to response buffer", copy_len);
                }

                Ok(())
            }
            _ => {
                warn!("🔧 Mock: Unknown command 0x{:x}", cmd_id);
                Err(anyhow::anyhow!("Mock TEE: Unknown command"))
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        debug!("🔧 Mock: Closing session");
    }
}

// Mock Parameter types
#[derive(Debug)]
pub struct ParamNone;

#[derive(Debug)]
pub struct ParamTmpRef<'a> {
    buffer: &'a mut [u8],
}

impl<'a> ParamTmpRef<'a> {
    pub fn new_output(buffer: &'a mut [u8]) -> Self {
        ParamTmpRef { buffer }
    }
}

// Mock Operation
pub struct Operation {
    started: u32,
    buffer: Option<*mut [u8]>,
}

impl Operation {
    pub fn new(started: u32, p0: ParamTmpRef, _p1: ParamNone, _p2: ParamNone, _p3: ParamNone) -> Self {
        let buffer_ptr = p0.buffer as *mut [u8];
        Operation { 
            started,
            buffer: Some(buffer_ptr),
        }
    }

    pub fn get_buffer_mut(&mut self) -> Option<&mut [u8]> {
        self.buffer.map(|ptr| unsafe { &mut *ptr })
    }
}

// 安全说明：这里的unsafe代码仅用于mock环境，
// 在真实TEE环境中由OP-TEE SDK保证内存安全