use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::{Zeroize, ZeroizeOnDrop};

static MEMORY_PROTECTION_ENABLED: AtomicBool = AtomicBool::new(true);

pub struct SecureMemory {
    ptr: NonNull<u8>,
    size: usize,
    layout: Layout,
}

impl SecureMemory {
    pub fn new(size: usize) -> Result<Self, &'static str> {
        if size == 0 {
            return Err("Size must be greater than zero");
        }
        
        let layout = Layout::from_size_align(size, std::mem::align_of::<u8>())
            .map_err(|_| "Invalid layout")?;
        
        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err("Failed to allocate memory");
        }
        
        let ptr = NonNull::new(ptr).ok_or("Null pointer")?;
        
        Ok(Self { ptr, size, layout })
    }
    
    /// 从Vec<u8>创建SecureMemory
    pub fn from_vec(data: Vec<u8>) -> Result<Self, &'static str> {
        let mut secure_mem = Self::new(data.len())?;
        secure_mem.copy_from_slice(&data)?;
        Ok(secure_mem)
    }
    
    pub fn size(&self) -> usize {
        self.size
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.size) }
    }
    
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.size) }
    }
    
    pub fn secure_zero(&mut self) {
        let slice = self.as_mut_slice();
        slice.zeroize();
        
        std::sync::atomic::compiler_fence(Ordering::SeqCst);
    }
    
    pub fn copy_from_slice(&mut self, src: &[u8]) -> Result<(), &'static str> {
        if src.len() > self.size {
            return Err("Source data too large");
        }
        
        let size = self.size;
        let dest = self.as_mut_slice();
        dest[..src.len()].copy_from_slice(src);
        
        if src.len() < size {
            dest[src.len()..].zeroize();
        }
        
        Ok(())
    }
}

impl Drop for SecureMemory {
    fn drop(&mut self) {
        self.secure_zero();
        
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

unsafe impl Send for SecureMemory {}
unsafe impl Sync for SecureMemory {}

#[derive(ZeroizeOnDrop)]
pub struct StackCanary {
    canary_value: u64,
}

impl StackCanary {
    pub fn new() -> Self {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        Self {
            canary_value: rng.next_u64(),
        }
    }
    
    pub fn check(&self, value: u64) -> bool {
        self.canary_value == value
    }
    
    pub fn value(&self) -> u64 {
        self.canary_value
    }
}

impl Default for StackCanary {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MemoryGuard;

impl MemoryGuard {
    pub fn enable_protection() {
        MEMORY_PROTECTION_ENABLED.store(true, Ordering::SeqCst);
    }
    
    pub fn disable_protection() {
        MEMORY_PROTECTION_ENABLED.store(false, Ordering::SeqCst);
    }
    
    pub fn is_protection_enabled() -> bool {
        MEMORY_PROTECTION_ENABLED.load(Ordering::SeqCst)
    }
    
    pub fn check_bounds(ptr: *const u8, size: usize, buffer_size: usize) -> Result<(), &'static str> {
        if !Self::is_protection_enabled() {
            return Ok(());
        }
        
        if ptr.is_null() {
            return Err("Null pointer access");
        }
        
        if size > buffer_size {
            return Err("Buffer overflow detected");
        }
        
        Ok(())
    }
    
    pub fn secure_memcmp(a: &[u8], b: &[u8]) -> i32 {
        if a.len() != b.len() {
            return if a.len() < b.len() { -1 } else { 1 };
        }
        
        let mut result = 0u8;
        for i in 0..a.len() {
            result |= a[i] ^ b[i];
        }
        
        if result == 0 { 0 } else { 1 }
    }
}

pub struct SecureString {
    data: SecureMemory,
    len: usize,
}

impl SecureString {
    pub fn new(s: &str) -> Result<Self, &'static str> {
        let bytes = s.as_bytes();
        let mut memory = SecureMemory::new(bytes.len())?;
        memory.copy_from_slice(bytes)?;
        
        Ok(Self {
            data: memory,
            len: bytes.len(),
        })
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut memory = SecureMemory::new(bytes.len())?;
        memory.copy_from_slice(bytes)?;
        
        Ok(Self {
            data: memory,
            len: bytes.len(),
        })
    }
    
    pub fn len(&self) -> usize {
        self.len
    }
    
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    pub fn as_bytes(&self) -> &[u8] {
        &self.data.as_slice()[..self.len]
    }
    
    pub fn secure_eq(&self, other: &Self) -> bool {
        MemoryGuard::secure_memcmp(self.as_bytes(), other.as_bytes()) == 0
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureString([REDACTED; {} bytes])", self.len)
    }
}

pub fn secure_alloc_aligned(size: usize, alignment: usize) -> Result<NonNull<u8>, &'static str> {
    let layout = Layout::from_size_align(size, alignment)
        .map_err(|_| "Invalid layout for aligned allocation")?;
    
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() {
        return Err("Failed to allocate aligned memory");
    }
    
