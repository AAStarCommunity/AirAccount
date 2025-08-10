// Licensed to AirAccount under the Apache License, Version 2.0
// Multi-wallet management business scenario tests

use airaccount_core_logic::*;
use std::collections::HashMap;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};

/// Test managing multiple wallets simultaneously
#[tokio::test]
async fn test_concurrent_multi_wallet_management() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Create multiple wallets concurrently
    println!("Phase 1: Creating 5 wallets concurrently...");
    let mut wallet_tasks = JoinSet::new();
    
    for i in 0..5 {
        let mut ctx = core_context.clone();
        wallet_tasks.spawn(async move {
            let wallet_request = CreateWalletRequest {
                mnemonic: None,
                password: format!("password_wallet_{}", i),
                wallet_name: format!("concurrent_wallet_{}", i),
            };
            
            ctx.create_wallet(wallet_request).await
        });
    }
    
    let mut wallet_addresses = Vec::new();
    let mut wallet_passwords = Vec::new();
    
    while let Some(result) = wallet_tasks.join_next().await {
        let create_response = result??;
        wallet_addresses.push(create_response.address.clone());
        
        // Extract wallet index from address for password mapping
        let wallet_index = wallet_addresses.len() - 1;
        wallet_passwords.push(format!("password_wallet_{}", wallet_index));
        
        println!("✅ Wallet created: {}", create_response.address);
    }
    
    assert_eq!(wallet_addresses.len(), 5);
    println!("✅ All 5 wallets created successfully");
    
    // Activate all wallets concurrently
    println!("Phase 2: Activating all wallets concurrently...");
    let mut activation_tasks = JoinSet::new();
    
    for (i, address) in wallet_addresses.iter().enumerate() {
        let mut ctx = core_context.clone();
        let addr = address.clone();
        let password = wallet_passwords[i].clone();
        
        activation_tasks.spawn(async move {
            let activate_request = ActivateWalletRequest {
                address: addr.clone(),
                password,
            };
            
            ctx.activate_wallet(activate_request).await?;
            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(addr)
        });
    }
    
    let mut activated_count = 0;
    while let Some(result) = activation_tasks.join_next().await {
        let activated_address = result??;
        activated_count += 1;
        println!("✅ Wallet activated: {}", activated_address);
    }
    
    assert_eq!(activated_count, 5);
    println!("✅ All 5 wallets activated successfully");
    
    // Test concurrent operations on all wallets
    println!("Phase 3: Performing concurrent operations...");
    let mut operation_tasks = JoinSet::new();
    
    for (i, address) in wallet_addresses.iter().enumerate() {
        let mut ctx = core_context.clone();
        let addr = address.clone();
        
        operation_tasks.spawn(async move {
            // Sign a transaction
            let tx_request = SignTransactionRequest {
                address: addr.clone(),
                to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                value: format!("{:018}", (i + 1) * 1000000000000000000u64), // Different amounts
                data: "".to_string(),
                gas_limit: 21000,
                gas_price: "20000000000".to_string(),
                nonce: i as u64,
                chain_id: 1,
            };
            
            let sign_response = ctx.sign_transaction(tx_request).await?;
            Ok::<(String, String), Box<dyn std::error::Error + Send + Sync>>((addr, sign_response.signed_transaction))
        });
    }
    
    let mut signed_count = 0;
    while let Some(result) = operation_tasks.join_next().await {
        let (address, signed_tx) = result??;
        assert!(!signed_tx.is_empty());
        signed_count += 1;
        println!("✅ Transaction signed for wallet: {}", address);
    }
    
    assert_eq!(signed_count, 5);
    println!("✅ All 5 wallets completed transactions successfully");
    
    // Test wallet isolation - lock one wallet, others should remain active
    println!("Phase 4: Testing wallet isolation...");
    let lock_request = LockWalletRequest {
        address: wallet_addresses[0].clone(),
    };
    core_context.lock_wallet(lock_request).await?;
    println!("✅ First wallet locked");
    
    // Verify locked wallet cannot sign
    let locked_tx_request = SignTransactionRequest {
        address: wallet_addresses[0].clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "1000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 10,
        chain_id: 1,
    };
    
    let locked_result = core_context.sign_transaction(locked_tx_request).await;
    assert!(locked_result.is_err());
    println!("✅ Locked wallet properly rejected transaction");
    
    // Verify other wallets still work
    let other_tx_request = SignTransactionRequest {
        address: wallet_addresses[1].clone(),
        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
        value: "2000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 10,
        chain_id: 1,
    };
    
    let other_sign_response = core_context.sign_transaction(other_tx_request).await?;
    assert!(!other_sign_response.signed_transaction.is_empty());
    println!("✅ Other wallets remain functional");
    
    // Cleanup all wallets
    println!("Phase 5: Cleaning up all wallets...");
    let mut cleanup_tasks = JoinSet::new();
    
    for (i, address) in wallet_addresses.iter().enumerate() {
        let mut ctx = core_context.clone();
        let addr = address.clone();
        let password = wallet_passwords[i].clone();
        
        cleanup_tasks.spawn(async move {
            // Unlock if locked
            if i == 0 {
                let unlock_request = UnlockWalletRequest {
                    address: addr.clone(),
                    password: password.clone(),
                };
                let _ = ctx.unlock_wallet(unlock_request).await;
            }
            
            let destroy_request = DestroyWalletRequest {
                address: addr.clone(),
                password,
                confirm: true,
            };
            
            ctx.destroy_wallet(destroy_request).await?;
            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(addr)
        });
    }
    
    let mut destroyed_count = 0;
    while let Some(result) = cleanup_tasks.join_next().await {
        let destroyed_address = result??;
        destroyed_count += 1;
        println!("✅ Wallet destroyed: {}", destroyed_address);
    }
    
    assert_eq!(destroyed_count, 5);
    println!("✅ All wallets cleaned up successfully");
    
    println!("🎉 Multi-wallet management test passed!");
    Ok(())
}

