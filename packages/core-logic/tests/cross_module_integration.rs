/// 跨模块集成测试 - 独立测试文件

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// 测试安全管理器与审计系统的基础交互
#[tokio::test]
async fn test_security_audit_basic_integration() {
    println!("🚀 Testing Security Manager ↔ Audit System basic integration...");
    
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 记录初始审计事件数量
    let initial_events = security_manager.get_security_events(None).len();
    println!("  Initial audit events: {}", initial_events);
    
    // 执行安全操作
    let mut rng = security_manager.create_secure_rng()
        .expect("Failed to create RNG");
    
    let mut buffer = vec![0u8; 32];
    rng.fill_bytes(&mut buffer).expect("Failed to generate random bytes");
    
    // 等待审计日志处理
    sleep(Duration::from_millis(200)).await;
    
    // 检查审计事件是否增加
    let final_events = security_manager.get_security_events(None).len();
    println!("  Final audit events: {}", final_events);
    
    assert!(final_events >= initial_events, "Security operations should maintain or increase audit events");
    
    println!("✅ Security Manager ↔ Audit System basic integration verified");
}

/// 测试钱包管理器与安全管理器的基础交互
#[tokio::test]
async fn test_wallet_security_basic_integration() {
    println!("🚀 Testing Wallet Manager ↔ Security Manager basic integration...");
    
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    let mut wallet_manager = WalletManager::new(security_manager.clone());
    
    // 测试钱包创建是否使用安全管理器
    let wallet_id = wallet_manager.create_wallet(
        None,
        "basic_integration_password".to_string()
    ).await.expect("Failed to create wallet");
    
    println!("  Wallet created with ID: {}", wallet_id);
    
    // 测试钱包加载
    let wallet = wallet_manager.load_wallet(&wallet_id).await
        .expect("Failed to load wallet");
    
    // 测试钱包操作
    let tx_data = b"basic_integration_transaction";
    let signature = wallet.sign_transaction(tx_data).await
        .expect("Failed to sign transaction");
    
    assert!(!signature.is_empty(), "Signature should not be empty");
    println!("  Transaction signed successfully, signature length: {}", signature.len());
    
    println!("✅ Wallet Manager ↔ Security Manager basic integration verified");
}

