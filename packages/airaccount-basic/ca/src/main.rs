// AirAccount Basic CA - 基础框架测试版本
// 基于 eth_wallet 例子实现最简单稳定的 CA-TA 通信

use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};
use anyhow::{bail, Result};
use clap::{Arg, Command as ClapCommand};

// 与 TA 匹配的 UUID
const AIRACCOUNT_TA_UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";
const OUTPUT_MAX_SIZE: usize = 1024;

// 命令定义（简化版本）
#[derive(Debug)]
enum Command {
    Hello = 0,
    Echo = 1,
    GetVersion = 2,
}

// 基础通信函数 - 完全复制 eth_wallet 模式
fn invoke_command(command: Command, input: &[u8]) -> optee_teec::Result<Vec<u8>> {
    let mut ctx = Context::new()?;
    let uuid = Uuid::parse_str(AIRACCOUNT_TA_UUID)
        .map_err(|_| optee_teec::Error::new(optee_teec::ErrorKind::ItemNotFound))?;
    let mut session = ctx.open_session(uuid)?;

    println!("CA: command: {:?}", command);
    
    // 严格按照 eth_wallet 模式设置参数
    // p0: 输入数据 (memref)
    let p0 = ParamTmpRef::new_input(input);
    
    // p1: 输出数据 (memref)
    let mut output = vec![0u8; OUTPUT_MAX_SIZE];
    let p1 = ParamTmpRef::new_output(output.as_mut_slice());
    
    // p2: 输出长度值 (value) - 关键！必须是 ValueInout
    let p2 = ParamValue::new(0, 0, ParamType::ValueInout);

    let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
    
    match session.invoke_command(command as u32, &mut operation) {
        Ok(()) => {
            println!("CA: invoke_command success");
            let output_len = operation.parameters().2.a() as usize;
            Ok(output[..output_len].to_vec())
        }
        Err(e) => {
            let output_len = operation.parameters().2.a() as usize;
            let err_message = String::from_utf8_lossy(&output[..output_len]);
            println!("CA: invoke_command failed: {:?}", err_message);
            Err(e)
        }
    }
}

// 基础功能函数
pub fn hello_world() -> Result<String> {
    println!("📞 Calling Hello World command...");
    let output = invoke_command(Command::Hello, &[])?;
    let response = String::from_utf8_lossy(&output);
    println!("✅ Hello World response: {}", response);
    Ok(response.to_string())
}

pub fn echo_message(message: &str) -> Result<String> {
    println!("📞 Calling Echo command with: '{}'", message);
    let output = invoke_command(Command::Echo, message.as_bytes())?;
    let response = String::from_utf8_lossy(&output);
    println!("✅ Echo response: {}", response);
    Ok(response.to_string())
}

pub fn get_version() -> Result<String> {
    println!("📞 Calling Get Version command...");
    let output = invoke_command(Command::GetVersion, &[])?;
    let response = String::from_utf8_lossy(&output);
    println!("✅ Version response: {}", response);
    Ok(response.to_string())
}

fn main() -> Result<()> {
    let app = ClapCommand::new("AirAccount Basic CA")
        .version("0.1.0")
        .about("基础框架测试版本 - 最简单稳定的 CA-TA 通信")
        .arg(
            Arg::new("command")
                .help("Command to execute")
                .value_parser(["hello", "echo", "version", "test", "interactive"])
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
            hello_world()?;
        }
        Some("echo") => {
            let message = matches
                .get_one::<String>("message")
                .ok_or_else(|| anyhow::anyhow!("Echo command requires a message"))?;
            echo_message(message)?;
        }
        Some("version") => {
            get_version()?;
        }
        Some("test") => {
            println!("🧪 === AirAccount Basic TA-CA Communication Tests ===");
            
            // Test 1: Hello World
            print!("Test 1 - Hello World: ");
            match hello_world() {
                Ok(response) => {
                    if response.contains("AirAccount") || response.contains("Hello") {
                        println!("✅ PASS");
                    } else {
                        println!("❌ FAIL (unexpected response: {})", response);
                    }
                }
                Err(e) => println!("❌ FAIL ({})", e),
            }
            
            // Test 2: Echo
            print!("Test 2 - Echo Test: ");
            match echo_message("Hello TEE!") {
                Ok(response) => {
                    if response == "Hello TEE!" {
                        println!("✅ PASS");
                    } else {
                        println!("❌ FAIL (expected: 'Hello TEE!', got: '{}')", response);
                    }
                }
                Err(e) => println!("❌ FAIL ({})", e),
            }
            
            // Test 3: Version
            print!("Test 3 - Version: ");
            match get_version() {
                Ok(response) => {
                    if !response.is_empty() {
                        println!("✅ PASS ({})", response);
                    } else {
                        println!("❌ FAIL (empty response)");
                    }
                }
                Err(e) => println!("❌ FAIL ({})", e),
            }
        }
        Some("interactive") => {
            println!("🚀 Starting AirAccount Basic Interactive Mode");
            println!("Commands: hello, echo <message>, version, quit");
            println!("=======================================");
            
            loop {
                use std::io::{self, Write};
                print!("AirAccount-Basic> ");
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
                        match hello_world() {
                            Ok(response) => println!("📨 {}", response),
                            Err(e) => println!("❌ Error: {}", e),
                        }
                    }
                    "version" => {
                        match get_version() {
                            Ok(response) => println!("📨 {}", response),
                            Err(e) => println!("❌ Error: {}", e),
                        }
                    }
                    input if input.starts_with("echo ") => {
                        let message = &input[5..];
                        match echo_message(message) {
                            Ok(response) => println!("📨 {}", response),
                            Err(e) => println!("❌ Error: {}", e),
                        }
                    }
                    "" => continue,
                    _ => println!("❓ Unknown command. Try: hello, echo <message>, version, quit"),
                }
            }
        }
        _ => {
            println!("❌ Usage: {} [hello|echo <message>|version|test|interactive]", 
                     std::env::args().next().unwrap_or("airaccount-basic-ca".to_string()));
        }
    }
    
    Ok(())
}