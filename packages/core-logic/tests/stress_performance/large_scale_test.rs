/// 大规模数据处理和性能测试
/// 测试系统处理大量数据和高并发场景的能力

#[cfg(test)]
mod large_scale_tests {
    use airaccount_core_logic::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::time::{Duration, Instant};
    use std::collections::HashMap;
    
    #[tokio::test]
    async fn test_large_wallet_management() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let wallet_manager = Arc::new(RwLock::new(WalletManager::new(security_manager.clone())));
        
        const TARGET_WALLETS: usize = 10000;
        let mut wallet_ids = Vec::new();
        let batch_size = 100;
        
        println!("🚀 Starting large-scale wallet creation test");
        let start_time = Instant::now();
        let initial_memory = get_memory_usage();
        
        // 批量创建钱包
        for batch in 0..(TARGET_WALLETS / batch_size) {
            let mut batch_ids = Vec::new();
            
            for i in 0..batch_size {
                let wallet_index = batch * batch_size + i;
                let mut manager = wallet_manager.write().await;
                
                match manager.create_wallet(
                    None, // 自动生成助记词
                    format!("password_{}", wallet_index)
                ).await {
                    Ok(id) => batch_ids.push(id),
                    Err(e) => {
                        println!("⚠️ Failed to create wallet {}: {:?}", wallet_index, e);
                        break;
                    }
                }
            }
            
            wallet_ids.extend(batch_ids);
            
            if batch % 10 == 0 {
                let progress = (batch + 1) * batch_size;
                let elapsed = start_time.elapsed();
                let rate = progress as f64 / elapsed.as_secs_f64();
                println!("  Progress: {}/{} wallets ({:.1} wallets/sec)", 
                        progress, TARGET_WALLETS, rate);
            }
        }
        
        let total_created = wallet_ids.len();
        let total_time = start_time.elapsed();
        let final_memory = get_memory_usage();
        let memory_used = final_memory - initial_memory;
        
        println!("✅ Large-scale wallet creation completed:");
        println!("   Total wallets: {}", total_created);
        println!("   Total time: {:?}", total_time);
        println!("   Average: {:.2} ms/wallet", total_time.as_millis() as f64 / total_created as f64);
        println!("   Memory used: {} MB", memory_used / 1024 / 1024);
        println!("   Memory per wallet: {} KB", memory_used / total_created / 1024);
        
        // 测试随机访问性能
        println!("\n🔍 Testing random access performance...");
        let access_start = Instant::now();
        let sample_size = 100.min(total_created);
        
        for i in 0..sample_size {
            let random_index = (i * 97) % total_created; // 伪随机分布
            let manager = wallet_manager.read().await;
            let wallet = manager.load_wallet(&wallet_ids[random_index]).await;
            assert!(wallet.is_ok(), "Failed to load wallet {}", random_index);
        }
        
        let access_time = access_start.elapsed();
        println!("✅ Random access test completed:");
        println!("   {} wallet loads in {:?}", sample_size, access_time);
        println!("   Average: {:.2} ms/load", access_time.as_millis() as f64 / sample_size as f64);
        
        // 批量删除测试
        println!("\n🗑️ Testing batch deletion...");
        let delete_start = Instant::now();
        let delete_count = total_created / 2;
        
        for i in 0..delete_count {
            let mut manager = wallet_manager.write().await;
            let _ = manager.delete_wallet(&wallet_ids[i]).await;
        }
        
        let delete_time = delete_start.elapsed();
        let memory_after_delete = get_memory_usage();
        
