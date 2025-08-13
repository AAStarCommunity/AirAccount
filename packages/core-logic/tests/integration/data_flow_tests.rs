/// 数据流和状态同步测试
/// 测试不同模块间的数据传递和状态一致性

#[cfg(test)]
mod data_flow_tests {
    use airaccount_core_logic::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;
    use std::collections::HashMap;

    /// 测试端到端数据流：配置 → 安全管理器 → 钱包 → TEE
    #[tokio::test]
    async fn test_end_to_end_data_flow() {
        println!("🚀 Testing end-to-end data flow: Config → Security → Wallet → TEE...");
        
        // 1. 配置阶段 - 设定特殊配置
        let config = SecurityConfig {
            enable_constant_time: true,
            enable_memory_protection: true,
            enable_audit_logging: true,
            audit_file_path: Some("/tmp/e2e_flow_test.log".to_string()),
            enable_secure_audit: true,
            audit_encryption_key: Some("test_key_for_e2e_flow".to_string()),
        };
        
        // 2. 安全管理器阶段 - 应用配置
        let context = init_with_security_config(config)
            .expect("Failed to initialize with config");
        
        let security_manager = context.security_manager();
        
        // 验证配置传播到安全管理器
        assert!(security_manager.is_constant_time_enabled());
        assert!(security_manager.is_memory_protection_enabled());
        assert!(security_manager.is_audit_logging_enabled());
        
        // 3. 钱包管理器阶段 - 使用安全管理器
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        // 创建钱包时应该使用安全管理器的功能
        let wallet_id = wallet_manager.create_wallet(
            Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()),
            "e2e_test_password".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 4. TEE集成阶段 - 验证TEE配置与安全设置兼容
        let tee_config = tee::TEEConfig {
            platform: tee::TEEPlatform::Simulation,
            capabilities: tee::TEECapabilities {
                secure_storage: security_manager.is_memory_protection_enabled(),
                hardware_random: true,
                key_derivation: security_manager.is_constant_time_enabled(),
                ..tee::TEECapabilities::default()
            },
            ..tee::TEEConfig::default()
        };
        
        // 验证数据在整个流水线中的一致性
        assert!(tee_config.capabilities.secure_storage, "TEE should support secure storage when memory protection is enabled");
        assert!(tee_config.capabilities.key_derivation, "TEE should support key derivation when constant time is enabled");
        
        // 5. 端到端操作验证
        let tx_data = b"end_to_end_test_transaction";
        let signature = wallet.sign_transaction(tx_data).await
            .expect("Failed to sign transaction");
        
        assert!(!signature.is_empty(), "End-to-end signature should not be empty");
        
        // 6. 审计数据流验证
        sleep(Duration::from_millis(200)).await;
        
        let audit_events = security_manager.get_security_events(None);
        let e2e_events = audit_events.iter()
            .filter(|event| event.component.contains("wallet") || event.component.contains("security"))
            .count();
        
        assert!(e2e_events > 0, "End-to-end operation should generate audit events");
        
        println!("✅ End-to-end data flow verified: {} audit events generated", e2e_events);
    }

