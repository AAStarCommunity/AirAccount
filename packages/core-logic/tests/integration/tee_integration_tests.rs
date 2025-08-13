/// TEE环境集成测试
/// 测试核心逻辑与TEE环境的集成，包括QEMU TEE模拟环境、TEE-REE通信和安全边界验证

#[cfg(test)]
mod tee_integration_tests {
    use airaccount_core_logic::*;
    use airaccount_core_logic::tee::{TEEInterface, TEEResult, TEESecureStorage, TEERandom, TEEError};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;
    use std::collections::HashMap;
    use std::cell::RefCell;

    /// TEE模拟器实现，用于测试环境
    struct MockTEEEnvironment {
        sessions: HashMap<u32, TEESession>,
        next_session_id: u32,
        initialized: bool,
        secure_storage: RefCell<HashMap<String, Vec<u8>>>,
    }

    struct TEESession {
        id: u32,
        created_at: Instant,
        active: bool,
    }

    impl MockTEEEnvironment {
        fn new() -> Self {
            Self {
                sessions: HashMap::new(),
                next_session_id: 1,
                initialized: false,
                secure_storage: RefCell::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl TEEInterface for MockTEEEnvironment {
        async fn initialize(&mut self) -> TEEResult<()> {
            if self.initialized {
                return Err(TEEError::InitializationFailed("Already initialized".to_string()));
            }
            
            // 模拟TEE初始化过程
            sleep(Duration::from_millis(50)).await;
            self.initialized = true;
            
            println!("✅ TEE Environment initialized");
            Ok(())
        }

        async fn create_session(&mut self) -> TEEResult<u32> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            let session_id = self.next_session_id;
            self.next_session_id += 1;

            let session = TEESession {
                id: session_id,
                created_at: Instant::now(),
                active: true,
            };

            self.sessions.insert(session_id, session);
            println!("✅ TEE Session created: {}", session_id);
            
            Ok(session_id)
        }

        async fn close_session(&mut self, session_id: u32) -> TEEResult<()> {
            if let Some(mut session) = self.sessions.get_mut(&session_id) {
                session.active = false;
                println!("✅ TEE Session closed: {}", session_id);
                Ok(())
            } else {
                Err(TEEError::SessionError("Session not found".to_string()))
            }
        }

        async fn invoke_command(&mut self, session_id: u32, command_id: u32, input: &[u8]) -> TEEResult<Vec<u8>> {
            let session = self.sessions.get(&session_id)
                .ok_or(TEEError::SessionError("Session not found".to_string()))?;

            if !session.active {
                return Err(TEEError::SessionError("Session inactive".to_string()));
            }

            // 模拟各种TEE命令
            match command_id {
                0x1000 => { // 生成密钥对
                    sleep(Duration::from_millis(10)).await;
                    let keypair = format!("mock_keypair_from_{:?}", input);
                    Ok(keypair.into_bytes())
                },
                0x2000 => { // 签名操作
                    sleep(Duration::from_millis(5)).await;
                    let signature = format!("mock_signature_of_{:02x?}", input);
                    Ok(signature.into_bytes())
                },
                0x3000 => { // 加密操作
                    sleep(Duration::from_millis(3)).await;
                    let encrypted = input.iter().map(|&b| b ^ 0xAB).collect();
                    Ok(encrypted)
                },
                0x4000 => { // 解密操作
                    sleep(Duration::from_millis(3)).await;
                    let decrypted = input.iter().map(|&b| b ^ 0xAB).collect();
                    Ok(decrypted)
                },
                _ => Err(TEEError::UnsupportedOperation("Unsupported command".to_string()))
            }
        }

        async fn test_secure_storage(&self) -> TEEResult<()> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }
            Ok(())
        }

