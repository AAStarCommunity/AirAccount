// Licensed to AirAccount under the Apache License, Version 2.0
// Batch audit logging system for improved performance

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Condvar};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::audit::{AuditLogEntry, AuditSink, AuditLevel, AuditEvent};

/// 批量审计日志配置
#[derive(Debug, Clone)]
pub struct BatchAuditConfig {
    /// 批处理大小
    pub batch_size: usize,
    /// 刷盘间隔（毫秒）
    pub flush_interval_ms: u64,
    /// 最大队列大小
    pub max_queue_size: usize,
    /// 工作线程数
    pub worker_threads: usize,
    /// 启用异步模式
    pub enable_async: bool,
}

impl Default for BatchAuditConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            flush_interval_ms: 1000, // 1秒
            max_queue_size: 10000,
            worker_threads: 2,
            enable_async: true,
        }
    }
}

/// 批量审计日志条目
#[derive(Debug, Clone)]
pub struct BatchLogEntry {
    pub entry: AuditLogEntry,
    pub timestamp: Instant,
}

/// 批量审计日志处理器
pub struct BatchAuditProcessor {
    config: BatchAuditConfig,
    queue: Arc<Mutex<VecDeque<BatchLogEntry>>>,
    condvar: Arc<Condvar>,
    sinks: Vec<Arc<dyn AuditSink + Send + Sync>>,
    workers: Vec<JoinHandle<()>>,
    shutdown: Arc<Mutex<bool>>,
    stats: Arc<Mutex<BatchAuditStats>>,
}

/// 批量审计统计
#[derive(Debug, Default, Clone)]
pub struct BatchAuditStats {
    pub total_entries: u64,
    pub batches_processed: u64,
    pub queue_size: usize,
    pub dropped_entries: u64,
    pub processing_time_ms: u64,
    pub last_flush_time: Option<Instant>,
}