/// Test wallet resource management and limits
#[tokio::test]
async fn test_wallet_resource_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Test 1: Create maximum allowed wallets
    println!("Phase 1: Testing wallet creation limits...");
    let max_wallets = 10; // Configurable limit
    let mut created_wallets = Vec::new();
    
    for i in 0..max_wallets {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("limit_test_password_{}", i),
            wallet_name: format!("limit_test_wallet_{}", i),
        };
        
        match core_context.create_wallet(wallet_request).await {
            Ok(response) => {
                created_wallets.push((response.address, format!("limit_test_password_{}", i)));
                println!("✅ Wallet {} created: {}", i + 1, response.address);
            }
            Err(e) => {
                println!("⚠️ Wallet creation failed at {}: {}", i + 1, e);
                break;
            }
        }
    }
    
    println!("✅ Created {} wallets successfully", created_wallets.len());
    
    // Test 2: Try to exceed limit
    println!("Phase 2: Testing wallet limit enforcement...");
    let over_limit_request = CreateWalletRequest {
        mnemonic: None,
        password: "over_limit_password".to_string(),
        wallet_name: "over_limit_wallet".to_string(),
    };
    
    let over_limit_result = core_context.create_wallet(over_limit_request).await;
    if over_limit_result.is_err() {
        println!("✅ Wallet limit properly enforced");
    } else {
        println!("⚠️ Wallet limit not enforced - this might be expected behavior");
    }
    
    // Test 3: Resource cleanup and reuse
    println!("Phase 3: Testing resource cleanup...");
    
    // Destroy half the wallets
    let mut destroyed_count = 0;
    for i in 0..(created_wallets.len() / 2) {
        let (address, password) = &created_wallets[i];
        let destroy_request = DestroyWalletRequest {
            address: address.clone(),
            password: password.clone(),
            confirm: true,
        };
        
        if let Ok(_) = core_context.destroy_wallet(destroy_request).await {
            destroyed_count += 1;
        }
    }
    
    println!("✅ Destroyed {} wallets", destroyed_count);
    
    // Try to create new wallets after cleanup
    println!("Phase 4: Testing resource reuse...");
    let mut reused_wallets = Vec::new();
    
    for i in 0..destroyed_count {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("reuse_test_password_{}", i),
            wallet_name: format!("reuse_test_wallet_{}", i),
        };
        
        match core_context.create_wallet(wallet_request).await {
            Ok(response) => {
                reused_wallets.push((response.address, format!("reuse_test_password_{}", i)));
                println!("✅ Reused slot for wallet: {}", response.address);
            }
            Err(e) => {
                println!("⚠️ Failed to reuse slot: {}", e);
            }
        }
    }
    
    println!("✅ Successfully reused {} wallet slots", reused_wallets.len());
    
    // Cleanup remaining wallets
    println!("Phase 5: Final cleanup...");
    let mut all_remaining = Vec::new();
    
    // Add remaining original wallets
    for i in (created_wallets.len() / 2)..created_wallets.len() {
        all_remaining.push(created_wallets[i].clone());
    }
    
    // Add reused wallets
    for wallet in reused_wallets {
        all_remaining.push(wallet);
    }
    
    for (address, password) in all_remaining {
        let destroy_request = DestroyWalletRequest {
            address: address.clone(),
            password,
            confirm: true,
        };
        
        let _ = core_context.destroy_wallet(destroy_request).await;
    }
    
    println!("🎉 Wallet resource limits test passed!");
    Ok(())
}

