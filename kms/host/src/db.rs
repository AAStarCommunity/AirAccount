//! SQLite persistence layer for KMS CA-side data.
//!
//! All wallet metadata, address index, and WebAuthn challenges are stored here.
//! If the DB is lost, wallets can be recovered from TA secure storage.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, TransactionBehavior};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const DEFAULT_DB_PATH: &str = "/root/shared/kms.db";

/// How long a 'pending' P256 session key placeholder is considered in-flight before
/// it is treated as stuck (host crash between allocate and activate). Used in both
/// the allocate quota count and the GC expiry query — must stay in sync.
const PENDING_TTL_SECS: i64 = 300; // 5 minutes

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
    created_at      TEXT NOT NULL,
    lifecycle_status TEXT NOT NULL DEFAULT 'active'
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

CREATE TABLE IF NOT EXISTS tx_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    op          TEXT NOT NULL,
    key_id      TEXT,
    addr        TEXT,
    webauthn    INTEGER NOT NULL DEFAULT 0,
    latency_ms  INTEGER NOT NULL,
    success     INTEGER NOT NULL DEFAULT 1,
    is_panic    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_keys (
    wallet_id               TEXT NOT NULL,
    agent_index             INTEGER NOT NULL,
    human_id                TEXT NOT NULL,
    agent_address           TEXT NOT NULL,
    public_key_compressed   TEXT NOT NULL,
    credential_hash         TEXT,
    credential_jwt          TEXT,
    credential_expires_at   INTEGER,
    status                  TEXT NOT NULL DEFAULT 'active',
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    revoked_at              TEXT,
    PRIMARY KEY (wallet_id, agent_index),
    FOREIGN KEY (wallet_id) REFERENCES wallets(key_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS jwt_secret_meta (
    kid          TEXT PRIMARY KEY,
    status       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    retired_at   TEXT,
    expires_at   INTEGER
);

CREATE TABLE IF NOT EXISTS p256_session_keys (
    wallet_id               TEXT NOT NULL,
    session_index           INTEGER NOT NULL,
    human_id                TEXT NOT NULL,
    pub_key_x               TEXT NOT NULL,
    pub_key_y               TEXT NOT NULL,
    credential_hash         TEXT,
    credential_expires_at   INTEGER,
    status                  TEXT NOT NULL DEFAULT 'pending',
    tee_deleted             INTEGER NOT NULL DEFAULT 0,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    revoked_at              TEXT,
    PRIMARY KEY (wallet_id, session_index),
    FOREIGN KEY (wallet_id) REFERENCES wallets(key_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_address_key ON address_index(key_id);
CREATE INDEX IF NOT EXISTS idx_challenge_expire ON challenges(expires_at);
CREATE INDEX IF NOT EXISTS idx_wallet_credential ON wallets(credential_id);
CREATE INDEX IF NOT EXISTS idx_tx_log_created ON tx_log(created_at);
CREATE INDEX IF NOT EXISTS idx_tx_log_op ON tx_log(op);
CREATE INDEX IF NOT EXISTS idx_agent_keys_human ON agent_keys(human_id);
CREATE INDEX IF NOT EXISTS idx_agent_keys_address ON agent_keys(agent_address);
CREATE INDEX IF NOT EXISTS idx_jwt_secret_meta_status ON jwt_secret_meta(status);
CREATE INDEX IF NOT EXISTS idx_p256_session_gc ON p256_session_keys(wallet_id, status, credential_expires_at);
"#;

// ── TX stats ──

#[derive(Debug, Default)]
pub struct TxStats {
    pub total_sign: i64,
    pub daily_sign: i64,
    pub total_ops: i64,
    pub daily_ops: i64,
    pub avg_sign_ms: f64,
    pub avg_derive_ms: f64,
    pub panic_count: i64,
    pub error_count: i64,
    pub webauthn_count: i64,
}

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

#[derive(Debug, Clone)]
pub struct AgentKeyRow {
    pub wallet_id: String,
    pub agent_index: u32,
    pub human_id: String,
    pub agent_address: String,
    pub public_key_compressed: String,
    pub credential_hash: Option<String>,
    pub credential_jwt: Option<String>,
    pub credential_expires_at: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JwtSecretMetaRow {
    pub kid: String,
    pub status: String,
    pub created_at: String,
    pub retired_at: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct P256SessionKeyRow {
    pub wallet_id: String,
    pub session_index: u32,
    pub human_id: String,
    pub pub_key_x: String,
    pub pub_key_y: String,
    pub credential_hash: Option<String>,
    pub credential_expires_at: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub revoked_at: Option<String>,
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
        // Prevent SQLITE_BUSY on schema init and migration: retry automatically for up to 5s
        // before returning an error. DDL operations on a shared WAL-mode DB can be transiently
        // locked by concurrent readers/writers.
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .context("Failed to set SQLite busy timeout")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize DB schema")?;
        // Migration: add tee_deleted column to DBs created before this column existed.
        // Uses PRAGMA table_info to distinguish "already exists" (safe to skip) from real
        // errors (disk full, corruption) that must propagate. TOCTOU is handled by re-verifying
        // on ALTER failure: if concurrent open already added the column, we treat it as success.
        {
            let check_col_exists = |c: &Connection| -> Result<bool> {
                let mut stmt = c
                    .prepare("PRAGMA table_info(p256_session_keys)")
                    .context("Failed to query p256_session_keys schema")?;
                let names: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<rusqlite::Result<_>>()
                    .context("Failed to read p256_session_keys schema")?;
                Ok(names.iter().any(|n| n == "tee_deleted"))
            };
            if !check_col_exists(&conn)? {
                match conn.execute_batch(
                    "ALTER TABLE p256_session_keys \
                     ADD COLUMN tee_deleted INTEGER NOT NULL DEFAULT 0;",
                ) {
                    Ok(()) => {}
                    Err(alter_err) => {
                        // Re-verify: concurrent process may have added the column between
                        // our check and the ALTER. Only propagate if column still absent.
                        if !check_col_exists(&conn).context("Re-check after ALTER TABLE failure")? {
                            return Err(alter_err)
                                .context("Failed to add tee_deleted column to p256_session_keys");
                        }
                    }
                }
            }
        }
        // Migration: add lifecycle_status column to wallets for DBs created before
        // issue #42 (dormant-key freeze). Same idempotent PRAGMA-check + ALTER pattern
        // as tee_deleted above. Existing rows default to 'active'.
        {
            let check_col_exists = |c: &Connection| -> Result<bool> {
                let mut stmt = c
                    .prepare("PRAGMA table_info(wallets)")
                    .context("Failed to query wallets schema")?;
                let names: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<rusqlite::Result<_>>()
                    .context("Failed to read wallets schema")?;
                Ok(names.iter().any(|n| n == "lifecycle_status"))
            };
            if !check_col_exists(&conn)? {
                match conn.execute_batch(
                    "ALTER TABLE wallets \
                     ADD COLUMN lifecycle_status TEXT NOT NULL DEFAULT 'active';",
                ) {
                    Ok(()) => {}
                    Err(alter_err) => {
                        // Re-verify: concurrent process may have added the column between
                        // our check and the ALTER. Only propagate if column still absent.
                        if !check_col_exists(&conn).context("Re-check after ALTER TABLE failure")? {
                            return Err(alter_err)
                                .context("Failed to add lifecycle_status column to wallets");
                        }
                    }
                }
            }
        }
        // stderr, not stdout: the `api-key generate` CLI prints the new key to
        // stdout, so keep this diagnostic off stdout to allow clean capture,
        // e.g. `KEY=$(api-key generate --label svc)`. The API server logs both
        // streams to the same file, so server-side behavior is unchanged.
        eprintln!("📦 SQLite DB opened: {}", path);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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
                w.key_id,
                w.address,
                w.public_key,
                w.derivation_path,
                w.description,
                w.key_usage,
                w.key_spec,
                w.origin,
                w.passkey_pubkey,
                w.credential_id,
                w.sign_count,
                w.status,
                w.error_msg,
                w.created_at,
            ],
        )
        .context("insert_wallet")?;
        Ok(())
    }

    pub fn get_wallet(&self, key_id: &str) -> Result<Option<WalletRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT key_id, address, public_key, derivation_path, description, key_usage, \
             key_spec, origin, passkey_pubkey, credential_id, sign_count, status, error_msg, \
             created_at FROM wallets WHERE key_id = ?1",
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
        &self,
        key_id: &str,
        address: &str,
        public_key: &str,
        derivation_path: &str,
        status: &str,
    ) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET address=?2, public_key=?3, derivation_path=?4, status=?5 \
             WHERE key_id=?1",
            params![key_id, address, public_key, derivation_path, status],
        )?;
        Ok(())
    }

    pub fn update_wallet_status(
        &self,
        key_id: &str,
        status: &str,
        error_msg: Option<&str>,
    ) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE wallets SET status=?2, error_msg=?3 WHERE key_id=?1",
            params![key_id, status, error_msg],
        )?;
        Ok(())
    }

    pub fn update_wallet_passkey(
        &self,
        key_id: &str,
        passkey_pubkey: &str,
        credential_id: Option<&str>,
    ) -> Result<()> {
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
             created_at FROM wallets ORDER BY created_at",
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

    // ── Key lifecycle (issue #42: dormant-key freeze) ──

    /// Last successful operation timestamp for a key, derived from tx_log
    /// (no dedicated column — the data already lives there). Returns the RFC3339
    /// string of MAX(created_at) over successful tx_log rows for this key, or
    /// None if the key has never had a successful logged operation.
    ///
    /// A tx_log row may identify the key by `key_id` OR by signing `addr` (the
    /// address-based Sign/SignHash paths log only the address). We therefore
    /// match on key_id, the wallet's primary address, AND any derived address in
    /// address_index — otherwise an actively-used address-mode key would look
    /// dormant and get wrongly frozen.
    pub fn last_used_at(&self, key_id: &str) -> Result<Option<String>> {
        let conn = self.lock();
        // MAX over a TEXT column returns NULL when no rows match; map to None.
        let v: Option<String> = conn
            .query_row(
                "SELECT MAX(created_at) FROM tx_log WHERE success=1 AND ( \
                   key_id=?1 \
                   OR addr=(SELECT address FROM wallets WHERE key_id=?1) \
                   OR addr IN (SELECT address FROM address_index WHERE key_id=?1) \
                 )",
                params![key_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .context("last_used_at")?;
        Ok(v)
    }

    /// Current lifecycle_status for a key ('active' | 'frozen'), or None if the
    /// key_id does not exist.
    pub fn get_lifecycle_status(&self, key_id: &str) -> Result<Option<String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT lifecycle_status FROM wallets WHERE key_id=?1")?;
        let mut rows = stmt.query_map(params![key_id], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Set lifecycle_status for a key. Returns true if a row was updated.
    pub fn set_lifecycle_status(&self, key_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock();
        let n = conn.execute(
            "UPDATE wallets SET lifecycle_status=?2 WHERE key_id=?1",
            params![key_id, status],
        )?;
        Ok(n > 0)
    }

    /// Auto-freeze dormant keys: set lifecycle_status='frozen' for every currently
    /// 'active' wallet whose last successful activity is older than `threshold_secs`.
    /// "Last activity" = the most recent successful tx_log row for the key, falling
    /// back to the wallet's created_at when it has no successful operations yet (so a
    /// freshly created, never-signed key is not frozen until it has genuinely aged).
    /// Returns the list of key_ids that were frozen.
    pub fn freeze_dormant_keys(&self, now_unix: i64, threshold_secs: i64) -> Result<Vec<String>> {
        let cutoff = now_unix - threshold_secs;
        let conn = self.lock();
        // created_at (wallets + tx_log) is RFC3339 text; strftime('%s', ...) parses it
        // to a unix epoch for comparison. COALESCE picks the latest signing activity,
        // falling back to wallet creation time.
        // Match tx_log rows by key_id OR by signing address (primary or derived),
        // mirroring last_used_at — an address-mode Sign logs only `addr`, so a
        // key_id-only join would miss active use and freeze a live key.
        let mut stmt = conn.prepare(
            "SELECT w.key_id FROM wallets w \
             WHERE w.lifecycle_status='active' \
               AND COALESCE( \
                     (SELECT CAST(strftime('%s', MAX(t.created_at)) AS INTEGER) \
                        FROM tx_log t WHERE t.success=1 AND ( \
                          t.key_id=w.key_id \
                          OR t.addr=w.address \
                          OR t.addr IN (SELECT address FROM address_index ai WHERE ai.key_id=w.key_id) \
                        )), \
                     CAST(strftime('%s', w.created_at) AS INTEGER) \
                   ) < ?1",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![cutoff], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for id in &ids {
            conn.execute(
                "UPDATE wallets SET lifecycle_status='frozen' \
                 WHERE key_id=?1 AND lifecycle_status='active'",
                params![id],
            )?;
        }
        Ok(ids)
    }

    // ── Address index ──

    pub fn upsert_address(
        &self,
        address: &str,
        key_id: &str,
        derivation_path: &str,
        public_key: Option<&str>,
    ) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT OR REPLACE INTO address_index (address, key_id, derivation_path, public_key) \
             VALUES (?1,?2,?3,?4)",
            params![address, key_id, derivation_path, public_key],
        )?;
        Ok(())
    }

    /// #52: look up the cached Ethereum address for a (key_id, derivation_path)
    /// pair. Used to verify a caller-supplied `from` against the real signing
    /// address before building an EIP-3009 authorization — a mismatch would
    /// revert on-chain (wasted gas). Returns None if the pair has not been
    /// derived/cached yet (caller should DeriveAddress first).
    pub fn address_for_key_path(
        &self,
        key_id: &str,
        derivation_path: &str,
    ) -> Result<Option<String>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT address FROM address_index WHERE key_id=?1 AND derivation_path=?2")?;
        let mut rows = stmt.query_map(params![key_id, derivation_path], |row| {
            row.get::<_, String>(0)
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
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

    // ── Agent keys ──

    fn map_agent_key_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentKeyRow> {
        Ok(AgentKeyRow {
            wallet_id: row.get(0)?,
            agent_index: row.get::<_, i64>(1)? as u32,
            human_id: row.get(2)?,
            agent_address: row.get(3)?,
            public_key_compressed: row.get(4)?,
            credential_hash: row.get(5)?,
            credential_jwt: row.get(6)?,
            credential_expires_at: row.get(7)?,
            status: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            revoked_at: row.get(11)?,
        })
    }

    pub fn insert_agent_key(&self, row: &AgentKeyRow) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO agent_keys (wallet_id, agent_index, human_id, agent_address, \
             public_key_compressed, credential_hash, credential_jwt, credential_expires_at, \
             status, created_at, updated_at, revoked_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                row.wallet_id,
                row.agent_index as i64,
                row.human_id,
                row.agent_address,
                row.public_key_compressed,
                row.credential_hash,
                row.credential_jwt,
                row.credential_expires_at,
                row.status,
                row.created_at,
                row.updated_at,
                row.revoked_at,
            ],
        )
        .context("insert_agent_key")?;
        Ok(())
    }

    pub fn get_agent_key(&self, wallet_id: &str, agent_index: u32) -> Result<Option<AgentKeyRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT wallet_id, agent_index, human_id, agent_address, public_key_compressed, \
             credential_hash, credential_jwt, credential_expires_at, status, created_at, \
             updated_at, revoked_at FROM agent_keys WHERE wallet_id=?1 AND agent_index=?2",
        )?;
        let mut rows = stmt.query_map(
            params![wallet_id, agent_index as i64],
            Self::map_agent_key_row,
        )?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn list_agent_keys_for_human(&self, human_id: &str) -> Result<Vec<AgentKeyRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT wallet_id, agent_index, human_id, agent_address, public_key_compressed, \
             credential_hash, credential_jwt, credential_expires_at, status, created_at, \
             updated_at, revoked_at FROM agent_keys WHERE human_id=?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![human_id], Self::map_agent_key_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn count_agent_keys_for_human(&self, human_id: &str) -> Result<i64> {
        let conn = self.lock();
        let count = conn.query_row(
            "SELECT COUNT(*) FROM agent_keys WHERE human_id=?1 AND status='active'",
            params![human_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Atomically allocate the next agent_index for a wallet.
    /// Uses MAX(agent_index)+1 within a single mutex acquisition so that
    /// concurrent requests cannot race to the same index.
    pub fn next_agent_index_for_wallet(&self, wallet_id: &str) -> Result<u32> {
        let conn = self.lock();
        let max_idx: Option<i64> = conn.query_row(
            "SELECT MAX(agent_index) FROM agent_keys WHERE wallet_id=?1",
            params![wallet_id],
            |row| row.get(0),
        )?;
        Ok(max_idx.map(|m| (m + 1) as u32).unwrap_or(0))
    }

    pub fn update_agent_credential(
        &self,
        wallet_id: &str,
        agent_index: u32,
        credential_hash: &str,
        credential_expires_at: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        conn.execute(
            "UPDATE agent_keys SET credential_hash=?3, \
             credential_expires_at=?4, updated_at=?5 WHERE wallet_id=?1 AND agent_index=?2",
            params![
                wallet_id,
                agent_index as i64,
                credential_hash,
                credential_expires_at,
                now,
            ],
        )
        .context("update_agent_credential")?;
        Ok(())
    }

    pub fn revoke_agent_key(&self, wallet_id: &str, agent_index: u32) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        let updated = conn.execute(
            "UPDATE agent_keys SET status='revoked', revoked_at=?3, updated_at=?3 \
             WHERE wallet_id=?1 AND agent_index=?2 AND status != 'revoked'",
            params![wallet_id, agent_index as i64, now],
        )?;
        Ok(updated > 0)
    }

    // ── JWT secret metadata ──

    fn map_jwt_secret_meta_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JwtSecretMetaRow> {
        Ok(JwtSecretMetaRow {
            kid: row.get(0)?,
            status: row.get(1)?,
            created_at: row.get(2)?,
            retired_at: row.get(3)?,
            expires_at: row.get(4)?,
        })
    }

    pub fn upsert_jwt_secret_meta(&self, row: &JwtSecretMetaRow) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO jwt_secret_meta (kid, status, created_at, retired_at, expires_at) \
             VALUES (?1,?2,?3,?4,?5) \
             ON CONFLICT(kid) DO UPDATE SET status=excluded.status, \
             retired_at=excluded.retired_at, expires_at=excluded.expires_at",
            params![
                row.kid,
                row.status,
                row.created_at,
                row.retired_at,
                row.expires_at
            ],
        )
        .context("upsert_jwt_secret_meta")?;
        Ok(())
    }

    pub fn list_jwt_secret_meta(&self) -> Result<Vec<JwtSecretMetaRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT kid, status, created_at, retired_at, expires_at \
             FROM jwt_secret_meta ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], Self::map_jwt_secret_meta_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_current_jwt_kid(&self) -> Result<Option<String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT kid FROM jwt_secret_meta WHERE status='current' ORDER BY created_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn list_all_agent_keys(&self) -> Result<Vec<AgentKeyRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT wallet_id, agent_index, human_id, agent_address, public_key_compressed, \
             credential_hash, credential_jwt, credential_expires_at, status, created_at, \
             updated_at, revoked_at FROM agent_keys ORDER BY human_id, agent_index",
        )?;
        let rows = stmt.query_map([], Self::map_agent_key_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── P256 Session Key methods ──

    fn map_p256_session_key_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<P256SessionKeyRow> {
        Ok(P256SessionKeyRow {
            wallet_id: row.get(0)?,
            session_index: row.get::<_, u32>(1)?,
            human_id: row.get(2)?,
            pub_key_x: row.get(3)?,
            pub_key_y: row.get(4)?,
            credential_hash: row.get(5)?,
            credential_expires_at: row.get(6)?,
            status: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            revoked_at: row.get(10)?,
        })
    }

    /// Atomically allocate a pending P256 session key slot.
    ///
    /// Uses `BEGIN IMMEDIATE` to prevent TOCTOU: between the active-key count check and
    /// the INSERT, no other writer can interleave (SQLite exclusive write lock held for
    /// the whole transaction). Without this, two concurrent creates for the same wallet
    /// could both read count=1, both pass the max=2 guard, and both insert — exceeding quota.
    ///
    /// Quota counting includes:
    /// - `active` rows whose credential has not yet expired
    /// - `pending` rows created within the last `PENDING_TTL_SECS` (stuck-pending guard)
    ///
    /// A stuck-pending row (host crashed after allocate but before activate/cleanup) is
    /// counted for `PENDING_TTL_SECS` seconds so concurrent creates don't overshoot quota;
    /// after that it's eligible for GC via `list_expired_p256_session_keys`.
    pub fn allocate_p256_session_key_pending(
        &self,
        wallet_id: &str,
        human_id: &str,
        now_unix: i64,
        max_active: i64,
    ) -> Result<u32> {
        let pending_cutoff = now_unix - PENDING_TTL_SECS;
        let now_rfc = Utc::now().to_rfc3339();
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let current_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM p256_session_keys \
             WHERE wallet_id=?1 \
               AND (
                 (status = 'active' AND (credential_expires_at IS NULL OR credential_expires_at > ?2))
                 OR (status = 'pending' AND CAST(strftime('%s', created_at) AS INTEGER) >= ?3)
               )",
            params![wallet_id, now_unix, pending_cutoff],
            |row| row.get(0),
        )?;

        if current_count >= max_active {
            return Err(anyhow::anyhow!(
                "Wallet already has {} active/recent-pending P256 session keys (max {}). \
                 Revoke an existing key or wait for stuck pending to expire ({}s).",
                current_count,
                max_active,
                PENDING_TTL_SECS
            ));
        }

        tx.execute(
            "INSERT INTO p256_session_keys \
             (wallet_id, session_index, human_id, pub_key_x, pub_key_y, \
              credential_hash, credential_expires_at, status, created_at, updated_at) \
             SELECT ?1, COALESCE(MAX(session_index)+1, 0), ?2, '', '', NULL, NULL, 'pending', ?3, ?3 \
             FROM p256_session_keys WHERE wallet_id=?1",
            params![wallet_id, human_id, now_rfc],
        )
        .context("allocate_p256_session_key_pending INSERT")?;

        let idx: i64 = tx.query_row(
            "SELECT session_index FROM p256_session_keys \
             WHERE wallet_id=?1 AND status='pending' ORDER BY session_index DESC LIMIT 1",
            params![wallet_id],
            |row| row.get(0),
        )?;

        tx.commit()?;
        Ok(idx as u32)
    }

    /// Atomically activate a pending P256 session key slot.
    ///
    /// Uses `BEGIN IMMEDIATE` for the same TOCTOU reason as `allocate_p256_session_key_pending`:
    /// a slow create could race with another create that completes first. By rechecking the
    /// active-key count inside the same exclusive transaction that does the UPDATE, we ensure
    /// the max-2 invariant is never violated even under concurrent requests.
    ///
    /// The active-count recheck uses the same quota definition as allocate (active + non-expired).
    /// If the recheck fails, the caller must delete the TEE key material it just created.
    pub fn activate_p256_session_key(
        &self,
        wallet_id: &str,
        session_index: u32,
        pub_key_x: &str,
        pub_key_y: &str,
        credential_hash: &str,
        credential_expires_at: i64,
        max_active: i64,
    ) -> Result<()> {
        let now_unix = Utc::now().timestamp();
        let now_rfc = Utc::now().to_rfc3339();
        let mut conn = self.lock();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let active_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM p256_session_keys \
             WHERE wallet_id=?1 \
               AND status='active' \
               AND (credential_expires_at IS NULL OR credential_expires_at > ?2)",
            params![wallet_id, now_unix],
            |row| row.get(0),
        )?;
        if active_count >= max_active {
            return Err(anyhow::anyhow!(
                "activate_p256_session_key: already {} active keys for wallet {} (max {}); \
                 slow create raced with another key. Caller will clean up TEE material.",
                active_count,
                wallet_id,
                max_active
            ));
        }

        let n = tx
            .execute(
                "UPDATE p256_session_keys SET pub_key_x=?3, pub_key_y=?4, \
                 credential_hash=?5, credential_expires_at=?6, status='active', updated_at=?7 \
                 WHERE wallet_id=?1 AND session_index=?2 AND status='pending'",
                params![
                    wallet_id,
                    session_index,
                    pub_key_x,
                    pub_key_y,
                    credential_hash,
                    credential_expires_at,
                    now_rfc
                ],
            )
            .context("activate_p256_session_key")?;
        if n == 0 {
            return Err(anyhow::anyhow!(
                "activate_p256_session_key: no pending row found for ({}, {})",
                wallet_id,
                session_index
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_p256_session_key(
        &self,
        wallet_id: &str,
        session_index: u32,
    ) -> Result<Option<P256SessionKeyRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT wallet_id, session_index, human_id, pub_key_x, pub_key_y, \
             credential_hash, credential_expires_at, status, created_at, updated_at, revoked_at \
             FROM p256_session_keys WHERE wallet_id=?1 AND session_index=?2",
        )?;
        let mut rows = stmt.query_map(
            params![wallet_id, session_index],
            Self::map_p256_session_key_row,
        )?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Delete a pending P256 session key placeholder after TA key generation fails.
    /// Only removes rows with status='pending'; does not affect active or revoked keys.
    pub fn delete_p256_session_key_pending(
        &self,
        wallet_id: &str,
        session_index: u32,
    ) -> Result<bool> {
        let conn = self.lock();
        let n = conn.execute(
            "DELETE FROM p256_session_keys WHERE wallet_id=?1 AND session_index=?2 AND status='pending'",
            params![wallet_id, session_index],
        )?;
        Ok(n > 0)
    }

    /// List P256 session key indices eligible for GC.
    /// Returns active/pending rows past their expiry and stuck-pending rows (created before
    /// gc_cutoff - PENDING_TTL_SECS) that were never activated (e.g., host crash after allocate).
    /// `exclude_session_index` lets the signing path skip the key currently being used.
    pub fn list_expired_p256_session_keys(
        &self,
        wallet_id: &str,
        gc_cutoff_unix: i64,
        exclude_session_index: Option<u32>,
    ) -> Result<Vec<u32>> {
        // stuck_pending_cutoff = gc_cutoff - PENDING_TTL_SECS.
        // Caller sets gc_cutoff = now - 60s (clock-drift grace window), so effective
        // stuck threshold is now - 360s (6 min). Pending rows older than this are GC-eligible.
        // The extra 60s is intentional: a pending that just crossed 5 min gets one more GC cycle
        // before actual deletion, giving in-flight TA calls a final chance to complete.
        let stuck_pending_cutoff = gc_cutoff_unix - PENDING_TTL_SECS;
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT session_index FROM p256_session_keys \
             WHERE wallet_id=?1 \
               AND (?3 IS NULL OR session_index != ?3) \
               AND (
                 -- Expired active or pending (credential_expires_at set and past)
                 (credential_expires_at IS NOT NULL AND credential_expires_at <= ?2 \
                  AND status IN ('active', 'pending'))
                 OR
                 -- Stuck pending: created long ago, NULL expiry, host crashed after allocate
                 (status = 'pending' \
                  AND CAST(strftime('%s', created_at) AS INTEGER) < ?4)
               )",
        )?;
        let excl: Option<i64> = exclude_session_index.map(|i| i as i64);
        let indices: Vec<u32> = stmt
            .query_map(
                params![wallet_id, gc_cutoff_unix, excl, stuck_pending_cutoff],
                |row| row.get(0),
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(indices)
    }

    /// Atomically claim a P256 session key for GC by setting status='revoked'.
    /// Status guard ensures we never touch an already-revoked row.
    /// Returns true if the row was claimed (rows_affected > 0), false if already revoked/gone.
    /// Callers should proceed with TEE deletion only when this returns true.
    pub fn mark_p256_session_key_gc(&self, wallet_id: &str, session_index: u32) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        let n = conn.execute(
            "UPDATE p256_session_keys SET status='revoked', revoked_at=?3, updated_at=?3 \
             WHERE wallet_id=?1 AND session_index=?2 AND status IN ('active', 'pending')",
            params![wallet_id, session_index, now],
        )?;
        Ok(n > 0)
    }

    /// Mark a P256 session key's TEE entry as confirmed-deleted (tee_deleted=1).
    /// Called after a successful tee.delete_p256_session_key(). Safe to call redundantly.
    pub fn mark_p256_tee_deleted(&self, wallet_id: &str, session_index: u32) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE p256_session_keys SET tee_deleted=1 \
             WHERE wallet_id=?1 AND session_index=?2",
            params![wallet_id, session_index],
        )?;
        Ok(())
    }

    /// Return session_index values for revoked keys whose TEE deletion was not yet confirmed.
    /// These have status='revoked' AND tee_deleted=0 — the TEE delete failed or was never attempted.
    /// Called at the start of each GC pass to retry phantom TEE cleanup.
    pub fn list_unconfirmed_tee_deletes(&self, wallet_id: &str) -> Result<Vec<u32>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT session_index FROM p256_session_keys \
             WHERE wallet_id=?1 AND status='revoked' AND tee_deleted=0",
        )?;
        let indices: Vec<u32> = stmt
            .query_map(params![wallet_id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(indices)
    }

    /// Check whether a P256 session key exists with status='revoked'.
    /// Used as a post-check after TEE signing to detect concurrent revocation (TOCTOU guard).
    pub fn p256_session_key_is_revoked(&self, wallet_id: &str, session_index: u32) -> Result<bool> {
        let conn = self.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM p256_session_keys \
             WHERE wallet_id=?1 AND session_index=?2 AND status='revoked'",
            params![wallet_id, session_index],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .context("p256_session_key_is_revoked")
    }

    /// Physically delete P256 session key rows that are fully cleaned up:
    /// status='revoked', tee_deleted=1, and revoked_at older than `older_than_unix`.
    /// Called by GC Pass 2 to prevent unbounded row accumulation.
    /// Returns the number of rows deleted.
    pub fn delete_confirmed_revoked_p256_session_keys(
        &self,
        wallet_id: &str,
        older_than_unix: i64,
    ) -> Result<usize> {
        let conn = self.lock();
        let n = conn.execute(
            "DELETE FROM p256_session_keys \
             WHERE wallet_id=?1 \
               AND status='revoked' \
               AND tee_deleted=1 \
               AND (revoked_at IS NULL \
                    OR CAST(strftime('%s', revoked_at) AS INTEGER) < ?2)",
            params![wallet_id, older_than_unix],
        )?;
        Ok(n)
    }

    pub fn retire_expired_jwt_secrets(&self, now_unix: i64) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        let updated = conn.execute(
            "UPDATE jwt_secret_meta SET status='retired', retired_at=?2 \
             WHERE expires_at IS NOT NULL AND expires_at <= ?1 AND status != 'retired'",
            params![now_unix, now],
        )?;
        Ok(updated)
    }

    // ── Challenge management ──

    pub fn store_challenge(
        &self,
        id: &str,
        challenge: &[u8],
        key_id: Option<&str>,
        purpose: &str,
        rp_id: &str,
        ttl_secs: i64,
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
             FROM challenges WHERE id=?1 AND expires_at > ?2",
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
        )
        .context("generate_api_key")?;
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
        let mut stmt =
            conn.prepare("SELECT api_key, label, created_at FROM api_keys ORDER BY created_at")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Revoke (delete) an API key.
    pub fn revoke_api_key(&self, key: &str) -> Result<bool> {
        let conn = self.lock();
        let deleted = conn.execute("DELETE FROM api_keys WHERE api_key = ?1", params![key])?;
        Ok(deleted > 0)
    }

    /// Check if any API keys exist in DB.
    pub fn has_api_keys(&self) -> Result<bool> {
        let conn = self.lock();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM api_keys", [], |row| row.get(0))?;
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

    // ── TX log ──

    pub fn record_tx(
        &self,
        op: &str,
        key_id: Option<&str>,
        addr: Option<&str>,
        webauthn: bool,
        latency_ms: u64,
        success: bool,
        is_panic: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO tx_log (op, key_id, addr, webauthn, latency_ms, success, is_panic, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![op, key_id, addr, webauthn as i32, latency_ms as i64,
                    success as i32, is_panic as i32, now],
        )?;
        Ok(())
    }

    pub fn get_tx_stats(&self) -> Result<TxStats> {
        let conn = self.lock();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let today_prefix = format!("{}%", today);

        let total_sign: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tx_log WHERE op IN ('Sign','SignHash') AND success=1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let daily_sign: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tx_log WHERE op IN ('Sign','SignHash') AND success=1 AND created_at LIKE ?1",
            params![&today_prefix], |r| r.get(0),
        ).unwrap_or(0);

        let total_ops: i64 = conn
            .query_row("SELECT COUNT(*) FROM tx_log WHERE success=1", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let daily_ops: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tx_log WHERE success=1 AND created_at LIKE ?1",
                params![&today_prefix],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let avg_sign_ms: f64 = conn.query_row(
            "SELECT COALESCE(AVG(latency_ms),0) FROM tx_log WHERE op IN ('Sign','SignHash') AND success=1",
            [], |r| r.get(0),
        ).unwrap_or(0.0);

        let avg_derive_ms: f64 = conn.query_row(
            "SELECT COALESCE(AVG(latency_ms),0) FROM tx_log WHERE op='DeriveAddress' AND success=1",
            [], |r| r.get(0),
        ).unwrap_or(0.0);

        let panic_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tx_log WHERE is_panic=1", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let error_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tx_log WHERE success=0", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let webauthn_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tx_log WHERE webauthn=1 AND success=1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        Ok(TxStats {
            total_sign,
            daily_sign,
            total_ops,
            daily_ops,
            avg_sign_ms,
            avg_derive_ms,
            panic_count,
            error_count,
            webauthn_count,
        })
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
        db.update_wallet_derived("w1", "0xaddr", "0xpub", "m/44'/60'/0'/0/0", "ready")
            .unwrap();
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
        db.upsert_address("0xaaaa", "w1", "m/44'/60'/0'/0/0", Some("0xpub"))
            .unwrap();
        let row = db.lookup_address("0xaaaa").unwrap().unwrap();
        assert_eq!(row.key_id, "w1");
        assert_eq!(row.public_key.as_deref(), Some("0xpub"));
    }

    #[test]
    fn challenge_store_and_consume() {
        let db = test_db();
        let challenge = vec![1u8, 2, 3, 4];
        db.store_challenge("c1", &challenge, None, "registration", "example.com", 300)
            .unwrap();
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
        db.update_wallet_passkey("w1", "0x04new", Some("cred-123"))
            .unwrap();
        let got = db.get_wallet("w1").unwrap().unwrap();
        assert_eq!(got.passkey_pubkey.as_deref(), Some("0x04new"));
        assert_eq!(got.credential_id.as_deref(), Some("cred-123"));
    }

    #[test]
    fn concurrent_access() {
        use std::thread;
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-concurrent")).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let db = db.clone();
                thread::spawn(move || {
                    let addr = format!("0xaddr{}", i);
                    db.upsert_address(&addr, "w-concurrent", &format!("m/0/{}", i), None)
                        .unwrap();
                    db.get_wallet("w-concurrent").unwrap().unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
        // All 10 addresses should exist
        for i in 0..10 {
            let addr = format!("0xaddr{}", i);
            assert!(db.lookup_address(&addr).unwrap().is_some());
        }
    }

    #[test]
    fn update_wallet_status_with_error() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-err")).unwrap();
        db.update_wallet_status("w-err", "error", Some("PBKDF2 timeout"))
            .unwrap();
        let got = db.get_wallet("w-err").unwrap().unwrap();
        assert_eq!(got.status, "error");
        assert_eq!(got.error_msg.as_deref(), Some("PBKDF2 timeout"));
    }

    #[test]
    fn update_sign_count() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-sc")).unwrap();
        db.update_wallet_sign_count("w-sc", 42).unwrap();
        let got = db.get_wallet("w-sc").unwrap().unwrap();
        assert_eq!(got.sign_count, 42);
    }

    #[test]
    fn cleanup_expired_challenges() {
        let db = test_db();
        // Store a challenge with -1 second TTL (already expired)
        db.store_challenge("c-expired", &[1, 2, 3], None, "auth", "localhost", -1)
            .unwrap();
        db.store_challenge("c-valid", &[4, 5, 6], None, "auth", "localhost", 300)
            .unwrap();

        let cleaned = db.cleanup_expired_challenges().unwrap();
        assert_eq!(cleaned, 1);

        // Valid challenge should still be consumable
        assert!(db.consume_challenge("c-valid").unwrap().is_some());
        // Expired one was cleaned up
        assert!(db.consume_challenge("c-expired").unwrap().is_none());
    }

    // ── Issue #42: dormant-key freeze + last_used_at ──

    #[test]
    fn lifecycle_status_defaults_active_and_round_trips() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-lc")).unwrap();
        assert_eq!(
            db.get_lifecycle_status("w-lc").unwrap().as_deref(),
            Some("active")
        );
        assert!(db.set_lifecycle_status("w-lc", "frozen").unwrap());
        assert_eq!(
            db.get_lifecycle_status("w-lc").unwrap().as_deref(),
            Some("frozen")
        );
        // Unknown key -> None
        assert!(db.get_lifecycle_status("nope").unwrap().is_none());
        assert!(!db.set_lifecycle_status("nope", "active").unwrap());
    }

    #[test]
    fn last_used_at_none_then_some() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-lu")).unwrap();
        // No tx_log rows yet.
        assert!(db.last_used_at("w-lu").unwrap().is_none());
        // A failed op does not count.
        db.record_tx("Sign", Some("w-lu"), None, false, 5, false, false)
            .unwrap();
        assert!(db.last_used_at("w-lu").unwrap().is_none());
        // A successful op sets it.
        db.record_tx("Sign", Some("w-lu"), None, false, 5, true, false)
            .unwrap();
        assert!(db.last_used_at("w-lu").unwrap().is_some());
    }

    #[test]
    fn freeze_dormant_by_created_at_fallback() {
        let db = test_db();
        // sample_wallet created_at is 2026-03-02 (well in the past relative to now).
        db.insert_wallet(&sample_wallet("w-old")).unwrap();
        let now = chrono::Utc::now().timestamp();
        // Threshold of 1 day: the never-used, old key is dormant -> frozen.
        let frozen = db.freeze_dormant_keys(now, 24 * 60 * 60).unwrap();
        assert_eq!(frozen, vec!["w-old".to_string()]);
        assert_eq!(
            db.get_lifecycle_status("w-old").unwrap().as_deref(),
            Some("frozen")
        );
    }

    #[test]
    fn freeze_skips_recently_used_keys() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-active")).unwrap();
        // Recent successful op (record_tx stamps now).
        db.record_tx("Sign", Some("w-active"), None, false, 5, true, false)
            .unwrap();
        let now = chrono::Utc::now().timestamp();
        // Even with a 1-second threshold, the just-now activity keeps it active.
        let frozen = db.freeze_dormant_keys(now, 1).unwrap();
        assert!(frozen.is_empty());
        assert_eq!(
            db.get_lifecycle_status("w-active").unwrap().as_deref(),
            Some("active")
        );
    }

    #[test]
    fn freeze_is_idempotent_and_skips_already_frozen() {
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-f")).unwrap();
        db.set_lifecycle_status("w-f", "frozen").unwrap();
        let now = chrono::Utc::now().timestamp();
        // Already frozen -> not returned again.
        let frozen = db.freeze_dormant_keys(now, 1).unwrap();
        assert!(frozen.is_empty());
    }

    #[test]
    fn freeze_skips_address_mode_recently_used() {
        // Regression (codex review): address-based Sign/SignHash log activity by
        // `addr` with key_id=None. last_used_at / freeze_dormant_keys must still
        // credit that activity to the wallet via its address (primary or derived),
        // otherwise an actively-used address-mode key looks dormant and is wrongly
        // frozen.
        let db = test_db();
        let mut w = sample_wallet("w-addr");
        w.address = Some("0xdeadbeef00000000000000000000000000000000".to_string());
        db.insert_wallet(&w).unwrap();
        // Recent op recorded by ADDRESS only (no key_id) — the address-mode path.
        db.record_tx("Sign", None, w.address.as_deref(), false, 5, true, false)
            .unwrap();
        let now = chrono::Utc::now().timestamp();
        // Even a 1-second threshold must not freeze it: the address activity counts.
        let frozen = db.freeze_dormant_keys(now, 1).unwrap();
        assert!(
            frozen.is_empty(),
            "address-mode activity must keep the key active"
        );
        assert_eq!(
            db.get_lifecycle_status("w-addr").unwrap().as_deref(),
            Some("active")
        );
        assert!(
            db.last_used_at("w-addr").unwrap().is_some(),
            "last_used_at must see the address-mode op"
        );
    }

    #[test]
    fn address_for_key_path_lookup() {
        // #52: GToken from-check resolves the signing address by (key_id, path).
        let db = test_db();
        db.insert_wallet(&sample_wallet("w-1")).unwrap(); // satisfy address_index FK
        db.upsert_address("0xABC123", "w-1", "m/44'/60'/0'/0/0", None)
            .unwrap();
        // exact (key_id, path) → hit
        assert_eq!(
            db.address_for_key_path("w-1", "m/44'/60'/0'/0/0")
                .unwrap()
                .as_deref(),
            Some("0xABC123")
        );
        // same key, different path → miss
        assert!(db
            .address_for_key_path("w-1", "m/44'/60'/0'/0/1")
            .unwrap()
            .is_none());
        // different key, same path → miss
        assert!(db
            .address_for_key_path("w-2", "m/44'/60'/0'/0/0")
            .unwrap()
            .is_none());
    }
}
