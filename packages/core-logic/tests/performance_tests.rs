#[cfg(test)]
mod performance_tests {
    use airaccount_core_logic::*;
    use airaccount_core_logic::security::audit;
    use std::time::{Instant, Duration};
    
    const BENCHMARK_ITERATIONS: usize = 10000;
    const LARGE_BENCHMARK_ITERATIONS: usize = 1000;
    
    #[test]
    fn bench_constant_time_comparison() {
        let data_a = vec![0x42u8; 32];
        let data_b = vec![0x42u8; 32];
        
        let start = Instant::now();
        for _ in 0..BENCHMARK_ITERATIONS {
            let _ = ConstantTimeOps::secure_compare(&data_a, &data_b);
        }
        let duration = start.elapsed();
        
        let ops_per_sec = BENCHMARK_ITERATIONS as f64 / duration.as_secs_f64();
        let avg_duration_ns = duration.as_nanos() / BENCHMARK_ITERATIONS as u128;
        
        println!("Constant time comparison (32 bytes):");
        println!("  {} operations in {:?}", BENCHMARK_ITERATIONS, duration);
        println!("  {:.2} ops/sec", ops_per_sec);
        println!("  {} ns/op average", avg_duration_ns);
        
        // 性能要求：至少每秒100万次操作
        assert!(ops_per_sec > 1_000_000.0, 
            "Constant time comparison too slow: {:.2} ops/sec", ops_per_sec);
    }
    
    #[test]
    fn bench_secure_memory_allocation() {
        let sizes = vec![64, 256, 1024, 4096];
        
        for size in sizes {
            let start = Instant::now();
            for _ in 0..LARGE_BENCHMARK_ITERATIONS {
                let _memory = SecureMemory::new(size).expect("Failed to allocate");
                // 内存会自动释放
            }
            let duration = start.elapsed();
            
            let ops_per_sec = LARGE_BENCHMARK_ITERATIONS as f64 / duration.as_secs_f64();
            let avg_duration_us = duration.as_micros() / LARGE_BENCHMARK_ITERATIONS as u128;
            
            println!("Secure memory allocation ({} bytes):", size);
            println!("  {} operations in {:?}", LARGE_BENCHMARK_ITERATIONS, duration);
            println!("  {:.2} ops/sec", ops_per_sec);
            println!("  {} μs/op average", avg_duration_us);
            
            // 性能要求：分配时间应该少于100微秒
            assert!(avg_duration_us < 100, 
                "Memory allocation too slow for size {}: {} μs", size, avg_duration_us);
        }
    }
    
    #[test]
    fn bench_secure_rng_generation() {
        let context = init_default().expect("Failed to initialize");
        let security_manager = context.security_manager();
        let mut rng = security_manager.create_secure_rng()
            .expect("Failed to create RNG");
        
        let buffer_sizes = vec![16, 32, 64, 256];
        
        for buffer_size in buffer_sizes {
            let mut buffer = vec![0u8; buffer_size];
            
            let start = Instant::now();
            for _ in 0..LARGE_BENCHMARK_ITERATIONS {
                rng.fill_bytes(&mut buffer).expect("Failed to generate random bytes");
            }
            let duration = start.elapsed();
            
            let bytes_per_sec = (LARGE_BENCHMARK_ITERATIONS * buffer_size) as f64 / duration.as_secs_f64();
            let avg_duration_us = duration.as_micros() / LARGE_BENCHMARK_ITERATIONS as u128;
            
            println!("Secure RNG generation ({} bytes):", buffer_size);
            println!("  {} operations in {:?}", LARGE_BENCHMARK_ITERATIONS, duration);
            println!("  {:.2} bytes/sec", bytes_per_sec);
            println!("  {} μs/op average", avg_duration_us);
            
            // 性能要求：应该能够每秒生成至少100KB的随机数据
            assert!(bytes_per_sec > 100_000.0, 
                "RNG generation too slow for {} bytes: {:.2} bytes/sec", 
                buffer_size, bytes_per_sec);
        }
    }
    
