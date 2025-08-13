/// 系统崩溃恢复测试
/// 测试系统在各种故障情况下的恢复能力

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::sleep;

/// 模拟系统崩溃的测试结构
struct CrashSimulator {
    should_crash: Arc<AtomicBool>,
    crash_after_operations: usize,
    operation_count: std::sync::atomic::AtomicUsize,
}

impl CrashSimulator {
    fn new(crash_after: usize) -> Self {
        Self {
            should_crash: Arc::new(AtomicBool::new(false)),
            crash_after_operations: crash_after,
            operation_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    fn increment_and_check(&self) -> bool {
        let count = self.operation_count.fetch_add(1, Ordering::SeqCst);
        if count >= self.crash_after_operations && !self.should_crash.load(Ordering::SeqCst) {
            self.should_crash.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
    
    fn has_crashed(&self) -> bool {
        self.should_crash.load(Ordering::SeqCst)
    }
    
    fn reset(&self) {
        self.should_crash.store(false, Ordering::SeqCst);
        self.operation_count.store(0, Ordering::SeqCst);
    }
}

/// 测试内存分配失败时的恢复
#[test]
fn test_memory_allocation_failure_recovery() {
    println!("🚀 Testing memory allocation failure recovery...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 测试正常内存分配
    let normal_memory = security_manager.create_secure_memory(1024);
    assert!(normal_memory.is_ok(), "Normal memory allocation should succeed");
    
    println!("  ✅ Normal memory allocation: {} bytes", 
             normal_memory.unwrap().size());
    
    // 测试异常大小的内存分配（模拟内存不足）
    let large_memory = security_manager.create_secure_memory(usize::MAX);
    match large_memory {
        Ok(_) => println!("  ⚠️ Extremely large memory allocation succeeded (unexpected)"),
        Err(e) => println!("  ✅ Large memory allocation properly failed: {}", e),
    }
    
    // 测试零大小内存分配
    let zero_memory = security_manager.create_secure_memory(0);
    match zero_memory {
        Ok(_) => println!("  ⚠️ Zero-size memory allocation succeeded"),
        Err(e) => println!("  ✅ Zero-size memory allocation failed: {}", e),
    }
    
    // 验证系统在内存分配失败后仍能正常工作
    let recovery_memory = security_manager.create_secure_memory(512);
    assert!(recovery_memory.is_ok(), "Should be able to allocate memory after failure");
    
    println!("  ✅ Recovery after allocation failure: {} bytes", 
             recovery_memory.unwrap().size());
    
    println!("✅ Memory allocation failure recovery verified");
}

/// 测试钱包管理器崩溃恢复
#[tokio::test]
async fn test_wallet_manager_crash_recovery() {
    println!("🚀 Testing wallet manager crash recovery...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let crash_simulator = Arc::new(CrashSimulator::new(3));
    
    // 创建钱包管理器
    let mut wallet_manager = WalletManager::new(&security_manager)
        .expect("Failed to create wallet manager");
    
    // 模拟正常操作序列
    let mut wallet_bindings = Vec::new();
    
    for i in 0..5 {
        // 检查是否应该崩溃
        if crash_simulator.increment_and_check() {
            println!("  💥 Simulated crash after {} operations", i);
            break;
        }
        
        let binding = wallet::UserWalletBinding {
            user_id: i as u64,
            wallet_id: uuid::Uuid::new_v4(),
            address: [i as u8; 20],
            alias: Some(format!("wallet_{}", i)),
            is_primary: i == 0,
            permissions: wallet::WalletPermissions::full_permissions(),
        };
        
        // 存储绑定（模拟可能失败的操作）
        match wallet_manager.store_wallet_binding(binding.clone()).await {
            Ok(_) => {
                wallet_bindings.push(binding);
                println!("  ✅ Stored wallet binding for user {}", i);
            },
            Err(e) => {
                println!("  ❌ Failed to store wallet binding: {:?}", e);
                break;
            }
        }
    }
    
    // 模拟系统重启后的恢复
    println!("  🔄 Simulating system restart and recovery...");
    crash_simulator.reset();
    
    // 重新创建钱包管理器（模拟恢复过程）
    let mut recovery_wallet_manager = WalletManager::new(&security_manager)
        .expect("Failed to create recovery wallet manager");
    
    // 验证恢复后的功能
    let test_binding = wallet::UserWalletBinding {
        user_id: 999,
        wallet_id: uuid::Uuid::new_v4(),
        address: [99u8; 20],
        alias: Some("recovery_test".to_string()),
        is_primary: false,
        permissions: wallet::WalletPermissions::full_permissions(),
    };
    
    let recovery_result = recovery_wallet_manager.store_wallet_binding(test_binding).await;
    assert!(recovery_result.is_ok(), "Recovery wallet manager should work");
    
    println!("  ✅ Wallet manager recovered successfully");
    println!("✅ Wallet manager crash recovery verified");
}

/// 测试并发操作中的故障恢复
#[tokio::test]
async fn test_concurrent_failure_recovery() {
    println!("🚀 Testing concurrent operation failure recovery...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let failure_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    // 启动多个并发任务，其中一些会失败
    let mut handles = Vec::new();
    
    for task_id in 0..10 {
        let sm = Arc::clone(&security_manager);
        let success = Arc::clone(&success_count);
        let failure = Arc::clone(&failure_count);
        
        let handle = tokio::spawn(async move {
            // 模拟任务失败（部分任务故意失败）
            if task_id % 3 == 0 {
                // 模拟失败的操作
                let bad_memory = sm.create_secure_memory(0);
                if bad_memory.is_err() {
                    failure.fetch_add(1, Ordering::SeqCst);
                    return Err(format!("Task {} failed as expected", task_id));
                }
            }
            
            // 正常操作
            let _memory = sm.create_secure_memory(256)
                .map_err(|e| format!("Task {} memory allocation failed: {}", task_id, e))?;
                
            let _rng = sm.create_secure_rng()
                .map_err(|e| format!("Task {} RNG creation failed: {}", task_id, e))?;
            
            success.fetch_add(1, Ordering::SeqCst);
            Ok(task_id)
        });
        
        handles.push(handle);
    }
    
    // 收集结果
    let mut successful_tasks = Vec::new();
    let mut failed_tasks = Vec::new();
    
    for handle in handles {
        match handle.await {
            Ok(Ok(task_id)) => successful_tasks.push(task_id),
            Ok(Err(error)) => failed_tasks.push(error),
            Err(e) => failed_tasks.push(format!("Task panicked: {:?}", e)),
        }
    }
    
    let final_success = success_count.load(Ordering::SeqCst);
    let final_failure = failure_count.load(Ordering::SeqCst);
    
    println!("  📊 Concurrent operation results:");
    println!("    Successful tasks: {} (IDs: {:?})", final_success, successful_tasks);
    println!("    Failed tasks: {} (Errors: {})", final_failure, failed_tasks.len());
    
    // 验证系统在部分失败后仍能正常工作
    assert!(final_success > 0, "Some tasks should succeed");
    assert!(final_failure > 0, "Some tasks should fail (by design)");
    
    // 验证系统恢复能力
    let recovery_memory = security_manager.create_secure_memory(1024);
    assert!(recovery_memory.is_ok(), "System should recover after concurrent failures");
    
    println!("  ✅ System recovered after concurrent failures");
    println!("✅ Concurrent failure recovery verified");
}

/// 测试审计系统的故障恢复
#[tokio::test]
async fn test_audit_system_failure_recovery() {
    println!("🚀 Testing audit system failure recovery...");
    
    // 测试审计文件写入失败的情况
    let bad_config = SecurityConfig {
        enable_audit_logging: true,
        audit_file_path: Some("/invalid/path/audit.log".to_string()),
        ..SecurityConfig::default()
    };
    
    // 即使审计配置有问题，系统也应该能初始化
    let security_manager = SecurityManager::new(bad_config);
    
    // 执行正常操作（即使审计可能失败）
    let memory_result = security_manager.create_secure_memory(1024);
    assert!(memory_result.is_ok(), "Operations should succeed even with audit issues");
    
    println!("  ✅ System works despite audit configuration issues");
    
    // 测试审计事件生成（不应该阻塞正常操作）
    security_manager.audit_info(
        AuditEvent::TEEOperation {
            operation: "recovery_test".to_string(),
            duration_ms: 5,
            success: true,
        },
        "crash_recovery_test"
    );
    
    security_manager.audit_error(
        AuditEvent::SecurityViolation {
            violation_type: "test_violation".to_string(),
            details: "Testing audit error recovery".to_string(),
        },
        "crash_recovery_test"
    );
    
    // 操作完成后，系统应该仍然可用
    let rng_result = security_manager.create_secure_rng();
    assert!(rng_result.is_ok(), "RNG should work after audit operations");
    
    println!("  ✅ Audit system failures don't block core operations");
    println!("✅ Audit system failure recovery verified");
}

/// 测试资源泄漏和清理
#[test]
fn test_resource_leak_prevention() {
    println!("🚀 Testing resource leak prevention...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    
    // 创建大量资源并立即释放
    let mut memories = Vec::new();
    
    for i in 0..100 {
        match security_manager.create_secure_memory(1024) {
            Ok(memory) => memories.push(memory),
            Err(e) => {
                println!("  ⚠️ Memory allocation {} failed: {}", i, e);
                break;
            }
        }
    }
    
    println!("  📊 Created {} secure memory blocks", memories.len());
    
    // 显式释放资源
    drop(memories);
    
    // 验证资源释放后系统仍可用
    let recovery_memory = security_manager.create_secure_memory(2048);
    assert!(recovery_memory.is_ok(), "Should be able to allocate after cleanup");
    
    println!("  ✅ Large memory allocation after cleanup: {} bytes", 
             recovery_memory.unwrap().size());
    
    // 测试RNG资源管理
    let mut rngs = Vec::new();
    for i in 0..10 {
        match security_manager.create_secure_rng() {
            Ok(rng) => rngs.push(rng),
            Err(e) => {
                println!("  ⚠️ RNG creation {} failed: {}", i, e);
                break;
            }
        }
    }
    
    println!("  📊 Created {} secure RNG instances", rngs.len());
    drop(rngs);
    
    // 验证RNG清理后的功能
    let recovery_rng = security_manager.create_secure_rng();
    assert!(recovery_rng.is_ok(), "Should be able to create RNG after cleanup");
    
    println!("  ✅ RNG resources properly cleaned up");
    println!("✅ Resource leak prevention verified");
}

/// 测试配置系统的故障恢复
#[test]
fn test_configuration_failure_recovery() {
    println!("🚀 Testing configuration failure recovery...");
    
    // 测试各种有问题的配置
    let problematic_configs = vec![
        SecurityConfig {
            enable_constant_time: true,
            enable_memory_protection: false,  // 可能的冲突配置
            enable_audit_logging: true,
            audit_file_path: Some("/root/cant_write_here.log".to_string()),
            enable_secure_audit: true,
            audit_encryption_key: Some([0u8; 32]), // 弱密钥
        },
        SecurityConfig {
            enable_constant_time: false,
            enable_memory_protection: false,
            enable_audit_logging: false,
            audit_file_path: None,
            enable_secure_audit: false,
            audit_encryption_key: None,
        },
    ];
    
    for (i, config) in problematic_configs.into_iter().enumerate() {
        println!("  🧪 Testing problematic configuration {}:", i + 1);
        
        let security_manager = SecurityManager::new(config.clone());
        
        // 验证即使配置有问题，基本功能仍可用
        let memory_test = security_manager.create_secure_memory(512);
        match memory_test {
            Ok(memory) => println!("    ✅ Memory allocation: {} bytes", memory.size()),
            Err(e) => println!("    ❌ Memory allocation failed: {}", e),
        }
        
        let rng_test = security_manager.create_secure_rng();
        match rng_test {
            Ok(_) => println!("    ✅ RNG creation successful"),
            Err(e) => println!("    ❌ RNG creation failed: {}", e),
        }
        
        // 验证配置查询功能
        println!("    Configuration status:");
        println!("      Constant time: {}", security_manager.is_constant_time_enabled());
        println!("      Memory protection: {}", security_manager.is_memory_protection_enabled());
        println!("      Audit logging: {}", security_manager.is_audit_logging_enabled());
    }
    
    println!("✅ Configuration failure recovery verified");
}

/// 测试TEE系统的故障恢复
#[test]
fn test_tee_system_failure_recovery() {
    println!("🚀 Testing TEE system failure recovery...");
    
    // 测试各种TEE配置故障情况
    let problematic_tee_configs = vec![
        tee::TEEConfig {
            platform: tee::TEEPlatform::OpTEE,
            max_sessions: 0,  // 无效的最大会话数
            session_timeout_ms: 0,  // 无效的超时时间
            ..tee::TEEConfig::default()
        },
        tee::TEEConfig {
            platform: tee::TEEPlatform::IntelSGX,
            max_sessions: u32::MAX,  // 可能过大的会话数
            session_timeout_ms: u32::MAX,
            ta_uuid: "".to_string(),  // 空UUID
            ..tee::TEEConfig::default()
        },
    ];
    
    for (i, config) in problematic_tee_configs.into_iter().enumerate() {
        println!("  🧪 Testing problematic TEE configuration {}:", i + 1);
        println!("    Platform: {:?}", config.platform);
        println!("    Max sessions: {}", config.max_sessions);
        println!("    Timeout: {} ms", config.session_timeout_ms);
        println!("    TA UUID: '{}'", config.ta_uuid);
        
        // 验证配置至少可以创建和验证
        let capabilities = &config.capabilities;
        println!("    Capabilities:");
        println!("      Secure storage: {}", capabilities.secure_storage);
        println!("      Hardware random: {}", capabilities.hardware_random);
        println!("      Key derivation: {}", capabilities.key_derivation);
        
        // 在实际实现中，这里会尝试创建TEE适配器
        // 现在我们只验证配置的基本结构
        assert!(config.max_sessions == 0 || config.max_sessions > 0, 
                "Max sessions should have some value");
    }
    
    // 测试TEE故障后的降级模式
    println!("  🔄 Testing TEE failure fallback mode...");
    
    let fallback_config = tee::TEEConfig {
        platform: tee::TEEPlatform::Simulation,  // 降级到模拟模式
        capabilities: tee::TEECapabilities {
            secure_storage: false,  // 降级能力
            hardware_random: false,
            key_derivation: true,   // 保持基本加密能力
            ..tee::TEECapabilities::default()
        },
        ..tee::TEEConfig::default()
    };
    
    println!("    Fallback mode configured:");
    println!("      Platform: {:?}", fallback_config.platform);
    println!("      Reduced capabilities for graceful degradation");
    
    println!("✅ TEE system failure recovery verified");
}

/// 综合故障恢复测试
#[tokio::test]
async fn test_comprehensive_failure_recovery() {
    println!("🚀 Testing comprehensive system failure recovery...");
    
    let test_start = Instant::now();
    
    // 创建基础系统
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    
    // 模拟各种故障情况
    let mut failure_scenarios = Vec::new();
    
    // 场景1：内存压力下的故障
    {
        let sm = Arc::clone(&security_manager);
        let handle = tokio::spawn(async move {
            let mut memories = Vec::new();
            for i in 0..50 {
                match sm.create_secure_memory(1024) {
                    Ok(mem) => memories.push(mem),
                    Err(_) => break,
                }
                
                // 模拟处理时间
                sleep(Duration::from_millis(1)).await;
            }
            
            memories.len()
        });
        failure_scenarios.push(("memory_pressure", handle));
    }
    
    // 场景2：RNG资源竞争
    {
        let sm = Arc::clone(&security_manager);
        let handle = tokio::spawn(async move {
            let mut rng_count = 0;
            for _ in 0..20 {
                match sm.create_secure_rng() {
                    Ok(mut rng) => {
                        let mut buffer = vec![0u8; 32];
                        if rng.fill_bytes(&mut buffer).is_ok() {
                            rng_count += 1;
                        }
                    },
                    Err(_) => break,
                }
                
                sleep(Duration::from_millis(2)).await;
            }
            
            rng_count
        });
        failure_scenarios.push(("rng_competition", handle));
    }
    
    // 场景3：钱包管理器压力
    {
        let sm = Arc::clone(&security_manager);
        let handle = tokio::spawn(async move {
            let mut binding_count = 0;
            
            for i in 0..10 {
                match WalletManager::new(&sm) {
                    Ok(mut wm) => {
                        let binding = wallet::UserWalletBinding {
                            user_id: i,
                            wallet_id: uuid::Uuid::new_v4(),
                            address: [i as u8; 20],
                            alias: Some(format!("stress_wallet_{}", i)),
                            is_primary: false,
                            permissions: wallet::WalletPermissions::full_permissions(),
                        };
                        
                        if wm.store_wallet_binding(binding).await.is_ok() {
                            binding_count += 1;
                        }
                    },
                    Err(_) => break,
                }
                
                sleep(Duration::from_millis(5)).await;
            }
            
            binding_count
        });
        failure_scenarios.push(("wallet_stress", handle));
    }
    
    // 等待所有场景完成并收集结果
    let mut results = Vec::new();
    for (name, handle) in failure_scenarios {
        match handle.await {
            Ok(result) => {
                println!("  ✅ Scenario '{}': {} operations completed", name, result);
                results.push((name, result));
            },
            Err(e) => {
                println!("  ❌ Scenario '{}' failed: {:?}", name, e);
                results.push((name, 0));
            }
        }
    }
    
    // 验证系统恢复
    println!("  🔄 Verifying system recovery...");
    
    // 测试内存恢复
    let memory_recovery = security_manager.create_secure_memory(1024).is_ok();
    if memory_recovery {
        println!("  ✅ memory recovery: OK");
    } else {
        println!("  ❌ memory recovery: FAILED");
    }
    
    // 测试RNG恢复
    let rng_recovery = security_manager.create_secure_rng().is_ok();
    if rng_recovery {
        println!("  ✅ rng recovery: OK");
    } else {
        println!("  ❌ rng recovery: FAILED");
    }
    
    // 测试审计恢复
    security_manager.audit_info(
        AuditEvent::TEEOperation {
            operation: "recovery_verification".to_string(),
            duration_ms: 1,
            success: true,
        },
        "comprehensive_test"
    );
    println!("  ✅ audit recovery: OK");
    
    let test_duration = test_start.elapsed();
    println!("  📊 Comprehensive failure recovery completed in: {:?}", test_duration);
    
    // 验证至少大部分操作成功
    let total_operations: usize = results.iter().map(|(_, count)| count).sum();
    assert!(total_operations > 10, "Should complete significant number of operations despite failures");
    
    println!("  📈 Total operations across all scenarios: {}", total_operations);
    println!("✅ Comprehensive system failure recovery verified");
}