use airaccount_core_logic::*;
use airaccount_core_logic::security::audit;
use std::sync::Arc;

#[test]
fn test_core_context_initialization() {
    let context = init_default().expect("Failed to initialize core context");
    assert!(context.is_initialized());
    assert!(context.validate().is_ok());
}

#[test]
fn test_security_manager_configuration() {
    let custom_config = SecurityConfig {
        enable_constant_time: true,
        enable_memory_protection: true,
        enable_audit_logging: false,
        audit_file_path: None,
        enable_secure_audit: false,
        audit_encryption_key: None,
    };
    
    let context = init_with_security_config(custom_config).expect("Failed to initialize with custom config");
    let security_manager = context.security_manager();
    
    assert!(security_manager.is_constant_time_enabled());
    assert!(security_manager.is_memory_protection_enabled());
    assert!(!security_manager.is_audit_logging_enabled());
}

#[test]
fn test_secure_memory_lifecycle() {
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    // 分配不同大小的安全内存
    let sizes = vec![64, 256, 1024, 4096];
    
    for size in sizes {
        let memory = security_manager.create_secure_memory(size)
            .expect(&format!("Failed to create secure memory of size {}", size));
        
        assert_eq!(memory.size(), size);
        
        // 内存会在离开作用域时自动清理
    }
}

#[test]
fn test_secure_rng_quality() {
    let context = init_default().expect("Failed to initialize");
    let security_manager = context.security_manager();
    
    let mut rng = security_manager.create_secure_rng()
        .expect("Failed to create secure RNG");
    
    // 生成多组随机数据并验证它们不相同
    let mut samples = Vec::new();
    for _ in 0..10 {
        let mut buffer = vec![0u8; 32];
        rng.fill_bytes(&mut buffer).expect("Failed to generate random bytes");
        samples.push(buffer);
    }
    
    // 验证所有样本都不相同
    for i in 0..samples.len() {
        for j in (i + 1)..samples.len() {
            assert_ne!(samples[i], samples[j], "Random samples should not be identical");
        }
    }
}

#[test]
fn test_constant_time_operations_correctness() {
    let data1 = SecureBytes::from_slice(b"test_data_12345");
    let data2 = SecureBytes::from_slice(b"test_data_12345");
    let data3 = SecureBytes::from_slice(b"different_data_");
    
    // 测试相等比较
    let eq_result = data1.constant_time_eq(&data2);
    assert!(bool::from(eq_result));
    
    // 测试不等比较
    let neq_result = data1.constant_time_eq(&data3);
    assert!(!bool::from(neq_result));
    
    // 测试条件选择 - subtle库的约定：当choice为true时选择第二个参数
    let selected = SecureBytes::conditional_select(&data3, &data1, eq_result);
    assert_eq!(selected.as_slice(), data1.as_slice());
}

#[test]
fn test_memory_protection_boundaries() {
    let mut memory = SecureMemory::new(100).expect("Failed to create secure memory");
    
    // 正常情况：数据适合缓冲区
    let small_data = b"small";
    assert!(memory.copy_from_slice(small_data).is_ok());
    assert_eq!(&memory.as_slice()[..small_data.len()], small_data);
    
    // 边界情况：数据恰好填满缓冲区
    let exact_data = vec![0x42u8; 100];
    assert!(memory.copy_from_slice(&exact_data).is_ok());
    assert_eq!(memory.as_slice(), &exact_data[..]);
    
    // 错误情况：数据超过缓冲区大小
    let large_data = vec![0x42u8; 101];
    assert!(memory.copy_from_slice(&large_data).is_err());
}

#[test]
fn test_stack_canary_protection() {
    let canary = StackCanary::new();
    let canary_value = canary.value();
    
    // 正确的金丝雀值应该验证成功
    assert!(canary.check(canary_value));
    
    // 错误的金丝雀值应该验证失败
    assert!(!canary.check(canary_value.wrapping_add(1)));
    assert!(!canary.check(0));
    assert!(!canary.check(u64::MAX));
}

