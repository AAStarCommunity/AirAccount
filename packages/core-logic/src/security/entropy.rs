// Licensed to AirAccount under the Apache License, Version 2.0
// Enhanced entropy source implementation for TEE environments

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
// use zeroize::ZeroizeOnDrop; // 保留以备将来使用
use serde::{Deserialize, Serialize};

use super::{AuditEvent, AuditLogger};

/// 熵质量评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyQuality {
    /// 熵的位数估计
    pub estimated_bits: f64,
    /// 样本数量
    pub sample_count: usize,
    /// 通过的统计测试数量
    pub tests_passed: u32,
    /// 总的统计测试数量
    pub total_tests: u32,
    /// 质量评级
    pub quality_rating: EntropyRating,
}

/// 熵质量评级
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum EntropyRating {
    /// 优秀 - 加密强度熵
    Excellent,
    /// 良好 - 可用于大多数应用
    Good,
    /// 一般 - 勉强可用
    Fair,
    /// 差 - 不建议使用
    Poor,
}

impl std::fmt::Display for EntropyRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntropyRating::Excellent => write!(f, "Excellent"),
            EntropyRating::Good => write!(f, "Good"),
            EntropyRating::Fair => write!(f, "Fair"),
            EntropyRating::Poor => write!(f, "Poor"),
        }
    }
}

/// 熵源错误类型
#[derive(Debug)]
pub enum EntropyError {
    /// 熵质量不足
    InsufficientQuality(EntropyRating),
    /// 硬件错误
    HardwareError(String),
    /// 配置错误
    ConfigError(String),
    /// 内部错误
    InternalError(String),
}

impl std::fmt::Display for EntropyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntropyError::InsufficientQuality(rating) => {
                write!(f, "Insufficient entropy quality: {}", rating)
            }
            EntropyError::HardwareError(msg) => write!(f, "Hardware entropy error: {}", msg),
            EntropyError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            EntropyError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for EntropyError {}

/// 熵源特性
pub trait EntropySource: Send + Sync {
    /// 收集熵数据
    fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError>;
    
    /// 估计熵质量
    fn entropy_estimate(&self) -> Result<f64, EntropyError>;
    
    /// 熵源名称
    fn name(&self) -> &'static str;
    
    /// 是否可用
    fn is_available(&self) -> bool;
}

/// 硬件随机数生成器（模拟）
pub struct HardwareRng {
    available: bool,
}

impl HardwareRng {
    pub fn new() -> Result<Self, EntropyError> {
        // 检测硬件RNG可用性（在真实TEE环境中）
        let available = Self::detect_hardware_rng();
        
        Ok(Self { available })
    }
    
    fn detect_hardware_rng() -> bool {
        // 模拟硬件检测
        // 在真实环境中，这里会检查ARM TrustZone的TRNG等
        std::env::var("MOCK_HARDWARE_RNG").unwrap_or_default() == "available"
    }
}

impl EntropySource for HardwareRng {
    fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        if !self.available {
            return Err(EntropyError::HardwareError("Hardware RNG not available".to_string()));
        }
        
        // 模拟硬件RNG
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        rng.fill_bytes(buf);
        
        Ok(())
    }
    
    fn entropy_estimate(&self) -> Result<f64, EntropyError> {
        if !self.available {
            return Err(EntropyError::HardwareError("Hardware RNG not available".to_string()));
        }
        
        // 硬件RNG通常提供接近理想的熵
        Ok(8.0) // 每字节8位熵
    }
    
    fn name(&self) -> &'static str {
        "HardwareRNG"
    }
    
    fn is_available(&self) -> bool {
        self.available
    }
}

/// 时序抖动收集器
pub struct TimingJitterCollector {
    last_timestamp: Option<u64>,
    _jitter_pool: Vec<u8>,
}

impl TimingJitterCollector {
    pub fn new() -> Self {
        Self {
            last_timestamp: None,
            _jitter_pool: Vec::with_capacity(1024),
        }
    }
}

impl EntropySource for TimingJitterCollector {
    fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        let mut collected = 0;
        
        while collected < buf.len() {
            // 收集高精度时间戳
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| EntropyError::InternalError("Failed to get timestamp".to_string()))?
                .as_nanos() as u64;
            
            if let Some(last) = self.last_timestamp {
                // 计算时序差值（抖动）
                let jitter = (now.wrapping_sub(last) & 0xFF) as u8;
                buf[collected] = jitter;
                collected += 1;
            }
            
            self.last_timestamp = Some(now);
            
            // 添加少量延迟以增加抖动
            std::thread::yield_now();
        }
        
        Ok(())
    }
    
    fn entropy_estimate(&self) -> Result<f64, EntropyError> {
        // 时序抖动通常每字节提供2-4位熵
        Ok(3.0)
    }
    
    fn name(&self) -> &'static str {
        "TimingJitter"
    }
    
    fn is_available(&self) -> bool {
        true
    }
}

