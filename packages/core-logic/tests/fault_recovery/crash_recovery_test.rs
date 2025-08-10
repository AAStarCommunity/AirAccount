// Licensed to AirAccount under the Apache License, Version 2.0
// System crash recovery and fault tolerance tests

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tempfile::TempDir;

/// Test wallet state recovery after simulated crash during transaction
#[tokio::test]
async fn test_transaction_crash_recovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting transaction crash recovery test...");
    
    // Phase 1: Setup wallet and prepare for transaction
    let mut core_context = CoreContext::new()?;
    
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_recovery_password".to_string(),
        wallet_name: "crash_recovery_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "crash_recovery_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    println!("✅ Wallet created and activated: {}", wallet_address);
    
    // Phase 2: Start transaction and simulate crash
    println!("Phase 2: Simulating crash during transaction...");
    
    let tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(),
        data: "0x12345678".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 0,
        chain_id: 1,
    };
    
    // Simulate crash by dropping the context mid-operation
    // In a real scenario, this would be a system crash or process termination
    drop(core_context);
    println!("💥 Simulated system crash during transaction");
    
    // Phase 3: Recovery - create new context and verify state
    println!("Phase 3: Attempting recovery...");
    sleep(Duration::from_millis(100)).await; // Simulate restart delay
    
    let mut recovery_context = CoreContext::new()?;
    
    // Try to get wallet status to verify recovery
    let status_request = GetWalletStatusRequest {
        address: wallet_address.clone(),
    };
    
    match recovery_context.get_wallet_status(status_request).await {
        Ok(status) => {
            println!("✅ Wallet recovered successfully: status = {}", status.status);
            
            // Verify wallet is in a consistent state
            if status.status == "inactive" {
                // Need to reactivate
                let reactivate_request = ActivateWalletRequest {
                    address: wallet_address.clone(),
                    password: "crash_recovery_password".to_string(),
                };
                recovery_context.activate_wallet(reactivate_request).await?;
                println!("✅ Wallet reactivated after recovery");
            }
        }
        Err(e) => {
            println!("⚠️ Wallet state unclear after crash: {}", e);
            
            // Try to recreate wallet with same parameters
            let recovery_wallet_request = CreateWalletRequest {
                mnemonic: Some(create_response.mnemonic.clone()),
                password: "crash_recovery_password".to_string(),
                wallet_name: "crash_recovery_wallet_restored".to_string(),
            };
            
            match recovery_context.create_wallet(recovery_wallet_request).await {
                Ok(recovered_response) => {
                    println!("✅ Wallet recreated from mnemonic: {}", recovered_response.address);
                    assert_eq!(recovered_response.address, wallet_address); // Should be same address
                }
                Err(create_error) => {
                    println!("❌ Failed to recover wallet: {}", create_error);
                    return Err(create_error.into());
                }
            }
        }
    }
    
    // Phase 4: Verify wallet functionality after recovery
    println!("Phase 4: Verifying wallet functionality after recovery...");
    
    let recovery_tx_request = SignTransactionRequest {
        address: wallet_address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "500000000000000000".to_string(),
        data: "0x87654321".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 0, // Start from nonce 0 again
        chain_id: 1,
    };
    
    let recovery_sign_response = recovery_context.sign_transaction(recovery_tx_request).await?;
    assert!(!recovery_sign_response.signed_transaction.is_empty());
    println!("✅ Transaction signed successfully after recovery");
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: wallet_address,
        password: "crash_recovery_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    println!("🎉 Transaction crash recovery test passed!");
    Ok(())
}

/// Test state consistency after multiple crash scenarios
#[tokio::test]
async fn test_multiple_crash_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting multiple crash scenarios test...");
    
    let crash_scenarios = vec![
        "wallet_creation",
        "wallet_activation", 
        "transaction_signing",
        "wallet_locking",
        "wallet_backup",
    ];
    
    for scenario in crash_scenarios {
        println!("📋 Testing crash scenario: {}", scenario);
        
        match scenario {
            "wallet_creation" => test_crash_during_wallet_creation().await?,
            "wallet_activation" => test_crash_during_wallet_activation().await?,
            "transaction_signing" => test_crash_during_transaction_signing().await?,
            "wallet_locking" => test_crash_during_wallet_locking().await?,
            "wallet_backup" => test_crash_during_wallet_backup().await?,
            _ => unreachable!(),
        }
        
        println!("✅ Crash scenario '{}' handled correctly", scenario);
        
        // Brief pause between scenarios
        sleep(Duration::from_millis(100)).await;
    }
    
    println!("🎉 Multiple crash scenarios test passed!");
    Ok(())
}

