use anyhow::{anyhow, Result};
use optee_teec::{Context, Operation, Session, Uuid};
use optee_teec::{ParamNone, ParamTmpRef};

// AirAccount Simple TA UUID
const AIRACCOUNT_TA_UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";

// Wallet commands
const CMD_HELLO_WORLD: u32 = 0;
const CMD_CREATE_WALLET: u32 = 10;
const CMD_DERIVE_ADDRESS: u32 = 12;
const CMD_SIGN_TRANSACTION: u32 = 13;
const CMD_GET_WALLET_INFO: u32 = 14;
const CMD_LIST_WALLETS: u32 = 15;
const CMD_TEST_SECURITY: u32 = 16;

pub fn test_wallet_functionality() -> Result<()> {
    println!("🚀 Starting AirAccount Wallet Functionality Test");
    println!("{}", "=".repeat(60));
    
    // Initialize client
    let uuid = Uuid::parse_str(AIRACCOUNT_TA_UUID)?;
    let mut context = Context::new()?;
    let mut session = context.open_session(uuid)?;
    
    println!("✅ Connected to AirAccount Simple TA");
    
    // Test 1: Hello World
    test_hello_world(&mut session)?;
    
    // Test 2: Create Wallet
    let wallet_id = test_create_wallet(&mut session)?;
    
    // Test 3: List Wallets
    test_list_wallets(&mut session)?;
    
    // Test 4: Get Wallet Info
    test_wallet_info(&mut session, wallet_id)?;
    
    // Test 5: Derive Address
    test_derive_address(&mut session, wallet_id)?;
    
    // Test 6: Sign Transaction
    test_sign_transaction(&mut session, wallet_id)?;
    
    // Test 7: Security Features Test
    test_security_features(&mut session)?;
    
    println!("\n🎉 All wallet functionality tests completed successfully!");
    Ok(())
}

fn invoke_command_simple(session: &mut Session, cmd: u32, input: Option<&[u8]>) -> Result<String> {
    let mut output_buffer = vec![0u8; 1024];
    let empty_buffer = vec![];
    
    let input_data = match input {
        Some(data) => data,
        None => &empty_buffer,
    };
    
    let p0 = ParamTmpRef::new_input(input_data);
    let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
    let mut operation = Operation::new(0, p0, p1, ParamNone, ParamNone);
    
    session.invoke_command(cmd, &mut operation)?;
    
    let response_len = output_buffer.iter().position(|&x| x == 0).unwrap_or(output_buffer.len());
    let response = String::from_utf8_lossy(&output_buffer[..response_len]);
    
    Ok(response.to_string())
}

fn test_hello_world(session: &mut Session) -> Result<()> {
    println!("\n📞 Test 1: Hello World");
    
    let response = invoke_command_simple(session, CMD_HELLO_WORLD, None)?;
    println!("📨 Response: {}", response);
    Ok(())
}

fn test_create_wallet(session: &mut Session) -> Result<u32> {
    println!("\n📞 Test 2: Create Wallet");
    
    let response = invoke_command_simple(session, CMD_CREATE_WALLET, None)?;
    println!("📨 Create Wallet Response: {}", response);
    
    // Extract wallet ID from response (assuming format "wallet_created:id=X")
    if let Some(id_part) = response.split("id=").nth(1) {
        if let Ok(wallet_id) = id_part.trim().parse::<u32>() {
            println!("✅ Wallet created with ID: {}", wallet_id);
            return Ok(wallet_id);
        }
    }
    
    Err(anyhow!("Failed to parse wallet ID from response"))
}

fn test_list_wallets(session: &mut Session) -> Result<()> {
    println!("\n📞 Test 3: List Wallets");
    
    let response = invoke_command_simple(session, CMD_LIST_WALLETS, None)?;
    println!("📨 List Wallets Response: {}", response);
    Ok(())
}

fn test_wallet_info(session: &mut Session, wallet_id: u32) -> Result<()> {
    println!("\n📞 Test 4: Get Wallet Info (ID: {})", wallet_id);
    
    let input_data = wallet_id.to_le_bytes();
    let response = invoke_command_simple(session, CMD_GET_WALLET_INFO, Some(&input_data))?;
    println!("📨 Wallet Info Response: {}", response);
    Ok(())
}

fn test_derive_address(session: &mut Session, wallet_id: u32) -> Result<()> {
    println!("\n📞 Test 5: Derive Address (ID: {})", wallet_id);
    
    let input_data = wallet_id.to_le_bytes();
    let response = invoke_command_simple(session, CMD_DERIVE_ADDRESS, Some(&input_data))?;
    println!("📨 Derive Address Response: {}", response);
    Ok(())
}

fn test_sign_transaction(session: &mut Session, wallet_id: u32) -> Result<()> {
    println!("\n📞 Test 6: Sign Transaction (ID: {})", wallet_id);
    
    // Create test transaction data: wallet_id + dummy hash
    let mut input_data = Vec::new();
    input_data.extend_from_slice(&wallet_id.to_le_bytes());
    input_data.extend_from_slice(b"test_transaction_hash_for_signing_demo");
    
    let response = invoke_command_simple(session, CMD_SIGN_TRANSACTION, Some(&input_data))?;
    println!("📨 Sign Transaction Response: {}", response);
    Ok(())
}

fn test_security_features(session: &mut Session) -> Result<()> {
    println!("\n📞 Test 7: Security Features Test");
    
    let response = invoke_command_simple(session, CMD_TEST_SECURITY, None)?;
    println!("📨 Security Test Response: {}", response);
    
    // Verify security features are working
    if response.contains("secure_memory:PASS") {
        println!("✅ Secure Memory Test: PASSED");
    } else {
        println!("❌ Secure Memory Test: FAILED");
    }
    
    if response.contains("constant_time:PASS") {
        println!("✅ Constant Time Operations Test: PASSED");
    } else {
        println!("❌ Constant Time Operations Test: FAILED");
    }
    
    if response.contains("audit_log:PASS") {
        println!("✅ Audit Logging Test: PASSED");
    } else {
        println!("❌ Audit Logging Test: FAILED");
    }
    
    Ok(())
}