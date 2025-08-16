// BIP39种子缓存机制 - 性能优化实现
// 根据性能评估报告，实现种子缓存可以将密钥派生延迟从12ms降低到2ms (83%提升)

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use sha2::{Sha256, Digest};
use zeroize::Zeroize;

/// 缓存条目结构
#[derive(Clone)]
struct CacheEntry {
    /// 缓存的种子数据（敏感数据）
    seed: [u8; 64],
    /// 缓存创建时间
    created_at: Instant,
    /// 最后访问时间
    last_accessed: Instant,
    /// 访问计数
    hit_count: u64,
}

impl Drop for CacheEntry {
    fn drop(&mut self) {
        // 安全清理敏感数据
        self.seed.zeroize();
    }
}

/// BIP39种子缓存管理器
/// 使用LRU策略和TTL过期机制
pub struct SeedCache {
    /// 缓存存储 - 使用助记词哈希作为key
    cache: Arc<RwLock<HashMap<[u8; 32], CacheEntry>>>,
    /// 最大缓存条目数
    max_entries: usize,
    /// 缓存TTL（生存时间）
    ttl: Duration,
    /// 缓存命中统计
    hit_count: Arc<RwLock<u64>>,
    /// 缓存未命中统计
    miss_count: Arc<RwLock<u64>>,
}

impl SeedCache {
    /// 创建新的种子缓存实例
    pub fn new(max_entries: usize, ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::with_capacity(max_entries))),
            max_entries,
            ttl: Duration::from_secs(ttl_seconds),
            hit_count: Arc::new(RwLock::new(0)),
            miss_count: Arc::new(RwLock::new(0)),
        }
    }

    /// 默认配置：10个条目，5分钟TTL
    pub fn default() -> Self {
        Self::new(10, 300)
    }

    /// 计算助记词的安全哈希（用作缓存key）
    fn hash_mnemonic(mnemonic: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(mnemonic.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// 从缓存获取种子，如果不存在则计算并缓存
    pub fn get_or_compute<F>(&self, mnemonic: &str, compute_fn: F) -> Result<[u8; 64], String>
    where
        F: FnOnce(&str) -> Result<[u8; 64], String>,
    {
        let key = Self::hash_mnemonic(mnemonic);
        
        // 尝试从缓存读取
        {
            let cache = self.cache.read().map_err(|e| format!("Cache lock error: {}", e))?;
            
            if let Some(entry) = cache.get(&key) {
                // 检查TTL
                if entry.created_at.elapsed() < self.ttl {
                    // 更新统计
                    *self.hit_count.write().unwrap() += 1;
                    
                    // 返回缓存的种子（克隆以避免借用问题）
                    return Ok(entry.seed);
                }
            }
        }
        
        // 缓存未命中，计算新种子
        *self.miss_count.write().unwrap() += 1;
        
        let seed = compute_fn(mnemonic)?;
        
        // 写入缓存
        {
            let mut cache = self.cache.write().map_err(|e| format!("Cache lock error: {}", e))?;
            
            // LRU淘汰策略：如果缓存满了，移除最老的条目
            if cache.len() >= self.max_entries {
                // 找到最老的条目
                if let Some(oldest_key) = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(k, _)| *k)
                {
                    // 移除并安全清理
                    if let Some(mut old_entry) = cache.remove(&oldest_key) {
                        old_entry.seed.zeroize();
                    }
                }
            }
            
            // 插入新条目
            let entry = CacheEntry {
                seed,
                created_at: Instant::now(),
                last_accessed: Instant::now(),
                hit_count: 0,
            };
            
            cache.insert(key, entry);
        }
        
        Ok(seed)
    }

    /// 清空缓存（安全清理所有敏感数据）
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        for (_, mut entry) in cache.drain() {
            entry.seed.zeroize();
        }
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> CacheStats {
        let hits = *self.hit_count.read().unwrap();
        let misses = *self.miss_count.read().unwrap();
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        
        CacheStats {
            hit_count: hits,
            miss_count: misses,
            hit_rate,
            cache_size: self.cache.read().unwrap().len(),
            max_size: self.max_entries,
        }
    }

    /// 预热缓存（批量加载常用种子）
    pub fn warm_up<F>(&self, mnemonics: &[String], compute_fn: F) -> Result<(), String>
    where
        F: Fn(&str) -> Result<[u8; 64], String>,
    {
        for mnemonic in mnemonics {
            self.get_or_compute(mnemonic, |m| compute_fn(m))?;
        }
        Ok(())
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
    pub cache_size: usize,
    pub max_size: usize,
}

impl Drop for SeedCache {
    fn drop(&mut self) {
        // 确保所有敏感数据被安全清理
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_compute_seed(mnemonic: &str) -> Result<[u8; 64], String> {
        // 模拟昂贵的PBKDF2计算
        std::thread::sleep(Duration::from_millis(10));
        let mut seed = [0u8; 64];
        seed[0] = mnemonic.len() as u8;
        Ok(seed)
    }

    #[test]
    fn test_cache_hit_performance() {
        let cache = SeedCache::new(5, 60);
        let mnemonic = "test mnemonic phrase";
        
        // 第一次调用 - 缓存未命中
        let start = Instant::now();
        let seed1 = cache.get_or_compute(mnemonic, mock_compute_seed).unwrap();
        let first_call_time = start.elapsed();
        
        // 第二次调用 - 缓存命中
        let start = Instant::now();
        let seed2 = cache.get_or_compute(mnemonic, mock_compute_seed).unwrap();
        let second_call_time = start.elapsed();
        
        // 验证种子相同
        assert_eq!(seed1, seed2);
        
        // 验证性能提升（缓存命中应该快得多）
        assert!(second_call_time < first_call_time / 10);
        
        // 验证统计
        let stats = cache.get_stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert!(stats.hit_rate > 49.0 && stats.hit_rate < 51.0);
    }

    #[test]
    fn test_lru_eviction() {
        let cache = SeedCache::new(2, 60);
        
        // 填满缓存
        cache.get_or_compute("mnemonic1", mock_compute_seed).unwrap();
        cache.get_or_compute("mnemonic2", mock_compute_seed).unwrap();
        
        // 添加第三个条目，应该淘汰最老的
        cache.get_or_compute("mnemonic3", mock_compute_seed).unwrap();
        
        let stats = cache.get_stats();
        assert_eq!(stats.cache_size, 2);
    }

    #[test]
    fn test_ttl_expiration() {
        let cache = SeedCache::new(5, 1); // 1秒TTL
        let mnemonic = "test mnemonic";
        
        // 第一次调用
        cache.get_or_compute(mnemonic, mock_compute_seed).unwrap();
        
        // 等待TTL过期
        std::thread::sleep(Duration::from_secs(2));
        
        // 应该重新计算
        let start = Instant::now();
        cache.get_or_compute(mnemonic, mock_compute_seed).unwrap();
        let elapsed = start.elapsed();
        
        // 验证确实重新计算了（时间较长）
        assert!(elapsed > Duration::from_millis(9));
    }

    #[test]
    fn test_cache_clear() {
        let cache = SeedCache::new(5, 60);
        
        cache.get_or_compute("mnemonic1", mock_compute_seed).unwrap();
        cache.get_or_compute("mnemonic2", mock_compute_seed).unwrap();
        
        assert_eq!(cache.get_stats().cache_size, 2);
        
        cache.clear();
        
        assert_eq!(cache.get_stats().cache_size, 0);
    }
}