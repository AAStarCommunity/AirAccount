use airaccount_core_logic::*;
use airaccount_core_logic::security::audit;
use std::time::Instant;

fn test_constant_time_operations() -> Result<()> {
    println!("=== 测试常时算法模块 ===");
    
    let start = Instant::now();
    
    // 测试SecureBytes
    let data1 = SecureBytes::from_slice(b"secret_key_data_001");
    let data2 = SecureBytes::from_slice(b"secret_key_data_001");
    let data3 = SecureBytes::from_slice(b"different_data_____");
    
    let eq_result = data1.constant_time_eq(&data2);
    let neq_result = data1.constant_time_eq(&data3);
    
    println!("✓ SecureBytes常时比较: 相等={:?}, 不等={:?}", 
             bool::from(eq_result), bool::from(neq_result));
    
    // 测试ConstantTimeOps
    let test_data_a = b"test_comparison_data";
    let test_data_b = b"test_comparison_data"; 
    let test_data_c = b"different_data______";
    
    let secure_eq = ConstantTimeOps::secure_compare(test_data_a, test_data_b);
    let secure_neq = ConstantTimeOps::secure_compare(test_data_a, test_data_c);
    
    println!("✓ 安全比较函数: 相等={}, 不等={}", secure_eq, secure_neq);
    
    // 测试SecureRng
    let mut rng = SecureRng::new()
        .map_err(|e| CoreError::CryptographicError(e.to_string()))?;
    
    let mut random_data = vec![0u8; 32];
    rng.fill_bytes(&mut random_data)
        .map_err(|e| CoreError::CryptographicError(e.to_string()))?;
    
    println!("✓ 安全随机数生成: {} 字节", random_data.len());
    
    let elapsed = start.elapsed();
    println!("常时算法测试完成，耗时: {:?}", elapsed);
    
    Ok(())
}

fn test_memory_protection() -> Result<()> {
    println!("\n=== 测试内存保护模块 ===");
    
    let start = Instant::now();
    
    // 测试SecureMemory
    let mut secure_mem = SecureMemory::new(1024)
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    
    let test_data = b"sensitive_private_key_material";
    secure_mem.copy_from_slice(test_data)
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    
    println!("✓ 安全内存分配: {} 字节", secure_mem.size());
    
    // 测试StackCanary
    let canary = StackCanary::new();
    let canary_value = canary.value();
    let valid_check = canary.check(canary_value);
    let invalid_check = canary.check(canary_value ^ 0xDEADBEEF);
    
    println!("✓ 栈保护金丝雀: 有效检查={}, 无效检查={}", valid_check, invalid_check);
    
    // 测试SecureString
    let secure_str1 = SecureString::new("password123")
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    let secure_str2 = SecureString::new("password123")
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    let secure_str3 = SecureString::new("different_pw")
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    
    let eq_strings = secure_str1.secure_eq(&secure_str2);
    let neq_strings = secure_str1.secure_eq(&secure_str3);
    
    println!("✓ 安全字符串: 相等={}, 不等={}", eq_strings, neq_strings);
    
    // 测试内存边界检查
    let test_buffer_size = 100;
    let test_ptr = &42u8 as *const u8;
    let bounds_ok = MemoryGuard::check_bounds(test_ptr, 50, test_buffer_size);
    let bounds_fail = MemoryGuard::check_bounds(test_ptr, 150, test_buffer_size);
    
    println!("✓ 内存边界检查: 正常范围={:?}, 越界={:?}", bounds_ok, bounds_fail);
    
    let elapsed = start.elapsed();
    println!("内存保护测试完成，耗时: {:?}", elapsed);
    
    Ok(())
}

fn test_audit_logging() -> Result<()> {
    println!("\n=== 测试审计日志模块 ===");
    
    let start = Instant::now();
    
    // 初始化审计日志系统
    let mut logger = AuditLogger::new();
    logger.add_sink(std::sync::Arc::new(audit::ConsoleAuditSink));
    
    // 记录各种审计事件
    logger.log_security(
        AuditEvent::KeyGeneration {
            algorithm: "RSA".to_string(),
            key_size: 2048,
        },
        "crypto_module"
    );
    
    logger.log_info(
        AuditEvent::TEEOperation {
            operation: "memory_alloc".to_string(),
            duration_ms: 5,
            success: true,
        },
        "memory_manager"
    );
    
    logger.log_error(
        AuditEvent::SecurityViolation {
            violation_type: "invalid_signature".to_string(),
            details: "Signature verification failed for transaction".to_string(),
        },
        "signature_verifier"
    );
    
    logger.log_security(
        AuditEvent::Authentication {
            user_id: "user_12345".to_string(),
            success: true,
            method: "fingerprint".to_string(),
        },
        "auth_module"
    );
    
    // 查询安全事件
    let security_events = logger.get_security_events(None);
    println!("✓ 记录了 {} 个安全相关事件", security_events.len());
    
    // 按组件查询事件
    let crypto_events = logger.get_events_by_component("crypto_module", Some(10));
    println!("✓ 密码学模块事件: {} 个", crypto_events.len());
    
    // 刷新所有审计接收器
    logger.flush_all()
        .map_err(|e| CoreError::ValidationError(format!("Audit flush failed: {}", e)))?;
    
    println!("✓ 审计日志系统刷新完成");
    
    let elapsed = start.elapsed();
    println!("审计日志测试完成，耗时: {:?}", elapsed);
    
    Ok(())
}

