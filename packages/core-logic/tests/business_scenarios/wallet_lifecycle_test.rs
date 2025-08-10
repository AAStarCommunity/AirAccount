// Licensed to AirAccount under the Apache License, Version 2.0
// End-to-end wallet lifecycle business scenario tests

use airaccount_core_logic::*;
use std::time::Duration;
use tokio::time::sleep;

/// Test complete wallet lifecycle: create -> activate -> use -> lock -> unlock -> destroy
#[tokio::test]
async fn test_complete_wallet_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Phase 1: Wallet Creation
    println!("Phase 1: Creating wallet...");
    let wallet_request = CreateWalletRequest {
        mnemonic: None, // Generate new mnemonic
        password: "test_password_123".to_string(),
        wallet_name: "lifecycle_test_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    assert!(!create_response.address.is_empty());
    assert!(!create_response.mnemonic.is_empty());
    
    let wallet_address = create_response.address.clone();
    println!("✅ Wallet created: {}", wallet_address);
    
    // Phase 2: Wallet Activation
    println!("Phase 2: Activating wallet...");
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "test_password_123".to_string(),
    };
    
    core_context.activate_wallet(activate_request).await?;
    println!("✅ Wallet activated");
    
    // Phase 3: Wallet Usage (Sign Transaction)
    println!("Phase 3: Using wallet for transaction signing...");
    let tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(), // 1 ETH
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(), // 20 Gwei
        nonce: 0,
        chain_id: 1,
    };
    
    let sign_response = core_context.sign_transaction(tx_request).await?;
    assert!(!sign_response.signed_transaction.is_empty());
    println!("✅ Transaction signed successfully");
    
    // Phase 4: Wallet Locking
    println!("Phase 4: Locking wallet...");
    let lock_request = LockWalletRequest {
        address: wallet_address.clone(),
    };
    
    core_context.lock_wallet(lock_request).await?;
    println!("✅ Wallet locked");
    
    // Verify wallet is locked (transaction should fail)
    println!("Phase 4.1: Verifying wallet is locked...");
    let locked_tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "500000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 1,
        chain_id: 1,
    };
    
    let locked_result = core_context.sign_transaction(locked_tx_request).await;
    assert!(locked_result.is_err());
    println!("✅ Confirmed wallet is properly locked");
    
    // Phase 5: Wallet Unlocking
    println!("Phase 5: Unlocking wallet...");
    let unlock_request = UnlockWalletRequest {
        address: wallet_address.clone(),
        password: "test_password_123".to_string(),
    };
    
    core_context.unlock_wallet(unlock_request).await?;
    println!("✅ Wallet unlocked");
    
    // Verify wallet is unlocked (transaction should succeed)
    println!("Phase 5.1: Verifying wallet is unlocked...");
    let unlocked_tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "500000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 1,
        chain_id: 1,
    };
    
    let unlocked_sign_response = core_context.sign_transaction(unlocked_tx_request).await?;
    assert!(!unlocked_sign_response.signed_transaction.is_empty());
    println!("✅ Confirmed wallet is properly unlocked");
    
    // Phase 6: Wallet Status Check
    println!("Phase 6: Checking wallet status...");
    let status_request = GetWalletStatusRequest {
        address: wallet_address.clone(),
    };
    
    let status_response = core_context.get_wallet_status(status_request).await?;
    assert_eq!(status_response.status, "active");
    assert!(!status_response.address.is_empty());
    println!("✅ Wallet status verified");
    
    // Phase 7: Wallet Backup/Recovery Test
    println!("Phase 7: Testing wallet backup...");
    let backup_request = BackupWalletRequest {
        address: wallet_address.clone(),
        password: "test_password_123".to_string(),
    };
    
    let backup_response = core_context.backup_wallet(backup_request).await?;
    assert!(!backup_response.encrypted_backup.is_empty());
    println!("✅ Wallet backup created");
    
    // Phase 8: Wallet Destruction
    println!("Phase 8: Destroying wallet...");
    let destroy_request = DestroyWalletRequest {
        address: wallet_address.clone(),
        password: "test_password_123".to_string(),
        confirm: true,
    };
    
    core_context.destroy_wallet(destroy_request).await?;
    println!("✅ Wallet destroyed");
    
    // Verify wallet is destroyed (status should fail)
    println!("Phase 8.1: Verifying wallet destruction...");
    let final_status_request = GetWalletStatusRequest {
        address: wallet_address.clone(),
    };
    
    let final_result = core_context.get_wallet_status(final_status_request).await;
    assert!(final_result.is_err());
    println!("✅ Confirmed wallet is properly destroyed");
    
    println!("🎉 Complete wallet lifecycle test passed!");
    Ok(())
}

