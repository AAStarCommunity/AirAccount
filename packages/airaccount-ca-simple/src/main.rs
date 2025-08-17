/*
 * AirAccount Simple CA - 简化版CA，专门用于测试TA通信
 * 不包含WebAuthn等复杂依赖，专注于基础TA-CA通信验证
 */

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use optee_teec::*;

// TA UUID
fn get_ta_uuid() -> Uuid {
    Uuid::parse_str("11223344-5566-7788-99aa-bbccddeeff01").unwrap()
}

// 命令ID
const CMD_HELLO_WORLD: u32 = 0;
const CMD_ECHO: u32 = 1;
const CMD_VERSION: u32 = 2;
const CMD_CREATE_WALLET: u32 = 10;

struct AirAccountClient {
    context: Context,
    session: Session,
}

impl AirAccountClient {
    fn new() -> Result<Self> {
        println!("🔧 Initializing AirAccount Simple Client...");
        
        let mut context = Context::new()?;
        println!("✅ TEE Context created successfully");
        
        let ta_uuid = get_ta_uuid();
        let session = context.open_session(ta_uuid.clone())?;
        println!("✅ Session opened with AirAccount TA (UUID: {})", ta_uuid);
        
        Ok(AirAccountClient { context, session })
    }
    
    fn hello_world(&mut self) -> Result<String> {
        println!("📞 Calling Hello World command...");
        
        let mut output_buffer = vec![0u8; 1024];
        
        // 按照TA期望的参数格式：p0=空输入, p1=输出缓冲区, p2=长度值
        let p0 = ParamTmpRef::new_input(&[]); // 空输入
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout); // 长度参数
        
        let mut operation = Operation::new(0, p0, p1, p2, ParamNone); 
        
        self.session.invoke_command(CMD_HELLO_WORLD, &mut operation)
            .map_err(|e| anyhow!("Hello World command failed: {:?}", e))?;
        
        // 获取实际输出长度（从 p2 参数）
        let output_len = operation.parameters().2.a() as usize;
        let response = String::from_utf8_lossy(&output_buffer[..output_len]);
        
        println!("✅ Hello World response: {}", response);
        Ok(response.to_string())
    }
    
    fn echo(&mut self, message: &str) -> Result<String> {
        println!("📞 Calling Echo command with: '{}'", message);
        
        let input_buffer = message.as_bytes();
        let mut output_buffer = vec![0u8; 1024];
        
        // 按照TA期望的参数格式：p0=输入缓冲区, p1=输出缓冲区, p2=长度值
        let p0 = ParamTmpRef::new_input(input_buffer);
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout); // 长度参数
        
        let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
        
        self.session.invoke_command(CMD_ECHO, &mut operation)
            .map_err(|e| anyhow!("Echo command failed: {:?}", e))?;
        
        // 获取实际输出长度
        let output_len = operation.parameters().2.a() as usize;
        let response = String::from_utf8_lossy(&output_buffer[..output_len]);
        
        println!("✅ Echo response: {}", response);
        Ok(response.to_string())
    }
    
    fn version(&mut self) -> Result<String> {
        println!("📞 Calling Version command...");
        
        let mut output_buffer = vec![0u8; 1024];
        
        let p0 = ParamTmpRef::new_input(&[]);
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout);
        
        let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
        
        self.session.invoke_command(CMD_VERSION, &mut operation)
            .map_err(|e| anyhow!("Version command failed: {:?}", e))?;
        
        let output_len = operation.parameters().2.a() as usize;
        let response = String::from_utf8_lossy(&output_buffer[..output_len]);
        
        println!("✅ Version response: {}", response);
        Ok(response.to_string())
    }
    
    fn security_check(&mut self) -> Result<String> {
        println!("📞 Calling Create Wallet command...");
        
        let mut output_buffer = vec![0u8; 1024];
        
        let p0 = ParamTmpRef::new_input(&[]);
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout);
        
        let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
        
        self.session.invoke_command(CMD_CREATE_WALLET, &mut operation)
            .map_err(|e| anyhow!("Create Wallet command failed: {:?}", e))?;
        
        let output_len = operation.parameters().2.a() as usize;
        let response = String::from_utf8_lossy(&output_buffer[..output_len]);
        
        println!("✅ Create Wallet response: {}", response);
        Ok(response.to_string())
    }
    
    fn run_tests(&mut self) -> Result<()> {
        println!("🧪 === AirAccount Simple TA-CA Communication Tests ===");
        
        let mut passed = 0;
        let mut total = 0;
        
        // Test 1: Hello World
        total += 1;
        print!("Test 1 - Hello World: ");
        match self.hello_world() {
            Ok(response) => {
                if response.contains("Hello") {
                    println!("✅ PASS");
                    passed += 1;
                } else {
                    println!("❌ FAIL (unexpected response: {})", response);
                }
            },
            Err(e) => println!("❌ FAIL ({})", e),
        }
        
        // Test 2: Echo
        total += 1;
        print!("Test 2 - Echo: ");
        match self.echo("Test Message") {
            Ok(response) => {
                if response == "Test Message" {
                    println!("✅ PASS");
                    passed += 1;
                } else {
                    println!("❌ FAIL (expected 'Test Message', got '{}')", response);
                }
            },
            Err(e) => println!("❌ FAIL ({})", e),
        }
        
        // Test 3: Version
        total += 1;
        print!("Test 3 - Version: ");
        match self.version() {
            Ok(response) => {
                if !response.is_empty() {
                    println!("✅ PASS");
                    passed += 1;
                } else {
                    println!("❌ FAIL (empty response)");
                }
            },
            Err(e) => println!("❌ FAIL ({})", e),
        }
        
        // Test 4: Create Wallet (CMD_ID=10)
        total += 1;
        print!("Test 4 - Create Wallet: ");
        match self.security_check() {
            Ok(response) => {
                if response.contains("wallet_created") || response.contains("id=") {
                    println!("✅ PASS");
                    passed += 1;
                } else {
                    println!("❌ FAIL (unexpected response: {})", response);
                }
            },
            Err(e) => println!("❌ FAIL ({})", e),
        }
        
        println!("\n🎉 === Test Suite Completed ===");
        println!("📊 Results: {}/{} tests passed ({:.1}%)", passed, total, (passed as f32 / total as f32) * 100.0);
        
        if passed == total {
            println!("🎉 All tests PASSED! TA-CA communication is working correctly.");
            Ok(())
        } else {
            Err(anyhow!("Some tests failed. Please check TA implementation."))
        }
    }
}