    #[test]
    fn bench_secure_string_operations() {
        let long_string = "x".repeat(256);
        let test_strings = vec![
            "short",
            "medium_length_string",
            "this_is_a_much_longer_string_for_testing_performance",
            &long_string,
        ];
        
        for test_str in test_strings {
            // 测试创建性能
            let start = Instant::now();
            for _ in 0..LARGE_BENCHMARK_ITERATIONS {
                let _secure_str = SecureString::new(test_str).expect("Failed to create secure string");
            }
            let create_duration = start.elapsed();
            
            // 测试比较性能
            let str1 = SecureString::new(test_str).expect("Failed to create string 1");
            let str2 = SecureString::new(test_str).expect("Failed to create string 2");
            
            let start = Instant::now();
            for _ in 0..BENCHMARK_ITERATIONS {
                let _ = str1.secure_eq(&str2);
            }
            let compare_duration = start.elapsed();
            
            let create_ops_per_sec = LARGE_BENCHMARK_ITERATIONS as f64 / create_duration.as_secs_f64();
            let compare_ops_per_sec = BENCHMARK_ITERATIONS as f64 / compare_duration.as_secs_f64();
            
            println!("Secure string operations ({} chars):", test_str.len());
            println!("  Creation: {:.2} ops/sec", create_ops_per_sec);
            println!("  Comparison: {:.2} ops/sec", compare_ops_per_sec);
            
            // 性能要求
            assert!(create_ops_per_sec > 10_000.0, 
                "String creation too slow for {} chars: {:.2} ops/sec", 
                test_str.len(), create_ops_per_sec);
            assert!(compare_ops_per_sec > 100_000.0, 
                "String comparison too slow for {} chars: {:.2} ops/sec", 
                test_str.len(), compare_ops_per_sec);
        }
    }
    
    #[test]
    fn bench_audit_logging_throughput() {
        use airaccount_core_logic::security::audit;
        use std::sync::Arc;
        
        let mut logger = audit::AuditLogger::new();
        logger.add_sink(Arc::new(NullAuditSink));
        
        let test_event = audit::AuditEvent::TEEOperation {
            operation: "benchmark_test".to_string(),
            duration_ms: 1,
            success: true,
        };
        
        let start = Instant::now();
        for i in 0..BENCHMARK_ITERATIONS {
            logger.log_info(test_event.clone(), &format!("component_{}", i % 10));
        }
        let duration = start.elapsed();
        
        let ops_per_sec = BENCHMARK_ITERATIONS as f64 / duration.as_secs_f64();
        let avg_duration_ns = duration.as_nanos() / BENCHMARK_ITERATIONS as u128;
        
        println!("Audit logging throughput:");
        println!("  {} operations in {:?}", BENCHMARK_ITERATIONS, duration);
        println!("  {:.2} logs/sec", ops_per_sec);
        println!("  {} ns/log average", avg_duration_ns);
        
        // 性能要求：应该能够每秒记录至少10万条审计日志
        assert!(ops_per_sec > 100_000.0, 
            "Audit logging too slow: {:.2} logs/sec", ops_per_sec);
    }
    
    #[test]
    fn bench_memory_protection_overhead() {
        // 测试开启和关闭内存保护的性能差异
        let test_ptr = &42u8 as *const u8;
        let buffer_size = 1000;
        let access_size = 100;
        
        // 开启保护的性能测试
        MemoryGuard::enable_protection();
        let start = Instant::now();
        for _ in 0..BENCHMARK_ITERATIONS {
            let _ = MemoryGuard::check_bounds(test_ptr, access_size, buffer_size);
        }
        let protected_duration = start.elapsed();
        
        // 关闭保护的性能测试
        MemoryGuard::disable_protection();
        let start = Instant::now();
        for _ in 0..BENCHMARK_ITERATIONS {
            let _ = MemoryGuard::check_bounds(test_ptr, access_size, buffer_size);
        }
        let unprotected_duration = start.elapsed();
        
        // 恢复保护状态
        MemoryGuard::enable_protection();
        
        let protected_ops_per_sec = BENCHMARK_ITERATIONS as f64 / protected_duration.as_secs_f64();
        let unprotected_ops_per_sec = BENCHMARK_ITERATIONS as f64 / unprotected_duration.as_secs_f64();
        let overhead_ratio = unprotected_duration.as_nanos() as f64 / protected_duration.as_nanos() as f64;
        
        println!("Memory protection overhead:");
        println!("  Protected: {:.2} ops/sec", protected_ops_per_sec);
        println!("  Unprotected: {:.2} ops/sec", unprotected_ops_per_sec);
        println!("  Overhead ratio: {:.2}x", 1.0 / overhead_ratio);
        
        // 保护开销应该是可接受的（不超过10倍）
        assert!(overhead_ratio > 0.1, 
            "Memory protection overhead too high: {:.2}x slower", 1.0 / overhead_ratio);
    }
    