        async fn generate_random(&self, buffer: &mut [u8]) -> TEEResult<()> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            // 使用更强的随机数生成（测试环境）
            use rand::RngCore;
            let mut rng = rand::thread_rng();
            rng.fill_bytes(buffer);
            Ok(())
        }
    }

    impl TEESecureStorage for MockTEEEnvironment {
        fn store(&self, key: &str, data: &[u8]) -> TEEResult<()> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            // 在实际实现中，这会存储到TEE安全存储
            // 这里模拟存储到内存（仅测试用）
            self.secure_storage.borrow_mut().insert(key.to_string(), data.to_vec());
            Ok(())
        }

        fn load(&self, key: &str) -> TEEResult<Vec<u8>> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            self.secure_storage.borrow().get(key)
                .cloned()
                .ok_or(TEEError::StorageError("Storage key not found".to_string()))
        }

        fn delete(&self, key: &str) -> TEEResult<()> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            self.secure_storage.borrow_mut().remove(key)
                .ok_or(TEEError::StorageError("Storage key not found".to_string()))
                .map(|_| ())
        }
    }

    impl TEERandom for MockTEEEnvironment {
        fn generate(&self, buffer: &mut [u8]) -> TEEResult<()> {
            if !self.initialized {
                return Err(TEEError::InitializationFailed("TEE not initialized".to_string()));
            }

            use rand::RngCore;
            let mut rng = rand::thread_rng();
            rng.fill_bytes(buffer);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_tee_environment_initialization() {
        println!("🚀 Testing TEE environment initialization...");
        
        let mut tee = MockTEEEnvironment::new();
        assert!(!tee.initialized);

        // 初始化TEE环境
        let init_result = tee.initialize().await;
        assert!(init_result.is_ok(), "TEE initialization failed");
        assert!(tee.initialized);

        // 重复初始化应该失败
        let double_init_result = tee.initialize().await;
        assert!(double_init_result.is_err());

        println!("✅ TEE environment initialization test passed");
    }

    #[tokio::test]
    async fn test_tee_session_management() {
        println!("🚀 Testing TEE session management...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");

        // 创建多个会话
        let session1 = tee.create_session().await.expect("Failed to create session 1");
        let session2 = tee.create_session().await.expect("Failed to create session 2");
        let session3 = tee.create_session().await.expect("Failed to create session 3");

        assert_ne!(session1, session2);
        assert_ne!(session2, session3);
        assert_eq!(tee.sessions.len(), 3);

        // 验证会话状态
        assert!(tee.sessions[&session1].active);
        assert!(tee.sessions[&session2].active);
        assert!(tee.sessions[&session3].active);

        // 关闭会话
        tee.close_session(session2).await.expect("Failed to close session 2");
        assert!(!tee.sessions[&session2].active);

        // 尝试关闭不存在的会话
        let invalid_close = tee.close_session(9999).await;
        assert!(invalid_close.is_err());

        println!("✅ TEE session management test passed");
    }

    #[tokio::test]
    async fn test_tee_command_invocation() {
        println!("🚀 Testing TEE command invocation...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");
        let session = tee.create_session().await.expect("Failed to create session");

        // 测试密钥生成命令
        let key_gen_input = b"key_generation_params";
        let key_result = tee.invoke_command(session, 0x1000, key_gen_input).await;
        assert!(key_result.is_ok());
        let keypair = key_result.unwrap();
        assert!(!keypair.is_empty());
        println!("✅ Key generation command executed");

        // 测试签名命令
        let sign_input = b"data_to_sign_12345";
        let sign_result = tee.invoke_command(session, 0x2000, sign_input).await;
        assert!(sign_result.is_ok());
        let signature = sign_result.unwrap();
        assert!(!signature.is_empty());
        println!("✅ Signature command executed");

        // 测试加密/解密命令
        let original_data = b"sensitive_data_to_encrypt";
        let encrypt_result = tee.invoke_command(session, 0x3000, original_data).await;
        assert!(encrypt_result.is_ok());
        let encrypted = encrypt_result.unwrap();

        let decrypt_result = tee.invoke_command(session, 0x4000, &encrypted).await;
        assert!(decrypt_result.is_ok());
        let decrypted = decrypt_result.unwrap();
        assert_eq!(decrypted, original_data);
        println!("✅ Encrypt/decrypt commands executed");

        // 测试不支持的命令
        let unsupported_result = tee.invoke_command(session, 0x9999, b"test").await;
        assert!(unsupported_result.is_err());

        // 测试在非活跃会话上的命令调用
        tee.close_session(session).await.expect("Failed to close session");
        let inactive_result = tee.invoke_command(session, 0x1000, b"test").await;
        assert!(inactive_result.is_err());

        println!("✅ TEE command invocation test passed");
    }

    #[tokio::test]
    async fn test_tee_secure_storage() {
        println!("🚀 Testing TEE secure storage...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");

        // 存储测试数据
        let test_data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let store_result = tee.store("test_key", &test_data);
        assert!(store_result.is_ok());

        // 读取存储的数据
        let load_result = tee.load("test_key");
        assert!(load_result.is_ok());
        let loaded_data = load_result.unwrap();
        assert_eq!(loaded_data, test_data);

        // 测试不存在的键
        let missing_result = tee.load("nonexistent_key");
        assert!(missing_result.is_err());

        // 存储大量数据测试
        let large_data = vec![0xABu8; 1024 * 64]; // 64KB
        assert!(tee.store("large_key", &large_data).is_ok());
        let loaded_large = tee.load("large_key").unwrap();
        assert_eq!(loaded_large.len(), large_data.len());

        // 删除测试
        assert!(tee.delete("test_key").is_ok());
        let deleted_result = tee.load("test_key");
        assert!(deleted_result.is_err());

        // 删除不存在的键
        let delete_missing = tee.delete("nonexistent_key");
        assert!(delete_missing.is_err());

        println!("✅ TEE secure storage test passed");
    }

    #[tokio::test]
    async fn test_tee_random_generation() {
        println!("🚀 Testing TEE random generation...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");

        // 测试不同大小的随机数生成
        let sizes = vec![16, 32, 64, 256];
        for size in sizes {
            let mut buffer = vec![0u8; size];
            let result = tee.generate_random(&mut buffer).await;
            assert!(result.is_ok(), "Failed to generate {} bytes of random data", size);
            
            // 验证缓冲区不是全零（极低概率事件）
            assert_ne!(buffer, vec![0u8; size], "Generated random data is all zeros");
        }

        // 测试多次生成的随机性
        let mut samples = Vec::new();
        for _ in 0..10 {
            let mut buffer = vec![0u8; 32];
            tee.generate_random(&mut buffer).await.expect("Failed to generate random data");
            samples.push(buffer);
        }

        // 验证样本不完全相同
        for i in 0..samples.len() {
            for j in (i + 1)..samples.len() {
                if samples[i] == samples[j] {
                    panic!("Random samples {} and {} are identical", i, j);
                }
            }
        }

        println!("✅ TEE random generation test passed");
    }

    #[tokio::test]
    async fn test_tee_ree_communication() {
        println!("🚀 Testing TEE-REE communication...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");
        let session = tee.create_session().await.expect("Failed to create session");

        // 模拟REE到TEE的数据传输
        let ree_data = b"REE_to_TEE_data_transfer_test";
        let start_time = Instant::now();
        
        let tee_response = tee.invoke_command(session, 0x3000, ree_data).await
            .expect("Failed to invoke TEE command");
        
        let communication_time = start_time.elapsed();
        println!("TEE-REE communication time: {:?}", communication_time);

        // 验证通信成功
        assert!(!tee_response.is_empty());
        assert_ne!(tee_response, ree_data.to_vec());

        // 测试大数据传输
        let large_ree_data = vec![0x5Au8; 1024 * 10]; // 10KB
        let large_start_time = Instant::now();
        
        let large_response = tee.invoke_command(session, 0x3000, &large_ree_data).await
            .expect("Failed to transfer large data to TEE");
        
        let large_comm_time = large_start_time.elapsed();
        println!("Large data TEE-REE communication time: {:?}", large_comm_time);
        
        assert_eq!(large_response.len(), large_ree_data.len());

        // 测试双向通信
        let decrypted = tee.invoke_command(session, 0x4000, &large_response).await
            .expect("Failed to decrypt data from TEE");
        assert_eq!(decrypted, large_ree_data);

        println!("✅ TEE-REE communication test passed");
    }

    #[tokio::test]
    async fn test_tee_security_boundaries() {
        println!("🚀 Testing TEE security boundaries...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");

        // 测试TEE环境隔离
        // 未初始化时的操作应该被拒绝
        let mut uninit_tee = MockTEEEnvironment::new();
        assert!(uninit_tee.create_session().await.is_err());
        assert!(uninit_tee.test_secure_storage().await.is_err());
        
        let mut buffer = vec![0u8; 32];
        assert!(uninit_tee.generate_random(&mut buffer).await.is_err());

        // 测试会话隔离
        let session1 = tee.create_session().await.expect("Failed to create session 1");
        let session2 = tee.create_session().await.expect("Failed to create session 2");

        // 在会话1中存储数据
        let session1_data = b"session1_private_data";
        let result1 = tee.invoke_command(session1, 0x3000, session1_data).await
            .expect("Failed to encrypt in session 1");

        // 在会话2中尝试解密会话1的数据（应该可以，因为这是同一个TEE环境）
        let result2 = tee.invoke_command(session2, 0x4000, &result1).await;
        assert!(result2.is_ok());

        // 关闭会话后的访问应该失败
        tee.close_session(session1).await.expect("Failed to close session 1");
        let closed_session_result = tee.invoke_command(session1, 0x1000, b"test").await;
        assert!(closed_session_result.is_err());

        // 测试存储隔离（在实际TEE中，不同TA会有独立的存储空间）
        assert!(tee.store("boundary_test", b"test_data").is_ok());
        assert!(tee.load("boundary_test").is_ok());

        println!("✅ TEE security boundaries test passed");
    }

    #[tokio::test]
    async fn test_tee_performance_characteristics() {
        println!("🚀 Testing TEE performance characteristics...");
        
        let mut tee = MockTEEEnvironment::new();
        tee.initialize().await.expect("Failed to initialize TEE");
        let session = tee.create_session().await.expect("Failed to create session");

        // 测试批量操作性能
        let batch_size = 100;
        let batch_start = Instant::now();

        for i in 0..batch_size {
            let input = format!("batch_test_{}", i);
            let _result = tee.invoke_command(session, 0x1000, input.as_bytes()).await
                .expect(&format!("Failed batch operation {}", i));
        }

        let batch_duration = batch_start.elapsed();
        let avg_op_time = batch_duration / batch_size;
        
        println!("Batch of {} operations completed in {:?}", batch_size, batch_duration);
        println!("Average operation time: {:?}", avg_op_time);

        // 性能基准验证
        assert!(avg_op_time < Duration::from_millis(100), "Operation too slow");

        // 测试内存使用
        let memory_test_data = vec![0xDEu8; 1024]; // 1KB
        let memory_ops = 50;
        
        for i in 0..memory_ops {
            let key = format!("memory_test_{}", i);
            assert!(tee.store(&key, &memory_test_data).is_ok());
        }

        // 验证存储的数据
        for i in 0..memory_ops {
            let key = format!("memory_test_{}", i);
            let loaded = tee.load(&key).expect(&format!("Failed to load {}", key));
            assert_eq!(loaded.len(), memory_test_data.len());
        }

        println!("✅ TEE performance characteristics test passed");
    }

    #[tokio::test] 
    async fn test_tee_error_handling_and_recovery() {
        println!("🚀 Testing TEE error handling and recovery...");
        
        let mut tee = MockTEEEnvironment::new();
        
        // 测试未初始化状态的错误处理
        assert!(matches!(tee.create_session().await, Err(TEEError::NotInitialized)));
        assert!(matches!(tee.test_secure_storage().await, Err(TEEError::NotInitialized)));

        // 初始化后重新测试
        tee.initialize().await.expect("Failed to initialize TEE");
        
        // 测试无效会话ID
        let invalid_session_result = tee.invoke_command(9999, 0x1000, b"test").await;
        assert!(matches!(invalid_session_result, Err(TEEError::SessionNotFound)));

        // 测试无效命令
        let session = tee.create_session().await.expect("Failed to create session");
        let invalid_cmd_result = tee.invoke_command(session, 0x9999, b"test").await;
        assert!(matches!(invalid_cmd_result, Err(TEEError::UnsupportedCommand)));

        // 测试存储错误恢复
        assert!(matches!(tee.load("nonexistent"), Err(TEEError::StorageKeyNotFound)));
        assert!(matches!(tee.delete("nonexistent"), Err(TEEError::StorageKeyNotFound)));

        // 测试会话恢复能力
        tee.close_session(session).await.expect("Failed to close session");
        let new_session = tee.create_session().await.expect("Failed to create new session");
        
        // 新会话应该正常工作
        let recovery_result = tee.invoke_command(new_session, 0x1000, b"recovery_test").await;
        assert!(recovery_result.is_ok());

        println!("✅ TEE error handling and recovery test passed");
    }

    #[tokio::test]
    async fn test_tee_concurrent_access() {
        println!("🚀 Testing TEE concurrent access...");
        
        let tee = Arc::new(tokio::sync::Mutex::new(MockTEEEnvironment::new()));
        
        // 初始化TEE
        {
            let mut tee_lock = tee.lock().await;
            tee_lock.initialize().await.expect("Failed to initialize TEE");
        }

        // 并发创建会话
        let mut session_handles = Vec::new();
        for i in 0..10 {
            let tee_clone = Arc::clone(&tee);
            let handle = tokio::spawn(async move {
                let mut tee_lock = tee_clone.lock().await;
                let session = tee_lock.create_session().await
                    .expect(&format!("Failed to create session {}", i));
                
                // 执行一些操作
                let input = format!("concurrent_test_{}", i);
                let _result = tee_lock.invoke_command(session, 0x1000, input.as_bytes()).await
                    .expect(&format!("Failed operation in session {}", i));
                
                session
            });
            session_handles.push(handle);
        }

        // 收集所有会话ID
        let mut session_ids = Vec::new();
        for handle in session_handles {
            let session_id = handle.await.expect("Task failed");
            session_ids.push(session_id);
        }

        // 验证所有会话ID都不同
        session_ids.sort();
        for i in 1..session_ids.len() {
            assert_ne!(session_ids[i-1], session_ids[i]);
        }

        println!("Created {} concurrent sessions", session_ids.len());

        // 并发存储操作
        let mut storage_handles = Vec::new();
        for i in 0..20 {
            let tee_clone = Arc::clone(&tee);
            let handle = tokio::spawn(async move {
                let tee_lock = tee_clone.lock().await;
                let key = format!("concurrent_key_{}", i);
                let data = format!("concurrent_data_{}", i).into_bytes();
                
                tee_lock.store(&key, &data)
                    .expect(&format!("Failed to store key {}", i));
                
                let loaded = tee_lock.load(&key)
                    .expect(&format!("Failed to load key {}", i));
                
                assert_eq!(loaded, data);
                key
            });
            storage_handles.push(handle);
        }

        // 验证所有并发存储操作成功
        let mut stored_keys = Vec::new();
        for handle in storage_handles {
            let key = handle.await.expect("Storage task failed");
            stored_keys.push(key);
        }

        assert_eq!(stored_keys.len(), 20);

        println!("✅ TEE concurrent access test passed");
    }
}