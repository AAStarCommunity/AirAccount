// Licensed to AirAccount under the Apache License, Version 2.0
// Secure memory pool implementation for improved allocation performance

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use zeroize::Zeroize;

/// 固定大小内存池
pub struct FixedSizePool {
    block_size: usize,
    _alignment: usize,
    free_blocks: Mutex<VecDeque<NonNull<u8>>>,
    allocated_blocks: Mutex<Vec<NonNull<u8>>>,
    layout: Layout,
    max_blocks: usize,
    current_blocks: Mutex<usize>,
}

impl FixedSizePool {
    /// 创建新的固定大小内存池
    pub fn new(block_size: usize, max_blocks: usize) -> Result<Self, &'static str> {
        let alignment = std::mem::align_of::<u64>().max(8); // 至少8字节对齐
        let layout = Layout::from_size_align(block_size, alignment)
            .map_err(|_| "Invalid layout")?;
        
        Ok(Self {
            block_size,
            _alignment: alignment,
            free_blocks: Mutex::new(VecDeque::new()),
            allocated_blocks: Mutex::new(Vec::new()),
            layout,
            max_blocks,
            current_blocks: Mutex::new(0),
        })
    }
    
    /// 分配内存块
    pub fn allocate(&self) -> Result<NonNull<u8>, &'static str> {
        // 先尝试从空闲列表获取
        {
            let mut free_blocks = self.free_blocks.lock()
                .map_err(|_| "Failed to lock free blocks")?;
            
            if let Some(block) = free_blocks.pop_front() {
                return Ok(block);
            }
        }
        
        // 检查是否达到最大块数限制
        {
            let current_count = self.current_blocks.lock()
                .map_err(|_| "Failed to lock block count")?;
            
            if *current_count >= self.max_blocks {
                return Err("Memory pool exhausted");
            }
        }
        
        // 分配新块
        let ptr = unsafe { alloc_zeroed(self.layout) };
        if ptr.is_null() {
            return Err("Failed to allocate memory");
        }
        
        let non_null_ptr = NonNull::new(ptr).ok_or("Null pointer")?;
        
        // 更新计数器
        {
            let mut current_count = self.current_blocks.lock()
                .map_err(|_| "Failed to lock block count")?;
            *current_count += 1;
        }
        
        // 记录分配的块
        {
            let mut allocated = self.allocated_blocks.lock()
                .map_err(|_| "Failed to lock allocated blocks")?;
            allocated.push(non_null_ptr);
        }
        
        Ok(non_null_ptr)
    }
    
    /// 释放内存块
    pub fn deallocate(&self, ptr: NonNull<u8>) -> Result<(), &'static str> {
        // 验证这个指针是否是由这个池分配的
        {
            let mut allocated = self.allocated_blocks.lock()
                .map_err(|_| "Failed to lock allocated blocks")?;
            
            // 查找并移除指针
            let position = allocated.iter().position(|&p| p == ptr);
            if let Some(pos) = position {
                allocated.remove(pos);
            } else {
                return Err("Invalid pointer for this pool");
            }
        }
        
        // 安全清零内存
        unsafe {
            let slice = std::slice::from_raw_parts_mut(ptr.as_ptr(), self.block_size);
            slice.zeroize();
        }
        
        // 将块加入空闲列表
        {
            let mut free_blocks = self.free_blocks.lock()
                .map_err(|_| "Failed to lock free blocks")?;
            free_blocks.push_back(ptr);
        }
        
        Ok(())
    }
    
    /// 获取池统计信息
    pub fn get_stats(&self) -> Result<PoolStats, &'static str> {
        let free_count = self.free_blocks.lock()
            .map_err(|_| "Failed to lock free blocks")?
            .len();
            
        let allocated_count = self.allocated_blocks.lock()
            .map_err(|_| "Failed to lock allocated blocks")?
            .len();
            
        let current_count = *self.current_blocks.lock()
            .map_err(|_| "Failed to lock block count")?;
        
        Ok(PoolStats {
            block_size: self.block_size,
            max_blocks: self.max_blocks,
            current_blocks: current_count,
            free_blocks: free_count,
            allocated_blocks: allocated_count,
            utilization: (allocated_count as f64 / self.max_blocks as f64) * 100.0,
        })
    }
}