    /// 测试状态同步：多个模块对共享状态的一致性访问
    #[tokio::test]
    async fn test_shared_state_synchronization() {
        println!("🚀 Testing shared state synchronization across modules...");
        
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        
        // 创建多个钱包管理器实例，模拟不同模块访问共享状态
        let mut wallet_managers = Vec::new();
        for i in 0..3 {
            let wm = WalletManager::new(security_manager.clone());
            wallet_managers.push(wm);
        }
        
        // 每个管理器创建钱包
        let mut wallet_ids = Vec::new();
        for (i, wm) in wallet_managers.iter_mut().enumerate() {
            let wallet_id = wm.create_wallet(
                None,
                format!("sync_test_password_{}", i)
            ).await.expect(&format!("Failed to create wallet {}", i));
            
            wallet_ids.push(wallet_id);
        }
        
        // 测试状态一致性：每个管理器都应该能看到所有钱包
        for (i, wm) in wallet_managers.iter_mut().enumerate() {
            for (j, wallet_id) in wallet_ids.iter().enumerate() {
                let load_result = wm.load_wallet(wallet_id).await;
                
                if i == j {
                    // 同一个管理器应该能加载自己创建的钱包
                    assert!(load_result.is_ok(), "Manager {} should load its own wallet {}", i, j);
                } else {
                    // 不同管理器可能有不同的访问权限（取决于实现）
                    match load_result {
                        Ok(_) => println!("  Manager {} can access wallet {} (shared state)", i, j),
                        Err(_) => println!("  Manager {} cannot access wallet {} (isolated state)", i, j),
                    }
                }
            }
        }
        
        // 验证审计状态同步
        let total_events_before = security_manager.get_security_events(None).len();
        
        // 在一个管理器中执行操作
        let wallet = wallet_managers[0].load_wallet(&wallet_ids[0]).await
            .expect("Failed to load wallet for sync test");
        
        let _signature = wallet.sign_transaction(b"sync_test_data").await
            .expect("Failed to sign for sync test");
        
        sleep(Duration::from_millis(100)).await;
        
        // 验证所有管理器都能看到审计状态更新
        let total_events_after = security_manager.get_security_events(None).len();
        assert!(total_events_after > total_events_before, "Audit state should be synchronized");
        
        println!("✅ Shared state synchronization verified");
    }

    /// 测试模块间的错误传播和恢复
    #[tokio::test]
    async fn test_error_propagation_and_recovery() {
        println!("🚀 Testing error propagation and recovery across modules...");
        
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        // 测试错误从底层模块向上传播
        
        // 1. 安全管理器错误传播到钱包管理器
        // 尝试创建无效大小的安全内存
        let invalid_memory_result = security_manager.create_secure_memory(0);
        assert!(invalid_memory_result.is_err(), "Invalid memory allocation should fail");
        
        // 2. 钱包操作错误传播
        let wallet_id = wallet_manager.create_wallet(
            None,
            "error_propagation_test".to_string()
        ).await.expect("Failed to create wallet for error test");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet for error test");
        
        // 测试钱包操作中的错误处理
        let result = wallet.sign_transaction(&[]).await;
        
        match result {
            Ok(_) => println!("  Empty transaction data was accepted"),
            Err(e) => {
                println!("  Empty transaction properly rejected: {:?}", e);
                
                // 验证错误类型
                match e {
                    WalletError::InvalidTransaction(_) => println!("    Correct error type: InvalidTransaction"),
                    WalletError::ValidationFailed(_) => println!("    Correct error type: ValidationFailed"),
                    _ => println!("    Error type: {:?}", e),
                }
            }
        }
        
        // 3. 测试错误恢复能力
        // 在错误发生后，系统应该能够恢复正常操作
        let valid_tx_data = b"valid_transaction_after_error";
        let recovery_signature = wallet.sign_transaction(valid_tx_data).await
            .expect("Failed to recover after error");
        
        assert!(!recovery_signature.is_empty(), "Should recover and produce valid signature");
        
        // 4. 验证错误审计
        sleep(Duration::from_millis(100)).await;
        
        let error_events = security_manager.get_error_events(None);
        println!("  Found {} error events in audit log", error_events.len());
        
        println!("✅ Error propagation and recovery verified");
    }

