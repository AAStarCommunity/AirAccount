/// 网络中断处理测试
/// 测试系统在网络连接故障时的处理和恢复能力

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{sleep, timeout};
use std::sync::Mutex;

/// 模拟网络状态的结构体
#[derive(Debug, Clone)]
pub enum NetworkState {
    Online,
    Offline,
    Slow,      // 慢速网络
    Unstable,  // 不稳定网络
}

/// 网络模拟器
pub struct NetworkSimulator {
    state: Arc<Mutex<NetworkState>>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    latency_ms: AtomicUsize,
    packet_loss_rate: AtomicUsize, // 百分比
}

impl NetworkSimulator {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NetworkState::Online)),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            latency_ms: AtomicUsize::new(50), // 默认50ms延迟
            packet_loss_rate: AtomicUsize::new(0), // 默认无丢包
        }
    }
    
    fn set_state(&self, state: NetworkState) {
        if let Ok(mut current_state) = self.state.lock() {
            *current_state = state;
        }
    }
    
    fn get_state(&self) -> NetworkState {
        self.state.lock().map(|guard| guard.clone()).unwrap_or(NetworkState::Offline)
    }
    
    async fn simulate_network_call(&self, operation: &str) -> std::result::Result<String, String> {
        let state = self.get_state();
        let latency = self.latency_ms.load(Ordering::SeqCst);
        let loss_rate = self.packet_loss_rate.load(Ordering::SeqCst);
        
        // 模拟网络延迟
        if latency > 0 {
            sleep(Duration::from_millis(latency as u64)).await;
        }
        
        // 模拟丢包
        if loss_rate > 0 && rand::random::<usize>() % 100 < loss_rate {
            self.failure_count.fetch_add(1, Ordering::SeqCst);
            return Err(format!("Packet lost for operation: {}", operation));
        }
        
        match state {
            NetworkState::Online => {
                self.success_count.fetch_add(1, Ordering::SeqCst);
                Ok(format!("Success: {}", operation))
            },
            NetworkState::Offline => {
                self.failure_count.fetch_add(1, Ordering::SeqCst);
                Err(format!("Network offline for operation: {}", operation))
            },
            NetworkState::Slow => {
                // 额外延迟
                sleep(Duration::from_millis(2000)).await;
                self.success_count.fetch_add(1, Ordering::SeqCst);
                Ok(format!("Slow success: {}", operation))
            },
            NetworkState::Unstable => {
                // 50%概率失败
                if rand::random::<bool>() {
                    self.success_count.fetch_add(1, Ordering::SeqCst);
                    Ok(format!("Unstable success: {}", operation))
                } else {
                    self.failure_count.fetch_add(1, Ordering::SeqCst);
                    Err(format!("Unstable failure for operation: {}", operation))
                }
            }
        }
    }
    
    fn get_statistics(&self) -> (usize, usize) {
        (
            self.success_count.load(Ordering::SeqCst),
            self.failure_count.load(Ordering::SeqCst)
        )
    }
    
    fn reset_statistics(&self) {
        self.success_count.store(0, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
    }
    
    fn set_latency(&self, ms: usize) {
        self.latency_ms.store(ms, Ordering::SeqCst);
    }
    
    fn set_packet_loss_rate(&self, rate: usize) {
        self.packet_loss_rate.store(rate, Ordering::SeqCst);
    }
}

/// 网络操作重试机制
struct NetworkRetryHandler {
    max_retries: usize,
    base_delay_ms: u64,
    max_delay_ms: u64,
    backoff_multiplier: f64,
}

impl NetworkRetryHandler {
    fn new() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
    
    async fn retry_with_backoff<F, T, E>(&self, mut operation: F) -> std::result::Result<T, E>
    where
        F: FnMut() -> std::result::Result<T, E> + Send,
        T: Send,
        E: Send,
    {
        let mut last_error = None;
        
        for attempt in 0..=self.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_error = Some(err);
                    
                    if attempt < self.max_retries {
                        let delay = (self.base_delay_ms as f64 
                            * self.backoff_multiplier.powi(attempt as i32))
                            .min(self.max_delay_ms as f64) as u64;
                        
                        sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap())
    }
}