impl Drop for FixedSizePool {
    fn drop(&mut self) {
        // 清理所有分配的内存块
        if let Ok(allocated) = self.allocated_blocks.lock() {
            for &ptr in allocated.iter() {
                unsafe {
                    // 安全清零
                    let slice = std::slice::from_raw_parts_mut(ptr.as_ptr(), self.block_size);
                    slice.zeroize();
                    
                    // 释放内存
                    dealloc(ptr.as_ptr(), self.layout);
                }
            }
        }
        
        // 清理空闲块
        if let Ok(free_blocks) = self.free_blocks.lock() {
            for &ptr in free_blocks.iter() {
                unsafe {
                    let slice = std::slice::from_raw_parts_mut(ptr.as_ptr(), self.block_size);
                    slice.zeroize();
                    dealloc(ptr.as_ptr(), self.layout);
                }
            }
        }
    }
}

/// 内存池统计信息
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub block_size: usize,
    pub max_blocks: usize,
    pub current_blocks: usize,
    pub free_blocks: usize,
    pub allocated_blocks: usize,
    pub utilization: f64,
}

/// 安全内存池管理器
pub struct SecureMemoryPool {
    pools: Vec<Arc<FixedSizePool>>,
    large_alloc_threshold: usize,
    total_allocated: Mutex<usize>,
    allocation_stats: Mutex<AllocationStats>,
}

/// 分配统计
#[derive(Debug, Default, Clone)]
pub struct AllocationStats {
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub peak_memory_usage: usize,
    pub current_memory_usage: usize,
    pub pool_hits: u64,
    pub pool_misses: u64,
}

impl SecureMemoryPool {
    /// 创建新的安全内存池
    pub fn new() -> Result<Self, &'static str> {
        let pools = vec![
            // 常用大小的内存池
            Arc::new(FixedSizePool::new(32, 1000)?),      // 32字节
            Arc::new(FixedSizePool::new(64, 800)?),       // 64字节
            Arc::new(FixedSizePool::new(128, 600)?),      // 128字节
            Arc::new(FixedSizePool::new(256, 400)?),      // 256字节
            Arc::new(FixedSizePool::new(512, 200)?),      // 512字节
            Arc::new(FixedSizePool::new(1024, 100)?),     // 1KB
            Arc::new(FixedSizePool::new(2048, 50)?),      // 2KB
            Arc::new(FixedSizePool::new(4096, 25)?),      // 4KB
        ];
        
