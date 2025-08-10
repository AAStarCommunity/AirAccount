// Licensed to AirAccount under the Apache License, Version 2.0
// High-concurrency stress tests

use airaccount_core_logic::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::sleep;
use tokio::sync::Semaphore;

/// Test 1000 concurrent wallet creation operations
#[tokio::test]
async fn test_concurrent_wallet_creation_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting concurrent wallet creation stress test...");
    
    let core_context = Arc::new(CoreContext::new()?);
    let concurrent_operations = 1000;
    let success_counter = Arc::new(AtomicU64::new(0));
    let error_counter = Arc::new(AtomicU64::new(0));
    
    // Semaphore to control concurrency level
    let semaphore = Arc::new(Semaphore::new(100)); // Max 100 concurrent operations
    
    let start_time = Instant::now();
    let mut tasks = JoinSet::new();
    
    for i in 0..concurrent_operations {
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let error_count = Arc::clone(&error_counter);
        let sem = Arc::clone(&semaphore);
        
        tasks.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            
            let wallet_request = CreateWalletRequest {
                mnemonic: None,
                password: format!("stress_test_password_{}", i),
                wallet_name: format!("stress_wallet_{}", i),
            };
            
            let operation_start = Instant::now();
            match ctx.create_wallet(wallet_request).await {
                Ok(response) => {
                    let operation_duration = operation_start.elapsed();
                    success_count.fetch_add(1, Ordering::Relaxed);
                    
                    if i % 100 == 0 {
                        println!("✅ Created wallet {} in {:?}: {}", i, operation_duration, response.address);
                    }
                    
                    // Cleanup immediately to avoid resource exhaustion
                    let destroy_request = DestroyWalletRequest {
                        address: response.address,
                        password: format!("stress_test_password_{}", i),
                        confirm: true,
                    };
                    let _ = ctx.destroy_wallet(destroy_request).await;
                }
                Err(e) => {
                    error_count.fetch_add(1, Ordering::Relaxed);
                    if i % 100 == 0 {
                        println!("❌ Failed to create wallet {}: {}", i, e);
                    }
                }
            }
        });
    }
    
    // Wait for all tasks to complete
    while let Some(_) = tasks.join_next().await {}
    
    let total_duration = start_time.elapsed();
    let successes = success_counter.load(Ordering::Relaxed);
    let errors = error_counter.load(Ordering::Relaxed);
    let success_rate = (successes as f64 / concurrent_operations as f64) * 100.0;
    
    println!("📊 Concurrent Wallet Creation Stress Test Results:");
    println!("  Total operations: {}", concurrent_operations);
    println!("  Successful: {}", successes);
    println!("  Failed: {}", errors);
    println!("  Success rate: {:.2}%", success_rate);
    println!("  Total duration: {:?}", total_duration);
    println!("  Operations per second: {:.2}", concurrent_operations as f64 / total_duration.as_secs_f64());
    println!("  Average time per operation: {:?}", total_duration / concurrent_operations);
    
    // Assert success criteria
    assert!(success_rate >= 95.0, "Success rate too low: {:.2}%", success_rate);
    assert!(total_duration.as_secs() <= 300, "Test took too long: {:?}", total_duration); // Max 5 minutes
    
    println!("🎉 Concurrent wallet creation stress test passed!");
    Ok(())
}