impl BatchAuditProcessor {
    /// 创建新的批量审计处理器
    pub fn new(config: BatchAuditConfig) -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::with_capacity(config.max_queue_size)));
        let condvar = Arc::new(Condvar::new());
        let shutdown = Arc::new(Mutex::new(false));
        let stats = Arc::new(Mutex::new(BatchAuditStats::default()));
        
        let mut processor = Self {
            config,
            queue,
            condvar,
            sinks: Vec::new(),
            workers: Vec::new(),
            shutdown,
            stats,
        };
        
        // 启动工作线程
        if processor.config.enable_async {
            processor.start_workers();
        }
        
        processor
    }
    
    /// 添加审计输出端
    pub fn add_sink(&mut self, sink: Arc<dyn AuditSink + Send + Sync>) {
        self.sinks.push(sink);
    }
    
    /// 记录审计事件
    pub fn log_event(&self, level: AuditLevel, event: AuditEvent, component: &str) -> Result<(), &'static str> {
        let entry = AuditLogEntry::new(level, event, component);
        self.log_entry(entry)
    }
    
    /// 记录审计条目
    pub fn log_entry(&self, entry: AuditLogEntry) -> Result<(), &'static str> {
        let batch_entry = BatchLogEntry {
            entry,
            timestamp: Instant::now(),
        };
        
        {
            let mut queue = self.queue.lock().map_err(|_| "Failed to lock queue")?;
            
            // 检查队列容量
            if queue.len() >= self.config.max_queue_size {
                // 丢弃最老的条目
                queue.pop_front();
                let mut stats = self.stats.lock().map_err(|_| "Failed to lock stats")?;
                stats.dropped_entries += 1;
            }
            
            queue.push_back(batch_entry);
            
            // 更新统计
            {
                let mut stats = self.stats.lock().map_err(|_| "Failed to lock stats")?;
                stats.total_entries += 1;
                stats.queue_size = queue.len();
            }
        }
        
        // 通知工作线程
        self.condvar.notify_one();
        
        // 如果是同步模式，立即处理
        if !self.config.enable_async {
            self.process_batch()?;
        }
        
        Ok(())
    }
    
    /// 手动刷盘
    pub fn flush(&self) -> Result<(), &'static str> {
        self.process_batch()?;
        
        // 刷盘所有sink
        for sink in &self.sinks {
            sink.flush().map_err(|_| "Sink flush failed")?;
        }
        
        // 更新统计
        {
            let mut stats = self.stats.lock().map_err(|_| "Failed to lock stats")?;
            stats.last_flush_time = Some(Instant::now());
        }
        
        Ok(())
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> Result<BatchAuditStats, &'static str> {
        let stats = self.stats.lock().map_err(|_| "Failed to lock stats")?;
        Ok(stats.clone())
    }
    
    /// 启动工作线程
    fn start_workers(&mut self) {
        for i in 0..self.config.worker_threads {
            let queue = Arc::clone(&self.queue);
            let condvar = Arc::clone(&self.condvar);
            let shutdown = Arc::clone(&self.shutdown);
            let stats = Arc::clone(&self.stats);
            let sinks = self.sinks.clone();
            let config = self.config.clone();
            
            let worker = thread::Builder::new()
                .name(format!("batch-audit-worker-{}", i))
                .spawn(move || {
                    Self::worker_loop(queue, condvar, shutdown, stats, sinks, config);
                })
                .expect("Failed to start worker thread");
                
            self.workers.push(worker);
        }
    }
    
    /// 工作线程主循环
    fn worker_loop(
        queue: Arc<Mutex<VecDeque<BatchLogEntry>>>,
        condvar: Arc<Condvar>,
        shutdown: Arc<Mutex<bool>>,
        stats: Arc<Mutex<BatchAuditStats>>,
        sinks: Vec<Arc<dyn AuditSink + Send + Sync>>,
        config: BatchAuditConfig,
    ) {
        let mut last_flush = Instant::now();
        
        loop {
            // 检查是否需要关闭
            {
                let shutdown_guard = shutdown.lock().unwrap();
                if *shutdown_guard {
                    break;
                }
            }
            
            let should_process = {
                let queue_guard = queue.lock().unwrap();
                
                // 等待条件满足或超时
                let timeout = Duration::from_millis(config.flush_interval_ms);
                let (queue_locked, _) = condvar.wait_timeout(queue_guard, timeout).unwrap();
                
                // 检查处理条件
                let queue_len = queue_locked.len();
                let time_elapsed = last_flush.elapsed().as_millis() as u64;
                
                queue_len >= config.batch_size || 
                (queue_len > 0 && time_elapsed >= config.flush_interval_ms)
            };
            
            if should_process {
                if let Err(e) = Self::process_queue_batch(&queue, &stats, &sinks, &config) {
                    eprintln!("Batch processing error: {}", e);
                }
                last_flush = Instant::now();
            }
        }
        
        // 关闭前处理剩余条目
        let _ = Self::process_queue_batch(&queue, &stats, &sinks, &config);
    }
    
    /// 处理队列中的批次
    fn process_queue_batch(
        queue: &Arc<Mutex<VecDeque<BatchLogEntry>>>,
        stats: &Arc<Mutex<BatchAuditStats>>,
        sinks: &[Arc<dyn AuditSink + Send + Sync>],
        config: &BatchAuditConfig,
    ) -> Result<(), &'static str> {
        let start_time = Instant::now();
        let batch = {
            let mut queue_guard = queue.lock().map_err(|_| "Failed to lock queue")?;
            let batch_size = std::cmp::min(queue_guard.len(), config.batch_size);
            
            if batch_size == 0 {
                return Ok(());
            }
            
            // 取出批次数据
            let mut batch = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                if let Some(entry) = queue_guard.pop_front() {
                    batch.push(entry);
                }
            }
            batch
        };
        
        // 处理批次
        for batch_entry in &batch {
            for sink in sinks {
                if let Err(e) = sink.log_entry(&batch_entry.entry) {
                    eprintln!("Sink error: {}", e);
                }
            }
        }
        
        // 更新统计
        {
            let mut stats_guard = stats.lock().map_err(|_| "Failed to lock stats")?;
            stats_guard.batches_processed += 1;
            stats_guard.processing_time_ms += start_time.elapsed().as_millis() as u64;
            
            let queue_guard = queue.lock().map_err(|_| "Failed to lock queue")?;
            stats_guard.queue_size = queue_guard.len();
        }
        
        Ok(())
    }
    
    /// 处理批次（同步模式）
    fn process_batch(&self) -> Result<(), &'static str> {
        Self::process_queue_batch(&self.queue, &self.stats, &self.sinks, &self.config)
    }
}

impl Drop for BatchAuditProcessor {
    fn drop(&mut self) {
        // 通知工作线程关闭
        {
            let mut shutdown = self.shutdown.lock().unwrap();
            *shutdown = true;
        }
        self.condvar.notify_all();
        
        // 等待工作线程完成
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        
        // 处理剩余的日志条目
        let _ = self.flush();
    }
}

/// 高性能审计日志记录器（基于批处理）
pub struct HighPerformanceAuditLogger {
    processor: BatchAuditProcessor,
}

