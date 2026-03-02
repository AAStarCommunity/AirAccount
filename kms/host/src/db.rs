//! SQLite persistence layer for KMS CA-side data.
//!
//! All wallet metadata, address index, and WebAuthn challenges are stored here.
//! If the DB is lost, wallets can be recovered from TA secure storage.

use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Utc;

const DEFAULT_DB_PATH: &str = "/root/shared/kms.db";

const SCHEMA: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS wallets (
    key_id          TEXT PRIMARY KEY,
    address         TEXT,
    public_key      TEXT,
    derivation_path TEXT,
    description     TEXT NOT NULL DEFAULT '',
    key_usage       TEXT NOT NULL DEFAULT 'SIGN_VERIFY',
    key_spec        TEXT NOT NULL DEFAULT 'ECC_SECG_P256K1',
    origin          TEXT NOT NULL DEFAULT 'EXTERNAL_KMS',
    passkey_pubkey  TEXT,
    credential_id   TEXT,
    sign_count      INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'creating',
    error_msg       TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS address_index (
    address         TEXT PRIMARY KEY,
    key_id          TEXT NOT NULL,
    derivation_path TEXT NOT NULL,
    public_key      TEXT,
    FOREIGN KEY (key_id) REFERENCES wallets(key_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS challenges (
    id              TEXT PRIMARY KEY,
    challenge       BLOB NOT NULL,
    key_id          TEXT,
    purpose         TEXT NOT NULL,
    rp_id           TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS api_keys (
    api_key         TEXT PRIMARY KEY,
    label           TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_address_key ON address_index(key_id);
CREATE INDEX IF NOT EXISTS idx_challenge_expire ON challenges(expires_at);
CREATE INDEX IF NOT EXISTS idx_wallet_credential ON wallets(credential_id);
"#;

// ── Row types ──

#[derive(Debug, Clone)]
pub struct WalletRow {
    pub key_id: String,
    pub address: Option<String>,
    pub public_key: Option<String>,
    pub derivation_path: Option<String>,
    pub description: String,
    pub key_usage: String,
    pub key_spec: String,
    pub origin: String,
    pub passkey_pubkey: Option<String>,
    pub credential_id: Option<String>,
    pub sign_count: u32,
    pub status: String,
    pub error_msg: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct AddressRow {
    pub address: String,
    pub key_id: String,
    pub derivation_path: String,
    pub public_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChallengeRow {
    pub id: String,
    pub challenge: Vec<u8>,
    pub key_id: Option<String>,
    pub purpose: String,
    pub rp_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}

// ── KmsDb ──

#[derive(Clone)]
pub struct KmsDb {
    conn: Arc<Mutex<Connection>>,
}

impl KmsDb {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite DB at {}", path))?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize DB schema")?;
        println!("📦 SQLite DB opened: {}", path);
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(DEFAULT_DB_PATH)
    }

    /// Open an in-memory DB (for tests)
    pub fn open_memory() -> Result<Self> {
        Self::open(":memory:")
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("DB mutex poisoned")
    }

    // ── Wallet CRUD ──

    pub fn insert_wallet(&self, w: &WalletRow) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO wallets (key_id, address, public_key, derivation_path, description, \
             key_usage, key_spec, origin, passkey_pubkey, credential_id, sign_count, status, \
             error_msg, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                w.key_id, w.address, w.public_key, w.derivation_path, w.description,
                w.key_usage, w.key_spec, w.origin, w.passkey_pubkey, w.credential_id,
                w.sign_count, w.status, w.error_msg, w.created_at,
            ],
        ).context("insert_wallet")?;
        Ok(())
    }

    pub fn get_wallet(&self, key_id: &str) -> Result<Option<WalletRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT key_id, address, public_key, derivation_path, description, key_usage, \
             key_spec, origin, passkey_pubkey, credential_id, sign_count, status, error_msg, \
             created_at FROM wallets WHERE key_id = ?1"
        )?;
        let mut rows = stmt.query_map(params![key_id], |row| {
            Ok(WalletRow {
                key_id: row.get(0)?,
                address: row.get(1)?,
                public_key: row.get(2)?,
                derivation_path: row.get(3)?,
                description: row.get(4)?,
                key_usage: row.get(5)?,
                key_spec: row.get(6)?,
                origin: row.get(7)?,
                passkey_pubkey: row.get(8)?,
                credential_id: row.get(9)?,
                sign_count: row.get(10)?,
                status: row.get(11)?,
                error_msg: row.get(12)?,
                created_at: row.get(13)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn wallet_exists(&self, key_id: &str) -> Result<bool> {
        let conn = self.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM wallets WHERE key_id = ?1",
            params![key_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn update_wallet_derived(
        &self, key_id: &str, address: &str, public_key: &str,
        derivation_path: &str, status: &str,
    ) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET address=?2, public_key=?3, derivation_path=?4, status=?5 \
             WHERE key_id=?1",
            params![key_id, address, public_key, derivation_path, status],
        )?;
        Ok(())
    }

    pub fn update_wallet_status(&self, key_id: &str, status: &str, error_msg: Option<&str>) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET status=?2, error_msg=?3 WHERE key_id=?1",
            params![key_id, status, error_msg],
        )?;
        Ok(())
    }

    pub fn update_wallet_passkey(&self, key_id: &str, passkey_pubkey: &str, credential_id: Option<&str>) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET passkey_pubkey=?2, credential_id=?3 WHERE key_id=?1",
            params![key_id, passkey_pubkey, credential_id],
        )?;
        Ok(())
    }

    pub fn update_wallet_sign_count(&self, key_id: &str, sign_count: u32) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET sign_count=?2 WHERE key_id=?1",
            params![key_id, sign_count],
        )?;
        Ok(())
    }

    pub fn delete_wallet(&self, key_id: &str) -> Result<()> {
        let conn = self.lock();
        conn.execute("DELETE FROM wallets WHERE key_id=?1", params![key_id])?;
        Ok(())
    }

    pub fn list_wallets(&self) -> Result<Vec<WalletRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT key_id, address, public_key, derivation_path, description, key_usage, \
             key_spec, origin, passkey_pubkey, credential_id, sign_count, status, error_msg, \
             created_at FROM wallets ORDER BY created_at"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WalletRow {
                key_id: row.get(0)?,
                address: row.get(1)?,
                public_key: row.get(2)?,
                derivation_path: row.get(3)?,
                description: row.get(4)?,
                key_usage: row.get(5)?,
                key_spec: row.get(6)?,
                origin: row.get(7)?,
                passkey_pubkey: row.get(8)?,
                credential_id: row.get(9)?,
                sign_count: row.get(10)?,
                status: row.get(11)?,
                error_msg: row.get(12)?,
                created_at: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── Address index ──

    pub fn upsert_address(&self, address: &str, key_id: &str, derivation_path: &str, public_key: Option<&str>) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR REPLACE INTO address_index (address, key_id, derivation_path, public_key) \
             VALUES (?1,?2,?3,?4)",
            params![address, key_id, derivation_path, public_key],
        )?;
        Ok(())
    }

    pub fn lookup_address(&self, address: &str) -> Result<Option<AddressRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT address, key_id, derivation_path, public_key FROM address_index WHERE address=?1"
        )?;
        let mut rows = stmt.query_map(params![address], |row| {
            Ok(AddressRow {
                address: row.get(0)?,
                key_id: row.get(1)?,
                derivation_path: row.get(2)?,
                public_key: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    // ── Challenge management ──

    pub fn store_challenge(
        &self, id: &str, challenge: &[u8], key_id: Option<&str>,
        purpose: &str, rp_id: &str, ttl_secs: i64,
    ) -> Result<()> {
        let now = current_unix();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO challenges (id, challenge, key_id, purpose, rp_id, created_at, expires_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![id, challenge, key_id, purpose, rp_id, now, now + ttl_secs],
        )?;
        Ok(())
    }

    /// Consume a challenge: returns it and deletes atomically. Returns None if expired or not found.
    pub fn consume_challenge(&self, id: &str) -> Result<Option<ChallengeRow>> {
        let conn = self.lock();
        let now = current_unix();
        let mut stmt = conn.prepare(
            "SELECT id, challenge, key_id, purpose, rp_id, created_at, expires_at \
             FROM challenges WHERE id=?1 AND expires_at > ?2"
        )?;
        let mut rows = stmt.query_map(params![id, now], |row| {
            Ok(ChallengeRow {
                id: row.get(0)?,
                challenge: row.get(1)?,
                key_id: row.get(2)?,
                purpose: row.get(3)?,
                rp_id: row.get(4)?,
                created_at: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;
        let result = match rows.next() {
            Some(r) => Some(r?),
            None => None,
        };
        drop(rows);
        drop(stmt);
        if result.is_some() {
            conn.execute("DELETE FROM challenges WHERE id=?1", params![id])?;
        }
        Ok(result)
    }

    // ── API keys ──

    /// Generate a new API key, store it, and return the plaintext key.
    pub fn generate_api_key(&self, label: &str) -> Result<String> {
        let key = format!("kms_{}", Uuid::new_v4().to_string().replace("-", ""));
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO api_keys (api_key, label, created_at) VALUES (?1, ?2, ?3)",
            params![key, label, now],
        ).context("generate_api_key")?;
        Ok(key)
    }

    /// Check if an API key is valid.
    pub fn validate_api_key(&self, key: &str) -> Result<bool> {
        let conn = self.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM api_keys WHERE api_key = ?1",
            params![key],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// List all API keys (returns key, label, created_at).
    pub fn list_api_keys(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT api_key, label, created_at FROM api_keys ORDER BY created_at"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Revoke (delete) an API key.
    pub fn revoke_api_key(&self, key: &str) -> Result<bool> {
        let conn = self.lock();
        let deleted = conn.execute(
            "DELETE FROM api_keys WHERE api_key = ?1",
            params![key],
        )?;
        Ok(deleted > 0)
    }

    /// Check if any API keys exist in DB.
    pub fn has_api_keys(&self) -> Result<bool> {
        let conn = self.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM api_keys", [], |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn cleanup_expired_challenges(&self) -> Result<usize> {
        let conn = self.lock();
        let deleted = conn.execute(
            "DELETE FROM challenges WHERE expires_at <= ?1",
            params![current_unix()],
        )?;
        Ok(deleted)
    }
}

fn current_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> KmsDb {
        KmsDb::open_memory().unwrap()
    }

    fn sample_wallet(key_id: &str) -> WalletRow {
        WalletRow {
            key_id: key_id.to_string(),
            address: None,
            public_key: None,
            derivation_path: None,
            description: "test".to_string(),
            key_usage: "SIGN_VERIFY".to_string(),
            key_spec: "ECC_SECG_P256K1".to_string(),
            origin: "EXTERNAL_KMS".to_string(),
            passkey_pubkey: Some("0x04abcd".to_string()),
            credential_id: None,
            sign_count: 0,
            status: "creating".to_string(),
            error_msg: None,
            created_at: "2026-03-02T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn insert_and_get_wallet() {
        let db = test_db();
        let w = sample_wallet("w1");
        db.insert_wallet(&w).unwrap();
        let got = db.get_wallet("w1").unwrap().unwrap();
        assert_eq!(got.key_id, "w1");
        assert_eq!(got.description, "test");
        assert_eq!(got.passkey_pubkey, Some("0x04abcd".to_string()));
    }

    #[test]
    fn wallet_not_found() {
        let db = test_db();
        assert!(db.get_wallet("nope").unwrap().is_none());
    }

    #[test]
    fn wallet_exists_check() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        assert!(db.wallet_exists("w1").unwrap());
        assert!(!db.wallet_exists("w2").unwrap());
    }

    #[test]
    fn update_wallet_derived() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        db.update_wallet_derived("w1", "0xaddr", "0xpub", "m/44'/60'/0'/0/0", "ready").unwrap();
        let got = db.get_wallet("w1").unwrap().unwrap();
        assert_eq!(got.address.as_deref(), Some("0xaddr"));
        assert_eq!(got.status, "ready");
    }

    #[test]
    fn delete_wallet_cascades_address() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        db.upsert_address("0xaddr", "w1", "m/0", None).unwrap();
        db.delete_wallet("w1").unwrap();
        assert!(db.lookup_address("0xaddr").unwrap().is_none());
    }

    #[test]
    fn list_wallets_returns_all() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        db.insert_wallet(&sample_wallet("w2")).unwrap();
        assert_eq!(db.list_wallets().unwrap().len(), 2);
    }

    #[test]
    fn address_upsert_and_lookup() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        db.upsert_address("0xaaaa", "w1", "m/44'/60'/0'/0/0", Some("0xpub")).unwrap();
        let row = db.lookup_address("0xaaaa").unwrap().unwrap();
        assert_eq!(row.key_id, "w1");
        assert_eq!(row.public_key.as_deref(), Some("0xpub"));
    }

    #[test]
    fn challenge_store_and_consume() {
        let db = test_db();
        let challenge = vec![1u8, 2, 3, 4];
        db.store_challenge("c1", &challenge, None, "registration", "example.com", 300).unwrap();
        let got = db.consume_challenge("c1").unwrap().unwrap();
        assert_eq!(got.challenge, challenge);
        assert_eq!(got.purpose, "registration");
        // Consumed — should be gone
        assert!(db.consume_challenge("c1").unwrap().is_none());
    }

    #[test]
    fn challenge_not_found() {
        let db = test_db();
        assert!(db.consume_challenge("nope").unwrap().is_none());
    }

    #[test]
    fn api_key_generate_and_validate() {
        let db = test_db();
        assert!(!db.has_api_keys().unwrap());
        let key = db.generate_api_key("test-service").unwrap();
        assert!(key.starts_with("kms_"));
        assert!(db.has_api_keys().unwrap());
        assert!(db.validate_api_key(&key).unwrap());
        assert!(!db.validate_api_key("kms_invalid").unwrap());
    }

    #[test]
    fn api_key_list_and_revoke() {
        let db = test_db();
        let k1 = db.generate_api_key("svc-a").unwrap();
        let _k2 = db.generate_api_key("svc-b").unwrap();
        assert_eq!(db.list_api_keys().unwrap().len(), 2);
        assert!(db.revoke_api_key(&k1).unwrap());
        assert!(!db.validate_api_key(&k1).unwrap());
        assert_eq!(db.list_api_keys().unwrap().len(), 1);
    }

    #[test]
    fn update_passkey() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w1")).unwrap();
        db.update_wallet_passkey("w1", "0x04new", Some("cred-123")).unwrap();
        let got = db.get_wallet("w1").unwrap().unwrap();
        assert_eq!(got.passkey_pubkey.as_deref(), Some("0x04new"));
        assert_eq!(got.credential_id.as_deref(), Some("cred-123"));
    }
}