        Ok(Self {
            pools,
            large_alloc_threshold: 8192, // 8KB以上使用直接分配
            total_allocated: Mutex::new(0),
            allocation_stats: Mutex::new(AllocationStats::default()),
        })
    }
    
    /// 分配安全内存
    pub fn allocate(&self, size: usize) -> Result<SecureMemoryBlock, &'static str> {
        let _start_time = std::time::Instant::now(); // 保留以备性能监控使用
        
        // 更新统计
        {
            let mut stats = self.allocation_stats.lock()
                .map_err(|_| "Failed to lock stats")?;
            stats.total_allocations += 1;
        }
        
        // 如果请求大小超过阈值，使用直接分配
        if size > self.large_alloc_threshold {
            return self.allocate_large(size);
        }
        
        // 寻找合适的内存池
        for pool in &self.pools {
            let pool_stats = pool.get_stats()?;
            if size <= pool_stats.block_size {
                match pool.allocate() {
                    Ok(ptr) => {
                        // 池分配成功
                        let block = SecureMemoryBlock {
                            ptr,
                            size: pool_stats.block_size,
                            actual_size: size,
                            pool: Some(Arc::clone(pool)),
                        };
                        
                        self.update_allocation_stats(pool_stats.block_size, true)?;
                        return Ok(block);
                    }
                    Err(_) => continue, // 尝试下一个池
                }
            }
        }
        
        // 所有池都无法分配，使用直接分配
        self.update_allocation_stats(0, false)?;
        self.allocate_large(size)
    }
    
    /// 大内存直接分配
    fn allocate_large(&self, size: usize) -> Result<SecureMemoryBlock, &'static str> {
        let layout = Layout::from_size_align(size, std::mem::align_of::<u64>())
            .map_err(|_| "Invalid layout")?;
        
        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err("Failed to allocate memory");
        }
        
        let non_null_ptr = NonNull::new(ptr).ok_or("Null pointer")?;
        
        let block = SecureMemoryBlock {
            ptr: non_null_ptr,
            size,
            actual_size: size,
            pool: None, // 直接分配，没有关联池
        };
        
        self.update_allocation_stats(size, false)?;
        Ok(block)
    }
    
    /// 更新分配统计
    fn update_allocation_stats(&self, allocated_size: usize, is_pool_hit: bool) -> Result<(), &'static str> {
        let mut total_allocated = self.total_allocated.lock()
            .map_err(|_| "Failed to lock total allocated")?;
        *total_allocated += allocated_size;
        
        let mut stats = self.allocation_stats.lock()
            .map_err(|_| "Failed to lock stats")?;
        
        stats.current_memory_usage += allocated_size;
        if stats.current_memory_usage > stats.peak_memory_usage {
            stats.peak_memory_usage = stats.current_memory_usage;
        }
        
        if is_pool_hit {
            stats.pool_hits += 1;
        } else {
            stats.pool_misses += 1;
        }
        
        Ok(())
    }
    
    /// 处理内存块释放
    pub fn deallocate(&self, block: &SecureMemoryBlock) -> Result<(), &'static str> {
        // 更新统计
        {
            let mut stats = self.allocation_stats.lock()
                .map_err(|_| "Failed to lock stats")?;
            stats.total_deallocations += 1;
            stats.current_memory_usage = stats.current_memory_usage.saturating_sub(block.size);
        }
        
        // 如果有关联的池，返回给池
        if let Some(pool) = &block.pool {
            pool.deallocate(block.ptr)?;
        } else {
            // 直接分配的内存，直接释放
            unsafe {
                let slice = std::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.size);
                slice.zeroize();
                
                let layout = Layout::from_size_align(block.size, std::mem::align_of::<u64>())
                    .map_err(|_| "Invalid layout")?;
                dealloc(block.ptr.as_ptr(), layout);
            }
        }
        
        let mut total_allocated = self.total_allocated.lock()
            .map_err(|_| "Failed to lock total allocated")?;
        *total_allocated = total_allocated.saturating_sub(block.size);
        
        Ok(())
    }
    
    /// 获取池的全面统计信息
    pub fn get_comprehensive_stats(&self) -> Result<MemoryPoolStats, &'static str> {
        let mut pool_stats = Vec::new();
        
        for pool in &self.pools {
            pool_stats.push(pool.get_stats()?);
        }
        
        let allocation_stats = self.allocation_stats.lock()
            .map_err(|_| "Failed to lock stats")?
            .clone();
        
        let total_allocated = *self.total_allocated.lock()
            .map_err(|_| "Failed to lock total allocated")?;
        
        Ok(MemoryPoolStats {
            pool_stats,
            allocation_stats,
            total_allocated,
            large_alloc_threshold: self.large_alloc_threshold,
        })
    }
}

/// 安全内存块
pub struct SecureMemoryBlock {
    ptr: NonNull<u8>,
    size: usize,
    actual_size: usize,
    pool: Option<Arc<FixedSizePool>>,
}

impl SecureMemoryBlock {
    /// 获取内存块的可变引用
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.actual_size) }
    }
    
    /// 获取内存块的不可变引用
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.actual_size) }
    }
    
    /// 获取实际大小
    pub fn len(&self) -> usize {
        self.actual_size
    }
    
    /// 获取分配大小
    pub fn capacity(&self) -> usize {
        self.size
    }
    
    /// 安全清零
    pub fn secure_zero(&mut self) {
        let slice = self.as_mut_slice();
        slice.zeroize();
    }
}