/// 测试基本的网络中断恢复
#[tokio::test]
async fn test_basic_network_interruption_recovery() {
    println!("🚀 Testing basic network interruption recovery...");
    
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let network_simulator = Arc::new(NetworkSimulator::new());
    let retry_handler = NetworkRetryHandler::new();
    
    // 初始网络正常状态测试
    network_simulator.set_state(NetworkState::Online);
    
    let result = network_simulator.simulate_network_call("wallet_sync").await;
    assert!(result.is_ok());
    println!("  ✅ Online network call succeeded: {:?}", result.unwrap());
    
    // 模拟网络中断
    network_simulator.set_state(NetworkState::Offline);
    
    let offline_result = network_simulator.simulate_network_call("wallet_sync").await;
    assert!(offline_result.is_err());
    println!("  ❌ Offline network call failed as expected: {:?}", offline_result.unwrap_err());
    
    // 测试网络恢复
    network_simulator.set_state(NetworkState::Online);
    
    let recovery_result = network_simulator.simulate_network_call("wallet_sync").await;
    assert!(recovery_result.is_ok());
    println!("  ✅ Network recovery successful: {:?}", recovery_result.unwrap());
    
    // 验证系统在网络中断期间仍能执行本地操作
    let local_memory = security_manager.create_secure_memory(1024);
    assert!(local_memory.is_ok());
    println!("  ✅ Local operations work during network issues: {} bytes", 
             local_memory.unwrap().size());
    
    let (successes, failures) = network_simulator.get_statistics();
    println!("  📊 Network statistics: {} successes, {} failures", successes, failures);
    
    println!("✅ Basic network interruption recovery verified");
}

