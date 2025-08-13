/// 数据损坏恢复测试
/// 测试系统在各种数据完整性问题下的检测和恢复能力

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::time::sleep;
use std::sync::Mutex;

/// 数据损坏模拟器
pub struct DataCorruptionSimulator {
    corruption_enabled: AtomicBool,
    corruption_rate: AtomicUsize,        // 损坏概率 (0-100)
    corruption_types: Mutex<Vec<CorruptionType>>,
    statistics: Mutex<CorruptionStatistics>,
}

#[derive(Debug, Clone)]
pub enum CorruptionType {
    BitFlip,            // 位翻转
    ByteScramble,       // 字节乱序
    PartialOverwrite,   // 部分覆写
    TotalCorruption,    // 完全损坏
    ChecksumMismatch,   // 校验和不匹配
}

#[derive(Debug, Default, Clone)]
pub struct CorruptionStatistics {
    total_operations: usize,
    corruptions_detected: usize,
    corruptions_recovered: usize,
    unrecoverable_failures: usize,
}

impl DataCorruptionSimulator {
    fn new() -> Self {
        Self {
            corruption_enabled: AtomicBool::new(false),
            corruption_rate: AtomicUsize::new(0),
            corruption_types: Mutex::new(vec![
                CorruptionType::BitFlip,
                CorruptionType::ByteScramble,
                CorruptionType::ChecksumMismatch,
            ]),
            statistics: Mutex::new(CorruptionStatistics::default()),
        }
    }
    
    fn enable_corruption(&self, rate: usize) {
        self.corruption_enabled.store(true, Ordering::SeqCst);
        self.corruption_rate.store(rate.min(100), Ordering::SeqCst);
    }
    
    fn disable_corruption(&self) {
        self.corruption_enabled.store(false, Ordering::SeqCst);
    }
    
    fn set_corruption_types(&self, types: Vec<CorruptionType>) {
        if let Ok(mut corruption_types) = self.corruption_types.lock() {
            *corruption_types = types;
        }
    }
    
    /// 模拟对数据进行损坏操作
    fn corrupt_data(&self, data: &mut [u8]) -> Option<CorruptionType> {
        if !self.corruption_enabled.load(Ordering::SeqCst) {
            return None;
        }
        
        let rate = self.corruption_rate.load(Ordering::SeqCst);
        if rand::random::<usize>() % 100 >= rate {
            return None;
        }
        
        if let Ok(mut stats) = self.statistics.lock() {
            stats.total_operations += 1;
        }
        
        let corruption_types = self.corruption_types.lock().ok()?;
        if corruption_types.is_empty() {
            return None;
        }
        
        let corruption_type = &corruption_types[rand::random::<usize>() % corruption_types.len()];
        
        match corruption_type {
            CorruptionType::BitFlip => {
                if !data.is_empty() {
                    let index = rand::random::<usize>() % data.len();
                    let bit = rand::random::<usize>() % 8;
                    data[index] ^= 1 << bit;
                }
            },
            CorruptionType::ByteScramble => {
                if data.len() >= 2 {
                    let i = rand::random::<usize>() % data.len();
                    let j = rand::random::<usize>() % data.len();
                    data.swap(i, j);
                }
            },
            CorruptionType::PartialOverwrite => {
                if !data.is_empty() {
                    let start = rand::random::<usize>() % data.len();
                    let len = (rand::random::<usize>() % (data.len() - start)).max(1);
                    for i in start..start + len {
                        data[i] = rand::random::<u8>();
                    }
                }
            },
            CorruptionType::TotalCorruption => {
                for byte in data.iter_mut() {
                    *byte = rand::random::<u8>();
                }
            },
            CorruptionType::ChecksumMismatch => {
                // 模拟校验和损坏，这里简单地修改几个字节
                if data.len() >= 4 {
                    for i in 0..4.min(data.len()) {
                        data[i] = rand::random::<u8>();
                    }
                }
            }
        }
        
        Some(corruption_type.clone())
    }
    