#[test]
fn test_audit_logging_integration() {
    let mut logger = audit::AuditLogger::new();
    logger.add_sink(Arc::new(MockAuditSink::new()));
    
    // 记录各种类型的事件
    logger.log_info(
        audit::AuditEvent::MemoryAllocation { size: 1024, secure: true },
        "integration_test"
    );
    
    logger.log_security(
        audit::AuditEvent::KeyGeneration {
            algorithm: "ECDSA".to_string(),
            key_size: 256,
            operation: "integration_test_key".to_string(),
            key_type: "ECDSA_integration".to_string(),
            duration_ms: 30,
            entropy_bits: 256,
        },
        "integration_test"
    );
    
    logger.log_error(
        audit::AuditEvent::SecurityViolation {
            violation_type: "test_violation".to_string(),
            details: "This is a test violation".to_string(),
        },
        "integration_test"
    );
    
    // 验证事件被正确记录
    let security_events = logger.get_security_events(None);
    assert_eq!(security_events.len(), 2); // 安全事件和错误事件
    
    let component_events = logger.get_events_by_component("integration_test", None);
    assert_eq!(component_events.len(), 3); // 所有事件
}

#[test]
fn test_secure_string_operations() {
    let str1 = SecureString::new("password123").expect("Failed to create secure string");
    let str2 = SecureString::new("password123").expect("Failed to create secure string");
    let str3 = SecureString::new("different").expect("Failed to create secure string");
    
    // 测试安全相等比较
    assert!(str1.secure_eq(&str2));
    assert!(!str1.secure_eq(&str3));
    
    // 测试长度
    assert_eq!(str1.len(), 11);
    assert_eq!(str3.len(), 9);
    assert!(!str1.is_empty());
    
    // 测试字节访问
    assert_eq!(str1.as_bytes(), b"password123");
    assert_eq!(str3.as_bytes(), b"different");
}

#[test]
fn test_memory_guard_protection() {
    // 启用保护
    MemoryGuard::enable_protection();
    assert!(MemoryGuard::is_protection_enabled());
    
    // 测试边界检查
    let test_ptr = &42u8 as *const u8;
    
    // 正常访问应该成功
    assert!(MemoryGuard::check_bounds(test_ptr, 50, 100).is_ok());
    
    // 越界访问应该失败
    assert!(MemoryGuard::check_bounds(test_ptr, 150, 100).is_err());
    
    // 空指针访问应该失败
    assert!(MemoryGuard::check_bounds(std::ptr::null(), 10, 100).is_err());
    
    // 测试禁用保护
    MemoryGuard::disable_protection();
    assert!(!MemoryGuard::is_protection_enabled());
    
    // 禁用后应该跳过检查
    assert!(MemoryGuard::check_bounds(test_ptr, 150, 100).is_ok());
    
    // 恢复保护状态
    MemoryGuard::enable_protection();
}

#[test]
fn test_error_handling_and_recovery() {
    // 测试无效配置的错误处理
    let invalid_memory = SecureMemory::new(0);
    assert!(invalid_memory.is_err());
    
    // 测试上下文验证
    let context = init_default().expect("Failed to initialize");
    assert!(context.validate().is_ok());
    
    // 测试安全不变式验证
    let security_manager = context.security_manager();
    assert!(security_manager.validate_security_invariants().is_ok());
}

#[test]
fn test_concurrent_operations() {
    use std::thread;
    use std::sync::Arc;
    
    let context = Arc::new(init_default().expect("Failed to initialize"));
    let mut handles = Vec::new();
    
    // 启动多个线程进行并发操作
    for i in 0..4 {
        let context_clone = Arc::clone(&context);
        let handle = thread::spawn(move || {
            let security_manager = context_clone.security_manager();
            
            // 并发分配内存
            for j in 0..10 {
                let size = 64 + j * 32;
                let _memory = security_manager.create_secure_memory(size)
                    .expect(&format!("Thread {} failed to create memory of size {}", i, size));
            }
            
            // 并发生成随机数
            let mut rng = security_manager.create_secure_rng()
                .expect(&format!("Thread {} failed to create RNG", i));
            
            for _ in 0..10 {
                let mut buffer = vec![0u8; 32];
                rng.fill_bytes(&mut buffer)
                    .expect(&format!("Thread {} failed to generate random bytes", i));
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

// Mock audit sink for testing
struct MockAuditSink {
    entries: Arc<std::sync::Mutex<Vec<audit::AuditLogEntry>>>,
}

impl MockAuditSink {
    fn new() -> Self {
        Self {
            entries: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
    
    #[allow(dead_code)]
    fn get_entries(&self) -> Vec<audit::AuditLogEntry> {
        self.entries.lock().unwrap().clone()
    }
}

impl audit::AuditSink for MockAuditSink {
    fn log_entry(&self, entry: &audit::AuditLogEntry) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.entries.lock().unwrap().push(entry.clone());
        Ok(())
    }
    
    fn flush(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}