/// Test 500 concurrent transaction signing operations
#[tokio::test]
async fn test_concurrent_transaction_signing_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting concurrent transaction signing stress test...");
    
    let mut core_context = CoreContext::new()?;
    let concurrent_transactions = 500;
    
    // Pre-create and activate wallets for testing
    println!("Phase 1: Setting up test wallets...");
    let wallet_count = 50; // Use 50 wallets for 500 transactions (10 tx per wallet)
    let mut wallets = Vec::new();
    
    for i in 0..wallet_count {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("signing_stress_password_{}", i),
            wallet_name: format!("signing_stress_wallet_{}", i),
        };
        
        let create_response = core_context.create_wallet(wallet_request).await?;
        
        let activate_request = ActivateWalletRequest {
            address: create_response.address.clone(),
            password: format!("signing_stress_password_{}", i),
        };
        core_context.activate_wallet(activate_request).await?;
        
        wallets.push(create_response.address);
        
        if i % 10 == 0 {
            println!("✅ Set up {} wallets", i + 1);
        }
    }
    
    println!("✅ All {} wallets set up and activated", wallet_count);
    
    // Phase 2: Concurrent signing stress test
    println!("Phase 2: Running concurrent signing operations...");
    
    let core_context = Arc::new(core_context);
    let success_counter = Arc::new(AtomicU64::new(0));
    let error_counter = Arc::new(AtomicU64::new(0));
    let total_signing_time = Arc::new(std::sync::Mutex::new(Duration::new(0, 0)));
    
    let start_time = Instant::now();
    let mut tasks = JoinSet::new();
    
    for i in 0..concurrent_transactions {
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let error_count = Arc::clone(&error_counter);
        let signing_time = Arc::clone(&total_signing_time);
        let wallet_address = wallets[i % wallet_count].clone();
        
        tasks.spawn(async move {
            let tx_request = SignTransactionRequest {
                address: wallet_address.clone(),
                to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                value: format!("{:018}", 1000000000000000000u64 + i as u64 * 1000000000000000u64),
                data: format!("0x{:064x}", i),
                gas_limit: 21000,
                gas_price: "20000000000".to_string(),
                nonce: (i / wallet_count) as u64, // Distribute nonces across wallets
                chain_id: 1,
            };
            
            let operation_start = Instant::now();
            match ctx.sign_transaction(tx_request).await {
                Ok(response) => {
                    let operation_duration = operation_start.elapsed();
                    success_count.fetch_add(1, Ordering::Relaxed);
                    
                    // Update total signing time
                    {
                        let mut total_time = signing_time.lock().unwrap();
                        *total_time += operation_duration;
                    }
                    
                    if i % 50 == 0 {
                        println!("✅ Signed transaction {} in {:?}", i, operation_duration);
                    }
                    
                    assert!(!response.signed_transaction.is_empty());
                }
                Err(e) => {
                    error_count.fetch_add(1, Ordering::Relaxed);
                    if i % 50 == 0 {
                        println!("❌ Failed to sign transaction {}: {}", i, e);
                    }
                }
            }
        });
    }
    
    // Wait for all signing operations to complete
    while let Some(_) = tasks.join_next().await {}
    
    let total_test_duration = start_time.elapsed();
    let successes = success_counter.load(Ordering::Relaxed);
    let errors = error_counter.load(Ordering::Relaxed);
    let success_rate = (successes as f64 / concurrent_transactions as f64) * 100.0;
    let total_signing_duration = total_signing_time.lock().unwrap().clone();
    let avg_signing_time = total_signing_duration / successes as u32;
    
    println!("📊 Concurrent Transaction Signing Stress Test Results:");
    println!("  Total transactions: {}", concurrent_transactions);
    println!("  Successful: {}", successes);
    println!("  Failed: {}", errors);
    println!("  Success rate: {:.2}%", success_rate);
    println!("  Total test duration: {:?}", total_test_duration);
    println!("  Total signing time: {:?}", total_signing_duration);
    println!("  Average signing time: {:?}", avg_signing_time);
    println!("  Transactions per second: {:.2}", successes as f64 / total_test_duration.as_secs_f64());
    
    // Phase 3: Cleanup
    println!("Phase 3: Cleaning up test wallets...");
    for (i, address) in wallets.iter().enumerate() {
        let destroy_request = DestroyWalletRequest {
            address: address.clone(),
            password: format!("signing_stress_password_{}", i),
            confirm: true,
        };
        let _ = core_context.destroy_wallet(destroy_request).await;
    }
    
    // Assert success criteria
    assert!(success_rate >= 99.0, "Success rate too low: {:.2}%", success_rate);
    assert!(avg_signing_time < Duration::from_millis(50), "Average signing time too slow: {:?}", avg_signing_time);
    
    println!("🎉 Concurrent transaction signing stress test passed!");
    Ok(())
}

