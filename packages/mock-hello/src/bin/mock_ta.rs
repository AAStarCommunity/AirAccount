// Mock Trusted Application - simulates TA behavior for testing

use airaccout_mock_hello::MockTA;
use std::io::{self, Read, Write};
use anyhow::Result;

fn main() -> Result<()> {
    println!("🔒 AirAccount Mock TA Starting...");
    println!("📝 This simulates a Trusted Application for testing purposes");
    
    let ta = MockTA::new();
    println!("✅ Mock TA initialized successfully");
    
    // Simple REPL for manual testing
    println!("\n=== Mock TA Interactive Mode ===");
    println!("Commands: 0=HelloWorld, 1=Echo, 2=GetVersion, 10=CreateWallet");
    println!("Type 'quit' to exit\n");
    
    loop {
        print!("MockTA> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input == "quit" || input == "exit" {
            break;
        }
        
        // Parse command
        if let Ok(cmd_id) = input.parse::<u32>() {
            // Create dummy input based on command
            let dummy_input = match cmd_id {
                0 => bincode::serialize(&airaccout_mock_hello::HelloWorldInput)?,
                1 => {
                    print("Enter message to echo: ");
                    io::stdout().flush()?;
                    let mut msg = String::new();
                    io::stdin().read_line(&mut msg)?;
                    bincode::serialize(&airaccout_mock_hello::EchoInput {
                        message: msg.trim().to_string(),
                    })?
                }
                2 => bincode::serialize(&airaccout_mock_hello::GetVersionInput)?,
                10 => bincode::serialize(&airaccout_mock_hello::CreateWalletInput)?,
                _ => {
                    println!("❌ Unknown command: {}", cmd_id);
                    continue;
                }
            };
            
            // Process command
            match ta.invoke_command(cmd_id, &dummy_input) {
                Ok(output) => {
                    println!("✅ Command executed successfully");
                    
                    // Try to decode and display output
                    match cmd_id {
                        0 => {
                            if let Ok(resp) = bincode::deserialize::<airaccout_mock_hello::HelloWorldOutput>(&output) {
                                println!("📤 Response: {}", resp.message);
                            }
                        }
                        1 => {
                            if let Ok(resp) = bincode::deserialize::<airaccout_mock_hello::EchoOutput>(&output) {
                                println!("📤 Response: {}", resp.response);
                            }
                        }
                        2 => {
                            if let Ok(resp) = bincode::deserialize::<airaccout_mock_hello::GetVersionOutput>(&output) {
                                println!("📤 Version: {}", resp.version);
                                println!("📤 Build Info: {}", resp.build_info);
                            }
                        }
                        10 => {
                            if let Ok(resp) = bincode::deserialize::<airaccout_mock_hello::CreateWalletOutput>(&output) {
                                println!("📤 Wallet ID: {}", resp.wallet_id);
                                println!("📤 Message: {}", resp.message);
                            }
                        }
                        _ => {
                            println!("📤 Raw output: {} bytes", output.len());
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Command failed: {}", e);
                }
            }
        } else {
            println!("❌ Invalid command. Please enter a number.");
        }
        
        println!(); // Empty line for readability
    }
    
    println!("👋 Mock TA shutting down...");
    Ok(())
}

fn print(msg: &str) {
    print!("{}", msg);
}