/// 内存池全面统计
#[derive(Debug, Clone)]
pub struct MemoryPoolStats {
    pub pool_stats: Vec<PoolStats>,
    pub allocation_stats: AllocationStats,
    pub total_allocated: usize,
    pub large_alloc_threshold: usize,
}

impl std::fmt::Display for MemoryPoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Memory Pool Statistics:")?;
        writeln!(f, "  Total Allocated: {} bytes", self.total_allocated)?;
        writeln!(f, "  Large Allocation Threshold: {} bytes", self.large_alloc_threshold)?;
        writeln!(f, "  Total Allocations: {}", self.allocation_stats.total_allocations)?;
        writeln!(f, "  Total Deallocations: {}", self.allocation_stats.total_deallocations)?;
        writeln!(f, "  Pool Hits: {}", self.allocation_stats.pool_hits)?;
        writeln!(f, "  Pool Misses: {}", self.allocation_stats.pool_misses)?;
        writeln!(f, "  Peak Memory Usage: {} bytes", self.allocation_stats.peak_memory_usage)?;
        writeln!(f, "  Current Memory Usage: {} bytes", self.allocation_stats.current_memory_usage)?;
        
        writeln!(f, "  Pool Details:")?;
        for (i, pool_stat) in self.pool_stats.iter().enumerate() {
            writeln!(f, "    Pool {}: {} bytes, {}/{} blocks ({:.1}% utilization)", 
                   i, pool_stat.block_size, pool_stat.allocated_blocks, 
                   pool_stat.max_blocks, pool_stat.utilization)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fixed_size_pool() {
        let pool = FixedSizePool::new(64, 10).unwrap();
        
        // 测试分配
        let ptr1 = pool.allocate().unwrap();
        let ptr2 = pool.allocate().unwrap();
        
        assert_ne!(ptr1, ptr2);
        
        // 测试释放
        assert!(pool.deallocate(ptr1).is_ok());
        assert!(pool.deallocate(ptr2).is_ok());
        
        // 测试统计
        let stats = pool.get_stats().unwrap();
        assert_eq!(stats.block_size, 64);
        assert_eq!(stats.free_blocks, 2);
    }
    
    #[test]
    fn test_secure_memory_pool() {
        let pool = SecureMemoryPool::new().unwrap();
        
        // 测试小块分配
        let block1 = pool.allocate(32).unwrap();
        assert_eq!(block1.len(), 32);
        assert!(block1.capacity() >= 32);
        
        // 测试大块分配
        let block2 = pool.allocate(10240).unwrap(); // 10KB
        assert_eq!(block2.len(), 10240);
        
        // 测试释放
        assert!(pool.deallocate(&block1).is_ok());
        assert!(pool.deallocate(&block2).is_ok());
        
        // 检查统计
        let stats = pool.get_comprehensive_stats().unwrap();
        assert_eq!(stats.allocation_stats.total_allocations, 2);
        assert_eq!(stats.allocation_stats.total_deallocations, 2);
    }
    
    #[test]
    fn test_memory_block_operations() {
        let pool = SecureMemoryPool::new().unwrap();
        let mut block = pool.allocate(128).unwrap();
        
        // 测试写入
        {
            let slice = block.as_mut_slice();
            slice[0] = 0xAA;
            slice[127] = 0xBB;
        }
        
        // 测试读取
        {
            let slice = block.as_slice();
            assert_eq!(slice[0], 0xAA);
            assert_eq!(slice[127], 0xBB);
        }
        
        // 测试清零
        block.secure_zero();
        {
            let slice = block.as_slice();
            assert_eq!(slice[0], 0x00);
            assert_eq!(slice[127], 0x00);
        }
        
        pool.deallocate(&block).unwrap();
    }
    
    #[test]
    fn test_pool_exhaustion() {
        let pool = FixedSizePool::new(64, 2).unwrap(); // 只允许2个块
        
        let _ptr1 = pool.allocate().unwrap();
        let _ptr2 = pool.allocate().unwrap();
        
        // 第三次分配应该失败
        assert!(pool.allocate().is_err());
        
        let stats = pool.get_stats().unwrap();
        assert_eq!(stats.allocated_blocks, 2);
        assert_eq!(stats.utilization, 100.0);
    }
}