/// 物理噪声收集器（模拟）
pub struct PhysicalNoiseCollector {
    _noise_buffer: Vec<u8>,
}

impl PhysicalNoiseCollector {
    pub fn new() -> Self {
        Self {
            _noise_buffer: Vec::with_capacity(512),
        }
    }
}

impl EntropySource for PhysicalNoiseCollector {
    fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        // 模拟物理噪声收集（在真实环境中可能来自传感器、内存、CPU等）
        use rand::{RngCore, SeedableRng};
        use rand_chacha::ChaCha20Rng;
        
        // 使用系统时间和进程状态作为种子
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        
        for byte in buf.iter_mut() {
            *byte = (rng.next_u32() & 0xFF) as u8;
        }
        
        Ok(())
    }
    
    fn entropy_estimate(&self) -> Result<f64, EntropyError> {
        // 物理噪声质量取决于硬件，这里给保守估计
        Ok(2.5)
    }
    
    fn name(&self) -> &'static str {
        "PhysicalNoise"
    }
    
    fn is_available(&self) -> bool {
        true
    }
}

/// TEE专用熵源
pub struct TEEEntropySource {
    hw_rng: Option<HardwareRng>,
    timing_jitter: TimingJitterCollector,
    physical_noise: PhysicalNoiseCollector,
    audit_logger: Option<Arc<AuditLogger>>,
    quality_threshold: EntropyRating,
}

impl TEEEntropySource {
    /// 创建新的TEE熵源
    pub fn new() -> Result<Self, EntropyError> {
        let hw_rng = match HardwareRng::new() {
            Ok(rng) => Some(rng),
            Err(_) => None, // 硬件RNG不可用时继续使用软件熵源
        };
        
        Ok(Self {
            hw_rng,
            timing_jitter: TimingJitterCollector::new(),
            physical_noise: PhysicalNoiseCollector::new(),
            audit_logger: None,
            quality_threshold: EntropyRating::Good,
        })
    }
    
    /// 设置审计日志记录器
    pub fn with_audit_logger(mut self, logger: Arc<AuditLogger>) -> Self {
        self.audit_logger = Some(logger);
        self
    }
    
    /// 设置熵质量阈值
    pub fn with_quality_threshold(mut self, threshold: EntropyRating) -> Self {
        self.quality_threshold = threshold;
        self
    }
    
    /// 收集高质量熵
    pub fn gather_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        let start_time = std::time::Instant::now();
        
        // 先尝试硬件RNG
        if let Some(hw_rng) = &mut self.hw_rng {
            if hw_rng.is_available() {
                match hw_rng.gather_entropy(buf) {
                    Ok(()) => {
                        self.audit_entropy_collection("HardwareRNG", buf.len(), start_time);
                        return Ok(());
                    }
                    Err(_) => {
                        // 硬件RNG失败，降级到软件熵源
                    }
                }
            }
        }
        
        // 使用混合熵源
        self.gather_mixed_entropy(buf)?;
        
        // 质量检查
        let quality = self.assess_entropy_quality(buf)?;
        if quality.quality_rating < self.quality_threshold {
            return Err(EntropyError::InsufficientQuality(quality.quality_rating));
        }
        