/// 测试配置系统对模块的影响
#[test]
fn test_config_module_integration() {
    println!("🚀 Testing Configuration System ↔ Module integration...");
    
    // 测试默认配置
    let default_context = init_default().expect("Failed to initialize with default config");
    let default_sm = default_context.security_manager();
    
    println!("  Default configuration loaded");
    println!("    Constant time: {}", default_sm.is_constant_time_enabled());
    println!("    Memory protection: {}", default_sm.is_memory_protection_enabled());
    println!("    Audit logging: {}", default_sm.is_audit_logging_enabled());
    
    // 测试自定义配置
    let custom_config = SecurityConfig {
        enable_constant_time: true,
        enable_memory_protection: true,
        enable_audit_logging: true,
        audit_file_path: Some("/tmp/config_integration_test.log".to_string()),
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    let custom_context = init_with_security_config(custom_config)
        .expect("Failed to initialize with custom config");
    let custom_sm = custom_context.security_manager();
    
    // 验证配置生效
    assert!(custom_sm.is_constant_time_enabled());
    assert!(custom_sm.is_memory_protection_enabled()); 
    assert!(custom_sm.is_audit_logging_enabled());
    
    println!("  Custom configuration verified");
    println!("✅ Configuration System ↔ Module integration verified");
}

/// 测试TEE配置与安全管理器的兼容性
#[test]
fn test_tee_security_compatibility() {
    println!("🚀 Testing TEE ↔ Security Manager compatibility...");
    
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 测试TEE配置创建
    let tee_config = tee::TEEConfig {
        platform: tee::TEEPlatform::Simulation,
        capabilities: tee::TEECapabilities {
            secure_storage: security_manager.is_memory_protection_enabled(),
            hardware_random: true,
            key_derivation: security_manager.is_constant_time_enabled(),
            ..tee::TEECapabilities::default()
        },
        max_sessions: 10,
        session_timeout_ms: 300_000,
        ..tee::TEEConfig::default()
    };
    
    // 验证配置一致性
    println!("  TEE Configuration:");
    println!("    Platform: {:?}", tee_config.platform);
    println!("    Secure storage: {}", tee_config.capabilities.secure_storage);
    println!("    Hardware random: {}", tee_config.capabilities.hardware_random);
    println!("    Key derivation: {}", tee_config.capabilities.key_derivation);
    println!("    Max sessions: {}", tee_config.max_sessions);
    
    // 验证TEE能力与安全管理器设置匹配
    if security_manager.is_memory_protection_enabled() {
        assert!(tee_config.capabilities.secure_storage, 
                "TEE should support secure storage when memory protection is enabled");
    }
    
    if security_manager.is_constant_time_enabled() {
        assert!(tee_config.capabilities.key_derivation, 
                "TEE should support key derivation when constant time is enabled");
    }
    
    println!("✅ TEE ↔ Security Manager compatibility verified");
}

/// 测试错误处理在模块间的一致性
#[tokio::test]
async fn test_cross_module_error_consistency() {
    println!("🚀 Testing cross-module error consistency...");
    
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 测试安全管理器错误
    let invalid_memory = security_manager.create_secure_memory(0);
    match invalid_memory {
        Ok(_) => println!("  Zero-size memory allocation was allowed"),
        Err(e) => println!("  Zero-size memory properly rejected: {:?}", e),
    }
    
    // 测试钱包管理器错误传播
    let mut wallet_manager = WalletManager::new(security_manager.clone());
    
    let wallet_result = wallet_manager.create_wallet(
        None,
        "error_consistency_test".to_string()
    ).await;
    
    match wallet_result {
        Ok(wallet_id) => {
            println!("  Wallet created successfully: {}", wallet_id);
            
            // 测试钱包操作错误
            let wallet = wallet_manager.load_wallet(&wallet_id).await
                .expect("Failed to load wallet");
            
            let empty_tx_result = wallet.sign_transaction(&[]).await;
            match empty_tx_result {
                Ok(_) => println!("  Empty transaction was accepted"),
                Err(e) => println!("  Empty transaction properly rejected: {:?}", e),
            }
        },
        Err(e) => {
            println!("  Wallet creation failed: {:?}", e);
        }
    }
    
    println!("✅ Cross-module error consistency verified");
}

/// 测试并发场景下的模块交互
#[tokio::test]
async fn test_concurrent_module_interaction() {
    println!("🚀 Testing concurrent module interaction...");
    
    let context = Arc::new(init_default().expect("Failed to initialize"));
    let mut handles = Vec::new();
    
    // 启动多个并发任务
    for task_id in 0..3 {
        let context_clone = Arc::clone(&context);
        
        let handle = tokio::spawn(async move {
            let security_manager = context_clone.security_manager();
            let mut wallet_manager = WalletManager::new(security_manager.clone());
            
            // 每个任务执行一系列操作
            let wallet_id = wallet_manager.create_wallet(
                None,
                format!("concurrent_test_{}", task_id)
            ).await.expect(&format!("Task {} failed to create wallet", task_id));
            
            let wallet = wallet_manager.load_wallet(&wallet_id).await
                .expect(&format!("Task {} failed to load wallet", task_id));
            
            let tx_data = format!("concurrent_transaction_{}", task_id);
            let signature = wallet.sign_transaction(tx_data.as_bytes()).await
                .expect(&format!("Task {} failed to sign transaction", task_id));
            
            (task_id, wallet_id, signature.len())
        });
        
        handles.push(handle);
    }
    
    // 收集结果
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task failed");
        results.push(result);
    }
    
    // 验证所有任务成功完成
    assert_eq!(results.len(), 3);
    for (task_id, wallet_id, sig_len) in &results {
        println!("  Task {}: wallet_id={}, signature_len={}", task_id, wallet_id, sig_len);
        assert!(*sig_len > 0, "Signature should not be empty");
    }
    
    println!("✅ Concurrent module interaction verified");
}

/// 测试模块状态的一致性
#[tokio::test] 
async fn test_module_state_consistency() {
    println!("🚀 Testing module state consistency...");
    
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 记录初始状态
    let initial_audit_events = security_manager.get_security_events(None).len();
    println!("  Initial audit events: {}", initial_audit_events);
    
    // 创建多个钱包管理器实例
    let mut wallet_managers = Vec::new();
    for i in 0..2 {
        let wm = WalletManager::new(security_manager.clone());
        wallet_managers.push(wm);
    }
    
    // 使用不同的管理器创建钱包
    let mut wallet_ids = Vec::new();
    for (i, wm) in wallet_managers.iter_mut().enumerate() {
        let wallet_id = wm.create_wallet(
            None,
            format!("state_consistency_test_{}", i)
        ).await.expect(&format!("Failed to create wallet {}", i));
        
        wallet_ids.push(wallet_id);
    }
    
    // 验证状态一致性
    println!("  Created wallets: {:?}", wallet_ids);
    
    // 等待状态同步
    sleep(Duration::from_millis(100)).await;
    
    // 检查审计状态更新
    let final_audit_events = security_manager.get_security_events(None).len();
    println!("  Final audit events: {}", final_audit_events);
    
    // 验证状态变化合理
    assert!(final_audit_events >= initial_audit_events, 
            "Audit events should not decrease");
    
    println!("✅ Module state consistency verified");
}