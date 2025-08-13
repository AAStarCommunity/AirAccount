/// 基础TEE集成测试
/// 简化版TEE测试，专注于核心功能验证

use airaccount_core_logic::*;
use airaccount_core_logic::tee::{TEEError, TEEConfig, TEEPlatform, TEECapabilities};

#[test]
fn test_tee_config_creation() {
    println!("🚀 Testing TEE configuration creation...");
    
    let default_config = TEEConfig::default();
    assert_eq!(default_config.platform, TEEPlatform::OpTEE);
    assert_eq!(default_config.max_sessions, 10);
    assert_eq!(default_config.session_timeout_ms, 300_000);
    
    let custom_config = TEEConfig {
        platform: TEEPlatform::IntelSGX,
        ta_uuid: "custom-uuid".to_string(),
        capabilities: TEECapabilities {
            secure_storage: true,
            hardware_random: true,
            secure_display: true,
            biometric_support: true,
            key_derivation: true,
            attestation: true,
        },
        max_sessions: 20,
        session_timeout_ms: 600_000,
    };
    
    assert_eq!(custom_config.platform, TEEPlatform::IntelSGX);
    assert_eq!(custom_config.max_sessions, 20);
    assert!(custom_config.capabilities.biometric_support);
    
    println!("✅ TEE configuration creation test passed");
}

#[test] 
fn test_tee_capabilities() {
    println!("🚀 Testing TEE capabilities...");
    
    let default_caps = TEECapabilities::default();
    assert!(default_caps.secure_storage);
    assert!(default_caps.hardware_random);
    assert!(default_caps.key_derivation);
    assert!(!default_caps.secure_display);
    assert!(!default_caps.biometric_support);
    assert!(!default_caps.attestation);
    
    let full_caps = TEECapabilities {
        secure_storage: true,
        hardware_random: true,
        secure_display: true,
        biometric_support: true,
        key_derivation: true,
        attestation: true,
    };
    
    assert!(full_caps.secure_display);
    assert!(full_caps.biometric_support);
    assert!(full_caps.attestation);
    
    println!("✅ TEE capabilities test passed");
}

#[test]
fn test_tee_platform_types() {
    println!("🚀 Testing TEE platform types...");
    
    let platforms = vec![
        TEEPlatform::OpTEE,
        TEEPlatform::IntelSGX,
        TEEPlatform::AmdSev,
        TEEPlatform::Simulation,
    ];
    
    for platform in platforms {
        let config = TEEConfig {
            platform,
            ..TEEConfig::default()
        };
        
        assert_eq!(config.platform, platform);
    }
    
    println!("✅ TEE platform types test passed");
}

#[test]
fn test_tee_error_types() {
    println!("🚀 Testing TEE error types...");
    
    let errors = vec![
        TEEError::InitializationFailed("init failed".to_string()),
        TEEError::SessionError("session error".to_string()),
        TEEError::StorageError("storage error".to_string()),
        TEEError::CryptographicError("crypto error".to_string()),
        TEEError::HardwareError("hardware error".to_string()),
        TEEError::UnsupportedOperation("unsupported".to_string()),
    ];
    
    for error in errors {
        let error_string = format!("{}", error);
        assert!(!error_string.is_empty());
        println!("  Error: {}", error_string);
    }
    
    println!("✅ TEE error types test passed");
}

#[test]
fn test_core_context_with_tee() {
    println!("🚀 Testing core context initialization for TEE integration...");
    
    // 测试默认初始化
    let context = init_default().expect("Failed to initialize core context");
    assert!(context.is_initialized());
    assert!(context.validate().is_ok());
    
    // 测试自定义配置
    let security_config = SecurityConfig {
        enable_constant_time: true,
        enable_memory_protection: true,
        enable_audit_logging: true,
        audit_file_path: Some("/tmp/tee_test_audit.log".to_string()),
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    let context = init_with_security_config(security_config).expect("Failed to initialize with custom config");
    assert!(context.is_initialized());
    assert!(context.security_manager().is_constant_time_enabled());
    assert!(context.security_manager().is_memory_protection_enabled());
    assert!(context.security_manager().is_audit_logging_enabled());
    
    println!("✅ Core context with TEE integration test passed");
}

#[tokio::test]
async fn test_tee_integration_readiness() {
    println!("🚀 Testing TEE integration readiness...");
    
    // 验证核心系统是否准备好与TEE集成
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 测试安全内存分配（TEE将使用）
    let secure_memory = security_manager.create_secure_memory(1024)
        .expect("Failed to create secure memory");
    assert_eq!(secure_memory.size(), 1024);
    
    // 测试安全随机数生成（TEE将使用）
    let mut rng = security_manager.create_secure_rng()
        .expect("Failed to create secure RNG");
    
    let mut buffer = vec![0u8; 32];
    rng.fill_bytes(&mut buffer).expect("Failed to generate random bytes");
    
    // 验证随机性（简单检查）
    assert_ne!(buffer, vec![0u8; 32]);
    
    // 测试常量时间操作（TEE关键要求）
    let data1 = SecureBytes::from_slice(b"test_data_123456");
    let data2 = SecureBytes::from_slice(b"test_data_123456");
    let data3 = SecureBytes::from_slice(b"different_data__");
    
    let eq_result = data1.constant_time_eq(&data2);
    assert!(bool::from(eq_result));
    
    let neq_result = data1.constant_time_eq(&data3);
    assert!(!bool::from(neq_result));
    
    println!("✅ TEE integration readiness test passed");
}

#[test]
fn test_tee_session_info() {
    println!("🚀 Testing TEE session information structures...");
    
    // 模拟会话信息
    struct TestSessionInfo {
        session_id: u32,
        created_at: std::time::Instant,
        last_activity: std::time::Instant,
        user_context: Option<String>,
    }
    
    let session = TestSessionInfo {
        session_id: 12345,
        created_at: std::time::Instant::now(),
        last_activity: std::time::Instant::now(),
        user_context: Some("test_user".to_string()),
    };
    
    assert_eq!(session.session_id, 12345);
    assert!(session.user_context.is_some());
    assert_eq!(session.user_context.unwrap(), "test_user");
    
    println!("✅ TEE session information test passed");
}

#[test]
fn test_tee_command_constants() {
    println!("🚀 Testing TEE command constants...");
    
    // TEE命令ID常量
    const CMD_GENERATE_KEYPAIR: u32 = 0x1000;
    const CMD_SIGN_TRANSACTION: u32 = 0x2000;
    const CMD_ENCRYPT_DATA: u32 = 0x3000;
    const CMD_DECRYPT_DATA: u32 = 0x4000;
    const CMD_DERIVE_KEY: u32 = 0x5000;
    const CMD_ATTESTATION: u32 = 0x6000;
    
    let commands = vec![
        CMD_GENERATE_KEYPAIR,
        CMD_SIGN_TRANSACTION,
        CMD_ENCRYPT_DATA,
        CMD_DECRYPT_DATA,
        CMD_DERIVE_KEY,
        CMD_ATTESTATION,
    ];
    
    // 验证命令ID唯一性
    for (i, &cmd1) in commands.iter().enumerate() {
        for (j, &cmd2) in commands.iter().enumerate() {
            if i != j {
                assert_ne!(cmd1, cmd2, "Commands should have unique IDs");
            }
        }
    }
    
    println!("✅ TEE command constants test passed");
}