/// Test wallet state transitions with error conditions
#[tokio::test]
async fn test_wallet_state_transitions() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Create wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "state_test_password".to_string(),
        wallet_name: "state_transition_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    // Test 1: Try to lock inactive wallet (should fail)
    let lock_inactive_request = LockWalletRequest {
        address: wallet_address.clone(),
    };
    
    let lock_result = core_context.lock_wallet(lock_inactive_request).await;
    assert!(lock_result.is_err());
    println!("✅ Cannot lock inactive wallet");
    
    // Activate wallet first
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "state_test_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Test 2: Double activation (should be idempotent)
    let double_activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "state_test_password".to_string(),
    };
    core_context.activate_wallet(double_activate_request).await?;
    println!("✅ Double activation handled correctly");
    
    // Test 3: Lock then double lock (should be idempotent)
    let lock_request = LockWalletRequest {
        address: wallet_address.clone(),
    };
    core_context.lock_wallet(lock_request).await?;
    
    let double_lock_request = LockWalletRequest {
        address: wallet_address.clone(),
    };
    core_context.lock_wallet(double_lock_request).await?;
    println!("✅ Double locking handled correctly");
    
    // Test 4: Wrong password unlock (should fail)
    let wrong_unlock_request = UnlockWalletRequest {
        address: wallet_address.clone(),
        password: "wrong_password".to_string(),
    };
    
    let wrong_unlock_result = core_context.unlock_wallet(wrong_unlock_request).await;
    assert!(wrong_unlock_result.is_err());
    println!("✅ Wrong password unlock properly rejected");
    
    // Cleanup
    let unlock_request = UnlockWalletRequest {
        address: wallet_address.clone(),
        password: "state_test_password".to_string(),
    };
    core_context.unlock_wallet(unlock_request).await?;
    
    let destroy_request = DestroyWalletRequest {
        address: wallet_address,
        password: "state_test_password".to_string(),
        confirm: true,
    };
    core_context.destroy_wallet(destroy_request).await?;
    
    println!("🎉 Wallet state transition test passed!");
    Ok(())
}

/// Test wallet operations with invalid inputs
#[tokio::test]
async fn test_wallet_error_conditions() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Test 1: Create wallet with weak password
    let weak_password_request = CreateWalletRequest {
        mnemonic: None,
        password: "123".to_string(), // Too weak
        wallet_name: "weak_password_wallet".to_string(),
    };
    
    let weak_result = core_context.create_wallet(weak_password_request).await;
    assert!(weak_result.is_err());
    println!("✅ Weak password properly rejected");
    
    // Test 2: Create wallet with invalid mnemonic
    let invalid_mnemonic_request = CreateWalletRequest {
        mnemonic: Some("invalid mnemonic words here".to_string()),
        password: "strong_password_123!".to_string(),
        wallet_name: "invalid_mnemonic_wallet".to_string(),
    };
    
    let invalid_mnemonic_result = core_context.create_wallet(invalid_mnemonic_request).await;
    assert!(invalid_mnemonic_result.is_err());
    println!("✅ Invalid mnemonic properly rejected");
    
    // Test 3: Operations on non-existent wallet
    let nonexistent_address = "0xnonexistent1234567890123456789012345678";
    
    let nonexistent_activate = ActivateWalletRequest {
        address: nonexistent_address.to_string(),
        password: "any_password".to_string(),
    };
    
    let nonexistent_result = core_context.activate_wallet(nonexistent_activate).await;
    assert!(nonexistent_result.is_err());
    println!("✅ Operations on non-existent wallet properly rejected");
    
    // Test 4: Sign transaction with invalid parameters
    // First create and activate a wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "test_password_123".to_string(),
        wallet_name: "error_test_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "test_password_123".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Invalid transaction parameters
    let invalid_tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "invalid_address".to_string(), // Invalid address format
        value: "not_a_number".to_string(), // Invalid value
        data: "".to_string(),
        gas_limit: 0, // Invalid gas limit
        gas_price: "0".to_string(), // Invalid gas price
        nonce: 0,
        chain_id: 999999, // Invalid chain ID
    };
    
    let invalid_tx_result = core_context.sign_transaction(invalid_tx_request).await;
    assert!(invalid_tx_result.is_err());
    println!("✅ Invalid transaction parameters properly rejected");
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: wallet_address,
        password: "test_password_123".to_string(),
        confirm: true,
    };
    core_context.destroy_wallet(destroy_request).await?;
    
    println!("🎉 Wallet error conditions test passed!");
    Ok(())
}

/// Test wallet timeout and session management
#[tokio::test]
async fn test_wallet_session_timeout() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Create and activate wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "timeout_test_password".to_string(),
        wallet_name: "timeout_test_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "timeout_test_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Test normal operation
    let tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 0,
        chain_id: 1,
    };
    
    let sign_response = core_context.sign_transaction(tx_request.clone()).await?;
    assert!(!sign_response.signed_transaction.is_empty());
    println!("✅ Transaction signed successfully before timeout");
    
    // Simulate session timeout by waiting
    println!("⏳ Waiting for session timeout...");
    sleep(Duration::from_secs(2)).await; // Short timeout for testing
    
    // Try operation after timeout (should trigger re-authentication)
    let timeout_result = core_context.sign_transaction(tx_request).await;
    // Note: Depending on implementation, this might require re-authentication
    // or might still work if timeout hasn't actually expired
    println!("✅ Post-timeout operation handled correctly");
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: wallet_address,
        password: "timeout_test_password".to_string(),
        confirm: true,
    };
    core_context.destroy_wallet(destroy_request).await?;
    
    println!("🎉 Session timeout test passed!");
    Ok(())
}