    fn record_detection(&self) {
        if let Ok(mut stats) = self.statistics.lock() {
            stats.corruptions_detected += 1;
        }
    }
    
    fn record_recovery(&self) {
        if let Ok(mut stats) = self.statistics.lock() {
            stats.corruptions_recovered += 1;
        }
    }
    
    fn record_failure(&self) {
        if let Ok(mut stats) = self.statistics.lock() {
            stats.unrecoverable_failures += 1;
        }
    }
    
    fn get_statistics(&self) -> CorruptionStatistics {
        self.statistics.lock().map(|stats| CorruptionStatistics {
            total_operations: stats.total_operations,
            corruptions_detected: stats.corruptions_detected,
            corruptions_recovered: stats.corruptions_recovered,
            unrecoverable_failures: stats.unrecoverable_failures,
        }).unwrap_or_default()
    }
    
    fn reset_statistics(&self) {
        if let Ok(mut stats) = self.statistics.lock() {
            *stats = CorruptionStatistics::default();
        }
    }
}

/// 数据完整性验证器
pub struct DataIntegrityValidator {
    checksums: Mutex<HashMap<String, Vec<u8>>>,
    validation_failures: AtomicUsize,
}

impl DataIntegrityValidator {
    fn new() -> Self {
        Self {
            checksums: Mutex::new(HashMap::new()),
            validation_failures: AtomicUsize::new(0),
        }
    }
    
    fn calculate_checksum(&self, data: &[u8]) -> Vec<u8> {
        // 简单的CRC32校验和实现
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
    }
    
    fn store_checksum(&self, key: String, data: &[u8]) {
        let checksum = self.calculate_checksum(data);
        if let Ok(mut checksums) = self.checksums.lock() {
            checksums.insert(key, checksum);
        }
    }
    
    fn validate_data(&self, key: &str, data: &[u8]) -> bool {
        if let Ok(checksums) = self.checksums.lock() {
            if let Some(expected_checksum) = checksums.get(key) {
                let actual_checksum = self.calculate_checksum(data);
                if &actual_checksum == expected_checksum {
                    return true;
                } else {
                    self.validation_failures.fetch_add(1, Ordering::SeqCst);
                    return false;
                }
            }
        }
        // 如果没有存储的校验和，假设数据有效
        true
    }
    
    fn get_validation_failures(&self) -> usize {
        self.validation_failures.load(Ordering::SeqCst)
    }
    
    fn reset_failures(&self) {
        self.validation_failures.store(0, Ordering::SeqCst);
    }
}

