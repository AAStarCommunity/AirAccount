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

    fn test_uuid() -> Uuid {
        Uuid::parse_str("4319f351-0b24-4097-b659-80ee4f824cdd").unwrap()
    }

    fn test_uuid2() -> Uuid {
        Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap()
    }

    fn make_metadata(wallet_id: Uuid, path: &str) -> AddressMetadata {
        AddressMetadata {
            wallet_id,
            derivation_path: path.to_string(),
            public_key: "0x04abcdef".to_string(),
            created_at: 1700000000,
        }
    }

    // ── AddressMetadata serialization ──

    #[test]
    fn metadata_json_roundtrip() {
        let id = test_uuid();
        let meta = make_metadata(id, "m/44'/60'/0'/0/0");
        let json = serde_json::to_string(&meta).unwrap();
        let decoded: AddressMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.wallet_id, decoded.wallet_id);
        assert_eq!(meta.derivation_path, decoded.derivation_path);
        assert_eq!(meta.public_key, decoded.public_key);
        assert_eq!(meta.created_at, decoded.created_at);
    }

    #[test]
    fn metadata_all_fields_present_in_json() {
        let meta = make_metadata(Uuid::nil(), "m/0");
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("wallet_id"));
        assert!(json.contains("derivation_path"));
        assert!(json.contains("public_key"));
        assert!(json.contains("created_at"));
    }

    #[test]
    fn metadata_deserialize_from_known_json() {
        let json = r#"{
            "wallet_id": "4319f351-0b24-4097-b659-80ee4f824cdd",
            "derivation_path": "m/44'/60'/0'/0/0",
            "public_key": "0x04aabb",
            "created_at": 1700000000
        }"#;
        let meta: AddressMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.wallet_id.to_string(), "4319f351-0b24-4097-b659-80ee4f824cdd");
        assert_eq!(meta.created_at, 1700000000);
    }

    // ── AddressMap serialization ──

    #[test]
    fn address_map_empty_roundtrip() {
        let map: AddressMap = HashMap::new();
        let json = serde_json::to_string_pretty(&map).unwrap();
        let decoded: AddressMap = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn address_map_multiple_entries() {
        let mut map: AddressMap = HashMap::new();
        let id1 = test_uuid();
        let id2 = test_uuid2();
        map.insert("0xaaaa".into(), make_metadata(id1, "m/44'/60'/0'/0/0"));
        map.insert("0xbbbb".into(), make_metadata(id2, "m/44'/60'/0'/0/1"));

        let json = serde_json::to_string(&map).unwrap();
        let decoded: AddressMap = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded["0xaaaa"].wallet_id, id1);
        assert_eq!(decoded["0xbbbb"].wallet_id, id2);
    }

    #[test]
    fn address_map_lookup_hit() {
        let mut map: AddressMap = HashMap::new();
        let id = test_uuid();
        map.insert("0xaddr1".into(), make_metadata(id, "m/44'/60'/0'/0/0"));
        let found = map.get("0xaddr1").cloned();
        assert!(found.is_some());
        assert_eq!(found.unwrap().wallet_id, id);
    }

    #[test]
    fn address_map_lookup_miss() {
        let map: AddressMap = HashMap::new();
        assert!(map.get("0xnotexist").is_none());
    }

    #[test]
    fn address_map_overwrite_entry() {
        let mut map: AddressMap = HashMap::new();
        let id1 = test_uuid();
        let id2 = test_uuid2();
        map.insert("0xaddr".into(), make_metadata(id1, "m/0"));
        map.insert("0xaddr".into(), make_metadata(id2, "m/1"));
        assert_eq!(map.len(), 1);
        assert_eq!(map["0xaddr"].wallet_id, id2);
        assert_eq!(map["0xaddr"].derivation_path, "m/1");
    }

    // ── JSON error handling ──

    #[test]
    fn invalid_json_returns_error() {
        let result: Result<AddressMap, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn missing_required_field_returns_error() {
        let json = r#"{"wallet_id":"4319f351-0b24-4097-b659-80ee4f824cdd"}"#;
        let result: Result<AddressMetadata, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ── current_timestamp ──

    #[test]
    fn timestamp_is_recent() {
        let ts = current_timestamp();
        assert!(ts > 1704067200, "timestamp too old: {}", ts);
        assert!(ts < 1893456000, "timestamp too far in future: {}", ts);
    }
}
