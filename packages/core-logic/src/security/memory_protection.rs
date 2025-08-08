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