impl HighPerformanceAuditLogger {
    /// 创建新的高性能审计日志记录器
    pub fn new(config: BatchAuditConfig) -> Self {
        Self {
            processor: BatchAuditProcessor::new(config),
        }
    }
    
    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(BatchAuditConfig::default())
    }
    
    /// 添加输出端
    pub fn add_sink(&mut self, sink: Arc<dyn AuditSink + Send + Sync>) {
        self.processor.add_sink(sink);
    }
    
    /// 记录信息级别事件
    pub fn log_info(&self, event: AuditEvent, component: &str) -> Result<(), &'static str> {
        self.processor.log_event(AuditLevel::Info, event, component)
    }
    
    /// 记录警告级别事件
    pub fn log_warning(&self, event: AuditEvent, component: &str) -> Result<(), &'static str> {
        self.processor.log_event(AuditLevel::Warning, event, component)
    }
    
    /// 记录错误级别事件
    pub fn log_error(&self, event: AuditEvent, component: &str) -> Result<(), &'static str> {
        self.processor.log_event(AuditLevel::Error, event, component)
    }
    
    /// 记录安全级别事件
    pub fn log_security(&self, event: AuditEvent, component: &str) -> Result<(), &'static str> {
        self.processor.log_event(AuditLevel::Security, event, component)
    }
    
    /// 刷盘
    pub fn flush(&self) -> Result<(), &'static str> {
        self.processor.flush()
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> Result<BatchAuditStats, &'static str> {
        self.processor.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::audit::ConsoleAuditSink;
    
    #[test]
    fn test_batch_audit_creation() {
        let config = BatchAuditConfig::default();
        let mut processor = BatchAuditProcessor::new(config);
        
        processor.add_sink(Arc::new(ConsoleAuditSink));
        
        let event = AuditEvent::SecurityOperation {
            operation: "test_batch".to_string(),
            details: "Batch processing test".to_string(),
            success: true,
            risk_level: "LOW".to_string(),
        };
        
        assert!(processor.log_event(AuditLevel::Info, event, "test").is_ok());
    }
    
    #[test]
    fn test_batch_processing_sync() {
        let config = BatchAuditConfig {
            enable_async: false,
            batch_size: 2,
            ..Default::default()
        };
        
        let mut processor = BatchAuditProcessor::new(config);
        processor.add_sink(Arc::new(ConsoleAuditSink));
        
        // 添加多个事件
        for i in 0..5 {
            let event = AuditEvent::SecurityOperation {
                operation: format!("test_operation_{}", i),
                details: format!("Test operation {}", i),
                success: true,
                risk_level: "LOW".to_string(),
            };
            
            assert!(processor.log_event(AuditLevel::Info, event, "test").is_ok());
        }
        
        let stats = processor.get_stats().unwrap();
        assert_eq!(stats.total_entries, 5);
    }
    
    #[test]
    fn test_high_performance_logger() {
        let mut logger = HighPerformanceAuditLogger::with_defaults();
        logger.add_sink(Arc::new(ConsoleAuditSink));
        
        let event = AuditEvent::KeyGeneration {
            algorithm: "ECDSA".to_string(),
            key_size: 256,
            operation: "test_key_gen".to_string(),
            key_type: "test_key".to_string(),
            duration_ms: 50,
            entropy_bits: 256,
        };
        
        assert!(logger.log_security(event, "test").is_ok());
        assert!(logger.flush().is_ok());
        
        let stats = logger.get_stats().unwrap();
        assert_eq!(stats.total_entries, 1);
    }
    
    #[test]
    fn test_queue_overflow_handling() {
        // 使用异步模式来避免立即处理
        let config = BatchAuditConfig {
            max_queue_size: 3,
            batch_size: 1000, // 设置大批次大小避免自动处理
            flush_interval_ms: 10000, // 设置长flush间隔
            enable_async: true, // 使用异步模式
            ..Default::default()
        };
        
        let mut processor = BatchAuditProcessor::new(config);
        processor.add_sink(Arc::new(ConsoleAuditSink));
        
        // 快速添加超过队列容量的事件以触发溢出
        for i in 0..10 {
            let event = AuditEvent::SecurityOperation {
                operation: format!("overflow_test_{}", i),
                details: "Testing queue overflow".to_string(),
                success: true,
                risk_level: "LOW".to_string(),
            };
            
            processor.log_event(AuditLevel::Info, event, "test").unwrap();
        }
        
        // 等待一小段时间让任何潜在的处理完成
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let stats = processor.get_stats().unwrap();
        // 应该有至少7个条目被丢弃（10 - 3 = 7）
        assert!(stats.dropped_entries >= 7, "Expected at least 7 dropped entries, got {}", stats.dropped_entries);
    }
}