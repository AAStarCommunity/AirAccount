use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use optee_teec::{Context, Operation, ParamType, Session, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};
use std::io::{self, Write};

mod wallet_test;

// AirAccount Simple TA UUID - matches the one in simple TA's build.rs
const AIRACCOUNT_TA_UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";

// Command constants - matches the TA implementation  
const CMD_HELLO_WORLD: u32 = 0;
const CMD_ECHO: u32 = 1;
const CMD_GET_VERSION: u32 = 2;

// Wallet commands (10-15)
const CMD_CREATE_WALLET: u32 = 10;
const CMD_REMOVE_WALLET: u32 = 11;
const CMD_DERIVE_ADDRESS: u32 = 12;
const CMD_SIGN_TRANSACTION: u32 = 13;
const CMD_GET_WALLET_INFO: u32 = 14;
const CMD_LIST_WALLETS: u32 = 15;

struct AirAccountClient {
    _context: Context,
    session: Session,
}

impl AirAccountClient {
    fn new() -> Result<Self> {
        println!("🔧 Initializing AirAccount Client...");
        
        // Parse UUID
        let uuid = Uuid::parse_str(AIRACCOUNT_TA_UUID)
            .map_err(|e| anyhow!("Failed to parse TA UUID: {}", e))?;
        
        // Initialize TEE context
        let mut context = Context::new()
            .map_err(|e| anyhow!("Failed to create TEE context: {:?}", e))?;
        
        println!("✅ TEE Context created successfully");
        
        // Open session with TA
        let session = context.open_session(uuid)
            .map_err(|e| anyhow!("Failed to open session with AirAccount TA: {:?}", e))?;
        
        println!("✅ Session opened with AirAccount TA (UUID: {})", AIRACCOUNT_TA_UUID);
        
        Ok(AirAccountClient {
            _context: context,
            session,
        })
    }
    
    fn hello_world(&mut self) -> Result<String> {
        println!("📞 Calling Hello World command...");
        
        let mut output_buffer = vec![0u8; 1024];
        
        // Empty input buffer
        let p0 = ParamTmpRef::new_input(&[]);
        // Output buffer for response
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        
        let mut operation = Operation::new(0, p0, p1, ParamNone, ParamNone); 
        
        self.session.invoke_command(CMD_HELLO_WORLD, &mut operation)
            .map_err(|e| anyhow!("Hello World command failed: {:?}", e))?;
        
        // Find the actual length of response
        let response_len = output_buffer.iter().position(|&x| x == 0).unwrap_or(output_buffer.len());
        let response = String::from_utf8_lossy(&output_buffer[..response_len]);
        
        println!("✅ Hello World response: {}", response);
        Ok(response.to_string())
    }
    
    fn echo(&mut self, message: &str) -> Result<String> {
        println!("📞 Calling Echo command with: '{}'", message);
        
        let input_buffer = message.as_bytes();
        let mut output_buffer = vec![0u8; 1024];
        
        // Input buffer with message
        let p0 = ParamTmpRef::new_input(input_buffer);
        // Output buffer for response
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        
        let mut operation = Operation::new(0, p0, p1, ParamNone, ParamNone);
        
        self.session.invoke_command(CMD_ECHO, &mut operation)
            .map_err(|e| anyhow!("Echo command failed: {:?}", e))?;
        
        // Find the actual length of response
        let response_len = output_buffer.iter().position(|&x| x == 0).unwrap_or(output_buffer.len());
        let response = String::from_utf8_lossy(&output_buffer[..response_len]);
        
        println!("✅ Echo response: {}", response);
        Ok(response.to_string())
    }
}

