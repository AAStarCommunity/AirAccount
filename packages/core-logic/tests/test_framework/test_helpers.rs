// 测试辅助工具

use airaccount_core_logic::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 创建测试用的安全管理器
pub fn create_test_security_manager() -> Arc<SecurityManager> {
    let config = EnhancedSecurityConfig::development();
    Arc::new(SecurityManager::new(config).expect("Failed to create security manager"))
}

/// 创建测试用的钱包管理器
pub async fn create_test_wallet_manager() -> WalletManager {
    let security_manager = create_test_security_manager();
    WalletManager::new(security_manager)
}

/// 生成随机测试数据
pub fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// 模拟网络延迟
pub async fn simulate_network_delay(min_ms: u64, max_ms: u64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let delay = rng.gen_range(min_ms..=max_ms);
    sleep(Duration::from_millis(delay)).await;
}

/// 测试重试逻辑
pub async fn retry_with_backoff<F, T, E>(
    mut f: F,
    max_retries: usize,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut delay = initial_delay;
    for attempt in 0..max_retries {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                sleep(delay).await;
                delay *= 2; // 指数退避
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// 内存使用监控
pub struct MemoryMonitor {
    initial_usage: usize,
    peak_usage: usize,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        Self {
            initial_usage: Self::current_usage(),
            peak_usage: 0,
        }
    }
    
    pub fn update(&mut self) {
        let current = Self::current_usage();
        if current > self.peak_usage {
            self.peak_usage = current;
        }
    }
    
    pub fn peak_delta(&self) -> usize {
        self.peak_usage.saturating_sub(self.initial_usage)
    }
    
    #[cfg(target_os = "linux")]
    fn current_usage() -> usize {
        // Linux实现：读取/proc/self/status
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
            .unwrap_or(0) * 1024 // 转换为字节
    }
    
    #[cfg(not(target_os = "linux"))]
    fn current_usage() -> usize {
        // 其他平台的简单实现
        0
    }
}

/// 并发测试辅助
pub struct ConcurrentTester {
    pub thread_count: usize,
    pub operations_per_thread: usize,
}

impl ConcurrentTester {
    pub fn new(threads: usize, ops_per_thread: usize) -> Self {
        Self {
            thread_count: threads,
            operations_per_thread: ops_per_thread,
        }
    }
    
    pub async fn run<F, T>(&self, operation: F) -> Vec<Duration>
    where
        F: Fn(usize) -> T + Send + Sync + 'static,
        T: std::future::Future<Output = ()> + Send + 'static,
    {
        use tokio::task::JoinSet;
        
        let operation = Arc::new(operation);
        let mut tasks = JoinSet::new();
        
        for thread_id in 0..self.thread_count {
            let op = Arc::clone(&operation);
            let ops_count = self.operations_per_thread;
            
            tasks.spawn(async move {
                let start = std::time::Instant::now();
                for i in 0..ops_count {
                    op(thread_id * ops_count + i).await;
                }
                start.elapsed()
            });
        }
        
        let mut durations = Vec::new();
        while let Some(result) = tasks.join_next().await {
            if let Ok(duration) = result {
                durations.push(duration);
            }
        }
        
        durations
    }
}