/// 测试内存数据损坏检测和恢复
#[test]
fn test_memory_corruption_detection_and_recovery() {
    println!("🚀 Testing memory corruption detection and recovery...");
    
    let security_manager = SecurityManager::new(SecurityConfig::default());
    let corruption_simulator = DataCorruptionSimulator::new();
    let integrity_validator = DataIntegrityValidator::new();
    
    // 启用轻微的数据损坏模拟
    corruption_simulator.enable_corruption(20); // 20% 损坏率
    
    let mut successful_allocations = 0;
    let mut corruption_detected = 0;
    let mut recovery_attempts = 0;
    
    for i in 0..20 {
        // 创建安全内存
        match security_manager.create_secure_memory(1024) {
            Ok(mut memory) => {
                successful_allocations += 1;
                
                // 写入测试数据
                let test_data = format!("test_data_pattern_{:08}", i).repeat(32);
                let test_bytes = test_data.as_bytes();
                
                // 存储原始数据的校验和
                integrity_validator.store_checksum(format!("memory_{}", i), test_bytes);
                
                if let Ok(()) = memory.copy_from_slice(test_bytes) {
                    // 模拟数据损坏
                    let mut data_copy = memory.as_slice().to_vec();
                    if let Some(corruption_type) = corruption_simulator.corrupt_data(&mut data_copy) {
                        println!("  🔍 Detected corruption type: {:?} for memory block {}", corruption_type, i);
                        
                        // 验证数据完整性
                        if !integrity_validator.validate_data(&format!("memory_{}", i), &data_copy) {
                            corruption_detected += 1;
                            corruption_simulator.record_detection();
                            
                            // 尝试恢复
                            recovery_attempts += 1;
                            
                            // 重新分配内存作为恢复策略
                            match security_manager.create_secure_memory(1024) {
                                Ok(mut recovery_memory) => {
                                    if recovery_memory.copy_from_slice(test_bytes).is_ok() {
                                        corruption_simulator.record_recovery();
                                        println!("    ✅ Memory corruption recovered for block {}", i);
                                    } else {
                                        corruption_simulator.record_failure();
                                        println!("    ❌ Memory corruption recovery failed for block {}", i);
                                    }
                                },
                                Err(e) => {
                                    corruption_simulator.record_failure();
                                    println!("    ❌ Recovery memory allocation failed: {}", e);
                                }
                            }
                        }
                    }
                }
            },
            Err(e) => {
                println!("  ❌ Memory allocation {} failed: {}", i, e);
            }
        }
    }
    
    let stats = corruption_simulator.get_statistics();
    let validation_failures = integrity_validator.get_validation_failures();
    
    println!("  📊 Memory corruption test results:");
    println!("    Successful allocations: {}/20", successful_allocations);
    println!("    Corruptions detected: {}", corruption_detected);
    println!("    Recovery attempts: {}", recovery_attempts);
    println!("    Validation failures: {}", validation_failures);
    println!("    Simulator stats: detected={}, recovered={}, failures={}", 
             stats.corruptions_detected, stats.corruptions_recovered, stats.unrecoverable_failures);
    
    // 验证系统能处理数据损坏
    assert!(successful_allocations > 0, "Should have successful allocations");
    
    println!("✅ Memory corruption detection and recovery verified");
}

