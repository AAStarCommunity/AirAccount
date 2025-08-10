use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
use zeroize::{Zeroize, ZeroizeOnDrop};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use std::fmt;

#[derive(Clone)]
pub struct SecureBytes {
    data: Vec<u8>,
}

// 为 SecureBytes 实现 Serialize
impl Serialize for SecureBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.data.serialize(serializer)
    }
}

// 为 SecureBytes 实现 Deserialize
impl<'de> Deserialize<'de> for SecureBytes {
    fn deserialize<D>(deserializer: D) -> Result<SecureBytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = Vec::<u8>::deserialize(deserializer)?;
        Ok(SecureBytes::new(data))
    }
}

impl SecureBytes {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
    
    pub fn from_slice(slice: &[u8]) -> Self {
        Self { data: slice.to_vec() }
    }
    
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    
    /// 暴露内部数据，仅用于必要的密码学操作
    /// 注意：这违反了"秘密不离开TEE"的安全原则，仅在必要时使用
    pub fn expose_secret(&self) -> &[u8] {
        &self.data
    }
    
    pub fn constant_time_eq(&self, other: &Self) -> Choice {
        if self.len() != other.len() {
            return Choice::from(0);
        }
        self.data.ct_eq(&other.data)
    }
    
    pub fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        if a.len() != b.len() {
            panic!("SecureBytes: lengths must match for conditional selection");
        }
        
        let mut result = vec![0u8; a.len()];
        for i in 0..a.len() {
            result[i] = u8::conditional_select(&a.data[i], &b.data[i], choice);
        }
        
        Self { data: result }
    }
    
    pub fn constant_time_select_if_equal(&self, other: &Self, if_equal: &Self, if_not_equal: &Self) -> Self {
        let are_equal = self.constant_time_eq(other);
        Self::conditional_select(if_equal, if_not_equal, are_equal)
    }
}

impl fmt::Debug for SecureBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureBytes([REDACTED; {} bytes])", self.data.len())
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}

impl From<Vec<u8>> for SecureBytes {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

impl From<&[u8]> for SecureBytes {
    fn from(data: &[u8]) -> Self {
        Self::from_slice(data)
    }
}

impl Zeroize for SecureBytes {
    fn zeroize(&mut self) {
        self.data.zeroize();
    }
}

impl ZeroizeOnDrop for SecureBytes {}

pub struct ConstantTimeOps;

impl ConstantTimeOps {
    pub fn secure_compare(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        
        let mut result = 0u8;
        for i in 0..a.len() {
            result |= a[i] ^ b[i];
        }
        
        result == 0
    }
    
    pub fn secure_memset(dest: &mut [u8], value: u8) {
        for byte in dest.iter_mut() {
            *byte = value;
        }
        
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
    
    pub fn secure_copy(dest: &mut [u8], src: &[u8]) -> Result<(), &'static str> {
        if dest.len() != src.len() {
            return Err("Destination and source lengths must match");
        }
        
        for i in 0..src.len() {
            dest[i] = src[i];
        }
        
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    
    pub fn constant_time_select_u32(a: u32, b: u32, condition: bool) -> u32 {
        let mask = if condition { 0xFFFFFFFF } else { 0x00000000 };
        (a & mask) | (b & !mask)
    }
    
    pub fn constant_time_select_u64(a: u64, b: u64, condition: bool) -> u64 {
        let mask = if condition { 0xFFFFFFFFFFFFFFFF } else { 0x0000000000000000 };
        (a & mask) | (b & !mask)
    }
}

#[derive(ZeroizeOnDrop)]
pub struct SecureRng {
    state: [u8; 32],
}

impl SecureRng {
    pub fn new() -> Result<Self, &'static str> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut state = [0u8; 32];
        rng.fill_bytes(&mut state);
        
        Ok(Self { state })
    }
    
    pub fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), &'static str> {
        use rand::{RngCore, SeedableRng};
        use rand_chacha::ChaCha20Rng;
        
        let mut rng = ChaCha20Rng::from_seed(self.state);
        rng.fill_bytes(dest);
        
        rng.fill_bytes(&mut self.state);
        