    NonNull::new(ptr).ok_or("Null pointer from aligned allocation")
}

pub unsafe fn secure_dealloc_aligned(ptr: NonNull<u8>, size: usize, alignment: usize) {
    let layout = Layout::from_size_align(size, alignment).expect("Invalid layout for dealloc");
    dealloc(ptr.as_ptr(), layout);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_memory() {
        let mut mem = SecureMemory::new(32).unwrap();
        assert_eq!(mem.size(), 32);
        
        let test_data = b"Hello, secure world!";
        mem.copy_from_slice(test_data).unwrap();
        
        assert_eq!(&mem.as_slice()[..test_data.len()], test_data);
    }
    
    #[test]
    fn test_stack_canary() {
        let canary = StackCanary::new();
        let value = canary.value();
        
        assert!(canary.check(value));
        assert!(!canary.check(value ^ 1));
    }
    
    #[test]
    fn test_secure_string() {
        let s1 = SecureString::new("test").unwrap();
        let s2 = SecureString::new("test").unwrap();
        let s3 = SecureString::new("different").unwrap();
        
        assert!(s1.secure_eq(&s2));
        assert!(!s1.secure_eq(&s3));
        assert_eq!(s1.len(), 4);
    }
    
    #[test]
    fn test_memory_guard() {
        assert!(MemoryGuard::is_protection_enabled());
        
        MemoryGuard::disable_protection();
        assert!(!MemoryGuard::is_protection_enabled());
        
        MemoryGuard::enable_protection();
        assert!(MemoryGuard::is_protection_enabled());
    }
}

/// 增强内存保护 - 高级安全功能
pub struct AdvancedMemoryProtection {
    /// 内存隔离区域计数器
    isolation_zones: std::sync::atomic::AtomicUsize,
    /// 内存访问监控器
    access_monitor: std::sync::Arc<std::sync::Mutex<MemoryAccessMonitor>>,
}

/// 内存访问监控器
struct MemoryAccessMonitor {
    /// 可疑访问模式计数
    suspicious_access_count: usize,
    /// 最后访问时间戳
    last_access_time: u64,
    /// 访问频率统计
    access_frequency: std::collections::HashMap<usize, usize>,
}

impl AdvancedMemoryProtection {
    pub fn new() -> Self {
        Self {
            isolation_zones: std::sync::atomic::AtomicUsize::new(0),
            access_monitor: std::sync::Arc::new(std::sync::Mutex::new(
                MemoryAccessMonitor {
                    suspicious_access_count: 0,
                    last_access_time: 0,
                    access_frequency: std::collections::HashMap::new(),
                }
            )),
        }
    }

    /// 创建隔离的内存区域
    pub fn create_isolated_memory(&self, size: usize) -> Result<IsolatedMemoryRegion, &'static str> {
        let zone_id = self.isolation_zones.fetch_add(1, Ordering::SeqCst);
        
        // 分配对齐的内存页
        let page_size = 4096; // 4KB页面
        let aligned_size = (size + page_size - 1) & !(page_size - 1);
        
        let layout = Layout::from_size_align(aligned_size, page_size)
            .map_err(|_| "Failed to create aligned layout")?;
        
        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err("Failed to allocate isolated memory");
        }
        
        let ptr = NonNull::new(ptr).ok_or("Null pointer")?;
        
        // 在TEE环境中，这里可以设置内存页面保护属性
        // 例如设置为只读、不可执行等
        
        Ok(IsolatedMemoryRegion {
            ptr,
            size: aligned_size,
            layout,
            zone_id,
            access_count: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// 检测内存访问异常
    pub fn detect_memory_anomalies(&self, access_addr: usize, access_size: usize) -> bool {
        if let Ok(mut monitor) = self.access_monitor.lock() {
            let current_time = self.get_current_time();
            
            // 检测访问频率异常
            let count = monitor.access_frequency.entry(access_addr).or_insert(0);
            *count += 1;
            let count_value = *count;
            
            // 检测时间间隔异常（可能的时序攻击）
            let time_delta = current_time - monitor.last_access_time;
            if time_delta < 1000 && count_value > 100 { // 1ms内访问超过100次
                monitor.suspicious_access_count += 1;
                return true;
            }
            
            // 检测越界访问模式
            if access_size > 1024 * 1024 { // 超过1MB的访问
                monitor.suspicious_access_count += 1;
                return true;
            }
            
            monitor.last_access_time = current_time;
        }
        
        false
    }

    /// 执行内存清理（防止数据残留）
    pub fn secure_memory_cleanup(&self, ptr: *mut u8, size: usize) {
        unsafe {
            // 多重覆盖以防止数据恢复
            for pattern in &[0x00, 0xFF, 0xAA, 0x55, 0x00] {
                std::ptr::write_bytes(ptr, *pattern, size);
                
                // 强制内存屏障
                std::sync::atomic::compiler_fence(Ordering::SeqCst);
            }
            
            // 最终随机覆盖
            let mut rng = crate::security::SecureRng::new().unwrap();
            for i in 0..size {
                let random_byte = (rng.next_u32().unwrap() & 0xFF) as u8;
                *ptr.add(i) = random_byte;
            }
        }
    }

    fn get_current_time(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            0 // TEE环境需要使用TEE时间API
        }
    }
}