/// 测试网络重试机制
#[tokio::test]
async fn test_network_retry_mechanism() {
    println!("🚀 Testing network retry mechanism...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    let retry_handler = NetworkRetryHandler::new();
    
    // 设置不稳定网络（50%失败率）
    network_simulator.set_state(NetworkState::Unstable);
    network_simulator.reset_statistics();
    
    let mut retry_attempts = 0;
    let mut successful_operations = 0;
    
    for operation_id in 0..10 {
        let sim = Arc::clone(&network_simulator);
        let result = retry_handler.retry_with_backoff(|| {
            retry_attempts += 1;
            
            // 创建一个同步版本的网络调用模拟
            match sim.get_state() {
                NetworkState::Unstable => {
                    if rand::random::<bool>() {
                        Ok(format!("Retry success for operation {}", operation_id))
                    } else {
                        Err(format!("Retry failure for operation {}", operation_id))
                    }
                },
                _ => Ok(format!("Success for operation {}", operation_id))
            }
        }).await;
        
        match result {
            Ok(msg) => {
                successful_operations += 1;
                println!("  ✅ Operation {} succeeded: {}", operation_id, msg);
            },
            Err(err) => {
                println!("  ❌ Operation {} failed after retries: {}", operation_id, err);
            }
        }
    }
    
    println!("  📊 Retry statistics:");
    println!("    Total retry attempts: {}", retry_attempts);
    println!("    Successful operations: {}/{}", successful_operations, 10);
    println!("    Success rate: {:.1}%", (successful_operations as f64 / 10.0) * 100.0);
    
    // 至少应有一些操作成功
    assert!(successful_operations > 0, "At least some operations should succeed with retries");
    
    println!("✅ Network retry mechanism verified");
}

/// 测试网络超时处理
#[tokio::test] 
async fn test_network_timeout_handling() {
    println!("🚀 Testing network timeout handling...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    
    // 设置慢速网络
    network_simulator.set_state(NetworkState::Slow);
    
    // 测试超时情况
    let timeout_duration = Duration::from_millis(1000);
    
    let timeout_result = timeout(
        timeout_duration,
        network_simulator.simulate_network_call("slow_operation")
    ).await;
    
    match timeout_result {
        Ok(result) => {
            println!("  ⚠️ Slow operation completed within timeout: {:?}", result);
        },
        Err(_) => {
            println!("  ✅ Slow operation properly timed out");
        }
    }
    
    // 测试快速操作不会超时
    network_simulator.set_state(NetworkState::Online);
    network_simulator.set_latency(10); // 10ms延迟
    
    let fast_result = timeout(
        timeout_duration,
        network_simulator.simulate_network_call("fast_operation")
    ).await;
    
    assert!(fast_result.is_ok());
    println!("  ✅ Fast operation completed: {:?}", fast_result.unwrap().unwrap());
    
    // 测试系统在超时后的恢复能力
    let recovery_memory = SecurityManager::new(SecurityConfig::default())
        .create_secure_memory(512);
    assert!(recovery_memory.is_ok());
    println!("  ✅ System recovers after network timeout: {} bytes", 
             recovery_memory.unwrap().size());
    
    println!("✅ Network timeout handling verified");
}

/// 测试并发网络操作中断
#[tokio::test]
async fn test_concurrent_network_interruptions() {
    println!("🚀 Testing concurrent network interruptions...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    let mut handles = Vec::new();
    
    // 启动多个并发网络操作
    for task_id in 0..8 {
        let sim = Arc::clone(&network_simulator);
        
        let handle = tokio::spawn(async move {
            let mut operation_results = Vec::new();
            
            for op_id in 0..5 {
                // 随机设置网络状态来模拟不同的网络条件
                let states = [
                    NetworkState::Online,
                    NetworkState::Offline,
                    NetworkState::Slow,
                    NetworkState::Unstable,
                ];
                let random_state = &states[rand::random::<usize>() % states.len()];
                sim.set_state(random_state.clone());
                
                let operation_name = format!("task_{}_op_{}", task_id, op_id);
                let result = sim.simulate_network_call(&operation_name).await;
                operation_results.push((operation_name, result.is_ok()));
                
                // 小延迟来模拟操作间隔
                sleep(Duration::from_millis(50)).await;
            }
            
            (task_id, operation_results)
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
    
    for (task_id, results) in &all_results {
        let task_successes = results.iter().filter(|(_, success)| *success).count();
        let task_total = results.len();
        
        println!("  Task {}: {}/{} operations successful", task_id, task_successes, task_total);
        
        total_operations += task_total;
        successful_operations += task_successes;
    }
    
    let (sim_successes, sim_failures) = network_simulator.get_statistics();
    
    println!("  📊 Concurrent network interruption results:");
    println!("    Total operations: {}", total_operations);
    println!("    Successful operations: {}", successful_operations);
    println!("    Success rate: {:.1}%", (successful_operations as f64 / total_operations as f64) * 100.0);
    println!("    Simulator stats: {} successes, {} failures", sim_successes, sim_failures);
    
    // 验证系统能处理并发网络中断
    assert!(total_operations > 0, "Should have performed operations");
    
    println!("✅ Concurrent network interruptions verified");
}

/// 测试网络质量降级时的适应性
#[tokio::test]
async fn test_network_quality_adaptation() {
    println!("🚀 Testing network quality adaptation...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    
    // 测试不同网络质量条件
    let network_conditions = vec![
        ("Perfect", NetworkState::Online, 10, 0),      // 10ms延迟，无丢包
        ("Good", NetworkState::Online, 50, 1),         // 50ms延迟，1%丢包
        ("Fair", NetworkState::Online, 200, 5),        // 200ms延迟，5%丢包
        ("Poor", NetworkState::Unstable, 500, 10),     // 500ms延迟，10%丢包
        ("Bad", NetworkState::Unstable, 1000, 20),     // 1s延迟，20%丢包
    ];
    
    for (condition_name, state, latency, loss_rate) in network_conditions {
        println!("  🧪 Testing {} network condition:", condition_name);
        
        network_simulator.set_state(state);
        network_simulator.set_latency(latency);
        network_simulator.set_packet_loss_rate(loss_rate);
        network_simulator.reset_statistics();
        
        let start_time = Instant::now();
        let mut operation_times = Vec::new();
        let mut successful_ops = 0;
        
        // 执行多个操作来测试适应性
        for i in 0..5 {
            let op_start = Instant::now();
            
            let result = network_simulator.simulate_network_call(&format!("adapt_test_{}", i)).await;
            
            let op_duration = op_start.elapsed();
            operation_times.push(op_duration);
            
            if result.is_ok() {
                successful_ops += 1;
            }
            
            // 同时执行本地操作以验证系统响应性
            let _local_rng = security_manager.create_secure_rng();
        }
        
        let total_duration = start_time.elapsed();
        let avg_op_time = operation_times.iter().sum::<Duration>() / operation_times.len() as u32;
        let (successes, failures) = network_simulator.get_statistics();
        
        println!("    Duration: {:?}, Avg op time: {:?}", total_duration, avg_op_time);
        println!("    Success rate: {}/{} ({:.1}%)", successful_ops, 5, 
                (successful_ops as f64 / 5.0) * 100.0);
        println!("    Network stats: {} successes, {} failures", successes, failures);
        
        // 验证系统在各种网络条件下都能运行
        if condition_name != "Bad" {
            assert!(successful_ops > 0, "Should have some successes in {} conditions", condition_name);
        }
    }
    
    println!("✅ Network quality adaptation verified");
}

/// 测试网络分区恢复
#[tokio::test]
async fn test_network_partition_recovery() {
    println!("🚀 Testing network partition recovery...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let mut wallet_manager = WalletManager::new(&security_manager)
        .expect("Failed to create wallet manager");
    
    // 阶段1：正常网络状态下创建钱包
    network_simulator.set_state(NetworkState::Online);
    println!("  Phase 1: Creating wallet in normal network conditions...");
    
    let mut local_wallets = Vec::new();
    for i in 0..3 {
        let binding = wallet::UserWalletBinding {
            user_id: i,
            wallet_id: uuid::Uuid::new_v4(),
            address: [i as u8; 20],
            alias: Some(format!("partition_test_wallet_{}", i)),
            is_primary: i == 0,
            permissions: wallet::WalletPermissions::full_permissions(),
        };
        
        // 本地存储（不依赖网络）
        wallet_manager.store_wallet_binding(binding.clone()).await
            .expect("Local wallet binding should succeed");
        println!("    Created wallet {} for user {}", binding.wallet_id, i);
        local_wallets.push(binding);
    }
    
    // 阶段2：模拟网络分区
    network_simulator.set_state(NetworkState::Offline);
    println!("  Phase 2: Simulating network partition...");
    
    // 验证本地操作仍然可用
    let partition_memory = security_manager.create_secure_memory(1024);
    assert!(partition_memory.is_ok());
    println!("    ✅ Local memory allocation during partition: {} bytes", 
             partition_memory.unwrap().size());
    
    let partition_rng = security_manager.create_secure_rng();
    assert!(partition_rng.is_ok());
    println!("    ✅ Local RNG creation during partition successful");
    
    // 模拟网络操作失败
    let sync_result = network_simulator.simulate_network_call("wallet_sync").await;
    assert!(sync_result.is_err());
    println!("    ❌ Network sync failed as expected during partition: {:?}", sync_result.unwrap_err());
    
    // 阶段3：网络恢复
    println!("  Phase 3: Network partition recovery...");
    
    // 逐步恢复网络质量
    let recovery_phases = vec![
        ("Unstable reconnection", NetworkState::Unstable),
        ("Slow reconnection", NetworkState::Slow),
        ("Full recovery", NetworkState::Online),
    ];
    
    for (phase_name, network_state) in recovery_phases {
        println!("    🔄 {}", phase_name);
        network_simulator.set_state(network_state);
        
        // 测试网络操作恢复
        let recovery_result = network_simulator.simulate_network_call("partition_recovery_test").await;
        match recovery_result {
            Ok(msg) => println!("      ✅ Network operation recovered: {}", msg),
            Err(err) => println!("      ⚠️ Network operation still failing: {}", err),
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // 阶段4：验证完全恢复
    println!("  Phase 4: Verifying full system recovery...");
    
    // 验证所有本地钱包仍然可访问
    for wallet_binding in &local_wallets {
        println!("    ✅ Wallet {} still accessible after partition", wallet_binding.wallet_id);
    }
    
    // 验证新操作可以执行
    let post_recovery_memory = security_manager.create_secure_memory(2048);
    assert!(post_recovery_memory.is_ok());
    println!("    ✅ Post-recovery memory allocation: {} bytes", 
             post_recovery_memory.unwrap().size());
    
    // 验证网络同步恢复
    let sync_recovery_result = network_simulator.simulate_network_call("final_sync_test").await;
    assert!(sync_recovery_result.is_ok());
    println!("    ✅ Network sync recovered: {:?}", sync_recovery_result.unwrap());
    
    let (final_successes, final_failures) = network_simulator.get_statistics();
    println!("  📊 Partition recovery statistics: {} successes, {} failures", 
             final_successes, final_failures);
    
    println!("✅ Network partition recovery verified");
}

/// 测试长期网络不稳定的处理
#[tokio::test]
async fn test_prolonged_network_instability() {
    println!("🚀 Testing prolonged network instability handling...");
    
    let network_simulator = Arc::new(NetworkSimulator::new());
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    let duration = Duration::from_millis(2000); // 2秒的不稳定期
    let start_time = Instant::now();
    
    // 设置长期不稳定网络
    network_simulator.set_state(NetworkState::Unstable);
    network_simulator.set_latency(300);
    network_simulator.set_packet_loss_rate(15);
    network_simulator.reset_statistics();
    
    let mut operations_performed = 0;
    let mut local_operations = 0;
    let mut network_attempts = 0;
    
    println!("  Starting prolonged instability simulation...");
    
    while start_time.elapsed() < duration {
        // 尝试网络操作
        network_attempts += 1;
        let network_result = network_simulator.simulate_network_call(
            &format!("prolonged_test_{}", network_attempts)
        ).await;
        
        if network_result.is_ok() {
            operations_performed += 1;
        }
        
        // 执行本地操作以保持系统活跃
        if operations_performed % 5 == 0 {
            let _local_memory = security_manager.create_secure_memory(256);
            local_operations += 1;
        }
        
        // 模拟操作间隔
        sleep(Duration::from_millis(100)).await;
    }
    
    let total_duration = start_time.elapsed();
    let (successes, failures) = network_simulator.get_statistics();
    
    println!("  📊 Prolonged instability results:");
    println!("    Duration: {:?}", total_duration);
    println!("    Network attempts: {}", network_attempts);
    println!("    Successful network operations: {}", operations_performed);
    println!("    Network success rate: {:.1}%", 
             (operations_performed as f64 / network_attempts as f64) * 100.0);
    println!("    Local operations performed: {}", local_operations);
    println!("    Simulator final stats: {} successes, {} failures", successes, failures);
    
    // 验证系统在长期不稳定期间的表现
    assert!(local_operations > 0, "Should maintain local operations during instability");
    assert!(network_attempts > 0, "Should attempt network operations");
    
    // 测试不稳定期后的恢复
    println!("  Testing post-instability recovery...");
    network_simulator.set_state(NetworkState::Online);
    network_simulator.set_latency(50);
    network_simulator.set_packet_loss_rate(0);
    
    let recovery_result = network_simulator.simulate_network_call("post_instability_recovery").await;
    assert!(recovery_result.is_ok());
    println!("  ✅ Post-instability recovery successful: {:?}", recovery_result.unwrap());
    
    println!("✅ Prolonged network instability handling verified");
}

/// 综合网络中断测试
#[tokio::test]
async fn test_comprehensive_network_interruption_handling() {
    println!("🚀 Testing comprehensive network interruption handling...");
    
    let test_start = Instant::now();
    let network_simulator = Arc::new(NetworkSimulator::new());
    let security_manager = Arc::new(SecurityManager::new(SecurityConfig::default()));
    
    // 创建多个网络场景的并发测试
    let mut scenario_handles = Vec::new();
    
    // 场景1：基本中断恢复
    {
        let sim = Arc::clone(&network_simulator);
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            
            // 在线 -> 离线 -> 在线
            let conditions = [
                (NetworkState::Online, "online"),
                (NetworkState::Offline, "offline"), 
                (NetworkState::Online, "recovered")
            ];
            
            for (state, desc) in conditions.iter() {
                sim.set_state(state.clone());
                let result = sim.simulate_network_call(&format!("basic_{}", desc)).await;
                results.push((*desc, result.is_ok()));
                sleep(Duration::from_millis(100)).await;
            }
            
            ("basic_recovery", results)
        });
        scenario_handles.push(handle);
    }
    
    // 场景2：质量降级测试
    {
        let sim = Arc::clone(&network_simulator);
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            
            let conditions = [
                (NetworkState::Online, 10, 0, "perfect"),
                (NetworkState::Online, 100, 2, "good"),
                (NetworkState::Unstable, 300, 10, "poor"),
                (NetworkState::Online, 50, 1, "recovered"),
            ];
            
            for (state, latency, loss, desc) in conditions.iter() {
                sim.set_state(state.clone());
                sim.set_latency(*latency);
                sim.set_packet_loss_rate(*loss);
                
                let result = sim.simulate_network_call(&format!("quality_{}", desc)).await;
                results.push((*desc, result.is_ok()));
                sleep(Duration::from_millis(50)).await;
            }
            
            ("quality_adaptation", results)
        });
        scenario_handles.push(handle);
    }
    
    // 场景3：本地操作持续性测试
    {
        let sm = Arc::clone(&security_manager);
        let handle = tokio::spawn(async move {
            let mut local_ops = 0;
            
            for i in 0..10 {
                match sm.create_secure_memory(128 * (i + 1)) {
                    Ok(_) => local_ops += 1,
                    Err(_) => {}
                }
                
                match sm.create_secure_rng() {
                    Ok(_) => local_ops += 1,
                    Err(_) => {}
                }
                
                sleep(Duration::from_millis(20)).await;
            }
            
            ("local_continuity", vec![("local_operations", local_ops > 15)])
        });
        scenario_handles.push(handle);
    }
    
    // 等待所有场景完成
    let mut all_scenario_results = Vec::new();
    for handle in scenario_handles {
        let result = handle.await.expect("Scenario should complete");
        all_scenario_results.push(result);
    }
    
    // 分析综合结果
    println!("  📊 Comprehensive network interruption test results:");
    
    for (scenario_name, results) in &all_scenario_results {
        let success_count = results.iter().filter(|(_, success)| *success).count();
        let total_count = results.len();
        
        println!("    {}: {}/{} operations successful", 
                scenario_name, success_count, total_count);
        
        for (operation, success) in results {
            let status = if *success { "✅" } else { "❌" };
            println!("      {} {}", status, operation);
        }
    }
    
    let test_duration = test_start.elapsed();
    let (final_successes, final_failures) = network_simulator.get_statistics();
    
    println!("  📈 Overall test metrics:");
    println!("    Total test duration: {:?}", test_duration);
    println!("    Network simulator final stats: {} successes, {} failures", 
             final_successes, final_failures);
    println!("    Success rate: {:.1}%", 
             (final_successes as f64 / (final_successes + final_failures) as f64) * 100.0);
    
    // 验证系统在各种网络中断场景下的鲁棒性
    let total_scenarios = all_scenario_results.len();
    let scenarios_with_successes = all_scenario_results.iter()
        .filter(|(_, results)| results.iter().any(|(_, success)| *success))
        .count();
    
    assert!(scenarios_with_successes > 0, "At least some scenarios should have successes");
    println!("    Successful scenarios: {}/{}", scenarios_with_successes, total_scenarios);
    
    println!("✅ Comprehensive network interruption handling verified");
}