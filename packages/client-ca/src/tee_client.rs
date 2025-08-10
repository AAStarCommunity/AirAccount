// Licensed to AirAccount under the Apache License, Version 2.0
// TEE Client - Communication with AirAccount Trusted Application

use anyhow::{Result, Context};
use log::{debug, info, warn};
use airaccount_proto::{WalletCommand, WalletResponse, AirAccountRequest, AirAccountResponse, 
                      command_ids, AIRACCOUNT_TA_UUID};
use uuid::Uuid as StdUuid;

#[cfg(feature = "real_tee")]
use optee_teec::{Context as TeeContext, Session, Operation, ParamNone, ParamTmpRef, Uuid};

#[cfg(feature = "mock_tee")]
use crate::mock_tee::{Context as TeeContext, Session, Operation, ParamNone, ParamTmpRef, Uuid};

pub struct TeeClient {
    context: TeeContext,
    session: Option<Session>,
}

impl TeeClient {
    pub fn new() -> Result<Self> {
        info!("Initializing TEE client context");
        
        // Initialize TEE context
        let context = TeeContext::new().context("Failed to create TEE context")?;
        
        Ok(Self {
            context,
            session: None,
        })
    }
    
    /// Connect to the AirAccount TA
    pub fn connect(&mut self) -> Result<()> {
        if self.session.is_some() {
            warn!("Already connected to TA");
            return Ok(());
        }
        
        info!("Connecting to AirAccount TA");
        
        // Parse TA UUID
        let ta_uuid = Uuid::parse_str(AIRACCOUNT_TA_UUID)
            .context("Failed to parse TA UUID")?;
        
        debug!("TA UUID: {}", AIRACCOUNT_TA_UUID);
        
        // Open session with TA
        let session = self.context.open_session(ta_uuid)
            .context("Failed to open session with AirAccount TA")?;
        
        self.session = Some(session);
        info!("Successfully connected to AirAccount TA");
        
        Ok(())
    }
    
    /// Disconnect from the TA
    pub fn disconnect(&mut self) {
        if let Some(session) = self.session.take() {
            info!("Disconnecting from AirAccount TA");
            drop(session);
        }
    }
    
    /// Send a wallet command to the TA
    pub async fn send_command(&mut self, command: WalletCommand) -> Result<WalletResponse> {
        // Ensure we're connected
        if self.session.is_none() {
            self.connect()?;
        }
        
        let session = self.session.as_mut()
            .context("No active session with TA")?;
        
        // Generate request ID
        let request_id = StdUuid::new_v4().to_string();
        
        // Create request
        let request = AirAccountRequest {
            request_id: request_id.clone(),
            command,
        };
        
        debug!("Sending request: {:?}", request);
        
        // Serialize request
        let request_data = serde_json::to_vec(&request)
            .context("Failed to serialize request")?;
        
        // Prepare operation parameters
        let mut buffer = vec![0u8; 4096]; // 4KB buffer for response
        buffer[..request_data.len()].copy_from_slice(&request_data);
        
        let p0 = ParamTmpRef::new_output(&mut buffer);
        let p1 = ParamNone;
        let p2 = ParamNone;
        let p3 = ParamNone;
        
        let mut operation = Operation::new(0, p0, p1, p2, p3);
        
        // Invoke command
        debug!("Invoking TA command: 0x{:x}", command_ids::WALLET_COMMAND);
        session.invoke_command(command_ids::WALLET_COMMAND, &mut operation)
            .context("Failed to invoke TA command")?;
        
        // Find the end of JSON data in buffer (look for null terminator or actual JSON end)
        let response_end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let response_data = &buffer[..response_end];
        
        debug!("Received {} bytes response", response_data.len());
        debug!("Response JSON: {}", String::from_utf8_lossy(response_data));
        
        // Deserialize response
        let response: AirAccountResponse = serde_json::from_slice(response_data)
            .context("Failed to deserialize response")?;
        
        debug!("Response: {:?}", response);
        
        // Validate response ID
        if response.request_id != request_id {
            warn!("Request ID mismatch: expected {}, got {}", request_id, response.request_id);
        }
        
        Ok(response.response)
    }
    
    /// Check if connected to TA
    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }
}

impl Drop for TeeClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_tee_client_creation() {
        let result = TeeClient::new();
        assert!(result.is_ok(), "Failed to create TEE client: {:?}", result.err());
    }
    
    #[tokio::test]
    async fn test_connection() {
        let mut client = TeeClient::new().expect("Failed to create TEE client");
        
        // Note: This test will fail if TA is not deployed
        let result = client.connect();
        if result.is_err() {
            println!("Connection failed (expected if TA not deployed): {:?}", result.err());
        }
    }
}