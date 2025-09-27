// Mock TEE environment for testing eth_wallet functionality
use anyhow::Result;

pub struct Random;

impl Random {
    pub fn generate(buf: &mut [u8]) -> Result<()> {
        // Use system random for testing
        use std::fs::File;
        use std::io::Read;

        let mut rng = File::open("/dev/urandom").unwrap();
        rng.read_exact(buf).unwrap();
        Ok(())
    }
}

// Mock secure storage for testing
pub struct MockSecureDB;

impl MockSecureDB {
    pub fn open(_name: &str) -> Result<Self> {
        Ok(Self)
    }

    pub fn put<T: serde::Serialize>(&self, _item: &T) -> Result<()> {
        // Mock storage - just succeed
        println!("Mock: Stored item in secure DB");
        Ok(())
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, _id: &uuid::Uuid) -> Result<T> {
        Err(anyhow::anyhow!("Mock: Item not found"))
    }

    pub fn delete_entry<T>(&self, _id: &uuid::Uuid) -> Result<()> {
        println!("Mock: Deleted item from secure DB");
        Ok(())
    }
}