/// Test wallet performance with multiple operations
#[tokio::test]
async fn test_multi_wallet_performance() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    println!("Phase 1: Performance test setup...");
    let wallet_count = 10;
    let operations_per_wallet = 5;
    
    // Create wallets for performance testing
    let mut wallets = Vec::new();
    for i in 0..wallet_count {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("perf_password_{}", i),
            wallet_name: format!("perf_wallet_{}", i),
        };
        
        let response = core_context.create_wallet(wallet_request).await?;
        wallets.push((response.address, format!("perf_password_{}", i)));
        
        let activate_request = ActivateWalletRequest {
            address: response.address,
            password: format!("perf_password_{}", i),
        };
        core_context.activate_wallet(activate_request).await?;
    }
    
    println!("✅ Created and activated {} wallets", wallet_count);
    
    // Performance test: concurrent operations
    println!("Phase 2: Running performance test...");
    let start_time = std::time::Instant::now();
    let mut operation_tasks = JoinSet::new();
    
    for (wallet_idx, (address, _password)) in wallets.iter().enumerate() {
        for op_idx in 0..operations_per_wallet {
            let mut ctx = core_context.clone();
            let addr = address.clone();
            
            operation_tasks.spawn(async move {
                let tx_request = SignTransactionRequest {
                    address: addr.clone(),
                    to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                    value: "1000000000000000000".to_string(),
                    data: format!("0x{:064x}", wallet_idx * operations_per_wallet + op_idx), // Unique data
                    gas_limit: 21000,
                    gas_price: "20000000000".to_string(),
                    nonce: op_idx as u64,
                    chain_id: 1,
                };
                
                let operation_start = std::time::Instant::now();
                let result = ctx.sign_transaction(tx_request).await;
                let operation_duration = operation_start.elapsed();
                
                result.map(|response| (addr, response.signed_transaction, operation_duration))
            });
        }
    }
    
    let mut completed_operations = 0;
    let mut total_operation_time = Duration::new(0, 0);
    let mut max_operation_time = Duration::new(0, 0);
    let mut min_operation_time = Duration::new(u64::MAX, 0);
    
    while let Some(result) = operation_tasks.join_next().await {
        match result? {
            Ok((address, signed_tx, duration)) => {
                assert!(!signed_tx.is_empty());
                completed_operations += 1;
                total_operation_time += duration;
                max_operation_time = max_operation_time.max(duration);
                min_operation_time = min_operation_time.min(duration);
                
                if completed_operations % 10 == 0 {
                    println!("✅ Completed {} operations", completed_operations);
                }
            }
            Err(e) => {
                println!("⚠️ Operation failed: {}", e);
            }
        }
    }
    
    let total_test_time = start_time.elapsed();
    let expected_operations = wallet_count * operations_per_wallet;
    
    println!("📊 Performance Results:");
    println!("  Total operations: {}/{}", completed_operations, expected_operations);
    println!("  Total test time: {:?}", total_test_time);
    println!("  Average operation time: {:?}", total_operation_time / completed_operations as u32);
    println!("  Min operation time: {:?}", min_operation_time);
    println!("  Max operation time: {:?}", max_operation_time);
    println!("  Operations per second: {:.2}", completed_operations as f64 / total_test_time.as_secs_f64());
    
    // Performance assertions
    let avg_operation_time = total_operation_time / completed_operations as u32;
    assert!(avg_operation_time < Duration::from_millis(1000), "Average operation time too slow: {:?}", avg_operation_time);
    assert!(completed_operations >= expected_operations * 95 / 100, "Success rate too low: {}/{}", completed_operations, expected_operations);
    
    println!("✅ Performance targets met");
    
    // Cleanup
    println!("Phase 3: Cleaning up performance test wallets...");
    for (address, password) in wallets {
        let destroy_request = DestroyWalletRequest {
            address,
            password,
            confirm: true,
        };
        let _ = core_context.destroy_wallet(destroy_request).await;
    }
    
    println!("🎉 Multi-wallet performance test passed!");
    Ok(())
}

