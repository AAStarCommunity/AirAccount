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