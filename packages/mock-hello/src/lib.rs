// Mock TA-CA communication library for testing AirAccount architecture
// This simulates the eth_wallet TA-CA pattern without requiring OP-TEE

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use anyhow::Result;

// Commands - following eth_wallet pattern
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Command {
    HelloWorld = 0,
    Echo = 1,
    GetVersion = 2,
    CreateWallet = 10,  // Extended commands for future wallet functionality
    GetWalletInfo = 11,
}

impl From<u32> for Command {
    fn from(value: u32) -> Self {
        match value {
            0 => Command::HelloWorld,
            1 => Command::Echo,
            2 => Command::GetVersion,
            10 => Command::CreateWallet,
            11 => Command::GetWalletInfo,
            _ => Command::HelloWorld, // Default fallback
        }
    }
}

// Input/Output structures - following eth_wallet pattern
#[derive(Debug, Serialize, Deserialize)]
pub struct HelloWorldInput;

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloWorldOutput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoInput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoOutput {
    pub response: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetVersionInput;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetVersionOutput {
    pub version: String,
    pub build_info: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWalletInput;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWalletOutput {
    pub wallet_id: Uuid,
    pub message: String,
}

// Mock TA implementation
pub struct MockTA {
    pub version: String,
}

impl MockTA {
    pub fn new() -> Self {
        Self {
            version: "AirAccount Mock TA v0.1.0".to_string(),
        }
    }
    
    pub fn invoke_command(&self, cmd_id: u32, input: &[u8]) -> Result<Vec<u8>> {
        let command = Command::from(cmd_id);
        println!("[MockTA] Processing command: {:?}", command);
        
        match command {
            Command::HelloWorld => {
                let _input: HelloWorldInput = bincode::deserialize(input)?;
                let output = HelloWorldOutput {
                    message: "Hello from AirAccount Mock TA!".to_string(),
                };
                Ok(bincode::serialize(&output)?)
            }
            
            Command::Echo => {
                let input: EchoInput = bincode::deserialize(input)?;
                let output = EchoOutput {
                    response: format!("Mock TA Echo: {}", input.message),
                };
                Ok(bincode::serialize(&output)?)
            }
            
            Command::GetVersion => {
                let _input: GetVersionInput = bincode::deserialize(input)?;
                let output = GetVersionOutput {
                    version: self.version.clone(),
                    build_info: format!("Built on {} at {}", env!("CARGO_PKG_VERSION"), chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")),
                };
                Ok(bincode::serialize(&output)?)
            }
            
            Command::CreateWallet => {
                let _input: CreateWalletInput = bincode::deserialize(input)?;
                let output = CreateWalletOutput {
                    wallet_id: Uuid::new_v4(),
                    message: "Mock wallet created successfully (simulation only)".to_string(),
                };
                Ok(bincode::serialize(&output)?)
            }
            
            Command::GetWalletInfo => {
                // Placeholder for future wallet info functionality
                let output = EchoOutput {
                    response: "Mock wallet info: Not implemented yet".to_string(),
                };
                Ok(bincode::serialize(&output)?)
            }
        }
    }
}

// Mock CA communication functions
pub struct MockCA {
    ta: MockTA,
}

impl MockCA {
    pub fn new() -> Self {
        Self {
            ta: MockTA::new(),
        }
    }
    
    fn invoke_command(&self, command: Command, input: &[u8]) -> Result<Vec<u8>> {
        println!("[MockCA] Invoking command: {:?}", command);
        
        // Simulate the OP-TEE session creation and command invocation
        let result = self.ta.invoke_command(command as u32, input)?;
        
        println!("[MockCA] Command completed successfully");
        Ok(result)
    }
    
    pub fn hello_world(&self) -> Result<String> {
        let input = HelloWorldInput;
        let serialized_input = bincode::serialize(&input)?;
        
        let output = self.invoke_command(Command::HelloWorld, &serialized_input)?;
        let response: HelloWorldOutput = bincode::deserialize(&output)?;
        
        Ok(response.message)
    }
    
    pub fn echo_message(&self, message: &str) -> Result<String> {
        let input = EchoInput {
            message: message.to_string(),
        };
        let serialized_input = bincode::serialize(&input)?;
        
        let output = self.invoke_command(Command::Echo, &serialized_input)?;
        let response: EchoOutput = bincode::deserialize(&output)?;
        
        Ok(response.response)
    }
    
    pub fn get_version(&self) -> Result<(String, String)> {
        let input = GetVersionInput;
        let serialized_input = bincode::serialize(&input)?;
        
        let output = self.invoke_command(Command::GetVersion, &serialized_input)?;
        let response: GetVersionOutput = bincode::deserialize(&output)?;
        
        Ok((response.version, response.build_info))
    }
    
    pub fn create_wallet(&self) -> Result<(Uuid, String)> {
        let input = CreateWalletInput;
        let serialized_input = bincode::serialize(&input)?;
        
        let output = self.invoke_command(Command::CreateWallet, &serialized_input)?;
        let response: CreateWalletOutput = bincode::deserialize(&output)?;
        
        Ok((response.wallet_id, response.message))
    }
}