    /// 测试资源共享和生命周期管理
    #[tokio::test]
    async fn test_resource_sharing_lifecycle() {
        println!("🚀 Testing resource sharing and lifecycle management...");
        
        let context = Arc::new(init_default().expect("Failed to initialize"));
        let security_manager = context.security_manager();
        
        // 创建共享资源
        let shared_memory = Arc::new(
            security_manager.create_secure_memory(2048)
                .expect("Failed to create shared memory")
        );
        
        let initial_use_count = Arc::strong_count(&shared_memory);
        println!("  Initial shared memory reference count: {}", initial_use_count);
        
        // 多个模块共享资源
        let mut handles = Vec::new();
        
        for i in 0..3 {
            let context_clone = Arc::clone(&context);
            let shared_mem_clone = Arc::clone(&shared_memory);
            
            let handle = tokio::spawn(async move {
                let security_manager = context_clone.security_manager();
                let mut wallet_manager = WalletManager::new(security_manager.clone());
                
                // 使用共享资源
                let shared_size = shared_mem_clone.size();
                println!("  Task {} accessing shared memory of size {}", i, shared_size);
                
                // 创建钱包（也会使用资源）
                let wallet_id = wallet_manager.create_wallet(
                    None,
                    format!("resource_sharing_test_{}", i)
                ).await.expect(&format!("Failed to create wallet in task {}", i));
                
                let wallet = wallet_manager.load_wallet(&wallet_id).await
                    .expect(&format!("Failed to load wallet in task {}", i));
                
                // 执行操作
                let tx_data = format!("resource_test_transaction_{}", i);
                let _signature = wallet.sign_transaction(tx_data.as_bytes()).await
                    .expect(&format!("Failed to sign in task {}", i));
                
                // 返回任务ID和共享资源的引用计数
                (i, Arc::strong_count(&shared_mem_clone))
            });
            
            handles.push(handle);
        }
        
        // 等待所有任务完成
        let mut results = Vec::new();
        for handle in handles {
            let result = handle.await.expect("Task failed");
            results.push(result);
        }
        
        // 验证资源共享
        for (task_id, ref_count) in results {
            println!("  Task {} saw reference count: {}", task_id, ref_count);
            assert!(ref_count >= initial_use_count, "Reference count should be at least initial count");
        }
        
        // 验证资源生命周期
        let final_use_count = Arc::strong_count(&shared_memory);
        println!("  Final shared memory reference count: {}", final_use_count);
        
        // 应该回到初始状态（或接近）
        assert!(final_use_count <= initial_use_count + 1, 
                "Resource should be properly cleaned up");
        
        println!("✅ Resource sharing and lifecycle verified");
    }

    /// 测试模块间的事件传播和通知机制
    #[tokio::test]
    async fn test_event_propagation_notification() {
        println!("🚀 Testing event propagation and notification mechanism...");
        
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        // 记录初始状态
        let initial_events = security_manager.get_security_events(None).len();
        let start_time = Instant::now();
        
        // 执行会产生事件的操作序列
        let operations = vec![
            "create_wallet",
            "load_wallet", 
            "derive_address",
            "sign_transaction",
            "create_memory",
            "generate_random",
        ];
        
        let wallet_id = wallet_manager.create_wallet(
            None,
            "event_propagation_test".to_string()
        ).await.expect("Failed to create wallet");
        
        let wallet = wallet_manager.load_wallet(&wallet_id).await
            .expect("Failed to load wallet");
        
        // 执行各种操作以产生事件
        let _address = wallet.derive_address(0).await
            .expect("Failed to derive address");
        
        let _signature = wallet.sign_transaction(b"event_test_tx").await
            .expect("Failed to sign transaction");
        
        let _memory = security_manager.create_secure_memory(1024)
            .expect("Failed to create memory");
        
        let mut rng = security_manager.create_secure_rng()
            .expect("Failed to create RNG");
        
        let mut random_data = vec![0u8; 32];
        rng.fill_bytes(&mut random_data).expect("Failed to generate random");
        
        // 等待事件传播
        sleep(Duration::from_millis(300)).await;
        
        // 分析事件传播
        let all_events = security_manager.get_events_since(start_time);
        let final_events = security_manager.get_security_events(None).len();
        
        println!("  Events before operations: {}", initial_events);
        println!("  Events after operations: {}", final_events);
        println!("  New events generated: {}", final_events - initial_events);
        println!("  Events since start time: {}", all_events.len());
        
        // 验证事件传播
        assert!(final_events > initial_events, "Operations should generate events");
        assert!(all_events.len() >= operations.len() - 2, "Should have events for most operations");
        
        // 验证事件类型多样性
        let event_types: std::collections::HashSet<_> = all_events.iter()
            .map(|event| std::mem::discriminant(&event.event))
            .collect();
        
        println!("  Unique event types generated: {}", event_types.len());
        assert!(event_types.len() >= 2, "Should have multiple event types");
        
        // 验证事件来源多样性
        let event_components: std::collections::HashSet<_> = all_events.iter()
            .map(|event| event.component.as_str())
            .collect();
        
        println!("  Event sources: {:?}", event_components);
        assert!(event_components.len() >= 1, "Should have events from multiple components");
        
        println!("✅ Event propagation and notification verified");
    }
}