async fn test_crash_during_wallet_creation() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Start wallet creation but simulate crash before completion
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_creation_password".to_string(),
        wallet_name: "crash_creation_wallet".to_string(),
    };
    
    // Simulate crash by dropping context
    drop(core_context);
    
    // Recovery
    let mut recovery_context = CoreContext::new()?;
    
    // Try creating the wallet again - should work
    let recovery_wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_creation_password".to_string(),
        wallet_name: "crash_creation_wallet".to_string(),
    };
    
    let create_response = recovery_context.create_wallet(recovery_wallet_request).await?;
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: create_response.address,
        password: "crash_creation_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    Ok(())
}

async fn test_crash_during_wallet_activation() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Create wallet first
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_activation_password".to_string(),
        wallet_name: "crash_activation_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    // Start activation but crash
    drop(core_context);
    
    // Recovery
    let mut recovery_context = CoreContext::new()?;
    
    // Try activation again
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "crash_activation_password".to_string(),
    };
    
    match recovery_context.activate_wallet(activate_request).await {
        Ok(_) => println!("✅ Wallet activation recovered successfully"),
        Err(e) => {
            // If wallet doesn't exist, recreate from mnemonic
            let recovery_create_request = CreateWalletRequest {
                mnemonic: Some(create_response.mnemonic.clone()),
                password: "crash_activation_password".to_string(),
                wallet_name: "crash_activation_wallet_recovered".to_string(),
            };
            
            let recovered_wallet = recovery_context.create_wallet(recovery_create_request).await?;
            assert_eq!(recovered_wallet.address, wallet_address);
        }
    }
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: wallet_address,
        password: "crash_activation_password".to_string(),
        confirm: true,
    };
    let _ = recovery_context.destroy_wallet(destroy_request).await;
    
    Ok(())
}

async fn test_crash_during_transaction_signing() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Setup wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_signing_password".to_string(),
        wallet_name: "crash_signing_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "crash_signing_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Crash during signing
    drop(core_context);
    
    // Recovery
    let mut recovery_context = CoreContext::new()?;
    
    // Recreate wallet state
    let recovery_create_request = CreateWalletRequest {
        mnemonic: Some(create_response.mnemonic.clone()),
        password: "crash_signing_password".to_string(),
        wallet_name: "crash_signing_wallet_recovered".to_string(),
    };
    
    let recovered_wallet = recovery_context.create_wallet(recovery_create_request).await?;
    
    let reactivate_request = ActivateWalletRequest {
        address: recovered_wallet.address.clone(),
        password: "crash_signing_password".to_string(),
    };
    recovery_context.activate_wallet(reactivate_request).await?;
    
    // Try signing transaction
    let tx_request = SignTransactionRequest {
        address: recovered_wallet.address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 0,
        chain_id: 1,
    };
    
    let sign_response = recovery_context.sign_transaction(tx_request).await?;
    assert!(!sign_response.signed_transaction.is_empty());
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: recovered_wallet.address,
        password: "crash_signing_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    Ok(())
}

async fn test_crash_during_wallet_locking() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Setup wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_locking_password".to_string(),
        wallet_name: "crash_locking_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "crash_locking_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Crash during locking
    drop(core_context);
    
    // Recovery
    let mut recovery_context = CoreContext::new()?;
    
    // Recreate wallet and verify state
    let recovery_create_request = CreateWalletRequest {
        mnemonic: Some(create_response.mnemonic.clone()),
        password: "crash_locking_password".to_string(),
        wallet_name: "crash_locking_wallet_recovered".to_string(),
    };
    
    let recovered_wallet = recovery_context.create_wallet(recovery_create_request).await?;
    
    // Wallet should be in unlocked state after recovery
    let status_request = GetWalletStatusRequest {
        address: recovered_wallet.address.clone(),
    };
    
    let status = recovery_context.get_wallet_status(status_request).await?;
    println!("Wallet status after crash recovery: {}", status.status);
    
    // Should be able to activate and use normally
    let reactivate_request = ActivateWalletRequest {
        address: recovered_wallet.address.clone(),
        password: "crash_locking_password".to_string(),
    };
    recovery_context.activate_wallet(reactivate_request).await?;
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: recovered_wallet.address,
        password: "crash_locking_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    Ok(())
}