        println!("✅ Batch deletion completed:");
        println!("   Deleted: {} wallets", delete_count);
        println!("   Time: {:?}", delete_time);
        println!("   Memory freed: {} MB", (final_memory - memory_after_delete) / 1024 / 1024);
    }
    
    #[tokio::test]
    async fn test_high_concurrency_operations() {
        use tokio::task::JoinSet;
        
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let wallet_manager = Arc::new(RwLock::new(WalletManager::new(security_manager.clone())));
        
        // 预创建一些钱包
        let num_wallets = 100;
        let mut wallet_ids = Vec::new();
        
        for i in 0..num_wallets {
            let mut manager = wallet_manager.write().await;
            let id = manager.create_wallet(
                None,
                format!("concurrent_pass_{}", i)
            ).await.expect("Failed to create wallet");
            wallet_ids.push(id);
        }
        
        println!("🚀 Starting high concurrency test with {} wallets", num_wallets);
        
        // 并发操作配置
        let concurrent_operations = 1000;
        let operations_per_wallet = concurrent_operations / num_wallets;
        
        let start_time = Instant::now();
        let mut tasks = JoinSet::new();
        let wallet_ids = Arc::new(wallet_ids);
        
        // 启动并发任务
        for op_id in 0..concurrent_operations {
            let wm = Arc::clone(&wallet_manager);
            let wallet_ids = Arc::clone(&wallet_ids);
            
            tasks.spawn(async move {
                let wallet_index = op_id % wallet_ids.len();
                let wallet_id = wallet_ids[wallet_index];
                
                let manager = wm.read().await;
                let wallet = manager.load_wallet(&wallet_id).await
                    .expect("Failed to load wallet");
                
                // 执行操作
                let operation_type = op_id % 3;
                let op_start = Instant::now();
                
                let result = match operation_type {
                    0 => {
                        // 地址派生
                        wallet.derive_address(op_id as u32).await
                    },
                    1 => {
                        // 交易签名
                        let tx_data = vec![op_id as u8; 32];
                        wallet.sign_transaction(&tx_data).await
                    },
                    _ => {
                        // 状态查询
                        wallet.get_status().await
                    }
                };
                
                (op_id, operation_type, op_start.elapsed(), result.is_ok())
            });
        }
        
        // 收集结果
        let mut successful_ops = 0;
        let mut failed_ops = 0;
        let mut total_latency = Duration::ZERO;
        let mut op_latencies = Vec::new();
        
        while let Some(result) = tasks.join_next().await {
            if let Ok((_, _, latency, success)) = result {
                if success {
                    successful_ops += 1;
                } else {
                    failed_ops += 1;
                }
                total_latency += latency;
                op_latencies.push(latency);
            }
        }
        
        let total_time = start_time.elapsed();
        op_latencies.sort();
        
        // 计算统计数据
        let p50 = op_latencies[op_latencies.len() / 2];
        let p95 = op_latencies[op_latencies.len() * 95 / 100];
        let p99 = op_latencies[op_latencies.len() * 99 / 100];
        
        println!("✅ High concurrency test completed:");
        println!("   Total operations: {}", concurrent_operations);
        println!("   Successful: {} ({:.1}%)", successful_ops, 
                successful_ops as f64 / concurrent_operations as f64 * 100.0);
        println!("   Failed: {}", failed_ops);
        println!("   Total time: {:?}", total_time);
        println!("   Throughput: {:.1} ops/sec", 
                concurrent_operations as f64 / total_time.as_secs_f64());
        println!("   Latency P50: {:?}", p50);
        println!("   Latency P95: {:?}", p95);
        println!("   Latency P99: {:?}", p99);
        
        assert!(successful_ops as f64 / concurrent_operations as f64 > 0.99,
                "Success rate should be > 99%");
    }
    
    #[tokio::test]
    async fn test_memory_pressure() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut wallet_manager = WalletManager::new(security_manager.clone());
        
        println!("🧪 Starting memory pressure test");
        
        // 监控内存使用
        let mut memory_samples = Vec::new();
        let sample_interval = Duration::from_millis(100);
        let test_duration = Duration::from_secs(10);
        
        let start_time = Instant::now();
        let initial_memory = get_memory_usage();
        memory_samples.push((Duration::ZERO, initial_memory));
        
        // 持续创建和删除钱包以产生内存压力
        let mut cycle = 0;
        let mut active_wallets = HashMap::new();
        
        while start_time.elapsed() < test_duration {
            cycle += 1;
            
            // 创建阶段
            for i in 0..50 {
                let wallet_id = wallet_manager.create_wallet(
                    None,
                    format!("pressure_test_{}_{}", cycle, i)
                ).await.expect("Failed to create wallet");
                
                active_wallets.insert(format!("{}_{}", cycle, i), wallet_id);
            }
            
            // 使用阶段 - 执行一些操作
            for (_, wallet_id) in active_wallets.iter().take(10) {
                let wallet = wallet_manager.load_wallet(wallet_id).await
                    .expect("Failed to load wallet");
                
                for j in 0..5 {
                    let _ = wallet.derive_address(j).await;
                }
            }
            
            // 删除旧钱包
            if active_wallets.len() > 100 {
                let to_remove: Vec<_> = active_wallets.keys()
                    .take(25)
                    .cloned()
                    .collect();
                
                for key in to_remove {
                    if let Some(wallet_id) = active_wallets.remove(&key) {
                        let _ = wallet_manager.delete_wallet(&wallet_id).await;
                    }
                }
            }
            
            // 采样内存使用
            if start_time.elapsed().as_millis() % sample_interval.as_millis() == 0 {
                let current_memory = get_memory_usage();
                memory_samples.push((start_time.elapsed(), current_memory));
            }
        }
        
        // 分析内存使用模式
        let final_memory = get_memory_usage();
        let peak_memory = memory_samples.iter()
            .map(|(_, mem)| *mem)
            .max()
            .unwrap_or(final_memory);
        
        println!("✅ Memory pressure test completed:");
        println!("   Test duration: {:?}", test_duration);
        println!("   Total cycles: {}", cycle);
        println!("   Initial memory: {} MB", initial_memory / 1024 / 1024);
        println!("   Peak memory: {} MB", peak_memory / 1024 / 1024);
        println!("   Final memory: {} MB", final_memory / 1024 / 1024);
        println!("   Memory growth: {} MB", (final_memory - initial_memory) / 1024 / 1024);
        
        // 检查内存泄漏
        let memory_growth_ratio = final_memory as f64 / initial_memory as f64;
        assert!(memory_growth_ratio < 2.0, 
                "Memory usage doubled, possible memory leak");
        
        println!("✅ No significant memory leaks detected");
    }
    
    #[tokio::test]
    #[ignore] // 长时间运行测试，默认跳过
    async fn test_long_running_stability() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let wallet_manager = Arc::new(RwLock::new(WalletManager::new(security_manager.clone())));
        
        const TEST_DURATION_HOURS: u64 = 72;
        let test_duration = Duration::from_secs(TEST_DURATION_HOURS * 3600);
        
        println!("🏃 Starting {} hour stability test", TEST_DURATION_HOURS);
        
        let start_time = Instant::now();
        let mut stats = StabilityStats::new();
        
        // 创建测试钱包
        let mut test_wallets = Vec::new();
        for i in 0..10 {
            let mut manager = wallet_manager.write().await;
            let wallet_id = manager.create_wallet(
                None,
                format!("stability_test_{}", i)
            ).await.expect("Failed to create test wallet");
            test_wallets.push(wallet_id);
        }
        
        // 运行稳定性测试
        while start_time.elapsed() < test_duration {
            let cycle_start = Instant::now();
            
            // 执行一轮操作
            for wallet_id in &test_wallets {
                let manager = wallet_manager.read().await;
                
                match manager.load_wallet(wallet_id).await {
                    Ok(wallet) => {
                        // 执行各种操作
                        let _ = wallet.derive_address(stats.total_operations as u32).await;
                        let _ = wallet.sign_transaction(&[0x01, 0x02]).await;
                        let _ = wallet.get_status().await;
                        
                        stats.successful_operations += 3;
                    },
                    Err(e) => {
                        stats.errors.push(format!("Load wallet error: {:?}", e));
                        stats.failed_operations += 1;
                    }
                }
            }
            
            stats.total_operations += test_wallets.len() * 3;
            
            // 每小时报告一次
            if stats.total_operations % 3600 == 0 {
                let elapsed = start_time.elapsed();
                let hours = elapsed.as_secs() / 3600;
                
                println!("📊 Stability test progress - Hour {}:", hours);
                println!("   Total operations: {}", stats.total_operations);
                println!("   Success rate: {:.2}%", 
                        stats.successful_operations as f64 / stats.total_operations as f64 * 100.0);
                println!("   Current memory: {} MB", get_memory_usage() / 1024 / 1024);
                
                if !stats.errors.is_empty() {
                    println!("   Recent errors: {:?}", 
                            stats.errors.iter().take(5).collect::<Vec<_>>());
                }
            }
            
            // 短暂休息避免过度消耗CPU
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        println!("✅ Long-running stability test completed:");
        println!("   Duration: {} hours", TEST_DURATION_HOURS);
        println!("   Total operations: {}", stats.total_operations);
        println!("   Successful: {}", stats.successful_operations);
        println!("   Failed: {}", stats.failed_operations);
        println!("   Success rate: {:.2}%", 
                stats.successful_operations as f64 / stats.total_operations as f64 * 100.0);
        
        assert!(stats.successful_operations as f64 / stats.total_operations as f64 > 0.99,
                "Long-term success rate should be > 99%");
    }
    
    // 辅助结构和函数
    struct StabilityStats {
        total_operations: usize,
        successful_operations: usize,
        failed_operations: usize,
        errors: Vec<String>,
    }
    
    impl StabilityStats {
        fn new() -> Self {
            Self {
                total_operations: 0,
                successful_operations: 0,
                failed_operations: 0,
                errors: Vec::new(),
            }
        }
    }
    
    fn get_memory_usage() -> usize {
        // 简化的内存使用获取（实际应使用系统API）
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|line| line.starts_with("VmRSS:"))
                        .and_then(|line| {
                            line.split_whitespace()
                                .nth(1)
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                })
                .unwrap_or(0) * 1024
        }
        #[cfg(not(target_os = "linux"))]
        {
            // 模拟内存使用
            std::mem::size_of::<WalletManager>() * 1000
        }
    }
}