fn main() -> Result<()> {
    let matches = Command::new("airaccount-ca-simple")
        .about("AirAccount Simple CA - Basic TA communication testing")
        .subcommand(Command::new("hello").about("Test Hello World command"))
        .subcommand(Command::new("echo").about("Test Echo command").arg(Arg::new("message").required(true)))
        .subcommand(Command::new("version").about("Get TA version"))
        .subcommand(Command::new("wallet").about("Create a new wallet"))
        .subcommand(Command::new("test").about("Run all tests"))
        .subcommand(Command::new("interactive").about("Interactive mode"))
        .get_matches();
    
    let mut client = AirAccountClient::new()?;
    
    match matches.subcommand() {
        Some(("hello", _)) => {
            client.hello_world()?;
        },
        Some(("echo", sub_m)) => {
            let message = sub_m.get_one::<String>("message").unwrap();
            client.echo(message)?;
        },
        Some(("version", _)) => {
            client.version()?;
        },
        Some(("wallet", _)) => {
            client.security_check()?;
        },
        Some(("test", _)) => {
            client.run_tests()?;
        },
        Some(("interactive", _)) => {
            println!("📝 AirAccount Simple Interactive Mode - Type 'help' for commands");
            loop {
                print!("AirAccount> ");
                use std::io::{self, Write};
                io::stdout().flush().unwrap();
                
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                
                match input {
                    "help" => {
                        println!("Available commands:");
                        println!("  hello    - Test Hello World");
                        println!("  echo <msg> - Test Echo with message");
                        println!("  version  - Get TA version");
                        println!("  wallet   - Create a new wallet");
                        println!("  test     - Run all tests");
                        println!("  quit     - Exit");
                    },
                    "hello" => { let _ = client.hello_world(); },
                    "version" => { let _ = client.version(); },
                    "wallet" => { let _ = client.security_check(); },
                    "test" => { let _ = client.run_tests(); },
                    "quit" | "exit" => break,
                    line if line.starts_with("echo ") => {
                        let message = &line[5..];
                        let _ = client.echo(message);
                    },
                    _ => println!("Unknown command. Type 'help' for available commands."),
                }
            }
        },
        _ => {
            // 默认运行测试
            client.run_tests()?;
        }
    }
    
    Ok(())
}