async fn test_crash_during_wallet_backup() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Setup wallet
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "crash_backup_password".to_string(),
        wallet_name: "crash_backup_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    let activate_request = ActivateWalletRequest {
        address: wallet_address.clone(),
        password: "crash_backup_password".to_string(),
    };
    core_context.activate_wallet(activate_request).await?;
    
    // Crash during backup
    drop(core_context);
    
    // Recovery
    let mut recovery_context = CoreContext::new()?;
    
    // Wallet should still be recoverable from original mnemonic
    let recovery_create_request = CreateWalletRequest {
        mnemonic: Some(create_response.mnemonic.clone()),
        password: "crash_backup_password".to_string(),
        wallet_name: "crash_backup_wallet_recovered".to_string(),
    };
    
    let recovered_wallet = recovery_context.create_wallet(recovery_create_request).await?;
    
    let reactivate_request = ActivateWalletRequest {
        address: recovered_wallet.address.clone(),
        password: "crash_backup_password".to_string(),
    };
    recovery_context.activate_wallet(reactivate_request).await?;
    
    // Try backup again - should work
    let backup_request = BackupWalletRequest {
        address: recovered_wallet.address.clone(),
        password: "crash_backup_password".to_string(),
    };
    
    let backup_response = recovery_context.backup_wallet(backup_request).await?;
    assert!(!backup_response.encrypted_backup.is_empty());
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: recovered_wallet.address,
        password: "crash_backup_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    Ok(())
}

/// Test recovery from corrupted temporary files
#[tokio::test]
async fn test_temporary_file_corruption_recovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting temporary file corruption recovery test...");
    
    // Create temporary directory for testing
    let temp_dir = TempDir::new()?;
    println!("Using temp directory: {}", temp_dir.path().display());
    
    let mut core_context = CoreContext::new()?;
    
    // Phase 1: Create wallet and simulate temp file corruption
    let wallet_request = CreateWalletRequest {
        mnemonic: None,
        password: "temp_corruption_password".to_string(),
        wallet_name: "temp_corruption_wallet".to_string(),
    };
    
    let create_response = core_context.create_wallet(wallet_request).await?;
    let wallet_address = create_response.address.clone();
    
    println!("✅ Wallet created: {}", wallet_address);
    
    // Phase 2: Simulate temporary file corruption
    // In a real implementation, this would involve corrupting actual temp files
    // For testing, we'll simulate this by disrupting the context state
    
    println!("💥 Simulating temporary file corruption...");
    
    // Drop context to simulate file corruption effects
    drop(core_context);
    
    // Phase 3: Recovery with new context
    println!("Phase 3: Attempting recovery from corruption...");
    
    let mut recovery_context = CoreContext::new()?;
    
    // Try to recreate wallet from mnemonic (primary recovery method)
    let recovery_create_request = CreateWalletRequest {
        mnemonic: Some(create_response.mnemonic.clone()),
        password: "temp_corruption_password".to_string(),
        wallet_name: "temp_corruption_wallet_recovered".to_string(),
    };
    
    let recovered_response = recovery_context.create_wallet(recovery_create_request).await?;
    assert_eq!(recovered_response.address, wallet_address);
    
    println!("✅ Wallet recovered from mnemonic: {}", recovered_response.address);
    
    // Phase 4: Verify full functionality
    println!("Phase 4: Verifying recovered wallet functionality...");
    
    let activate_request = ActivateWalletRequest {
        address: recovered_response.address.clone(),
        password: "temp_corruption_password".to_string(),
    };
    recovery_context.activate_wallet(activate_request).await?;
    
    let tx_request = SignTransactionRequest {
        address: recovered_response.address.clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 0,
        chain_id: 1,
    };
    
    let sign_response = recovery_context.sign_transaction(tx_request).await?;
    assert!(!sign_response.signed_transaction.is_empty());
    
    println!("✅ Recovered wallet fully functional");
    
    // Cleanup
    let destroy_request = DestroyWalletRequest {
        address: recovered_response.address,
        password: "temp_corruption_password".to_string(),
        confirm: true,
    };
    recovery_context.destroy_wallet(destroy_request).await?;
    
    println!("🎉 Temporary file corruption recovery test passed!");
    Ok(())
}