    #[test]
    fn bench_concurrent_operations() {
        use std::thread;
        use std::sync::{Arc, Barrier};
        
        let context = Arc::new(init_default().expect("Failed to initialize"));
        let thread_count = 4;
        let operations_per_thread = LARGE_BENCHMARK_ITERATIONS / thread_count;
        let barrier = Arc::new(Barrier::new(thread_count));
        
        let start_time = Instant::now();
        let handles: Vec<_> = (0..thread_count).map(|_| {
            let context = Arc::clone(&context);
            let barrier = Arc::clone(&barrier);
            
            thread::spawn(move || {
                let security_manager = context.security_manager();
                
                // 等待所有线程准备就绪
                barrier.wait();
                
                let thread_start = Instant::now();
                
                // 执行并发操作
                for _ in 0..operations_per_thread {
                    // 分配内存
                    let _memory = security_manager.create_secure_memory(256)
                        .expect("Failed to allocate memory");
                    
                    // 生成随机数
                    let mut rng = security_manager.create_secure_rng()
                        .expect("Failed to create RNG");
                    let mut buffer = [0u8; 32];
                    rng.fill_bytes(&mut buffer).expect("Failed to generate random bytes");
                }
                
                thread_start.elapsed()
            })
        }).collect();
        
        let thread_durations: Vec<Duration> = handles.into_iter()
            .map(|h| h.join().expect("Thread panicked"))
            .collect();
        
        let total_duration = start_time.elapsed();
        let total_operations = thread_count * operations_per_thread;
        let ops_per_sec = total_operations as f64 / total_duration.as_secs_f64();
        
        println!("Concurrent operations performance:");
        println!("  {} threads, {} ops/thread", thread_count, operations_per_thread);
        println!("  Total time: {:?}", total_duration);
        println!("  {:.2} ops/sec (combined)", ops_per_sec);
        
        // 验证并发性能是合理的
        assert!(ops_per_sec > 1000.0, 
            "Concurrent operations too slow: {:.2} ops/sec", ops_per_sec);
        
        // 验证各线程的执行时间相对均匀
        let max_duration = thread_durations.iter().max().unwrap();
        let min_duration = thread_durations.iter().min().unwrap();
        let duration_ratio = max_duration.as_nanos() as f64 / min_duration.as_nanos() as f64;
        
        println!("  Thread duration variance: {:.2}x", duration_ratio);
        
        // 线程执行时间差异不应该超过2倍
        assert!(duration_ratio < 2.0, 
            "Thread execution time variance too high: {:.2}x", duration_ratio);
    }
    
    #[test]
    fn bench_memory_scaling() {
        let sizes = vec![1024, 10240, 102400, 1024000]; // 1KB to 1MB
        
        for size in sizes {
            let start = Instant::now();
            let memory = SecureMemory::new(size).expect("Failed to allocate");
            let allocation_time = start.elapsed();
            
            // 测试内存清零时间
            let start = Instant::now();
            std::mem::drop(memory);
            let cleanup_time = start.elapsed();
            
            let alloc_mb_per_sec = (size as f64 / (1024.0 * 1024.0)) / allocation_time.as_secs_f64();
            let cleanup_mb_per_sec = (size as f64 / (1024.0 * 1024.0)) / cleanup_time.as_secs_f64();
            
            println!("Memory scaling ({} bytes):", size);
            println!("  Allocation: {:.2} MB/sec", alloc_mb_per_sec);
            println!("  Cleanup: {:.2} MB/sec", cleanup_mb_per_sec);
            
            // 确保内存操作时间随大小线性增长（而不是指数增长）
            if size >= 102400 { // 对于大内存才检查性能
                assert!(alloc_mb_per_sec > 100.0, 
                    "Memory allocation too slow for {} bytes: {:.2} MB/sec", 
                    size, alloc_mb_per_sec);
            }
        }
    }
    
    // Null audit sink for performance testing
    struct NullAuditSink;
    
    impl audit::AuditSink for NullAuditSink {
        fn log_entry(&self, _entry: &audit::AuditLogEntry) -> std::result::Result<(), Box<dyn std::error::Error>> {
            // 什么也不做，用于测试纯粹的日志记录开销
            Ok(())
        }
        
        fn flush(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }
}