/// Test resource contention under high load
#[tokio::test]
async fn test_resource_contention_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting resource contention stress test...");
    
    let core_context = Arc::new(CoreContext::new()?);
    let operations_count = 1000;
    
    // Different types of operations to stress different resources
    let success_counters = Arc::new([
        AtomicU64::new(0), // Wallet creation
        AtomicU64::new(0), // Wallet activation
        AtomicU64::new(0), // Transaction signing
        AtomicU64::new(0), // Wallet status check
        AtomicU64::new(0), // Wallet lock/unlock
    ]);
    
    let start_time = Instant::now();
    let mut tasks = JoinSet::new();
    
    // Create a pool of wallets for testing
    println!("Phase 1: Creating wallet pool...");
    let mut wallet_pool = Vec::new();
    for i in 0..20 {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("contention_password_{}", i),
            wallet_name: format!("contention_wallet_{}", i),
        };
        
        let response = core_context.create_wallet(wallet_request).await?;
        
        let activate_request = ActivateWalletRequest {
            address: response.address.clone(),
            password: format!("contention_password_{}", i),
        };
        core_context.activate_wallet(activate_request).await?;
        
        wallet_pool.push((response.address, format!("contention_password_{}", i)));
    }
    
    println!("✅ Created pool of {} wallets", wallet_pool.len());
    
    // Phase 2: Mixed concurrent operations
    println!("Phase 2: Running mixed concurrent operations...");
    
    for i in 0..operations_count {
        let ctx = Arc::clone(&core_context);
        let counters = Arc::clone(&success_counters);
        let pool = wallet_pool.clone();
        
        tasks.spawn(async move {
            let operation_type = i % 5; // 5 different operation types
            
            match operation_type {
                0 => {
                    // Wallet creation + immediate destruction
                    let wallet_request = CreateWalletRequest {
                        mnemonic: None,
                        password: format!("temp_password_{}", i),
                        wallet_name: format!("temp_wallet_{}", i),
                    };
                    
                    if let Ok(response) = ctx.create_wallet(wallet_request).await {
                        let destroy_request = DestroyWalletRequest {
                            address: response.address,
                            password: format!("temp_password_{}", i),
                            confirm: true,
                        };
                        let _ = ctx.destroy_wallet(destroy_request).await;
                        counters[0].fetch_add(1, Ordering::Relaxed);
                    }
                }
                1 => {
                    // Wallet re-activation
                    let (address, password) = &pool[i % pool.len()];
                    let activate_request = ActivateWalletRequest {
                        address: address.clone(),
                        password: password.clone(),
                    };
                    if ctx.activate_wallet(activate_request).await.is_ok() {
                        counters[1].fetch_add(1, Ordering::Relaxed);
                    }
                }
                2 => {
                    // Transaction signing
                    let (address, _) = &pool[i % pool.len()];
                    let tx_request = SignTransactionRequest {
                        address: address.clone(),
                        to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                        value: format!("{:018}", i as u64 * 1000000000000000u64),
                        data: format!("0x{:08x}", i),
                        gas_limit: 21000,
                        gas_price: "20000000000".to_string(),
                        nonce: (i / pool.len()) as u64,
                        chain_id: 1,
                    };
                    if ctx.sign_transaction(tx_request).await.is_ok() {
                        counters[2].fetch_add(1, Ordering::Relaxed);
                    }
                }
                3 => {
                    // Status check
                    let (address, _) = &pool[i % pool.len()];
                    let status_request = GetWalletStatusRequest {
                        address: address.clone(),
                    };
                    if ctx.get_wallet_status(status_request).await.is_ok() {
                        counters[3].fetch_add(1, Ordering::Relaxed);
                    }
                }
                4 => {
                    // Lock/Unlock cycle
                    let (address, password) = &pool[i % pool.len()];
                    let lock_request = LockWalletRequest {
                        address: address.clone(),
                    };
                    if ctx.lock_wallet(lock_request).await.is_ok() {
                        sleep(Duration::from_millis(1)).await; // Brief pause
                        let unlock_request = UnlockWalletRequest {
                            address: address.clone(),
                            password: password.clone(),
                        };
                        if ctx.unlock_wallet(unlock_request).await.is_ok() {
                            counters[4].fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                _ => unreachable!(),
            }
        });
    }
    
    // Wait for all operations to complete
    while let Some(_) = tasks.join_next().await {}
    
    let total_duration = start_time.elapsed();
    let operation_names = ["Creation", "Activation", "Signing", "Status", "Lock/Unlock"];
    let mut total_successes = 0;
    
    println!("📊 Resource Contention Stress Test Results:");
    println!("  Total duration: {:?}", total_duration);
    
    for (i, name) in operation_names.iter().enumerate() {
        let successes = success_counters[i].load(Ordering::Relaxed);
        total_successes += successes;
        let expected = operations_count / 5; // Each operation type gets 1/5 of total
        let success_rate = (successes as f64 / expected as f64) * 100.0;
        
        println!("  {}: {}/{} ({:.1}%)", name, successes, expected, success_rate);
    }
    
    println!("  Total successful operations: {}/{}", total_successes, operations_count);
    println!("  Overall success rate: {:.2}%", (total_successes as f64 / operations_count as f64) * 100.0);
    println!("  Operations per second: {:.2}", total_successes as f64 / total_duration.as_secs_f64());
    
    // Phase 3: Cleanup wallet pool
    println!("Phase 3: Cleaning up wallet pool...");
    for (address, password) in wallet_pool {
        let destroy_request = DestroyWalletRequest {
            address,
            password,
            confirm: true,
        };
        let _ = core_context.destroy_wallet(destroy_request).await;
    }
    
    // Assert success criteria
    let overall_success_rate = (total_successes as f64 / operations_count as f64) * 100.0;
    assert!(overall_success_rate >= 90.0, "Overall success rate too low: {:.2}%", overall_success_rate);
    
    println!("🎉 Resource contention stress test passed!");
    Ok(())
}

/// Test system behavior under memory pressure
#[tokio::test]
async fn test_memory_pressure_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting memory pressure stress test...");
    
    let core_context = Arc::new(CoreContext::new()?);
    let wallet_count = 1000; // Create many wallets to use memory
    
    // Phase 1: Create many wallets to consume memory
    println!("Phase 1: Creating {} wallets to generate memory pressure...", wallet_count);
    
    let mut wallets = Vec::new();
    let start_time = Instant::now();
    let mut creation_times = Vec::new();
    
    for i in 0..wallet_count {
        let creation_start = Instant::now();
        
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("memory_pressure_password_{}", i),
            wallet_name: format!("memory_pressure_wallet_{}", i),
        };
        
        match core_context.create_wallet(wallet_request).await {
            Ok(response) => {
                let creation_duration = creation_start.elapsed();
                creation_times.push(creation_duration);
                wallets.push((response.address, format!("memory_pressure_password_{}", i)));
                
                if i % 100 == 0 {
                    println!("✅ Created {} wallets, latest took {:?}", i + 1, creation_duration);
                }
            }
            Err(e) => {
                println!("⚠️ Failed to create wallet {}: {}", i, e);
                break;
            }
        }
        
        // Check for memory pressure indicators
        if creation_times.len() >= 10 {
            let recent_avg = creation_times[creation_times.len()-10..].iter().sum::<Duration>() / 10;
            let early_avg = creation_times[0..10.min(creation_times.len())].iter().sum::<Duration>() / 10;
            
            // If creation time has increased significantly, we might be under memory pressure
            if recent_avg > early_avg * 3 {
                println!("⚠️ Detected significant performance degradation at {} wallets", i + 1);
            }
        }
    }
    
    let creation_phase_duration = start_time.elapsed();
    let successful_creations = wallets.len();
    
    println!("✅ Created {} wallets in {:?}", successful_creations, creation_phase_duration);
    
    if !creation_times.is_empty() {
        let avg_creation_time = creation_times.iter().sum::<Duration>() / creation_times.len() as u32;
        let max_creation_time = creation_times.iter().max().unwrap();
        let min_creation_time = creation_times.iter().min().unwrap();
        
        println!("📊 Creation Performance:");
        println!("  Average: {:?}", avg_creation_time);
        println!("  Min: {:?}", min_creation_time);
        println!("  Max: {:?}", max_creation_time);
    }
    
    // Phase 2: Perform operations under memory pressure
    println!("Phase 2: Performing operations under memory pressure...");
    
    let operations_count = 200;
    let success_counter = Arc::new(AtomicU64::new(0));
    let mut tasks = JoinSet::new();
    
    let operation_start_time = Instant::now();
    
    for i in 0..operations_count {
        if wallets.is_empty() {
            break;
        }
        
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let (address, _password) = wallets[i % wallets.len()].clone();
        
        tasks.spawn(async move {
            // Activate wallet
            let activate_request = ActivateWalletRequest {
                address: address.clone(),
                password: format!("memory_pressure_password_{}", i % 1000),
            };
            
            if ctx.activate_wallet(activate_request).await.is_ok() {
                // Sign transaction
                let tx_request = SignTransactionRequest {
                    address: address.clone(),
                    to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                    value: format!("{:018}", (i + 1) as u64 * 1000000000000000000u64),
                    data: format!("0x{:064x}", i),
                    gas_limit: 21000,
                    gas_price: "20000000000".to_string(),
                    nonce: i as u64 / 100, // Distribute nonces
                    chain_id: 1,
                };
                
                if ctx.sign_transaction(tx_request).await.is_ok() {
                    success_count.fetch_add(1, Ordering::Relaxed);
                    
                    if i % 50 == 0 {
                        println!("✅ Completed operation {} under memory pressure", i);
                    }
                }
            }
        });
    }
    
    // Wait for operations to complete
    while let Some(_) = tasks.join_next().await {}
    
    let operations_duration = operation_start_time.elapsed();
    let successful_operations = success_counter.load(Ordering::Relaxed);
    
    println!("✅ Completed {} operations in {:?} under memory pressure", successful_operations, operations_duration);
    
    // Phase 3: Cleanup and memory release test
    println!("Phase 3: Cleaning up wallets and testing memory release...");
    
    let cleanup_start_time = Instant::now();
    let mut cleanup_times = Vec::new();
    
    for (i, (address, password)) in wallets.into_iter().enumerate() {
        let cleanup_start = Instant::now();
        
        let destroy_request = DestroyWalletRequest {
            address,
            password,
            confirm: true,
        };
        
        if core_context.destroy_wallet(destroy_request).await.is_ok() {
            let cleanup_duration = cleanup_start.elapsed();
            cleanup_times.push(cleanup_duration);
            
            if i % 200 == 0 {
                println!("✅ Cleaned up {} wallets", i + 1);
            }
        }
    }
    
    let total_cleanup_duration = cleanup_start_time.elapsed();
    
    if !cleanup_times.is_empty() {
        let avg_cleanup_time = cleanup_times.iter().sum::<Duration>() / cleanup_times.len() as u32;
        println!("✅ Cleanup completed in {:?}, average per wallet: {:?}", total_cleanup_duration, avg_cleanup_time);
    }
    
    // Final performance assessment
    println!("📊 Memory Pressure Stress Test Results:");
    println!("  Wallets created: {}/{}", successful_creations, wallet_count);
    println!("  Operations completed: {}/{}", successful_operations, operations_count);
    println!("  Creation phase: {:?}", creation_phase_duration);
    println!("  Operations phase: {:?}", operations_duration);
    println!("  Cleanup phase: {:?}", total_cleanup_duration);
    
    // Assert success criteria
    assert!(successful_creations >= wallet_count * 90 / 100, "Too few wallets created: {}/{}", successful_creations, wallet_count);
    
    let operation_success_rate = if operations_count > 0 {
        (successful_operations as f64 / operations_count.min(successful_creations) as f64) * 100.0
    } else {
        100.0
    };
    
    assert!(operation_success_rate >= 85.0, "Operation success rate too low under memory pressure: {:.2}%", operation_success_rate);
    
    println!("🎉 Memory pressure stress test passed!");
    Ok(())
}