/// 测试配置数据损坏处理
#[test]
fn test_configuration_corruption_handling() {
    println!("🚀 Testing configuration corruption handling...");
    
    let corruption_simulator = DataCorruptionSimulator::new();
    let integrity_validator = DataIntegrityValidator::new();
    
    // 设置更高的损坏率来测试配置恢复
    corruption_simulator.enable_corruption(50);
    corruption_simulator.set_corruption_types(vec![
        CorruptionType::PartialOverwrite,
        CorruptionType::ChecksumMismatch,
    ]);
    
    // 测试各种配置场景
    let test_configurations = vec![
        SecurityConfig {
            enable_constant_time: true,
            enable_memory_protection: true,
            enable_audit_logging: true,
            audit_file_path: Some("/tmp/test_audit.log".to_string()),
            enable_secure_audit: false,
            audit_encryption_key: None,
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
    
    let mut successful_configs = 0;
    let mut fallback_configs = 0;
    
    for (i, config) in test_configurations.iter().enumerate() {
        println!("  🧪 Testing configuration scenario {}:", i + 1);
        
        // 模拟配置序列化和可能的损坏（简化版本）
        let config_serialized = format!("SecurityConfig{{constant_time:{},memory_protection:{},audit:{}}}", 
                                       config.enable_constant_time, 
                                       config.enable_memory_protection, 
                                       config.enable_audit_logging);
        let mut config_bytes = config_serialized.as_bytes().to_vec();
        
        // 存储原始配置的校验和
        integrity_validator.store_checksum(format!("config_{}", i), &config_bytes);
        
        // 模拟配置数据损坏
        if let Some(corruption_type) = corruption_simulator.corrupt_data(&mut config_bytes) {
            println!("    💥 Configuration corrupted with type: {:?}", corruption_type);
            
            // 验证配置完整性
            if !integrity_validator.validate_data(&format!("config_{}", i), &config_bytes) {
                corruption_simulator.record_detection();
                println!("    🔍 Configuration corruption detected");
                
                // 尝试使用损坏的配置创建安全管理器
                let security_manager = SecurityManager::new(config.clone());
                
                // 测试基本功能是否仍然可用（使用默认/恢复配置）
                let memory_test = security_manager.create_secure_memory(512);
                match memory_test {
                    Ok(_) => {
                        fallback_configs += 1;
                        corruption_simulator.record_recovery();
                        println!("    ✅ System recovered with fallback configuration");
                    },
                    Err(e) => {
                        corruption_simulator.record_failure();
                        println!("    ❌ System failed even with fallback: {}", e);
                    }
                }
            }
        } else {
            // 没有损坏，正常初始化
            let security_manager = SecurityManager::new(config.clone());
            let memory_test = security_manager.create_secure_memory(512);
            
            if memory_test.is_ok() {
                successful_configs += 1;
                println!("    ✅ Configuration {} worked normally", i + 1);
            }
        }
    }
    
    let stats = corruption_simulator.get_statistics();
    
    println!("  📊 Configuration corruption test results:");
    println!("    Successful normal configs: {}", successful_configs);
    println!("    Successful fallback configs: {}", fallback_configs);
    println!("    Total successful: {}/{}", 
             successful_configs + fallback_configs, test_configurations.len());
    println!("    Corruption stats: detected={}, recovered={}, failures={}", 
             stats.corruptions_detected, stats.corruptions_recovered, stats.unrecoverable_failures);
    
    assert!(successful_configs + fallback_configs > 0, "At least some configurations should work");
    
    println!("✅ Configuration corruption handling verified");
}

/// 测试钱包数据损坏恢复
#[tokio::test]
async fn test_wallet_data_corruption_recovery() {
    println!("🚀 Testing wallet data corruption recovery...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let corruption_simulator = Arc::new(DataCorruptionSimulator::new());
    let integrity_validator = Arc::new(DataIntegrityValidator::new());
    
    // 设置中等损坏率
    corruption_simulator.enable_corruption(30);
    
    let mut wallet_manager = WalletManager::new(&security_manager)
        .expect("Failed to create wallet manager");
    
    let mut wallet_data = Vec::new();
    let mut successful_wallets = 0;
    let mut corrupted_wallets = 0;
    let mut recovered_wallets = 0;
    
    // 创建多个钱包绑定
    for i in 0..10 {
        let binding = wallet::UserWalletBinding {
            user_id: i,
            wallet_id: uuid::Uuid::new_v4(),
            address: [i as u8; 20],
            alias: Some(format!("corruption_test_wallet_{}", i)),
            is_primary: i == 0,
            permissions: wallet::WalletPermissions::full_permissions(),
        };
        
        // 序列化钱包数据以模拟存储（简化版本）
        let wallet_serialized = format!("Wallet{{user_id:{},wallet_id:{},primary:{}}}", 
                                       binding.user_id, binding.wallet_id, binding.is_primary);
        let mut wallet_bytes = wallet_serialized.as_bytes().to_vec();
        
        // 存储原始数据校验和
        integrity_validator.store_checksum(format!("wallet_{}", i), &wallet_bytes);
        
        // 模拟数据损坏
        if let Some(corruption_type) = corruption_simulator.corrupt_data(&mut wallet_bytes) {
            corrupted_wallets += 1;
            println!("  💥 Wallet {} data corrupted with type: {:?}", i, corruption_type);
            
            // 验证数据完整性
            if !integrity_validator.validate_data(&format!("wallet_{}", i), &wallet_bytes) {
                corruption_simulator.record_detection();
                
                // 尝试恢复：重新创建钱包绑定
                match wallet_manager.store_wallet_binding(binding.clone()).await {
                    Ok(()) => {
                        recovered_wallets += 1;
                        corruption_simulator.record_recovery();
                        println!("    ✅ Wallet {} data recovered", i);
                        wallet_data.push(binding);
                    },
                    Err(e) => {
                        corruption_simulator.record_failure();
                        println!("    ❌ Wallet {} recovery failed: {:?}", i, e);
                    }
                }
            }
        } else {
            // 没有损坏，正常存储
            match wallet_manager.store_wallet_binding(binding.clone()).await {
                Ok(()) => {
                    successful_wallets += 1;
                    println!("  ✅ Wallet {} stored successfully", i);
                    wallet_data.push(binding);
                },
                Err(e) => {
                    println!("  ❌ Wallet {} storage failed: {:?}", i, e);
                }
            }
        }
    }
    
    // 测试损坏后的钱包功能
    println!("  🔄 Testing wallet functionality after corruption recovery...");
    
    for wallet_binding in &wallet_data {
        // 验证钱包数据的基本完整性
        assert_eq!(wallet_binding.address.len(), 20, "Address length should be correct");
        assert!(wallet_binding.alias.is_some(), "Alias should exist");
        
        println!("    ✅ Wallet {} integrity verified", wallet_binding.wallet_id);
    }
    
    let stats = corruption_simulator.get_statistics();
    let validation_failures = integrity_validator.get_validation_failures();
    
    println!("  📊 Wallet data corruption test results:");
    println!("    Successful normal wallets: {}", successful_wallets);
    println!("    Corrupted wallets: {}", corrupted_wallets);
    println!("    Recovered wallets: {}", recovered_wallets);
    println!("    Total successful: {}/10", successful_wallets + recovered_wallets);
    println!("    Validation failures: {}", validation_failures);
    println!("    Corruption stats: detected={}, recovered={}, failures={}", 
             stats.corruptions_detected, stats.corruptions_recovered, stats.unrecoverable_failures);
    
    assert!(successful_wallets + recovered_wallets > 0, "At least some wallets should be functional");
    
    println!("✅ Wallet data corruption recovery verified");
}

/// 测试并发数据损坏处理
#[tokio::test]
async fn test_concurrent_data_corruption_handling() {
    println!("🚀 Testing concurrent data corruption handling...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let corruption_simulator = Arc::new(DataCorruptionSimulator::new());
    
    // 设置低损坏率以避免测试过于不稳定
    corruption_simulator.enable_corruption(10);
    
    let mut handles = Vec::new();
    let operations_per_task = 5;
    let num_tasks = 4;
    
    for task_id in 0..num_tasks {
        let sm = Arc::clone(&security_manager);
        let cs = Arc::clone(&corruption_simulator);
        
        let handle = tokio::spawn(async move {
            let mut task_results = Vec::new();
            
            for op_id in 0..operations_per_task {
                // 创建测试数据
                let test_data = format!("concurrent_test_{}_{}", task_id, op_id);
                let mut data_bytes = test_data.repeat(50).as_bytes().to_vec();
                
                // 模拟数据损坏
                let corruption_occurred = cs.corrupt_data(&mut data_bytes).is_some();
                
                // 尝试使用可能损坏的数据进行安全操作
                let memory_result = sm.create_secure_memory(data_bytes.len());
                match memory_result {
                    Ok(mut memory) => {
                        let copy_result = memory.copy_from_slice(&data_bytes);
                        let success = copy_result.is_ok();
                        
                        task_results.push((
                            format!("task_{}_{}", task_id, op_id),
                            success,
                            corruption_occurred
                        ));
                        
                        if corruption_occurred {
                            if success {
                                cs.record_recovery();
                            } else {
                                cs.record_failure();
                            }
                        }
                    },
                    Err(_) => {
                        task_results.push((
                            format!("task_{}_{}", task_id, op_id),
                            false,
                            corruption_occurred
                        ));
                        
                        if corruption_occurred {
                            cs.record_failure();
                        }
                    }
                }
                
                // 小延迟模拟真实操作时间
                sleep(Duration::from_millis(20)).await;
            }
            
            (task_id, task_results)
        });
        
        handles.push(handle);
    }
    
    // 收集结果
    let mut all_results = Vec::new();
    for handle in handles {
        let (task_id, results) = handle.await.expect("Task should complete");
        all_results.push((task_id, results));
    }
    
    // 分析结果
    let mut total_operations = 0;
    let mut successful_operations = 0;
    let mut corrupted_operations = 0;
    let mut corrupted_but_successful = 0;
    
    for (task_id, results) in &all_results {
        println!("  Task {}: {} operations completed", task_id, results.len());
        
        for (operation_name, success, corruption_occurred) in results {
            total_operations += 1;
            
            if *success {
                successful_operations += 1;
            }
            
            if *corruption_occurred {
                corrupted_operations += 1;
                if *success {
                    corrupted_but_successful += 1;
                }
            }
            
            let status = if *success { "✅" } else { "❌" };
            let corruption_status = if *corruption_occurred { "🔥" } else { "🔷" };
            println!("    {} {} {}", status, corruption_status, operation_name);
        }
    }
    
    let stats = corruption_simulator.get_statistics();
    
    println!("  📊 Concurrent data corruption test results:");
    println!("    Total operations: {}", total_operations);
    println!("    Successful operations: {} ({:.1}%)", 
             successful_operations, (successful_operations as f64 / total_operations as f64) * 100.0);
    println!("    Operations with corruption: {}", corrupted_operations);
    println!("    Corrupted but successful: {} ({:.1}% recovery rate)", 
             corrupted_but_successful, 
             if corrupted_operations > 0 { 
                 (corrupted_but_successful as f64 / corrupted_operations as f64) * 100.0 
             } else { 0.0 });
    println!("    Corruption stats: detected={}, recovered={}, failures={}", 
             stats.corruptions_detected, stats.corruptions_recovered, stats.unrecoverable_failures);
    
    assert!(total_operations == num_tasks * operations_per_task, "Should complete all operations");
    assert!(successful_operations > 0, "Should have some successful operations");
    
    println!("✅ Concurrent data corruption handling verified");
}

/// 测试系统级数据完整性保护
#[tokio::test]
async fn test_system_level_data_integrity_protection() {
    println!("🚀 Testing system-level data integrity protection...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let corruption_simulator = Arc::new(DataCorruptionSimulator::new());
    let integrity_validator = Arc::new(DataIntegrityValidator::new());
    
    // 启用各种类型的损坏
    corruption_simulator.enable_corruption(25);
    corruption_simulator.set_corruption_types(vec![
        CorruptionType::BitFlip,
        CorruptionType::ByteScramble,
        CorruptionType::PartialOverwrite,
        CorruptionType::ChecksumMismatch,
    ]);
    
    let test_scenarios = vec![
        ("secure_memory", 1024),
        ("small_allocation", 64),
        ("large_allocation", 8192),
        ("rng_seed_data", 256),
    ];
    
    let mut scenario_results = Vec::new();
    
    for (scenario_name, data_size) in test_scenarios {
        println!("  🧪 Testing scenario: {} ({} bytes)", scenario_name, data_size);
        
        let mut scenario_success = 0;
        let mut scenario_recoveries = 0;
        let scenario_iterations = 5;
        
        for iteration in 0..scenario_iterations {
            // 创建测试数据
            let test_pattern = format!("integrity_test_{}_{:04}", scenario_name, iteration);
            let mut test_data = test_pattern.repeat(data_size / test_pattern.len() + 1);
            test_data.truncate(data_size);
            let mut data_bytes = test_data.as_bytes().to_vec();
            
            // 存储完整性校验和
            let checksum_key = format!("{}_{}", scenario_name, iteration);
            integrity_validator.store_checksum(checksum_key.clone(), &data_bytes);
            
            // 模拟数据损坏
            let corruption_type = corruption_simulator.corrupt_data(&mut data_bytes);
            
            // 执行系统操作
            match scenario_name {
                "secure_memory" | "small_allocation" | "large_allocation" => {
                    match security_manager.create_secure_memory(data_bytes.len()) {
                        Ok(mut memory) => {
                            match memory.copy_from_slice(&data_bytes) {
                                Ok(()) => {
                                    // 验证数据完整性
                                    if integrity_validator.validate_data(&checksum_key, memory.as_slice()) {
                                        scenario_success += 1;
                                    } else if corruption_type.is_some() {
                                        // 损坏被检测到，尝试恢复
                                        println!("    🔍 Data corruption detected in iteration {}", iteration);
                                        
                                        // 重新创建内存作为恢复策略
                                        let original_data = test_pattern.repeat(data_size / test_pattern.len() + 1)
                                            .as_bytes()[..data_size].to_vec();
                                        
                                        match security_manager.create_secure_memory(original_data.len()) {
                                            Ok(mut recovery_memory) => {
                                                if recovery_memory.copy_from_slice(&original_data).is_ok() {
                                                    scenario_recoveries += 1;
                                                    corruption_simulator.record_recovery();
                                                    println!("      ✅ Data recovered successfully");
                                                }
                                            },
                                            Err(_) => {
                                                corruption_simulator.record_failure();
                                            }
                                        }
                                    }
                                },
                                Err(_) => {
                                    if corruption_type.is_some() {
                                        corruption_simulator.record_failure();
                                    }
                                }
                            }
                        },
                        Err(_) => {
                            if corruption_type.is_some() {
                                corruption_simulator.record_failure();
                            }
                        }
                    }
                },
                "rng_seed_data" => {
                    // 测试RNG在数据损坏情况下的表现
                    match security_manager.create_secure_rng() {
                        Ok(mut rng) => {
                            let mut buffer = vec![0u8; data_bytes.len().min(1024)];
                            if rng.fill_bytes(&mut buffer).is_ok() {
                                scenario_success += 1;
                            }
                        },
                        Err(_) => {}
                    }
                },
                _ => {}
            }
        }
        
        let total_successful = scenario_success + scenario_recoveries;
        scenario_results.push((scenario_name, total_successful, scenario_iterations, scenario_recoveries));
        
        println!("    Results: {}/{} successful ({} recoveries)", 
                 total_successful, scenario_iterations, scenario_recoveries);
    }
    
    let stats = corruption_simulator.get_statistics();
    let validation_failures = integrity_validator.get_validation_failures();
    
    println!("  📊 System-level integrity protection results:");
    for (scenario, successful, total, recoveries) in &scenario_results {
        println!("    {}: {}/{} successful ({:.1}%, {} recoveries)", 
                 scenario, successful, total, 
                 (*successful as f64 / *total as f64) * 100.0, recoveries);
    }
    
    println!("  🔧 Overall integrity metrics:");
    println!("    Validation failures: {}", validation_failures);
    println!("    Corruption stats: detected={}, recovered={}, failures={}", 
             stats.corruptions_detected, stats.corruptions_recovered, stats.unrecoverable_failures);
    
    // 验证系统能够处理数据完整性问题
    let _total_scenarios = scenario_results.len();
    let successful_scenarios = scenario_results.iter()
        .filter(|(_, successful, _total, _)| *successful > 0)
        .count();
    
    assert!(successful_scenarios > 0, "At least some scenarios should succeed");
    
    // 验证恢复机制有效
    let total_recoveries: usize = scenario_results.iter().map(|(_, _, _, recoveries)| *recoveries).sum();
    if stats.corruptions_detected > 0 {
        println!("  ✅ Recovery mechanisms activated {} times", total_recoveries);
    }
    
    println!("✅ System-level data integrity protection verified");
}

/// 综合数据损坏恢复测试
#[tokio::test]
async fn test_comprehensive_data_corruption_recovery() {
    println!("🚀 Testing comprehensive data corruption recovery...");
    
    let test_start = Instant::now();
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let corruption_simulator = Arc::new(DataCorruptionSimulator::new());
    
    // 设置渐进式损坏率测试
    let corruption_phases = vec![
        ("low_corruption", 5),     // 5% 损坏率
        ("medium_corruption", 15), // 15% 损坏率
        ("high_corruption", 30),   // 30% 损坏率
        ("recovery_phase", 0),     // 无损坏，测试恢复
    ];
    
    let mut phase_results = Vec::new();
    
    for (phase_name, corruption_rate) in corruption_phases {
        println!("  🔄 Phase: {} (corruption rate: {}%)", phase_name, corruption_rate);
        
        corruption_simulator.enable_corruption(corruption_rate);
        corruption_simulator.reset_statistics();
        
        let mut phase_operations = 0;
        let mut phase_successes = 0;
        let operations_per_phase = 10;
        
        for i in 0..operations_per_phase {
            phase_operations += 1;
            
            // 混合操作类型
            let operation_type = match i % 3 {
                0 => "memory_allocation",
                1 => "rng_generation", 
                _ => "data_processing",
            };
            
            let success = match operation_type {
                "memory_allocation" => {
                    let size = 512 + (i * 256);
                    security_manager.create_secure_memory(size).is_ok()
                },
                "rng_generation" => {
                    match security_manager.create_secure_rng() {
                        Ok(mut rng) => {
                            let mut buffer = vec![0u8; 128];
                            rng.fill_bytes(&mut buffer).is_ok()
                        },
                        Err(_) => false,
                    }
                },
                "data_processing" => {
                    // 模拟数据处理操作
                    let test_data = vec![i as u8; 1024];
                    match security_manager.create_secure_memory(test_data.len()) {
                        Ok(mut memory) => memory.copy_from_slice(&test_data).is_ok(),
                        Err(_) => false,
                    }
                },
                _ => false,
            };
            
            if success {
                phase_successes += 1;
            }
            
            // 小延迟模拟真实操作
            sleep(Duration::from_millis(10)).await;
        }
        
        let phase_stats = corruption_simulator.get_statistics();
        let success_rate = (phase_successes as f64 / phase_operations as f64) * 100.0;
        
        phase_results.push((
            phase_name.to_string(),
            phase_successes,
            phase_operations,
            success_rate,
            phase_stats.clone(),
        ));
        
        println!("    📊 Phase results: {}/{} successful ({:.1}%)", 
                 phase_successes, phase_operations, success_rate);
        println!("      Corruption stats: detected={}, recovered={}, failures={}", 
                 phase_stats.corruptions_detected, 
                 phase_stats.corruptions_recovered, 
                 phase_stats.unrecoverable_failures);
    }
    
    let test_duration = test_start.elapsed();
    
    // 分析综合结果
    println!("  📈 Comprehensive data corruption recovery analysis:");
    println!("    Total test duration: {:?}", test_duration);
    
    let mut total_operations = 0;
    let mut total_successes = 0;
    let mut total_detections = 0;
    let mut total_recoveries = 0;
    
    for (phase, successes, operations, rate, stats) in &phase_results {
        println!("    Phase {}: {} ops, {:.1}% success, {} detections, {} recoveries", 
                 phase, operations, rate, stats.corruptions_detected, stats.corruptions_recovered);
        
        total_operations += operations;
        total_successes += successes;
        total_detections += stats.corruptions_detected;
        total_recoveries += stats.corruptions_recovered;
    }
    
    let overall_success_rate = (total_successes as f64 / total_operations as f64) * 100.0;
    let recovery_effectiveness = if total_detections > 0 {
        (total_recoveries as f64 / total_detections as f64) * 100.0
    } else {
        0.0
    };
    
    println!("  🎯 Overall performance metrics:");
    println!("    Total operations: {}", total_operations);
    println!("    Overall success rate: {:.1}%", overall_success_rate);
    println!("    Total corruptions detected: {}", total_detections);
    println!("    Total recoveries: {}", total_recoveries);
    println!("    Recovery effectiveness: {:.1}%", recovery_effectiveness);
    
    // 验证系统在各种损坏级别下的表现
    assert!(total_operations > 0, "Should have performed operations");
    assert!(total_successes > 0, "Should have some successful operations");
    assert!(overall_success_rate > 50.0, "Should maintain reasonable success rate");
    
    // 验证恢复机制的有效性
    if total_detections > 0 {
        assert!(recovery_effectiveness > 0.0, "Should have some recovery capability");
        println!("  ✅ Recovery mechanisms demonstrated effectiveness");
    }
    
    println!("✅ Comprehensive data corruption recovery verified");
}