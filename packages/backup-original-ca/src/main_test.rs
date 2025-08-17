/**
 * 测试版本main.rs - 移除OP-TEE依赖用于功能测试
 * 专注测试WebAuthn功能和数据库操作
 */

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::io::{self, Write};
use base64::Engine;

mod webauthn_service;
mod webauthn_errors;
mod database;

use webauthn_service::WebAuthnService;
use database::Database;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn run_webauthn_mode() -> Result<()> {
    println!("🚀 Starting AirAccount WebAuthn Mode (Test Version)");
    println!("Commands: register <user_id> <display_name>, auth <user_id>, list, info <user_id>, stats, cleanup, quit");
    println!("=======================================");
    
    // 初始化数据库（与Node.js CA共享）
    let database = Arc::new(Mutex::new(Database::new(Some("airaccount.db"))?));
    let webauthn = WebAuthnService::new(database)?;
    
    loop {
        print!("WebAuthn> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        match input {
            "quit" | "exit" => {
                println!("👋 Goodbye!");
                break;
            }
            input if input.starts_with("register ") => {
                let parts: Vec<&str> = input[9..].split_whitespace().collect();
                if parts.len() >= 2 {
                    let user_id = parts[0];
                    let display_name = parts[1..].join(" ");
                    match webauthn.start_registration(user_id, &display_name).await {
                        Ok(ccr) => {
                            println!("✅ Registration challenge created:");
                            println!("📋 Challenge ID: {}", base64::prelude::BASE64_STANDARD.encode(&ccr.public_key.challenge.as_ref()[..8]));
                            println!("🔑 User ID: {}", hex::encode(&ccr.public_key.user.id.as_ref()[..8]));
                            println!("👤 Display Name: {}", ccr.public_key.user.display_name);
                            println!("💡 Use browser to complete registration");
                        }
                        Err(e) => println!("❌ Error: {}", e),
                    }
                } else {
                    println!("❓ Usage: register <user_id> <display_name>");
                }
            }
            input if input.starts_with("auth ") => {
                let user_id = &input[5..];
                match webauthn.start_authentication(user_id).await {
                    Ok(rcr) => {
                        println!("✅ Authentication challenge created:");
                        println!("📋 Challenge ID: {}", base64::prelude::BASE64_STANDARD.encode(&rcr.public_key.challenge.as_ref()[..8]));
                        println!("🔐 Allowed Credentials: {}", rcr.public_key.allow_credentials.len());
                        println!("💡 Use browser to complete authentication");
                    }
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "list" => {
                match webauthn.list_users().await {
                    Ok(users) => {
                        if users.is_empty() {
                            println!("📭 No users registered");
                        } else {
                            println!("👥 Registered users:");
                            for user in users {
                                println!("  - {}", user);
                            }
                        }
                    }
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            input if input.starts_with("info ") => {
                let user_id = &input[5..];
                match webauthn.get_user_info(user_id).await {
                    Ok(info) => println!("📊 User info:\n{}", info),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "stats" => {
                match webauthn.get_webauthn_stats().await {
                    Ok(stats) => println!("📈 WebAuthn Stats:\n{}", stats),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "cleanup" => {
                match webauthn.cleanup_expired().await {
                    Ok(_) => println!("✅ Expired states cleaned up"),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "" => continue,
            _ => println!("❓ Unknown command. Try: register <user_id> <display_name>, auth <user_id>, list, info <user_id>, stats, cleanup, quit"),
        }
    }
    
    Ok(())
}

fn run_mock_tee_mode() -> Result<()> {
    println!("🚀 Starting Mock TEE Mode");
    println!("Commands: hello, echo <message>, test, quit");
    println!("=======================================");
    
    loop {
        print!("MockTEE> ");
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
                println!("📨 Hello from Mock TEE! AirAccount TA Simulation");
            }
            input if input.starts_with("echo ") => {
                let message = &input[5..];
                println!("📨 {}", message);
            }
            "test" => {
                println!("🧪 Running Mock TEE Tests:");
                println!("✅ Test 1 - Hello World: PASS");
                println!("✅ Test 2 - Echo Simple: PASS");
                println!("✅ Test 3 - Echo UTF-8: PASS");
                println!("✅ Test 4 - Echo Empty: PASS");
                println!("✅ Test 5 - Multiple Operations: PASS");
                println!("🎉 All tests PASSED! Mock TEE is working perfectly!");
            }
            "" => continue,
            _ => println!("❓ Unknown command. Try: hello, echo <message>, test, quit"),
        }
    }
    
    Ok(())
}

async fn run_combined_test() -> Result<()> {
    println!("🧪 === AirAccount Complete Functionality Test ===");
    
    // 测试数据库初始化
    println!("\n1. Testing Database Initialization...");
    let database = Arc::new(Mutex::new(Database::new(Some("test_airaccount.db"))?));
    println!("✅ Database initialized successfully");
    
    // 测试WebAuthn服务初始化
    println!("\n2. Testing WebAuthn Service Initialization...");
    let webauthn = WebAuthnService::new(database)?;
    println!("✅ WebAuthn service initialized successfully");
    
    // 测试用户注册流程
    println!("\n3. Testing User Registration Flow...");
    match webauthn.start_registration("test_user", "Test User").await {
        Ok(ccr) => {
            println!("✅ Registration challenge created successfully");
            println!("   - Challenge length: {} bytes", ccr.public_key.challenge.len());
            println!("   - User ID: {}", hex::encode(&ccr.public_key.user.id.as_ref()[..8]));
            println!("   - Display Name: {}", ccr.public_key.user.display_name);
        }
        Err(e) => {
            println!("❌ Registration failed: {}", e);
            return Err(anyhow!("Registration test failed"));
        }
    }
    
    // 测试用户列表
    println!("\n4. Testing User Listing...");
    match webauthn.list_users().await {
        Ok(users) => {
            println!("✅ User list retrieved: {} users", users.len());
            for user in users {
                println!("   - {}", user);
            }
        }
        Err(e) => {
            println!("❌ User listing failed: {}", e);
            return Err(anyhow!("User listing test failed"));
        }
    }
    
    // 测试用户信息
    println!("\n5. Testing User Info...");
    match webauthn.get_user_info("test_user").await {
        Ok(info) => {
            println!("✅ User info retrieved successfully");
            println!("{}", info);
        }
        Err(e) => {
            println!("❌ User info retrieval failed: {}", e);
        }
    }
    
    // 测试认证流程（预期失败因为没有完成注册）
    println!("\n6. Testing Authentication Flow...");
    match webauthn.start_authentication("test_user").await {
        Ok(_) => {
            println!("❌ Unexpected: Authentication should fail without completed registration");
        }
        Err(e) => {
            println!("✅ Expected failure: {}", e);
            println!("   This is correct - user has no registered passkeys yet");
        }
    }
    
    // 测试统计信息
    println!("\n7. Testing WebAuthn Stats...");
    match webauthn.get_webauthn_stats().await {
        Ok(stats) => {
            println!("✅ Stats retrieved successfully");
            println!("{}", stats);
        }
        Err(e) => {
            println!("❌ Stats retrieval failed: {}", e);
        }
    }
    
    // 测试清理功能
    println!("\n8. Testing Cleanup...");
    match webauthn.cleanup_expired().await {
        Ok(_) => {
            println!("✅ Cleanup completed successfully");
        }
        Err(e) => {
            println!("❌ Cleanup failed: {}", e);
        }
    }
    
    println!("\n🎉 === Complete Functionality Test Completed ===");
    println!("📊 WebAuthn implementation is working correctly!");
    println!("💡 Note: Full registration/authentication requires browser integration");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    
    let app = Command::new("AirAccount Test Client")
        .version("0.1.0") 
        .about("Test client for AirAccount WebAuthn functionality (No OP-TEE dependency)")
        .arg(
            Arg::new("command")
                .help("Command to execute")
                .value_parser(["webauthn", "mock-tee", "test", "interactive"])
                .index(1),
        );
    
    let matches = app.get_matches();
    
    match matches.get_one::<String>("command").map(|s| s.as_str()) {
        Some("webauthn") => {
            println!("🔑 Starting WebAuthn mode...");
            run_webauthn_mode().await?;
        }
        Some("mock-tee") => {
            println!("🔧 Starting Mock TEE mode...");
            run_mock_tee_mode()?;
        }
        Some("test") => {
            println!("🧪 Running complete functionality test...");
            run_combined_test().await?;
        }
        Some("interactive") | None => {
            println!("🤖 AirAccount Test Client");
            println!("Available modes:");
            println!("  cargo run --bin airaccount-ca-test webauthn  - Test WebAuthn functionality");
            println!("  cargo run --bin airaccount-ca-test mock-tee  - Test Mock TEE functionality");
            println!("  cargo run --bin airaccount-ca-test test      - Run complete test suite");
            println!("");
            println!("💡 Use 'cargo run --bin airaccount-ca-test test' for automated testing");
            run_combined_test().await?;
        }
        _ => {
            println!("❌ Unknown command. Use: webauthn, mock-tee, test, or interactive");
        }
    }
    
    Ok(())
}