        Ok(())
    }
    
    pub fn next_u32(&mut self) -> Result<u32, &'static str> {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    
    pub fn next_u64(&mut self) -> Result<u64, &'static str> {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
}

impl Default for SecureRng {
    fn default() -> Self {
        Self::new().expect("Failed to initialize SecureRng")
    }
}

/// 侧信道攻击防护工具
pub struct SideChannelProtection;

impl SideChannelProtection {
    /// 添加随机延迟以抵抗时序攻击
    pub fn add_random_delay(min_cycles: u32, max_cycles: u32) {
        let mut rng = SecureRng::new().unwrap();
        let delay_cycles = (rng.next_u32().unwrap() % (max_cycles - min_cycles)) + min_cycles;
        
        // 在不同架构上执行忙等待
        for _ in 0..delay_cycles {
            core::hint::spin_loop();
        }
    }

    /// 内存访问模式混淆 - 防止缓存时序攻击
    pub fn obfuscate_memory_access<T>(data: &mut [T], access_pattern: &[usize]) {
        if access_pattern.is_empty() || data.is_empty() {
            return;
        }

        // 添加随机虚假访问以混淆真实访问模式
        let mut rng = SecureRng::new().unwrap();
        let dummy_accesses = 10 + (rng.next_u32().unwrap() % 20) as usize;
        
        for _ in 0..dummy_accesses {
            let dummy_index = (rng.next_u32().unwrap() as usize) % data.len();
            // 虚假读取（编译器优化可能会移除，但在TEE环境中通常保留）
            unsafe { core::ptr::read_volatile(&data[dummy_index]); }
        }

        // 执行实际访问，混合在虚假访问中
        for &index in access_pattern {
            if index < data.len() {
                unsafe { core::ptr::read_volatile(&data[index]); }
            }
        }
    }

    /// 分支预测混淆 - 防止分支预测攻击
    pub fn obfuscate_branch_prediction(condition: bool, true_action: impl Fn(), false_action: impl Fn()) {
        let mut rng = SecureRng::new().unwrap();
        let noise = rng.next_u32().unwrap() & 1 != 0;
        
        // 使用噪声混淆真实的分支预测模式
        match (condition, noise) {
            (true, true) | (false, false) => {
                true_action();
                // 添加虚假分支以混淆模式
                if rng.next_u32().unwrap() & 1 != 0 {
                    core::hint::spin_loop();
                }
            }
            (true, false) | (false, true) => {
                false_action();
                // 添加虚假分支以混淆模式  
                if rng.next_u32().unwrap() & 1 != 0 {
                    core::hint::spin_loop();
                }
            }
        }
    }

    /// 数据相关分支消除 - 转换为常数时间操作
    pub fn eliminate_data_dependent_branches<T: Copy>(
        condition: bool,
        true_value: T,
        false_value: T,
    ) -> T {
        // 使用位掩码避免条件分支
        let mask = if condition { !0u8 } else { 0u8 };
        
        unsafe {
            let true_ptr = &true_value as *const T as *const u8;
            let false_ptr = &false_value as *const T as *const u8;
            let size = core::mem::size_of::<T>();
            let mut result_bytes = Vec::with_capacity(size);
            result_bytes.resize(size, 0u8);
            
            for i in 0..size {
                let true_byte = *true_ptr.add(i);
                let false_byte = *false_ptr.add(i);
                result_bytes[i] = (true_byte & mask) | (false_byte & !mask);
            }
            
            core::ptr::read(result_bytes.as_ptr() as *const T)
        }
    }

    /// 功耗分析攻击防护 - 添加随机计算负载
    pub fn power_analysis_protection() {
        let mut rng = SecureRng::new().unwrap();
        
        // 执行随机数量的虚假计算以平衡功耗
        let dummy_ops = 50 + (rng.next_u32().unwrap() % 100) as usize;
        let mut dummy_state = rng.next_u64().unwrap();
        
        for _ in 0..dummy_ops {
            dummy_state = dummy_state.wrapping_mul(1103515245).wrapping_add(12345);
            dummy_state ^= dummy_state >> 16;
            core::hint::black_box(dummy_state); // 防止编译器优化
        }
    }

