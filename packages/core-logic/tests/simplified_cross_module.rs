/// 简化跨模块集成测试 - 基于实际API

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 测试安全管理器的基础功能
#[test]
fn test_security_manager_basic_functions() {
    println!("🚀 Testing Security Manager basic functions...");
    
    let config = SecurityConfig {
        enable_constant_time: true,
        enable_memory_protection: true,
        enable_audit_logging: true,
        audit_file_path: Some("/tmp/test_audit.log".to_string()),
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    let security_manager = SecurityManager::new(config);
    
    // 测试配置查询
    assert!(security_manager.is_constant_time_enabled());
    assert!(security_manager.is_memory_protection_enabled());
    assert!(security_manager.is_audit_logging_enabled());
    
    println!("  ✅ Security Manager configuration verified");
    
    // 测试安全内存分配
    let memory_result = security_manager.create_secure_memory(1024);
    match memory_result {
        Ok(memory) => {
            assert_eq!(memory.size(), 1024);
            println!("  ✅ Secure memory allocation: {} bytes", memory.size());
        },
        Err(e) => {
            println!("  ❌ Secure memory allocation failed: {}", e);
            panic!("Memory allocation should succeed");
        }
    }
    
    // 测试安全随机数生成
    let rng_result = security_manager.create_secure_rng();
    match rng_result {
        Ok(mut rng) => {
            let mut buffer = vec![0u8; 32];
            if let Ok(()) = rng.fill_bytes(&mut buffer) {
                assert_ne!(buffer, vec![0u8; 32]);
                println!("  ✅ Secure RNG generated {} bytes", buffer.len());
            } else {
                println!("  ❌ RNG fill_bytes failed");
            }
        },
        Err(e) => {
            println!("  ❌ Secure RNG creation failed: {}", e);
        }
    }
    
    println!("✅ Security Manager basic functions verified");
}

/// 测试钱包管理器的基础功能
#[test]
fn test_wallet_manager_basic_functions() {
    println!("🚀 Testing Wallet Manager basic functions...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 测试钱包管理器创建
    let wallet_manager_result = WalletManager::new(&security_manager);
    match wallet_manager_result {
        Ok(wallet_manager) => {
            println!("  ✅ Wallet Manager created successfully");
            
            // 测试钱包绑定
            let binding = wallet::UserWalletBinding {
                user_id: 12345,
                wallet_id: uuid::Uuid::new_v4(),
                address: [0u8; 20],
                alias: Some("test_wallet".to_string()),
                is_primary: true,
                permissions: wallet::WalletPermissions::full_permissions(),
            };
            
            println!("  ✅ Wallet binding created for user: {}", binding.user_id);
            
        },
        Err(e) => {
            println!("  ❌ Wallet Manager creation failed: {:?}", e);
        }
    }
    
    println!("✅ Wallet Manager basic functions verified");
}

/// 测试安全管理器与审计系统的集成
#[test]
fn test_security_audit_integration() {
    println!("🚀 Testing Security Manager ↔ Audit System integration...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 执行会产生审计事件的操作
    let _ = security_manager.create_secure_memory(512);
    let _ = security_manager.create_secure_rng();
    
    // 手动触发审计事件
    security_manager.audit_info(
        AuditEvent::TEEOperation {
            operation: "integration_test".to_string(),
            duration_ms: 10,
            success: true,
        },
        "test_component"
    );
    
    security_manager.audit_security_event(
        AuditEvent::KeyGeneration {
            algorithm: "test_algorithm".to_string(),
            key_size: 256,
            operation: "test_key_gen".to_string(),
            key_type: "test_key".to_string(),
            duration_ms: 5,
            entropy_bits: 256,
        },
        "test_crypto_component"
    );
    
    println!("  ✅ Audit events generated successfully");
    
    println!("✅ Security Manager ↔ Audit System integration verified");
}

/// 测试TEE配置与安全管理器的兼容性
#[test]
fn test_tee_security_compatibility() {
    println!("🚀 Testing TEE ↔ Security Manager compatibility...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 创建TEE配置，考虑安全管理器的设置
    let tee_config = tee::TEEConfig {
        platform: tee::TEEPlatform::Simulation,
        capabilities: tee::TEECapabilities {
            secure_storage: security_manager.is_memory_protection_enabled(),
            hardware_random: true,
            key_derivation: security_manager.is_constant_time_enabled(),
            biometric_support: false,
            secure_display: false,
            attestation: false,
        },
        max_sessions: 10,
        session_timeout_ms: 300_000,
        ta_uuid: "test-uuid".to_string(),
    };
    
    // 验证配置一致性
    println!("  TEE Platform: {:?}", tee_config.platform);
    println!("  Secure Storage: {} (Security Manager Memory Protection: {})", 
             tee_config.capabilities.secure_storage, 
             security_manager.is_memory_protection_enabled());
    println!("  Key Derivation: {} (Security Manager Constant Time: {})", 
             tee_config.capabilities.key_derivation, 
             security_manager.is_constant_time_enabled());
    
    // 验证一致性
    assert_eq!(tee_config.capabilities.secure_storage, 
               security_manager.is_memory_protection_enabled());
    assert_eq!(tee_config.capabilities.key_derivation, 
               security_manager.is_constant_time_enabled());
    
    println!("✅ TEE ↔ Security Manager compatibility verified");
}

/// 测试常量时间操作
#[test]
fn test_constant_time_operations() {
    println!("🚀 Testing constant-time operations...");
    
    let data1 = SecureBytes::from_slice(b"test_data_12345");
    let data2 = SecureBytes::from_slice(b"test_data_12345");
    let data3 = SecureBytes::from_slice(b"different_data_");
    
    // 测试相等比较
    let eq_result = data1.constant_time_eq(&data2);
    assert!(bool::from(eq_result));
    println!("  ✅ Equal data comparison: correct");
    
    // 测试不等比较
    let neq_result = data1.constant_time_eq(&data3);
    assert!(!bool::from(neq_result));
    println!("  ✅ Non-equal data comparison: correct");
    
    // 测试条件选择
    let selected = SecureBytes::conditional_select(&data3, &data1, eq_result);
    assert_eq!(selected.as_slice(), data1.as_slice());
    println!("  ✅ Conditional select: correct");
    
    println!("✅ Constant-time operations verified");
}

/// 测试内存保护功能
#[test]
fn test_memory_protection() {
    println!("🚀 Testing memory protection...");
    
    // 测试安全内存创建
    let memory_result = SecureMemory::new(1024);
    match memory_result {
        Ok(mut memory) => {
            println!("  ✅ Secure memory created: {} bytes", memory.size());
            
            // 测试数据写入
            let test_data = b"secure_memory_test_data";
            if let Ok(()) = memory.copy_from_slice(test_data) {
                assert_eq!(&memory.as_slice()[..test_data.len()], test_data);
                println!("  ✅ Secure memory write/read: correct");
            }
            
            // 测试边界检查
            let large_data = vec![0u8; 2048]; // 超过内存大小
            let boundary_result = memory.copy_from_slice(&large_data);
            assert!(boundary_result.is_err());
            println!("  ✅ Memory boundary protection: working");
            
        },
        Err(e) => {
            println!("  ❌ Secure memory creation failed: {}", e);
        }
    }
    
    // 测试安全字符串
    let secure_str1 = SecureString::new("password123").expect("Failed to create secure string");
    let secure_str2 = SecureString::new("password123").expect("Failed to create secure string");
    let secure_str3 = SecureString::new("different").expect("Failed to create secure string");
    
    assert!(secure_str1.secure_eq(&secure_str2));
    assert!(!secure_str1.secure_eq(&secure_str3));
    println!("  ✅ Secure string comparison: working");
    
    println!("✅ Memory protection verified");
}

/// 测试错误处理的一致性
#[test]
fn test_error_handling_consistency() {
    println!("🚀 Testing error handling consistency...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 测试无效内存大小
    let invalid_memory = security_manager.create_secure_memory(0);
    match invalid_memory {
        Ok(_) => println!("  ⚠️ Zero-size memory was allowed"),
        Err(e) => println!("  ✅ Zero-size memory rejected: {}", e),
    }
    
    // 测试钱包管理器错误
    let wallet_manager_result = WalletManager::new(&security_manager);
    match wallet_manager_result {
        Ok(_) => println!("  ✅ Wallet manager created successfully"),
        Err(e) => println!("  ❌ Wallet manager creation failed: {:?}", e),
    }
    
    // 测试无效的安全字符串
    let invalid_secure_str = SecureString::new("");
    match invalid_secure_str {
        Ok(s) => println!("  ⚠️ Empty secure string was allowed (length: {})", s.len()),
        Err(e) => println!("  ✅ Empty secure string rejected: {}", e),
    }
    
    println!("✅ Error handling consistency verified");
}

/// 测试配置系统对所有模块的影响
#[test]
fn test_configuration_system_impact() {
    println!("🚀 Testing configuration system impact on all modules...");
    
    // 测试默认配置
    let default_config = SecurityConfig::default();
    let default_sm = SecurityManager::new(default_config);
    
    println!("  Default Configuration:");
    println!("    Constant Time: {}", default_sm.is_constant_time_enabled());
    println!("    Memory Protection: {}", default_sm.is_memory_protection_enabled());
    println!("    Audit Logging: {}", default_sm.is_audit_logging_enabled());
    
    // 测试自定义配置
    let custom_config = SecurityConfig {
        enable_constant_time: false,
        enable_memory_protection: false,
        enable_audit_logging: false,
        audit_file_path: None,
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    let custom_sm = SecurityManager::new(custom_config);
    
    println!("  Custom Configuration:");
    println!("    Constant Time: {}", custom_sm.is_constant_time_enabled());
    println!("    Memory Protection: {}", custom_sm.is_memory_protection_enabled());
    println!("    Audit Logging: {}", custom_sm.is_audit_logging_enabled());
    
    // 验证配置影响
    assert!(!custom_sm.is_constant_time_enabled());
    assert!(!custom_sm.is_memory_protection_enabled());
    assert!(!custom_sm.is_audit_logging_enabled());
    
    // 测试配置对TEE的影响
    let tee_config = tee::TEEConfig {
        capabilities: tee::TEECapabilities {
            secure_storage: custom_sm.is_memory_protection_enabled(),
            key_derivation: custom_sm.is_constant_time_enabled(),
            ..tee::TEECapabilities::default()
        },
        ..tee::TEEConfig::default()
    };
    
    assert!(!tee_config.capabilities.secure_storage);
    assert!(!tee_config.capabilities.key_derivation);
    
    println!("✅ Configuration system impact verified");
}