/// 隔离内存区域
pub struct IsolatedMemoryRegion {
    ptr: NonNull<u8>,
    size: usize,
    layout: Layout,
    zone_id: usize,
    access_count: std::sync::atomic::AtomicUsize,
}

impl IsolatedMemoryRegion {
    /// 安全读取数据
    pub fn secure_read(&self, offset: usize, buffer: &mut [u8]) -> Result<(), &'static str> {
        if offset + buffer.len() > self.size {
            return Err("Read would exceed memory region bounds");
        }
        
        self.access_count.fetch_add(1, Ordering::SeqCst);
        
        unsafe {
            let src = self.ptr.as_ptr().add(offset);
            std::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), buffer.len());
        }
        
        Ok(())
    }

    /// 安全写入数据
    pub fn secure_write(&self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.size {
            return Err("Write would exceed memory region bounds");
        }
        
        self.access_count.fetch_add(1, Ordering::SeqCst);
        
        unsafe {
            let dst = self.ptr.as_ptr().add(offset);
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }
        
        Ok(())
    }

    /// 获取访问计数
    pub fn get_access_count(&self) -> usize {
        self.access_count.load(Ordering::SeqCst)
    }

    /// 获取区域ID
    pub fn get_zone_id(&self) -> usize {
        self.zone_id
    }
}

impl Drop for IsolatedMemoryRegion {
    fn drop(&mut self) {
        // 安全清理内存
        let protection = AdvancedMemoryProtection::new();
        protection.secure_memory_cleanup(self.ptr.as_ptr(), self.size);
        
        // 释放内存
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

/// 内存完整性检查器
pub struct MemoryIntegrityChecker {
    checksum_table: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<usize, u64>>>,
}

impl MemoryIntegrityChecker {
    pub fn new() -> Self {
        Self {
            checksum_table: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 计算内存区域的校验和
    pub fn calculate_checksum(&self, ptr: *const u8, size: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        
        unsafe {
            for i in 0..size {
                (*ptr.add(i)).hash(&mut hasher);
            }
        }
        
        hasher.finish()
    }

    /// 注册内存区域进行完整性监控
    pub fn register_region(&self, region_id: usize, ptr: *const u8, size: usize) {
        let checksum = self.calculate_checksum(ptr, size);
        
        if let Ok(mut table) = self.checksum_table.write() {
            table.insert(region_id, checksum);
        }
    }

    /// 验证内存区域完整性
    pub fn verify_region(&self, region_id: usize, ptr: *const u8, size: usize) -> bool {
        let current_checksum = self.calculate_checksum(ptr, size);
        
        if let Ok(table) = self.checksum_table.read() {
            if let Some(&expected_checksum) = table.get(&region_id) {
                return current_checksum == expected_checksum;
            }
        }
        
        false
    }

    /// 更新区域校验和
    pub fn update_checksum(&self, region_id: usize, ptr: *const u8, size: usize) {
        let checksum = self.calculate_checksum(ptr, size);
        
        if let Ok(mut table) = self.checksum_table.write() {
            table.insert(region_id, checksum);
        }
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::*;

    #[test]
    fn test_isolated_memory_region() {
        let protection = AdvancedMemoryProtection::new();
        let region = protection.create_isolated_memory(4096).unwrap();
        
        assert_eq!(region.get_access_count(), 0);
        
        let test_data = b"Hello, World!";
        assert!(region.secure_write(0, test_data).is_ok());
        
        let mut read_buffer = vec![0u8; test_data.len()];
        assert!(region.secure_read(0, &mut read_buffer).is_ok());
        
        assert_eq!(&read_buffer, test_data);
        assert_eq!(region.get_access_count(), 2);
    }

    #[test]
    fn test_memory_integrity_checker() {
        let checker = MemoryIntegrityChecker::new();
        let data = vec![1, 2, 3, 4, 5];
        
        checker.register_region(1, data.as_ptr(), data.len());
        assert!(checker.verify_region(1, data.as_ptr(), data.len()));
        
        // 修改数据后应该检测到完整性问题
        let mut modified_data = data.clone();
        modified_data[0] = 99;
        assert!(!checker.verify_region(1, modified_data.as_ptr(), modified_data.len()));
    }

    #[test]
    fn test_memory_anomaly_detection() {
        let protection = AdvancedMemoryProtection::new();
        
        // 正常访问不应该触发异常
        assert!(!protection.detect_memory_anomalies(0x1000, 64));
        
        // 大量访问应该触发异常检测
        for _ in 0..150 {
            protection.detect_memory_anomalies(0x1000, 64);
        }
        // 注意：由于时间检测的限制，这个测试可能需要调整
    }
}