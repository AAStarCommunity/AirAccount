// AirAccount Basic Client Application - following eth_wallet pattern

use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

const OUTPUT_MAX_SIZE: usize = 1024;
const TA_UUID: &str = "6e256cba-fc4d-4941-ad09-2ca1860342dd";

// Command IDs matching the TA
#[derive(Debug)]
#[repr(u32)]
pub enum Command {
    HelloWorld = 0,
    Echo = 1,
    GetVersion = 2,
}

#[derive(Parser)]
#[command(name = "airaccout-basic")]
#[command(about = "AirAccount Basic TA Test Client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Say hello to the TA
    Hello,
    /// Echo a message through the TA
    Echo { message: String },
    /// Get TA version
    Version,
    /// Run all tests
    Test,
}

fn invoke_command(command: Command, input: &[u8]) -> optee_teec::Result<Vec<u8>> {
    let mut ctx = Context::new()?;
    let uuid = Uuid::parse_str(TA_UUID)
        .map_err(|_| optee_teec::Error::new(optee_teec::ErrorKind::ItemNotFound))?;
    let mut session = ctx.open_session(uuid)?;

    println!("CA: Invoking command: {:?}", command);
    
    // Input buffer
    let p0 = ParamTmpRef::new_input(input);
    // Output buffer
    let mut output = vec![0u8; OUTPUT_MAX_SIZE];
    let p1 = ParamTmpRef::new_output(output.as_mut_slice());
    // Output buffer size
    let p2 = ParamValue::new(0, 0, ParamType::ValueInout);

    let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
    
    match session.invoke_command(command as u32, &mut operation) {
        Ok(()) => {
            println!("CA: Command executed successfully");
            let output_len = operation.parameters().2.a() as usize;
            Ok(output[..output_len].to_vec())
        }
        Err(e) => {
            let output_len = operation.parameters().2.a() as usize;
            let err_message = String::from_utf8_lossy(&output[..output_len]);
            println!("CA: Command failed: {}", err_message);
            Err(e)
        }
    }
}

pub fn hello_world() -> Result<String> {
    let output = invoke_command(Command::HelloWorld, &[])?;
    let response = String::from_utf8(output)?;
    Ok(response)
}

pub fn echo_message(message: &str) -> Result<String> {
    let output = invoke_command(Command::Echo, message.as_bytes())?;
    let response = String::from_utf8(output)?;
    Ok(response)
}

pub fn get_version() -> Result<String> {
    let output = invoke_command(Command::GetVersion, &[])?;
    let response = String::from_utf8(output)?;
    Ok(response)
}

fn run_tests() -> Result<()> {
    println!("=== Running AirAccount Basic TA Tests ===\n");
    
    // Test 1: Hello World
    print!("Test 1 - Hello World: ");
    match hello_world() {
        Ok(response) => {
            println!("✅ PASS");
            println!("  Response: {}", response);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("  Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 2: Echo
    print!("\nTest 2 - Echo Message: ");
    let test_message = "Hello from Client Application!";
    match echo_message(test_message) {
        Ok(response) => {
            println!("✅ PASS");
            println!("  Input: {}", test_message);
            println!("  Response: {}", response);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("  Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 3: Version
    print!("\nTest 3 - Get Version: ");
    match get_version() {
        Ok(response) => {
            println!("✅ PASS");
            println!("  Version: {}", response);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("  Error: {}", e);
            return Err(e);
        }
    }
    
    println!("\n=== All Tests Completed Successfully! ===");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Hello => {
            let response = hello_world()?;
            println!("TA Response: {}", response);
        }
        Commands::Echo { message } => {
            let response = echo_message(&message)?;
            println!("TA Response: {}", response);
        }
        Commands::Version => {
            let response = get_version()?;
            println!("TA Version: {}", response);
        }
        Commands::Test => {
            run_tests()?;
        }
    }
    
    Ok(())
}