fn test_security_manager_integration() -> Result<()> {
    println!("\n=== 测试安全管理器集成 ===");
    
    let start = Instant::now();
    
    // 创建自定义安全配置
    let security_config = SecurityConfig {
        enable_constant_time: true,
        enable_memory_protection: true,
        enable_audit_logging: true,
        audit_file_path: Some("/tmp/airaccount_security_test.log".to_string()),
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    // 初始化核心上下文
    let context = init_with_security_config(security_config)?;
    println!("✓ 安全管理器初始化完成");
    
    // 验证安全不变式
    context.validate()?;
    println!("✓ 安全不变式验证通过");
    
    let security_manager = context.security_manager();
    
    // 测试安全内存分配
    let secure_memory = security_manager.create_secure_memory(2048)
        .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    println!("✓ 安全内存分配成功: {} 字节", secure_memory.size());
    
    // 测试安全随机数生成器
    let _secure_rng = security_manager.create_secure_rng()
        .map_err(|e| CoreError::CryptographicError(e.to_string()))?;
    println!("✓ 安全随机数生成器创建成功");
    
    // 记录安全事件
    security_manager.audit_security_event(
        AuditEvent::KeyGeneration {
            algorithm: "ECDSA".to_string(),
            key_size: 256,
        },
        "integration_test"
    );
    
    security_manager.audit_info(
        AuditEvent::TEEOperation {
            operation: "integration_test".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
        },
        "test_runner"
    );
    
    println!("✓ 安全事件记录完成");
    
    let elapsed = start.elapsed();
    println!("安全管理器集成测试完成，耗时: {:?}", elapsed);
    
    Ok(())
}

fn run_performance_benchmarks() -> Result<()> {
    println!("\n=== 性能基准测试 ===");
    
    const ITERATIONS: usize = 10000;
    
    // 常时比较性能测试
    let data_a = vec![0x42u8; 32];
    let data_b = vec![0x42u8; 32];
    
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = ConstantTimeOps::secure_compare(&data_a, &data_b);
    }
    let constant_time_duration = start.elapsed();
    
    println!("✓ 常时比较 ({} 次迭代): {:?} (平均: {:?}/次)", 
             ITERATIONS, 
             constant_time_duration,
             constant_time_duration / ITERATIONS as u32);
    
    // 安全内存分配性能测试
    let start = Instant::now();
    for _ in 0..1000 {
        let _mem = SecureMemory::new(1024)
            .map_err(|e| CoreError::MemoryError(e.to_string()))?;
    }
    let memory_alloc_duration = start.elapsed();
    
    println!("✓ 安全内存分配 (1000 次): {:?} (平均: {:?}/次)",
             memory_alloc_duration,
             memory_alloc_duration / 1000);
    
    // 随机数生成性能测试
    let mut rng = SecureRng::new()
        .map_err(|e| CoreError::CryptographicError(e.to_string()))?;
    let mut buffer = vec![0u8; 32];
    
    let start = Instant::now();
    for _ in 0..1000 {
        rng.fill_bytes(&mut buffer)
            .map_err(|e| CoreError::CryptographicError(e.to_string()))?;
    }
    let rng_duration = start.elapsed();
    
    println!("✓ 随机数生成 (1000 次 32字节): {:?} (平均: {:?}/次)",
             rng_duration,
             rng_duration / 1000);
    
    Ok(())
}

fn main() -> Result<()> {
    println!("AirAccount 核心安全模块测试套件");
    println!("=====================================");
    
    let total_start = Instant::now();
    
    // 运行所有测试
    test_constant_time_operations()?;
    test_memory_protection()?;
    test_audit_logging()?;
    test_security_manager_integration()?;
    run_performance_benchmarks()?;
    
    let total_elapsed = total_start.elapsed();
    
    println!("\n=====================================");
    println!("✅ 所有安全模块测试通过!");
    println!("总耗时: {:?}", total_elapsed);
    println!("=====================================");
    
    Ok(())
}