fn run_interactive_mode() -> Result<()> {
    println!("🚀 Starting AirAccount Interactive Mode");
    println!("Commands: hello, echo <message>, quit");
    println!("=======================================");
    
    let mut client = AirAccountClient::new()?;
    
    loop {
        print!("AirAccount> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        match input {
            "quit" | "exit" => {
                println!("👋 Goodbye!");
                break;
            }
            "hello" => {
                match client.hello_world() {
                    Ok(response) => println!("📨 {}", response),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            input if input.starts_with("echo ") => {
                let message = &input[5..];
                match client.echo(message) {
                    Ok(response) => println!("📨 {}", response),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "" => continue,
            _ => println!("❓ Unknown command. Try: hello, echo <message>, quit"),
        }
    }
    
    Ok(())
}

fn run_test_suite() -> Result<()> {
    println!("🧪 === AirAccount TA-CA Communication Tests ===");
    
    let mut client = AirAccountClient::new()?;
    let mut passed = 0;
    let mut total = 0;
    
    // Test 1: Hello World
    total += 1;
    print!("Test 1 - Hello World: ");
    match client.hello_world() {
        Ok(response) => {
            if response.contains("AirAccount") {
                println!("✅ PASS");
                passed += 1;
            } else {
                println!("❌ FAIL (unexpected response: {})", response);
            }
        }
        Err(e) => println!("❌ FAIL ({})", e),
    }
    
    // Test 2: Echo Simple Message
    total += 1;
    print!("Test 2 - Echo Simple: ");
    match client.echo("Hello TEE!") {
        Ok(response) => {
            if response == "Hello TEE!" {
                println!("✅ PASS");
                passed += 1;
            } else {
                println!("❌ FAIL (expected: 'Hello TEE!', got: '{}')", response);
            }
        }
        Err(e) => println!("❌ FAIL ({})", e),
    }
    
    // Test 3: Echo UTF-8 Message
    total += 1;
    print!("Test 3 - Echo UTF-8: ");
    let utf8_msg = "你好 AirAccount! 🚀";
    match client.echo(utf8_msg) {
        Ok(response) => {
            if response == utf8_msg {
                println!("✅ PASS");
                passed += 1;
            } else {
                println!("❌ FAIL (UTF-8 handling issue)");
            }
        }
        Err(e) => println!("❌ FAIL ({})", e),
    }
    
    // Test 4: Empty Echo
    total += 1;
    print!("Test 4 - Echo Empty: ");
    match client.echo("") {
        Ok(response) => {
            if response.is_empty() {
                println!("✅ PASS");
                passed += 1;
            } else {
                println!("❌ FAIL (expected empty, got: '{}')", response);
            }
        }
        Err(e) => println!("❌ FAIL ({})", e),
    }
    
    // Test 5: Multiple Operations
    total += 1;
    print!("Test 5 - Multiple Operations: ");
    let mut multi_passed = true;
    for i in 0..5 {
        match client.echo(&format!("Message {}", i)) {
            Ok(response) => {
                if response != format!("Message {}", i) {
                    multi_passed = false;
                    break;
                }
            }
            Err(_) => {
                multi_passed = false;
                break;
            }
        }
    }
    
    if multi_passed {
        println!("✅ PASS (5/5 operations)");
        passed += 1;
    } else {
        println!("❌ FAIL (multi-operation test failed)");
    }
    
    println!("");
    println!("🎉 === Test Suite Completed ===");
    println!("📊 Results: {}/{} tests passed ({:.1}%)", passed, total, (passed as f32 / total as f32) * 100.0);
    
    if passed == total {
        println!("🎉 All tests PASSED! TA-CA communication is working perfectly!");
        Ok(())
    } else {
        Err(anyhow!("Some tests failed. Please check TA implementation."))
    }
}

fn main() -> Result<()> {
    let app = Command::new("AirAccount Client Application")
        .version("0.1.0") 
        .about("Client application for communicating with AirAccount Trusted Application")
        .arg(
            Arg::new("command")
                .help("Command to execute")
                .value_parser(["hello", "echo", "test", "interactive", "wallet"])
                .index(1),
        )
        .arg(
            Arg::new("message")
                .help("Message for echo command")
                .index(2),
        );
    
    let matches = app.get_matches();
    
    match matches.get_one::<String>("command").map(|s| s.as_str()) {
        Some("hello") => {
            let mut client = AirAccountClient::new()?;
            client.hello_world()?;
        }
        Some("echo") => {
            let message = matches
                .get_one::<String>("message")
                .ok_or_else(|| anyhow!("Echo command requires a message"))?;
            let mut client = AirAccountClient::new()?;
            client.echo(message)?;
        }
        Some("test") => {
            run_test_suite()?;
        }
        Some("wallet") => {
            println!("🏦 Running wallet functionality tests...");
            wallet_test::test_wallet_functionality()?;
        }
        Some("interactive") | None => {
            run_interactive_mode()?;
        }
        _ => {
            println!("❌ Unknown command. Use: hello, echo <message>, test, wallet, or interactive");
        }
    }
    
    Ok(())
}