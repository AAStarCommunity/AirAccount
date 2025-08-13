// 测试框架基础设施模块

pub mod test_helpers;

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 测试上下文，用于跟踪测试执行
pub struct TestContext {
    pub name: String,
    pub start_time: Instant,
    pub metrics: Arc<RwLock<TestMetrics>>,
}

/// 测试指标收集
#[derive(Default, Clone)]
pub struct TestMetrics {
    pub total_operations: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub total_duration: Duration,
    pub peak_memory_usage: usize,
    pub average_latency: Duration,
}

impl TestContext {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            start_time: Instant::now(),
            metrics: Arc::new(RwLock::new(TestMetrics::default())),
        }
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    pub async fn record_operation(&self, success: bool, latency: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_operations += 1;
        if success {
            metrics.successful_operations += 1;
        } else {
            metrics.failed_operations += 1;
        }
        
        // 更新平均延迟
        let total_latency = metrics.average_latency.as_nanos() * metrics.total_operations as u128;
        metrics.average_latency = Duration::from_nanos(
            (total_latency / metrics.total_operations as u128) as u64
        );
    }
    
    pub async fn get_metrics(&self) -> TestMetrics {
        self.metrics.read().await.clone()
    }
}

/// 测试运行器trait
pub trait TestRunner {
    type Result;
    
    async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn run(&mut self) -> Result<Self::Result, Box<dyn std::error::Error>>;
    async fn teardown(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    
    async fn execute(&mut self) -> Result<Self::Result, Box<dyn std::error::Error>> {
        self.setup().await?;
        let result = self.run().await;
        self.teardown().await?;
        result
    }
}