// Licensed to AirAccount under the Apache License, Version 2.0
// SIMD-accelerated memory operations for enhanced performance

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// SIMD功能检测
pub struct SimdCapabilities {
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_neon: bool,
    pub has_sse4_2: bool,
}

impl SimdCapabilities {
    /// 检测当前系统的SIMD功能
    pub fn detect() -> Self {
        Self {
            has_avx2: Self::has_avx2(),
            has_avx512: Self::has_avx512(),
            has_neon: Self::has_neon(),
            has_sse4_2: Self::has_sse4_2(),
        }
    }
    
    #[cfg(target_arch = "x86_64")]
    fn has_avx2() -> bool {
        is_x86_feature_detected!("avx2")
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn has_avx2() -> bool { false }
    
    #[cfg(target_arch = "x86_64")]
    fn has_avx512() -> bool {
        is_x86_feature_detected!("avx512f")
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn has_avx512() -> bool { false }
    
    #[cfg(target_arch = "aarch64")]
    fn has_neon() -> bool {
        // NEON在AArch64上默认可用
        true
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    fn has_neon() -> bool { false }
    
    #[cfg(target_arch = "x86_64")]
    fn has_sse4_2() -> bool {
        is_x86_feature_detected!("sse4.2")
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn has_sse4_2() -> bool { false }
}

/// SIMD加速的内存操作
pub struct SimdMemoryOps {
    capabilities: SimdCapabilities,
}

impl SimdMemoryOps {
    /// 创建新的SIMD内存操作实例
    pub fn new() -> Self {
        Self {
            capabilities: SimdCapabilities::detect(),
        }
    }
    
    /// 安全清零内存（SIMD加速）
    pub fn secure_zero(&self, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }
        
        // 选择最优的清零实现
        if self.capabilities.has_avx512 && data.len() >= 64 {
            self.secure_zero_avx512(data);
        } else if self.capabilities.has_avx2 && data.len() >= 32 {
            self.secure_zero_avx2(data);
        } else if self.capabilities.has_neon && data.len() >= 16 {
            self.secure_zero_neon(data);
        } else if self.capabilities.has_sse4_2 && data.len() >= 16 {
            self.secure_zero_sse(data);
        } else {
            self.secure_zero_scalar(data);
        }
        
        // 编译器栅栏，防止优化
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
    
    /// AVX512清零实现
    #[cfg(target_arch = "x86_64")]
    fn secure_zero_avx512(&self, data: &mut [u8]) {
        unsafe {
            let zero = _mm512_setzero_si512();
            let mut ptr = data.as_mut_ptr();
            let end = ptr.add(data.len());
            
            // 处理64字节对齐的块
            while ptr.add(64) <= end {
                _mm512_storeu_si512(ptr as *mut __m512i, zero);
                ptr = ptr.add(64);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr) as usize;
            if remaining > 0 {
                let remaining_slice = std::slice::from_raw_parts_mut(ptr, remaining);
                self.secure_zero_scalar(remaining_slice);
            }
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_zero_avx512(&self, data: &mut [u8]) {
        self.secure_zero_scalar(data);
    }
    
    /// AVX2清零实现
    #[cfg(target_arch = "x86_64")]
    fn secure_zero_avx2(&self, data: &mut [u8]) {
        unsafe {
            let zero = _mm256_setzero_si256();
            let mut ptr = data.as_mut_ptr();
            let end = ptr.add(data.len());
            
            // 处理32字节对齐的块
            while ptr.add(32) <= end {
                _mm256_storeu_si256(ptr as *mut __m256i, zero);
                ptr = ptr.add(32);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr) as usize;
            if remaining > 0 {
                let remaining_slice = std::slice::from_raw_parts_mut(ptr, remaining);
                self.secure_zero_scalar(remaining_slice);
            }
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_zero_avx2(&self, data: &mut [u8]) {
        self.secure_zero_scalar(data);
    }
    
    /// NEON清零实现
    #[cfg(target_arch = "aarch64")]
    fn secure_zero_neon(&self, data: &mut [u8]) {
        unsafe {
            let zero = vdupq_n_u8(0);
            let mut ptr = data.as_mut_ptr();
            let end = ptr.add(data.len());
            
            // 处理16字节对齐的块
            while ptr.add(16) <= end {
                vst1q_u8(ptr, zero);
                ptr = ptr.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr) as usize;
            if remaining > 0 {
                let remaining_slice = std::slice::from_raw_parts_mut(ptr, remaining);
                self.secure_zero_scalar(remaining_slice);
            }
        }
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    fn secure_zero_neon(&self, data: &mut [u8]) {
        self.secure_zero_scalar(data);
    }
    
    /// SSE清零实现
    #[cfg(target_arch = "x86_64")]
    fn secure_zero_sse(&self, data: &mut [u8]) {
        unsafe {
            let zero = _mm_setzero_si128();
            let mut ptr = data.as_mut_ptr();
            let end = ptr.add(data.len());
            
            // 处理16字节对齐的块
            while ptr.add(16) <= end {
                _mm_storeu_si128(ptr as *mut __m128i, zero);
                ptr = ptr.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr) as usize;
            if remaining > 0 {
                let remaining_slice = std::slice::from_raw_parts_mut(ptr, remaining);
                self.secure_zero_scalar(remaining_slice);
            }
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_zero_sse(&self, data: &mut [u8]) {
        self.secure_zero_scalar(data);
    }
    
    /// 标量清零实现（后备方案）
    fn secure_zero_scalar(&self, data: &mut [u8]) {
        // 使用volatile写入防止编译器优化
        for byte in data.iter_mut() {
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }
    
    /// SIMD加速的内存比较
    pub fn secure_compare(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        
        if a.is_empty() {
            return true;
        }
        
        // 选择最优的比较实现
        if self.capabilities.has_avx2 && a.len() >= 32 {
            self.secure_compare_avx2(a, b)
        } else if self.capabilities.has_neon && a.len() >= 16 {
            self.secure_compare_neon(a, b)
        } else if self.capabilities.has_sse4_2 && a.len() >= 16 {
            self.secure_compare_sse(a, b)
        } else {
            self.secure_compare_scalar(a, b)
        }
    }
    
    /// AVX2比较实现
    #[cfg(target_arch = "x86_64")]
    fn secure_compare_avx2(&self, a: &[u8], b: &[u8]) -> bool {
        unsafe {
            let mut result = 0u32;
            let mut ptr_a = a.as_ptr();
            let mut ptr_b = b.as_ptr();
            let end = ptr_a.add(a.len());
            
            // 处理32字节块
            while ptr_a.add(32) <= end {
                let va = _mm256_loadu_si256(ptr_a as *const __m256i);
                let vb = _mm256_loadu_si256(ptr_b as *const __m256i);
                let cmp = _mm256_cmpeq_epi8(va, vb);
                let mask = _mm256_movemask_epi8(cmp) as u32;
                result |= !mask; // 累积差异
                
                ptr_a = ptr_a.add(32);
                ptr_b = ptr_b.add(32);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr_a) as usize;
            if remaining > 0 {
                let remaining_a = std::slice::from_raw_parts(ptr_a, remaining);
                let remaining_b = std::slice::from_raw_parts(ptr_b, remaining);
                if !self.secure_compare_scalar(remaining_a, remaining_b) {
                    result |= 1;
                }
            }
            
            result == 0
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_compare_avx2(&self, a: &[u8], b: &[u8]) -> bool {
        self.secure_compare_scalar(a, b)
    }
    
    /// NEON比较实现
    #[cfg(target_arch = "aarch64")]
    fn secure_compare_neon(&self, a: &[u8], b: &[u8]) -> bool {
        unsafe {
            let mut result = 0u8;
            let mut ptr_a = a.as_ptr();
            let mut ptr_b = b.as_ptr();
            let end = ptr_a.add(a.len());
            
            // 处理16字节块
            while ptr_a.add(16) <= end {
                let va = vld1q_u8(ptr_a);
                let vb = vld1q_u8(ptr_b);
                let cmp = vceqq_u8(va, vb);
                
                // 检查是否所有位都相等
                let min_val = vminvq_u8(cmp);
                result |= !min_val;
                
                ptr_a = ptr_a.add(16);
                ptr_b = ptr_b.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr_a) as usize;
            if remaining > 0 {
                let remaining_a = std::slice::from_raw_parts(ptr_a, remaining);
                let remaining_b = std::slice::from_raw_parts(ptr_b, remaining);
                if !self.secure_compare_scalar(remaining_a, remaining_b) {
                    result |= 1;
                }
            }
            
            result == 0
        }
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    fn secure_compare_neon(&self, a: &[u8], b: &[u8]) -> bool {
        self.secure_compare_scalar(a, b)
    }
    
    /// SSE比较实现
    #[cfg(target_arch = "x86_64")]
    fn secure_compare_sse(&self, a: &[u8], b: &[u8]) -> bool {
        unsafe {
            let mut result = 0u32;
            let mut ptr_a = a.as_ptr();
            let mut ptr_b = b.as_ptr();
            let end = ptr_a.add(a.len());
            
            // 处理16字节块
            while ptr_a.add(16) <= end {
                let va = _mm_loadu_si128(ptr_a as *const __m128i);
                let vb = _mm_loadu_si128(ptr_b as *const __m128i);
                let cmp = _mm_cmpeq_epi8(va, vb);
                let mask = _mm_movemask_epi8(cmp) as u32;
                result |= !mask; // 累积差异
                
                ptr_a = ptr_a.add(16);
                ptr_b = ptr_b.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(ptr_a) as usize;
            if remaining > 0 {
                let remaining_a = std::slice::from_raw_parts(ptr_a, remaining);
                let remaining_b = std::slice::from_raw_parts(ptr_b, remaining);
                if !self.secure_compare_scalar(remaining_a, remaining_b) {
                    result |= 1;
                }
            }
            
            result == 0
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_compare_sse(&self, a: &[u8], b: &[u8]) -> bool {
        self.secure_compare_scalar(a, b)
    }
    
    /// 标量比较实现（常时）
    fn secure_compare_scalar(&self, a: &[u8], b: &[u8]) -> bool {
        let mut result = 0u8;
        
        for i in 0..a.len() {
            result |= a[i] ^ b[i];
        }
        
        result == 0
    }
    
    /// SIMD加速的内存复制
    pub fn secure_copy(&self, dest: &mut [u8], src: &[u8]) -> Result<(), &'static str> {
        if dest.len() != src.len() {
            return Err("Destination and source lengths must match");
        }
        
        if dest.is_empty() {
            return Ok(());
        }
        
        // 选择最优的复制实现
        if self.capabilities.has_avx2 && src.len() >= 32 {
            self.secure_copy_avx2(dest, src);
        } else if self.capabilities.has_neon && src.len() >= 16 {
            self.secure_copy_neon(dest, src);
        } else if self.capabilities.has_sse4_2 && src.len() >= 16 {
            self.secure_copy_sse(dest, src);
        } else {
            self.secure_copy_scalar(dest, src);
        }
        
        // 编译器栅栏
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    
    /// AVX2复制实现
    #[cfg(target_arch = "x86_64")]
    fn secure_copy_avx2(&self, dest: &mut [u8], src: &[u8]) {
        unsafe {
            let mut dest_ptr = dest.as_mut_ptr();
            let mut src_ptr = src.as_ptr();
            let end = src_ptr.add(src.len());
            
            // 处理32字节块
            while src_ptr.add(32) <= end {
                let data = _mm256_loadu_si256(src_ptr as *const __m256i);
                _mm256_storeu_si256(dest_ptr as *mut __m256i, data);
                
                src_ptr = src_ptr.add(32);
                dest_ptr = dest_ptr.add(32);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(src_ptr) as usize;
            if remaining > 0 {
                let remaining_dest = std::slice::from_raw_parts_mut(dest_ptr, remaining);
                let remaining_src = std::slice::from_raw_parts(src_ptr, remaining);
                self.secure_copy_scalar(remaining_dest, remaining_src);
            }
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_copy_avx2(&self, dest: &mut [u8], src: &[u8]) {
        self.secure_copy_scalar(dest, src);
    }
    
    /// NEON复制实现
    #[cfg(target_arch = "aarch64")]
    fn secure_copy_neon(&self, dest: &mut [u8], src: &[u8]) {
        unsafe {
            let mut dest_ptr = dest.as_mut_ptr();
            let mut src_ptr = src.as_ptr();
            let end = src_ptr.add(src.len());
            
            // 处理16字节块
            while src_ptr.add(16) <= end {
                let data = vld1q_u8(src_ptr);
                vst1q_u8(dest_ptr, data);
                
                src_ptr = src_ptr.add(16);
                dest_ptr = dest_ptr.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(src_ptr) as usize;
            if remaining > 0 {
                let remaining_dest = std::slice::from_raw_parts_mut(dest_ptr, remaining);
                let remaining_src = std::slice::from_raw_parts(src_ptr, remaining);
                self.secure_copy_scalar(remaining_dest, remaining_src);
            }
        }
    }
    
    #[cfg(not(target_arch = "aarch64"))]
    fn secure_copy_neon(&self, dest: &mut [u8], src: &[u8]) {
        self.secure_copy_scalar(dest, src);
    }
    
    /// SSE复制实现
    #[cfg(target_arch = "x86_64")]
    fn secure_copy_sse(&self, dest: &mut [u8], src: &[u8]) {
        unsafe {
            let mut dest_ptr = dest.as_mut_ptr();
            let mut src_ptr = src.as_ptr();
            let end = src_ptr.add(src.len());
            
            // 处理16字节块
            while src_ptr.add(16) <= end {
                let data = _mm_loadu_si128(src_ptr as *const __m128i);
                _mm_storeu_si128(dest_ptr as *mut __m128i, data);
                
                src_ptr = src_ptr.add(16);
                dest_ptr = dest_ptr.add(16);
            }
            
            // 处理剩余字节
            let remaining = end.offset_from(src_ptr) as usize;
            if remaining > 0 {
                let remaining_dest = std::slice::from_raw_parts_mut(dest_ptr, remaining);
                let remaining_src = std::slice::from_raw_parts(src_ptr, remaining);
                self.secure_copy_scalar(remaining_dest, remaining_src);
            }
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    fn secure_copy_sse(&self, dest: &mut [u8], src: &[u8]) {
        self.secure_copy_scalar(dest, src);
    }
    
    /// 标量复制实现
    fn secure_copy_scalar(&self, dest: &mut [u8], src: &[u8]) {
        for i in 0..src.len() {
            unsafe {
                std::ptr::write_volatile(&mut dest[i], src[i]);
            }
        }
    }
    
    /// 获取SIMD能力信息
    pub fn get_capabilities(&self) -> &SimdCapabilities {
        &self.capabilities
    }
}

impl Default for SimdMemoryOps {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capabilities_detection() {
        let caps = SimdCapabilities::detect();
        
        // 至少应该有基本的比较功能
        #[cfg(target_arch = "x86_64")]
        {
            // x86_64应该至少有SSE支持
            println!("AVX2: {}, AVX512: {}, SSE4.2: {}", caps.has_avx2, caps.has_avx512, caps.has_sse4_2);
        }
        
        #[cfg(target_arch = "aarch64")]
        {
            // AArch64应该有NEON支持
            assert!(caps.has_neon);
        }
    }
    
    #[test]
    fn test_simd_secure_zero() {
        let ops = SimdMemoryOps::new();
        let mut data = vec![0xAA; 1024]; // 填充测试数据
        
        ops.secure_zero(&mut data);
        
        // 验证所有字节都被清零
        for &byte in &data {
            assert_eq!(byte, 0);
        }
    }
    
    #[test]
    fn test_simd_secure_compare() {
        let ops = SimdMemoryOps::new();
        
        let data1 = vec![0x55; 128];
        let data2 = vec![0x55; 128];
        let data3 = vec![0xAA; 128];
        
        // 相同数据应该返回true
        assert!(ops.secure_compare(&data1, &data2));
        
        // 不同数据应该返回false
        assert!(!ops.secure_compare(&data1, &data3));
        
        // 不同长度应该返回false
        let data4 = vec![0x55; 64];
        assert!(!ops.secure_compare(&data1, &data4));
    }
    
    #[test]
    fn test_simd_secure_copy() {
        let ops = SimdMemoryOps::new();
        
        let src = (0..=255u8).collect::<Vec<u8>>();
        let mut dest = vec![0u8; 256];
        
        ops.secure_copy(&mut dest, &src).unwrap();
        
        // 验证复制结果
        for i in 0..256 {
            assert_eq!(dest[i], i as u8);
        }
    }
    
    #[test]
    fn test_simd_operations_with_various_sizes() {
        let ops = SimdMemoryOps::new();
        
        // 测试不同大小的数据
        for size in [1, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 1023, 1024] {
            let mut data = vec![0xFF; size];
            ops.secure_zero(&mut data);
            
            for &byte in &data {
                assert_eq!(byte, 0);
            }
        }
    }
    
    #[test]
    fn test_constant_time_property() {
        let ops = SimdMemoryOps::new();
        
        let data1 = vec![0x00; 1024];
        let data2 = vec![0xFF; 1024];
        let data3 = vec![0x00; 1024];
        
        // 比较操作应该在常时内完成（这里只是功能测试）
        let start1 = std::time::Instant::now();
        let result1 = ops.secure_compare(&data1, &data2);
        let time1 = start1.elapsed();
        
        let start2 = std::time::Instant::now();
        let result2 = ops.secure_compare(&data1, &data3);
        let time2 = start2.elapsed();
        
        assert!(!result1); // 不同数据
        assert!(result2);  // 相同数据
        
        // 时间差不应该太大（这是一个粗略的测试）
        println!("Time1: {:?}, Time2: {:?}", time1, time2);
    }
}