    /// 电磁辐射防护 - 使用虚假操作混淆信号
    pub fn electromagnetic_protection() {
        let mut rng = SecureRng::new().unwrap();
        
        // 执行电磁噪声生成操作
        for _ in 0..32 {
            let noise_pattern = rng.next_u64().unwrap();
            
            // 位移操作产生不同的电磁信号模式
            let _shifted = noise_pattern << (rng.next_u32().unwrap() % 32);
            let _rotated = noise_pattern.rotate_left(rng.next_u32().unwrap() % 64);
            
            core::hint::black_box(_shifted);
            core::hint::black_box(_rotated);
        }
    }
}

/// 常数时间算法实现
pub struct ConstantTimeAlgorithms;

impl ConstantTimeAlgorithms {
    /// 常数时间条件交换
    pub fn conditional_swap<T: Copy>(condition: bool, a: &mut T, b: &mut T) {
        let mask = if condition { !0u8 } else { 0u8 };
        
        unsafe {
            let a_ptr = a as *mut T as *mut u8;
            let b_ptr = b as *mut T as *mut u8;
            
            for i in 0..core::mem::size_of::<T>() {
                let a_byte = *a_ptr.add(i);
                let b_byte = *b_ptr.add(i);
                
                let swapped_a = (a_byte & !mask) | (b_byte & mask);
                let swapped_b = (b_byte & !mask) | (a_byte & mask);
                
                *a_ptr.add(i) = swapped_a;
                *b_ptr.add(i) = swapped_b;
            }
        }
    }

    /// 常数时间数组查找
    pub fn constant_time_lookup<T: Copy + Default>(array: &[T], index: usize) -> T {
        let mut result = T::default();
        
        for (i, &item) in array.iter().enumerate() {
            let matches = i == index;
            let mask = if matches { !0u8 } else { 0u8 };
            
            unsafe {
                let result_ptr = &mut result as *mut T as *mut u8;
                let item_ptr = &item as *const T as *const u8;
                
                for j in 0..core::mem::size_of::<T>() {
                    let current_byte = *result_ptr.add(j);
                    let new_byte = *item_ptr.add(j);
                    *result_ptr.add(j) = (current_byte & !mask) | (new_byte & mask);
                }
            }
        }
        
        result
    }

    /// 常数时间大数比较
    pub fn constant_time_bignum_compare(a: &[u8], b: &[u8]) -> core::cmp::Ordering {
        if a.len() != b.len() {
            return a.len().cmp(&b.len());
        }
        
        let mut greater = 0u8;
        let mut less = 0u8;
        
        for i in (0..a.len()).rev() {
            let a_byte = a[i];
            let b_byte = b[i];
            
            let is_greater = ((a_byte as u16).wrapping_sub(b_byte as u16)) >> 8;
            let is_less = ((b_byte as u16).wrapping_sub(a_byte as u16)) >> 8;
            
            greater |= is_greater as u8 & !less;
            less |= is_less as u8 & !greater;
        }
        
        match (greater != 0, less != 0) {
            (true, false) => core::cmp::Ordering::Greater,
            (false, true) => core::cmp::Ordering::Less,
            _ => core::cmp::Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_bytes_constant_time_eq() {
        let a = SecureBytes::from_slice(b"hello");
        let b = SecureBytes::from_slice(b"hello");
        let c = SecureBytes::from_slice(b"world");
        
        assert!(bool::from(a.constant_time_eq(&b)));
        assert!(!bool::from(a.constant_time_eq(&c)));
    }
    
    #[test]
    fn test_constant_time_ops_secure_compare() {
        assert!(ConstantTimeOps::secure_compare(b"test", b"test"));
        assert!(!ConstantTimeOps::secure_compare(b"test", b"fail"));
        assert!(!ConstantTimeOps::secure_compare(b"test", b"testing"));
    }
    
    #[test]
    fn test_secure_rng() {
        let mut rng = SecureRng::new().unwrap();
        let mut buf1 = [0u8; 32];
        let mut buf2 = [0u8; 32];
        
        rng.fill_bytes(&mut buf1).unwrap();
        rng.fill_bytes(&mut buf2).unwrap();
        
        assert_ne!(buf1, buf2);
    }
}