        self.audit_entropy_collection("MixedSources", buf.len(), start_time);
        Ok(())
    }
    
    /// 收集混合熵源
    fn gather_mixed_entropy(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        let chunk_size = buf.len() / 2;
        
        // 从时序抖动收集一半
        self.timing_jitter.gather_entropy(&mut buf[..chunk_size])?;
        
        // 从物理噪声收集另一半
        self.physical_noise.gather_entropy(&mut buf[chunk_size..])?;
        
        // 混合熵源数据
        for i in 0..chunk_size {
            buf[i] ^= buf[i + chunk_size];
        }
        
        Ok(())
    }
    
    /// 评估熵质量
    fn assess_entropy_quality(&self, data: &[u8]) -> Result<EntropyQuality, EntropyError> {
        let mut tests_passed = 0u32;
        let total_tests = 4u32;
        
        // 1. 频率测试（0和1的比例）
        if self.frequency_test(data) {
            tests_passed += 1;
        }
        
        // 2. 游程测试
        if self.runs_test(data) {
            tests_passed += 1;
        }
        
        // 3. 熵估计测试
        let estimated_entropy = self.estimate_shannon_entropy(data);
        if estimated_entropy > 7.0 {
            tests_passed += 1;
        }
        
        // 4. 连续性测试
        if self.serial_correlation_test(data) {
            tests_passed += 1;
        }
        
        // 根据测试结果确定质量等级
        let quality_rating = match tests_passed {
            4 => EntropyRating::Excellent,
            3 => EntropyRating::Good,
            2 => EntropyRating::Fair,
            _ => EntropyRating::Poor,
        };
        
        Ok(EntropyQuality {
            estimated_bits: estimated_entropy,
            sample_count: data.len(),
            tests_passed,
            total_tests,
            quality_rating,
        })
    }
    
    /// 频率测试 - 检查0和1的分布均匀性
    fn frequency_test(&self, data: &[u8]) -> bool {
        let mut ones = 0;
        let total_bits = data.len() * 8;
        
        for &byte in data {
            ones += byte.count_ones() as usize;
        }
        
        let frequency = ones as f64 / total_bits as f64;
        
        // 期望频率应该接近0.5
        (frequency - 0.5).abs() < 0.1
    }
    
    /// 游程测试 - 检查连续相同位的长度分布
    fn runs_test(&self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        
        let mut runs = 0;
        let mut current_bit = data[0] & 1;
        
        for &byte in data {
            for i in 0..8 {
                let bit = (byte >> i) & 1;
                if bit != current_bit {
                    runs += 1;
                    current_bit = bit;
                }
            }
        }
        
        let total_bits = data.len() * 8;
        let expected_runs = (total_bits - 1) as f64 / 2.0;
        let actual_runs = runs as f64;
        
        // 游程数应该接近期望值
        (actual_runs - expected_runs).abs() < expected_runs * 0.2
    }
    
    /// 估计香农熵
    fn estimate_shannon_entropy(&self, data: &[u8]) -> f64 {
        let mut frequency = [0u32; 256];
        
        for &byte in data {
            frequency[byte as usize] += 1;
        }
        
        let length = data.len() as f64;
        let mut entropy = 0.0;
        
        for &count in &frequency {
            if count > 0 {
                let probability = count as f64 / length;
                entropy -= probability * probability.log2();
            }
        }
        
        entropy
    }
    
    /// 连续相关性测试
    fn serial_correlation_test(&self, data: &[u8]) -> bool {
        if data.len() < 2 {
            return false;
        }
        
        let mut correlation = 0.0;
        for i in 0..data.len() - 1 {
            correlation += (data[i] as f64) * (data[i + 1] as f64);
        }
        
        correlation /= (data.len() - 1) as f64;
        
        // 低相关性表示良好的随机性
        correlation.abs() < 32.0 // 经验阈值
    }
    
    /// 审计熵收集事件
    fn audit_entropy_collection(&self, source: &str, bytes: usize, start_time: std::time::Instant) {
        if let Some(logger) = &self.audit_logger {
            let _duration_ms = start_time.elapsed().as_millis() as u64; // 保留以备性能监控使用
            logger.log_info(
                AuditEvent::SecurityOperation {
                    operation: format!("entropy_collection_{}", source),
                    details: format!("Collected {} bytes of entropy", bytes),
                    success: true,
                    risk_level: "LOW".to_string(),
                },
                "entropy_source"
            );
        }
    }
}

impl Default for TEEEntropySource {
    fn default() -> Self {
        Self::new().expect("Failed to create TEE entropy source")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hardware_rng_availability() {
        let hw_rng = HardwareRng::new();
        assert!(hw_rng.is_ok());
    }
    
    #[test]
    fn test_timing_jitter_collector() {
        let mut collector = TimingJitterCollector::new();
        let mut buf = [0u8; 32];
        
        assert!(collector.gather_entropy(&mut buf).is_ok());
        assert!(collector.entropy_estimate().unwrap() > 0.0);
        
        // 检查是否有实际的熵
        let all_zero = buf.iter().all(|&x| x == 0);
        assert!(!all_zero);
    }
    
    #[test]
    fn test_entropy_quality_assessment() {
        let mut entropy_source = TEEEntropySource::new().unwrap();
        let mut buf = [0u8; 256];
        
        assert!(entropy_source.gather_entropy(&mut buf).is_ok());
        
        let quality = entropy_source.assess_entropy_quality(&buf).unwrap();
        assert!(quality.estimated_bits > 0.0);
        assert!(quality.tests_passed > 0);
    }
    
    #[test]
    fn test_entropy_mixing() {
        let mut entropy_source = TEEEntropySource::new().unwrap();
        let mut buf1 = [0u8; 64];
        let mut buf2 = [0u8; 64];
        
        assert!(entropy_source.gather_entropy(&mut buf1).is_ok());
        assert!(entropy_source.gather_entropy(&mut buf2).is_ok());
        
        // 两次收集的熵应该不同
        assert_ne!(buf1, buf2);
    }
    
    #[test]
    fn test_frequency_test() {
        let entropy_source = TEEEntropySource::new().unwrap();
        
        // 测试良好的随机数据
        let good_data = [0xAA, 0x55, 0xAA, 0x55]; // 交替模式
        assert!(entropy_source.frequency_test(&good_data));
        
        // 测试糟糕的数据
        let bad_data = [0xFF, 0xFF, 0xFF, 0xFF]; // 全1
        assert!(!entropy_source.frequency_test(&bad_data));
    }
}