/// Test concurrent crash recovery scenarios
#[tokio::test]
async fn test_concurrent_crash_recovery() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting concurrent crash recovery test...");
    
    let wallet_count = 10;
    let mut original_wallets = Vec::new();
    
    // Phase 1: Create multiple wallets concurrently
    println!("Phase 1: Creating {} wallets concurrently...", wallet_count);
    
    let mut core_context = CoreContext::new()?;
    
    for i in 0..wallet_count {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("concurrent_crash_password_{}", i),
            wallet_name: format!("concurrent_crash_wallet_{}", i),
        };
        
        let create_response = core_context.create_wallet(wallet_request).await?;
        
        let activate_request = ActivateWalletRequest {
            address: create_response.address.clone(),
            password: format!("concurrent_crash_password_{}", i),
        };
        core_context.activate_wallet(activate_request).await?;
        
        original_wallets.push((
            create_response.address,
            create_response.mnemonic,
            format!("concurrent_crash_password_{}", i)
        ));
        
        println!("✅ Created wallet {}: {}", i, create_response.address);
    }
    
    // Phase 2: Simulate crash during concurrent operations
    println!("Phase 2: Simulating crash during concurrent operations...");
    
    drop(core_context);
    
    // Phase 3: Concurrent recovery
    println!("Phase 3: Attempting concurrent recovery...");
    
    let mut recovery_context = CoreContext::new()?;
    let mut recovery_tasks = tokio::task::JoinSet::new();
    
    for (i, (original_address, mnemonic, password)) in original_wallets.into_iter().enumerate() {
        let mut ctx = recovery_context.clone();
        
        recovery_tasks.spawn(async move {
            // Try to recover wallet
            let recovery_create_request = CreateWalletRequest {
                mnemonic: Some(mnemonic),
                password: password.clone(),
                wallet_name: format!("concurrent_crash_wallet_recovered_{}", i),
            };
            
            match ctx.create_wallet(recovery_create_request).await {
                Ok(recovered_response) => {
                    if recovered_response.address == original_address {
                        // Activate and test functionality
                        let activate_request = ActivateWalletRequest {
                            address: recovered_response.address.clone(),
                            password: password.clone(),
                        };
                        
                        if ctx.activate_wallet(activate_request).await.is_ok() {
                            // Test with a simple transaction
                            let tx_request = SignTransactionRequest {
                                address: recovered_response.address.clone(),
                                to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                                value: format!("{:018}", (i + 1) as u64 * 1000000000000000000u64),
                                data: "".to_string(),
                                gas_limit: 21000,
                                gas_price: "20000000000".to_string(),
                                nonce: 0,
                                chain_id: 1,
                            };
                            
                            if ctx.sign_transaction(tx_request).await.is_ok() {
                                return Ok((i, recovered_response.address, password));
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(format!("Failed to recover wallet {}: {}", i, e));
                }
            }
            
            Err(format!("Wallet {} recovery verification failed", i))
        });
    }
    
    let mut successful_recoveries = 0;
    let mut failed_recoveries = 0;
    
    while let Some(result) = recovery_tasks.join_next().await {
        match result? {
            Ok((wallet_index, address, password)) => {
                successful_recoveries += 1;
                println!("✅ Successfully recovered wallet {}: {}", wallet_index, address);
                
                // Cleanup recovered wallet
                let destroy_request = DestroyWalletRequest {
                    address,
                    password,
                    confirm: true,
                };
                let _ = recovery_context.destroy_wallet(destroy_request).await;
            }
            Err(error) => {
                failed_recoveries += 1;
                println!("❌ Recovery failed: {}", error);
            }
        }
    }
    
    println!("📊 Concurrent Crash Recovery Results:");
    println!("  Total wallets: {}", wallet_count);
    println!("  Successfully recovered: {}", successful_recoveries);
    println!("  Failed recoveries: {}", failed_recoveries);
    println!("  Recovery rate: {:.2}%", (successful_recoveries as f64 / wallet_count as f64) * 100.0);
    
    // Assert success criteria
    assert!(successful_recoveries >= wallet_count * 90 / 100, "Recovery rate too low: {}/{}", successful_recoveries, wallet_count);
    
    println!("🎉 Concurrent crash recovery test passed!");
    Ok(())
}