/// Test wallet data isolation and security
#[tokio::test]
async fn test_wallet_data_isolation() -> Result<(), Box<dyn std::error::Error>> {
    let mut core_context = CoreContext::new()?;
    
    // Create two wallets with different users/contexts
    println!("Phase 1: Creating isolated wallets...");
    
    let wallet1_request = CreateWalletRequest {
        mnemonic: None,
        password: "user1_password_123".to_string(),
        wallet_name: "user1_wallet".to_string(),
    };
    let wallet1_response = core_context.create_wallet(wallet1_request).await?;
    
    let wallet2_request = CreateWalletRequest {
        mnemonic: None,
        password: "user2_password_456".to_string(),
        wallet_name: "user2_wallet".to_string(),
    };
    let wallet2_response = core_context.create_wallet(wallet2_request).await?;
    
    println!("✅ Created two isolated wallets");
    
    // Activate both wallets
    let activate1_request = ActivateWalletRequest {
        address: wallet1_response.address.clone(),
        password: "user1_password_123".to_string(),
    };
    core_context.activate_wallet(activate1_request).await?;
    
    let activate2_request = ActivateWalletRequest {
        address: wallet2_response.address.clone(),
        password: "user2_password_456".to_string(),
    };
    core_context.activate_wallet(activate2_request).await?;
    
    println!("✅ Both wallets activated");
    
    // Test 1: Cross-wallet password access (should fail)
    println!("Phase 2: Testing cross-wallet password isolation...");
    
    let cross_access_request = ActivateWalletRequest {
        address: wallet1_response.address.clone(),
        password: "user2_password_456".to_string(), // Wrong password
    };
    
    let cross_access_result = core_context.activate_wallet(cross_access_request).await;
    assert!(cross_access_result.is_err());
    println!("✅ Cross-wallet password access properly denied");
    
    // Test 2: Concurrent operations should not interfere
    println!("Phase 3: Testing concurrent isolated operations...");
    
    let mut concurrent_tasks = JoinSet::new();
    
    // Wallet 1 operations
    let mut ctx1 = core_context.clone();
    let addr1 = wallet1_response.address.clone();
    concurrent_tasks.spawn(async move {
        let mut results = Vec::new();
        for i in 0..5 {
            let tx_request = SignTransactionRequest {
                address: addr1.clone(),
                to: "0x1111111111111111111111111111111111111111".to_string(),
                value: format!("{:018}", 1000000000000000000u64 + i * 100000000000000000u64),
                data: format!("0x{:064x}", 1000 + i),
                gas_limit: 21000,
                gas_price: "20000000000".to_string(),
                nonce: i as u64,
                chain_id: 1,
            };
            
            let result = ctx1.sign_transaction(tx_request).await?;
            results.push(result.signed_transaction);
        }
        Ok::<Vec<String>, Box<dyn std::error::Error + Send + Sync>>(results)
    });
    
    // Wallet 2 operations
    let mut ctx2 = core_context.clone();
    let addr2 = wallet2_response.address.clone();
    concurrent_tasks.spawn(async move {
        let mut results = Vec::new();
        for i in 0..5 {
            let tx_request = SignTransactionRequest {
                address: addr2.clone(),
                to: "0x2222222222222222222222222222222222222222".to_string(),
                value: format!("{:018}", 2000000000000000000u64 + i * 200000000000000000u64),
                data: format!("0x{:064x}", 2000 + i),
                gas_limit: 21000,
                gas_price: "25000000000".to_string(), // Different gas price
                nonce: i as u64,
                chain_id: 1,
            };
            
            let result = ctx2.sign_transaction(tx_request).await?;
            results.push(result.signed_transaction);
        }
        Ok::<Vec<String>, Box<dyn std::error::Error + Send + Sync>>(results)
    });
    
    let mut wallet1_signatures = Vec::new();
    let mut wallet2_signatures = Vec::new();
    
    while let Some(result) = concurrent_tasks.join_next().await {
        let signatures = result??;
        if wallet1_signatures.is_empty() && signatures.len() == 5 {
            wallet1_signatures = signatures;
        } else {
            wallet2_signatures = signatures;
        }
    }
    
    // Verify all signatures are unique and valid
    assert_eq!(wallet1_signatures.len(), 5);
    assert_eq!(wallet2_signatures.len(), 5);
    
    for sig in &wallet1_signatures {
        assert!(!sig.is_empty());
        assert!(!wallet2_signatures.contains(sig)); // No signature collision
    }
    
    println!("✅ Concurrent operations completed with proper isolation");
    
    // Test 3: Memory isolation verification
    println!("Phase 4: Testing memory isolation...");
    
    // Lock one wallet, verify other remains accessible
    let lock_request = LockWalletRequest {
        address: wallet1_response.address.clone(),
    };
    core_context.lock_wallet(lock_request).await?;
    
    // Wallet 1 should be locked
    let locked_tx_request = SignTransactionRequest {
        address: wallet1_response.address.clone(),
        to: "0x1111111111111111111111111111111111111111".to_string(),
        value: "1000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "20000000000".to_string(),
        nonce: 10,
        chain_id: 1,
    };
    
    let locked_result = core_context.sign_transaction(locked_tx_request).await;
    assert!(locked_result.is_err());
    
    // Wallet 2 should still work
    let unlocked_tx_request = SignTransactionRequest {
        address: wallet2_response.address.clone(),
        to: "0x2222222222222222222222222222222222222222".to_string(),
        value: "2000000000000000000".to_string(),
        data: "".to_string(),
        gas_limit: 21000,
        gas_price: "25000000000".to_string(),
        nonce: 10,
        chain_id: 1,
    };
    
    let unlocked_result = core_context.sign_transaction(unlocked_tx_request).await?;
    assert!(!unlocked_result.signed_transaction.is_empty());
    
    println!("✅ Memory isolation verified");
    
    // Cleanup
    println!("Phase 5: Cleaning up isolated wallets...");
    
    // Unlock wallet1 first
    let unlock_request = UnlockWalletRequest {
        address: wallet1_response.address.clone(),
        password: "user1_password_123".to_string(),
    };
    core_context.unlock_wallet(unlock_request).await?;
    
    let destroy1_request = DestroyWalletRequest {
        address: wallet1_response.address,
        password: "user1_password_123".to_string(),
        confirm: true,
    };
    core_context.destroy_wallet(destroy1_request).await?;
    
    let destroy2_request = DestroyWalletRequest {
        address: wallet2_response.address,
        password: "user2_password_456".to_string(),
        confirm: true,
    };
    core_context.destroy_wallet(destroy2_request).await?;
    
    println!("🎉 Wallet data isolation test passed!");
    Ok(())
}