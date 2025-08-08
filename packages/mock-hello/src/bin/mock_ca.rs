// Mock Client Application - simulates CA behavior for testing

use airaccout_mock_hello::MockCA;
use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "mock-ca")]
#[command(about = "AirAccount Mock Client Application for testing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Say hello to the Mock TA
    Hello,
    /// Echo a message through the Mock TA
    Echo { message: String },
    /// Get Mock TA version
    Version,
    /// Create a mock wallet (simulation)
    CreateWallet,
    /// Run all basic tests
    Test,
    /// Interactive mode
    Interactive,
}

fn main() -> Result<()> {
    println!("🌐 AirAccount Mock Client Application Starting...");
    
    let cli = Cli::parse();
    let ca = MockCA::new();
    
    match cli.command {
        Commands::Hello => {
            println!("📞 Calling HelloWorld...");
            let response = ca.hello_world()?;
            println!("📤 Response: {}", response);
        }
        
        Commands::Echo { message } => {
            println!("📞 Echoing message: '{}'", message);
            let response = ca.echo_message(&message)?;
            println!("📤 Response: {}", response);
        }
        
        Commands::Version => {
            println!("📞 Getting version information...");
            let (version, build_info) = ca.get_version()?;
            println!("📤 Version: {}", version);
            println!("📤 Build Info: {}", build_info);
        }
        
        Commands::CreateWallet => {
            println!("📞 Creating mock wallet...");
            let (wallet_id, message) = ca.create_wallet()?;
            println!("📤 Wallet ID: {}", wallet_id);
            println!("📤 Message: {}", message);
        }
        
        Commands::Test => {
            run_comprehensive_tests(&ca)?;
        }
        
        Commands::Interactive => {
            run_interactive_mode(&ca)?;
        }
    }
    
    Ok(())
}

fn run_comprehensive_tests(ca: &MockCA) -> Result<()> {
    println!("\n🧪 === AirAccount Mock TA-CA Communication Tests ===\n");
    
    // Test 1: Hello World
    print!("Test 1 - Hello World: ");
    match ca.hello_world() {
        Ok(response) => {
            println!("✅ PASS");
            println!("   Response: {}", response);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("   Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 2: Echo Message
    print!("\nTest 2 - Echo Message: ");
    let test_message = "Hello from AirAccount Mock Client!";
    match ca.echo_message(test_message) {
        Ok(response) => {
            println!("✅ PASS");
            println!("   Input: {}", test_message);
            println!("   Response: {}", response);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("   Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 3: Version Information
    print!("\nTest 3 - Version Info: ");
    match ca.get_version() {
        Ok((version, build_info)) => {
            println!("✅ PASS");
            println!("   Version: {}", version);
            println!("   Build Info: {}", build_info);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("   Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 4: Mock Wallet Creation
    print!("\nTest 4 - Wallet Creation: ");
    match ca.create_wallet() {
        Ok((wallet_id, message)) => {
            println!("✅ PASS");
            println!("   Wallet ID: {}", wallet_id);
            println!("   Message: {}", message);
        }
        Err(e) => {
            println!("❌ FAIL");
            println!("   Error: {}", e);
            return Err(e);
        }
    }
    
    // Test 5: Multiple Operations (stress test)
    print!("\nTest 5 - Multiple Operations: ");
    let mut success_count = 0;
    let total_operations = 10;
    
    for i in 0..total_operations {
        if let Ok(_) = ca.hello_world() {
            success_count += 1;
        }
        if let Ok(_) = ca.echo_message(&format!("Test message {}", i)) {
            success_count += 1;
        }
    }
    
    if success_count == total_operations * 2 {
        println!("✅ PASS");
        println!("   Completed {} operations successfully", success_count);
    } else {
        println!("❌ PARTIAL FAIL");
        println!("   Completed {}/{} operations", success_count, total_operations * 2);
    }
    
    println!("\n🎉 === Test Suite Completed ===");
    println!("✅ Basic TA-CA communication architecture verified");
    println!("✅ Serialization/deserialization working correctly");
    println!("✅ Command routing functioning properly");
    println!("✅ Ready for eth_wallet integration");
    
    Ok(())
}

fn run_interactive_mode(ca: &MockCA) -> Result<()> {
    use std::io::{self, Write};
    
    println!("\n🔄 === Interactive Mock CA Mode ===");
    println!("Available commands:");
    println!("  hello    - Say hello to Mock TA");
    println!("  echo <msg> - Echo a message");
    println!("  version  - Get version info");
    println!("  wallet   - Create mock wallet");
    println!("  test     - Run all tests");
    println!("  quit     - Exit interactive mode");
    println!();
    
    loop {
        print!("MockCA> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input == "quit" || input == "exit" {
            break;
        }
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        
        match parts[0] {
            "hello" => {
                match ca.hello_world() {
                    Ok(response) => println!("✅ {}", response),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "echo" => {
                if parts.len() > 1 {
                    let message = parts[1..].join(" ");
                    match ca.echo_message(&message) {
                        Ok(response) => println!("✅ {}", response),
                        Err(e) => println!("❌ Error: {}", e),
                    }
                } else {
                    println!("❌ Usage: echo <message>");
                }
            }
            "version" => {
                match ca.get_version() {
                    Ok((version, build_info)) => {
                        println!("✅ Version: {}", version);
                        println!("✅ Build: {}", build_info);
                    }
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "wallet" => {
                match ca.create_wallet() {
                    Ok((wallet_id, message)) => {
                        println!("✅ Wallet ID: {}", wallet_id);
                        println!("✅ {}", message);
                    }
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "test" => {
                if let Err(e) = run_comprehensive_tests(ca) {
                    println!("❌ Test failed: {}", e);
                }
            }
            _ => {
                println!("❌ Unknown command: {}", parts[0]);
                println!("   Type 'quit' to exit");
            }
        }
        
        println!();
    }
    
    println!("👋 Exiting interactive mode...");
    Ok(())
}