/// Test deadlock detection and prevention
#[tokio::test]
async fn test_deadlock_prevention_stress() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting deadlock prevention stress test...");
    
    let core_context = Arc::new(CoreContext::new()?);
    
    // Create a small set of wallets that will be heavily contended
    println!("Phase 1: Setting up contended wallets...");
    let contention_wallets = 5;
    let mut wallets = Vec::new();
    
    for i in 0..contention_wallets {
        let wallet_request = CreateWalletRequest {
            mnemonic: None,
            password: format!("deadlock_test_password_{}", i),
            wallet_name: format!("deadlock_test_wallet_{}", i),
        };
        
        let response = core_context.create_wallet(wallet_request).await?;
        
        let activate_request = ActivateWalletRequest {
            address: response.address.clone(),
            password: format!("deadlock_test_password_{}", i),
        };
        core_context.activate_wallet(activate_request).await?;
        
        wallets.push((response.address, format!("deadlock_test_password_{}", i)));
    }
    
    println!("✅ Set up {} contended wallets", contention_wallets);
    
    // Phase 2: Heavy contention operations
    println!("Phase 2: Running heavy contention operations...");
    
    let operations_per_pattern = 100;
    let success_counter = Arc::new(AtomicU64::new(0));
    let timeout_counter = Arc::new(AtomicU64::new(0));
    
    let mut tasks = JoinSet::new();
    
    // Pattern 1: Multiple threads rapidly locking/unlocking the same wallet
    for i in 0..operations_per_pattern {
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let timeout_count = Arc::clone(&timeout_counter);
        let (address, password) = wallets[i % wallets.len()].clone();
        
        tasks.spawn(async move {
            let timeout_duration = Duration::from_secs(10);
            let operation_start = Instant::now();
            
            // Attempt lock-unlock cycle with timeout
            let result = tokio::time::timeout(timeout_duration, async {
                let lock_request = LockWalletRequest { address: address.clone() };
                ctx.lock_wallet(lock_request).await?;
                
                // Small delay to increase contention
                sleep(Duration::from_millis(1)).await;
                
                let unlock_request = UnlockWalletRequest { address, password };
                ctx.unlock_wallet(unlock_request).await?;
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }).await;
            
            match result {
                Ok(Ok(())) => {
                    success_count.fetch_add(1, Ordering::Relaxed);
                    if i % 20 == 0 {
                        println!("✅ Lock/unlock cycle {} completed in {:?}", i, operation_start.elapsed());
                    }
                }
                Ok(Err(e)) => {
                    if i % 20 == 0 {
                        println!("⚠️ Lock/unlock cycle {} failed: {}", i, e);
                    }
                }
                Err(_) => {
                    timeout_count.fetch_add(1, Ordering::Relaxed);
                    if i % 20 == 0 {
                        println!("⏱️ Lock/unlock cycle {} timed out", i);
                    }
                }
            }
        });
    }
    
    // Pattern 2: Multiple threads accessing different wallets in different orders
    for i in 0..operations_per_pattern {
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let timeout_count = Arc::clone(&timeout_counter);
        let wallets_clone = wallets.clone();
        
        tasks.spawn(async move {
            let timeout_duration = Duration::from_secs(10);
            
            // Access wallets in different orders to create potential deadlocks
            let wallet_order: Vec<usize> = if i % 2 == 0 {
                (0..wallets_clone.len()).collect()
            } else {
                (0..wallets_clone.len()).rev().collect()
            };
            
            let result = tokio::time::timeout(timeout_duration, async {
                for &wallet_idx in &wallet_order {
                    let (address, _) = &wallets_clone[wallet_idx];
                    
                    let status_request = GetWalletStatusRequest {
                        address: address.clone(),
                    };
                    
                    ctx.get_wallet_status(status_request).await?;
                    
                    // Brief pause to increase contention window
                    sleep(Duration::from_micros(100)).await;
                }
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }).await;
            
            match result {
                Ok(Ok(())) => {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok(Err(_)) => {
                    // Operation failed but didn't timeout - that's ok
                }
                Err(_) => {
                    timeout_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }
    
    // Pattern 3: Mixed operations on same wallets
    for i in 0..operations_per_pattern {
        let ctx = Arc::clone(&core_context);
        let success_count = Arc::clone(&success_counter);
        let (address, password) = wallets[i % wallets.len()].clone();
        
        tasks.spawn(async move {
            let timeout_duration = Duration::from_secs(15);
            
            let result = tokio::time::timeout(timeout_duration, async {
                // Mix of operations
                match i % 3 {
                    0 => {
                        // Transaction signing
                        let tx_request = SignTransactionRequest {
                            address: address.clone(),
                            to: "0x742d35Cc6635C0532925a3b8D2020d4820b41e8".to_string(),
                            value: format!("{:018}", (i + 1) as u64 * 1000000000000000000u64),
                            data: "".to_string(),
                            gas_limit: 21000,
                            gas_price: "20000000000".to_string(),
                            nonce: (i / wallets.len()) as u64,
                            chain_id: 1,
                        };
                        ctx.sign_transaction(tx_request).await?;
                    }
                    1 => {
                        // Wallet backup
                        let backup_request = BackupWalletRequest {
                            address: address.clone(),
                            password: password.clone(),
                        };
                        ctx.backup_wallet(backup_request).await?;
                    }
                    2 => {
                        // Status check + re-activation
                        let status_request = GetWalletStatusRequest {
                            address: address.clone(),
                        };
                        ctx.get_wallet_status(status_request).await?;
                        
                        let activate_request = ActivateWalletRequest {
                            address,
                            password,
                        };
                        ctx.activate_wallet(activate_request).await?;
                    }
                    _ => unreachable!(),
                }
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }).await;
            
            if result.is_ok() {
                success_count.fetch_add(1, Ordering::Relaxed);
            }
        });
    }
    
    // Wait for all operations with a global timeout
    println!("⏳ Waiting for all operations to complete...");
    let global_timeout = Duration::from_secs(60);
    let all_tasks_start = Instant::now();
    
    let mut completed_tasks = 0;
    while let Some(_) = tokio::time::timeout(global_timeout, tasks.join_next()).await {
        completed_tasks += 1;
        if completed_tasks % 50 == 0 {
            println!("✅ Completed {} tasks", completed_tasks);
        }
        
        if all_tasks_start.elapsed() > global_timeout {
            println!("⚠️ Global timeout reached, stopping task execution");
            break;
        }
    }
    
    let total_operations = operations_per_pattern * 3;
    let successes = success_counter.load(Ordering::Relaxed);
    let timeouts = timeout_counter.load(Ordering::Relaxed);
    
    println!("📊 Deadlock Prevention Stress Test Results:");
    println!("  Total operations: {}", total_operations);
    println!("  Completed tasks: {}", completed_tasks);
    println!("  Successful operations: {}", successes);
    println!("  Timed out operations: {}", timeouts);
    println!("  Success rate: {:.2}%", (successes as f64 / total_operations as f64) * 100.0);
    println!("  Timeout rate: {:.2}%", (timeouts as f64 / total_operations as f64) * 100.0);
    
    // Phase 3: Cleanup
    println!("Phase 3: Cleaning up test wallets...");
    for (address, password) in wallets {
        // Unlock first in case it's locked
        let unlock_request = UnlockWalletRequest {
            address: address.clone(),
            password: password.clone(),
        };
        let _ = core_context.unlock_wallet(unlock_request).await;
        
        let destroy_request = DestroyWalletRequest {
            address,
            password,
            confirm: true,
        };
        let _ = core_context.destroy_wallet(destroy_request).await;
    }
    
    // Assert success criteria
    assert!(timeouts < total_operations / 10, "Too many timeouts (potential deadlocks): {}", timeouts);
    assert!(successes >= total_operations * 70 / 100, "Success rate too low: {}/{}", successes, total_operations);
    
    println!("🎉 Deadlock prevention stress test passed!");
    Ok(())
}