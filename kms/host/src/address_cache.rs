//! Address Cache Module - Manages address_map.json for fast lookups
//!
//! This module provides Normal World caching for address → (wallet_id, derivation_path) mappings
//! The cache can be rebuilt from TEE if lost, using the kms-recovery-cli tool

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const ADDRESS_MAP_PATH: &str = "/root/shared/address_map.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressMetadata {
    pub wallet_id: Uuid,
    pub derivation_path: String,
    pub public_key: String,
    pub created_at: u64,
}

pub type AddressMap = HashMap<String, AddressMetadata>;

/// Load address map from disk
pub fn load_address_map() -> Result<AddressMap> {
    let path = Path::new(ADDRESS_MAP_PATH);

    if !path.exists() {
        // Return empty map if file doesn't exist
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(path)
        .context("Failed to read address_map.json")?;

    let map: AddressMap = serde_json::from_str(&content)
        .context("Failed to parse address_map.json")?;

    Ok(map)
}

/// Save address map to disk
pub fn save_address_map(map: &AddressMap) -> Result<()> {
    let content = serde_json::to_string_pretty(map)
        .context("Failed to serialize address_map")?;

    fs::write(ADDRESS_MAP_PATH, content)
        .context("Failed to write address_map.json")?;

    Ok(())
}

/// Add or update a single address entry
pub fn update_address_entry(
    address: &str,
    wallet_id: Uuid,
    derivation_path: &str,
    public_key: &str,
) -> Result<()> {
    let mut map = load_address_map()?;

    map.insert(address.to_string(), AddressMetadata {
        wallet_id,
        derivation_path: derivation_path.to_string(),
        public_key: public_key.to_string(),
        created_at: current_timestamp(),
    });

    save_address_map(&map)?;
    Ok(())
}

/// Lookup address in cache
pub fn lookup_address(address: &str) -> Result<Option<AddressMetadata>> {
    let map = load_address_map()?;
    Ok(map.get(address).cloned())
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_metadata_serialization() {
        let metadata = AddressMetadata {
            wallet_id: Uuid::new_v4(),
            derivation_path: "m/44'/60'/0'/0/0".to_string(),
            public_key: "0x04...".to_string(),
            created_at: 1234567890,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: AddressMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.wallet_id, deserialized.wallet_id);
        assert_eq!(metadata.derivation_path, deserialized.derivation_path);
    }
}
