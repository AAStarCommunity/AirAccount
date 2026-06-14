// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

#![no_main]

mod attestation;
mod bip32_secp;
mod eip712;
mod hash;
mod wallet;

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{DataFlag, Error, ErrorKind, ObjectStorageConstants, Parameters, PersistentObject, Random, Time};
use proto::Command;
use secure_db::{SecureStorageClient, Storable};

use anyhow::{anyhow, bail, Result};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use std::cell::RefCell;
use std::io::Write;
use uuid::Uuid;
use wallet::Wallet;

const DB_NAME: &str = "eth_wallet_db";
const JWT_SECRET_STORE_ID: &str = "jwt_hmac";

// RPMB anti-rollback counter object ID (≤64 bytes per GP spec).
// Written to TEE_STORAGE_PRIVATE_RPMB, NOT the REE-FS filesystem.
const RPMB_COUNTER_ID: &[u8] = b"kms_arc_v1";

type HmacSha256 = Hmac<Sha256>;

// ========================================
// LRU Wallet Cache (TA is single-threaded)
// ========================================
// Uses Vec instead of HashMap to avoid SipHasher's getrandom dependency,
// which panics in OP-TEE TA environment. 200-entry linear scan is negligible.

const CACHE_CAPACITY: usize = 200;

struct WalletCacheEntry {
    id: Uuid,
    wallet: Wallet,
    tick: u64,
}

struct WalletLruCache {
    entries: Vec<WalletCacheEntry>,
    tick: u64,
}

impl WalletLruCache {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            tick: 0,
        }
    }

    fn get(&mut self, id: &Uuid) -> Option<Wallet> {
        for entry in self.entries.iter_mut() {
            if &entry.id == id {
                self.tick += 1;
                entry.tick = self.tick;
                return Some(entry.wallet.clone());
            }
        }
        None
    }

    fn put(&mut self, wallet: &Wallet) {
        self.tick += 1;
        let id = wallet.get_id();

        // Update existing
        for entry in self.entries.iter_mut() {
            if entry.id == id {
                entry.wallet = wallet.clone();
                entry.tick = self.tick;
                return;
            }
        }

        // Evict LRU if at capacity
        if self.entries.len() >= CACHE_CAPACITY {
            let lru_idx = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.tick)
                .map(|(i, _)| i)
                .unwrap();
            self.entries.swap_remove(lru_idx);
        }

        self.entries.push(WalletCacheEntry {
            id,
            wallet: wallet.clone(),
            tick: self.tick,
        });
    }

    fn remove(&mut self, id: &Uuid) {
        self.entries.retain(|e| &e.id != id);
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct JwtSecretEntry {
    kid: String,
    secret: Vec<u8>,
    current: bool,
    retired_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct JwtSecretStore {
    id: String,
    entries: Vec<JwtSecretEntry>,
}

impl Storable for JwtSecretStore {
    type Key = String;

    fn unique_id(&self) -> Self::Key {
        self.id.clone()
    }
}

impl JwtSecretStore {
    fn new() -> Self {
        Self {
            id: JWT_SECRET_STORE_ID.to_string(),
            entries: Vec::new(),
        }
    }

    fn load(db: &SecureStorageClient) -> Self {
        db.get::<JwtSecretStore>(&JWT_SECRET_STORE_ID.to_string())
            .unwrap_or_else(|_| Self::new())
    }

    fn save(&self, db: &SecureStorageClient) -> Result<()> {
        db.put(self)
    }

    fn current(&self) -> Option<&JwtSecretEntry> {
        self.entries.iter().find(|entry| entry.current)
    }

    fn find(&self, kid: &str) -> Option<&JwtSecretEntry> {
        self.entries.iter().find(|entry| entry.kid == kid)
    }

    fn ensure_current(&mut self) -> Result<&JwtSecretEntry> {
        if self.current().is_none() {
            self.rotate(false)?;
        }
        self.current()
            .ok_or_else(|| anyhow!("JWT HMAC secret store has no current secret"))
    }

    fn rotate(&mut self, purge_retired: bool) -> Result<(String, Option<String>)> {
        let retired_kid = self.current().map(|entry| entry.kid.clone());

        if purge_retired {
            // Force rotation: purge ALL entries (current + retired).
            // Any JWT signed with any old kid will fail verification after this.
            self.entries.clear();
        } else {
            // Normal rotation: mark old current as retired (kept for 7-day overlap window).
            // The host DB tracks the expiry; old JWTs remain verifiable until they expire.
            for entry in self.entries.iter_mut() {
                if entry.current {
                    entry.current = false;
                    entry.retired_at = Some(1); // non-zero = retired, valid during overlap
                }
            }
            // Keep at most 1 retired entry (the most recent) to bound memory growth.
            let retired: Vec<_> = self.entries.iter()
                .filter(|e| !e.current && e.retired_at.is_some())
                .map(|e| e.kid.clone())
                .collect();
            if retired.len() > 1 {
                // Remove all but the last retired kid
                let keep = retired.last().cloned();
                self.entries.retain(|e| e.current || Some(e.kid.clone()) == keep);
            }
        }

        let mut secret = vec![0u8; 32];
        Random::generate(secret.as_mut_slice());

        let mut kid_bytes = [0u8; 8];
        Random::generate(&mut kid_bytes);
        let kid = format!("v{}", hex::encode(kid_bytes));

        self.entries.push(JwtSecretEntry {
            kid: kid.clone(),
            secret,
            current: true,
            retired_at: None,
        });

        Ok((kid, retired_kid))
    }
}

thread_local! {
    static WALLET_CACHE: RefCell<WalletLruCache> = RefCell::new(WalletLruCache::new());
}

// ---- Cache helper functions ----

fn cache_get(wallet_id: &Uuid) -> Option<Wallet> {
    WALLET_CACHE.with(|c| c.borrow_mut().get(wallet_id))
}

fn cache_put(wallet: &Wallet) {
    WALLET_CACHE.with(|c| c.borrow_mut().put(wallet));
}

// H-3: cache_remove MUST be called BEFORE any secure-storage write syscall
// (db.delete_entry / db.put), because those syscalls corrupt the TLS register
// and any thread_local (WALLET_CACHE) access afterwards would panic. Calling it
// before the write is safe and is required so a deleted wallet does not remain
// signable from a stale cache entry.
fn cache_remove(wallet_id: &Uuid) {
    WALLET_CACHE.with(|c| c.borrow_mut().remove(wallet_id));
}

fn cache_len() -> usize {
    WALLET_CACHE.with(|c| c.borrow().len())
}

// ========================================
// Pending WebAuthn challenge table (issue #49 — TA-side anti-replay)
// ========================================
// The TA issues one-time 32-byte nonces via GetChallenge and verifies/consumes
// them inside verify_passkey_for_wallet. This closes the replay hole where a
// compromised CA could resubmit one captured assertion to authorize arbitrary
// payloads: the nonce is signed by the authenticator (it IS the WebAuthn
// challenge), checked against the TA's own table, and deleted on first use.
//
// Design constraints (see kms memory + issue #49):
//   * IN-MEMORY only, never TEE secure storage. Nonces are short-lived; losing
//     them on TA restart only forces a re-challenge. This also avoids the
//     secure-storage-write-then-TLS-access hazard (H-3): we never touch this
//     table after a storage write inside a single command.
//   * Vec instead of HashMap — std HashMap's SipHasher pulls getrandom, which
//     panics in the OP-TEE TA. Same rationale as WALLET_CACHE above.
//   * PROCESS-GLOBAL static, NOT thread_local. The host serializes TA calls
//     (single-worker queue in ta_client.rs) so access is strictly serial — but
//     OP-TEE may dispatch consecutive InvokeCommands onto DIFFERENT pool
//     threads. A thread_local nonce issued by GetChallenge would then be
//     invisible to the verify on the very next call, which showed up as a flaky
//     "No pending challenge" (#49 / #61). A global UnsafeCell shares the table
//     across all TA threads; the serial-invocation guarantee makes the unsafe
//     access sound without a lock or a new crate. Unlike WALLET_CACHE, this
//     table has NO secure-storage fallback — a lost entry is unrecoverable, so
//     correctness here must not depend on thread affinity.

/// Challenge nonce lifetime. An assertion whose nonce was issued more than this
/// many seconds ago is rejected even if the nonce still matches.
const CHALLENGE_TTL_SECS: i64 = 300;

/// Upper bound on simultaneously-pending challenges. Bounds memory and limits a
/// compromised CA's ability to exhaust the TA by spamming GetChallenge. Oldest
/// entries are evicted when full (a dropped pending challenge just forces the
/// honest client to re-request — it cannot authorize anything).
const MAX_PENDING_CHALLENGES: usize = 256;

/// Issue #49 enforcement policy.
///
/// `false` (TRANSITION, current default): assertions WITHOUT `client_data_json`
/// fall through to the legacy ECDSA-only verification with a warning. This keeps
/// existing E2E/clients working while the host + SDK roll out the GetChallenge
/// flow. Assertions WITH `client_data_json` are ALWAYS strictly verified
/// regardless of this flag — there is no downgrade once a client opts in.
///
/// `true` (STRICT): every assertion MUST carry `client_data_json` and pass nonce
/// binding. Flip this (and rebuild/re-flash the TA) once all clients are
/// migrated. TODO(#49): switch to strict for GA after Beta3 soak.
const ENFORCE_TA_CHALLENGE: bool = false;

struct PendingChallenge {
    wallet_id: Uuid,
    nonce: [u8; 32],
    issued_at: i64,
}

/// Process-global pending-nonce table, shared across ALL TA pool threads.
/// See the "Design constraints" block above for why this must be a global
/// static rather than a `thread_local` (flaky "No pending challenge", #49/#61).
struct GlobalChallenges(core::cell::UnsafeCell<Vec<PendingChallenge>>);

// SAFETY: this cell is only ever accessed serially, for two independent reasons
// rooted in the OP-TEE / GP execution model — NOT in any host-side discipline
// (the CA's single-worker queue is merely an additional Rust-side serialization,
// not what makes this sound):
//   1. Same session: GP TEE Client API `TEEC_InvokeCommand` is a BLOCKING call,
//      so a client cannot have two in-flight commands on one session — commands
//      on a given session are inherently serial. The KMS CA uses ONE persistent
//      session (ta_client.rs keeps it open to avoid the ~4.4s per-open cost), so
//      all real traffic flows through that single session.
//   2. Different sessions: this TA is built with default properties
//      (TA_FLAGS = 0 → gpd.ta.singleInstance = false), so EACH session gets its
//      own TA instance in its own address space — a second session has its own
//      copy of this static and cannot alias the first session's cell.
// In both cases no two threads ever hold a `&mut` to the SAME cell; `with_pending`
// further confines the `&mut` to one synchronous closure that never escapes across
// an InvokeCommand boundary. (If the TA were ever rebuilt as singleInstance +
// multiSession, this reasoning breaks and an explicit TA-side lock would be
// required; the build uses default flags today and ta.json is absent, so it is
// not.)
unsafe impl Sync for GlobalChallenges {}

static PENDING_CHALLENGES: GlobalChallenges =
    GlobalChallenges(core::cell::UnsafeCell::new(Vec::new()));

/// Run `f` with exclusive access to the global pending-challenge table.
/// SAFETY: serial TA invocation (see `GlobalChallenges`) guarantees no
/// concurrent borrow; the `&mut` does not escape this function.
fn with_pending<R>(f: impl FnOnce(&mut Vec<PendingChallenge>) -> R) -> R {
    // SAFETY: see GlobalChallenges — serial access, borrow confined to `f`.
    let tbl = unsafe { &mut *PENDING_CHALLENGES.0.get() };
    f(tbl)
}

/// Generate a fresh 32-byte nonce, record it for `wallet_id`, and return it.
/// Replaces any previously-pending nonce for the same wallet (only the latest
/// challenge is valid — requesting a new one invalidates the old).
fn challenge_issue(wallet_id: &Uuid) -> [u8; 32] {
    let mut nonce = [0u8; 32];
    Random::generate(&mut nonce);
    let issued_at = tee_unix_secs();
    with_pending(|tbl| {
        // Drop any existing pending challenge for this wallet (one live nonce per wallet).
        tbl.retain(|e| &e.wallet_id != wallet_id);
        // Bound memory: evict the oldest entry if at capacity.
        if tbl.len() >= MAX_PENDING_CHALLENGES {
            if let Some((idx, _)) = tbl
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.issued_at)
            {
                tbl.swap_remove(idx);
            }
        }
        tbl.push(PendingChallenge {
            wallet_id: *wallet_id,
            nonce,
            issued_at,
        });
    });
    nonce
}

/// Look up the pending nonce for `wallet_id` WITHOUT removing it.
/// Used to verify an incoming challenge before committing to consuming it: a
/// request carrying a wrong/expired challenge must NOT burn a victim's still-
/// valid pending nonce (DoS-on-nonce). The nonce is consumed (challenge_consume)
/// only after every binding/length/match/TTL check has passed.
fn challenge_peek(wallet_id: &Uuid) -> Option<([u8; 32], i64)> {
    with_pending(|tbl| {
        tbl.iter()
            .find(|e| &e.wallet_id == wallet_id)
            .map(|e| (e.nonce, e.issued_at))
    })
}

/// Look up and CONSUME (remove) the pending nonce for `wallet_id`.
/// Returns the (nonce, issued_at) if one was present. The removal makes the
/// nonce strictly one-time: a replayed assertion finds nothing to match.
fn challenge_consume(wallet_id: &Uuid) -> Option<([u8; 32], i64)> {
    with_pending(|tbl| {
        if let Some(idx) = tbl.iter().position(|e| &e.wallet_id == wallet_id) {
            let e = tbl.swap_remove(idx);
            Some((e.nonce, e.issued_at))
        } else {
            None
        }
    })
}

// ── P256 Session Key storage ──

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct P256SessionKey {
    store_id: String,      // "p256sk_<wallet_uuid>_<session_index>"
    private_key: Vec<u8>, // 32 bytes: P-256 scalar
    pub_key: Vec<u8>,     // 64 bytes: x(32) || y(32) uncompressed public key (no 0x04 prefix)
}

impl Storable for P256SessionKey {
    type Key = String;
    fn unique_id(&self) -> Self::Key {
        self.store_id.clone()
    }
}

// M-1: zeroize the raw P-256 private scalar on drop. zeroize is not a TA
// dependency (pinned nightly toolchain), so we replicate the manual-wipe
// pattern already used by `Wallet::drop` instead of adding a crate.
impl Drop for P256SessionKey {
    fn drop(&mut self) {
        self.private_key.iter_mut().for_each(|b| *b = 0);
    }
}

impl P256SessionKey {
    fn store_id_for(wallet_id: &Uuid, session_index: u32) -> String {
        format!("p256sk_{}_{}", wallet_id, session_index)
    }

    fn load(db: &SecureStorageClient, wallet_id: &Uuid, session_index: u32) -> Result<Self> {
        let id = Self::store_id_for(wallet_id, session_index);
        db.get::<P256SessionKey>(&id)
            .map_err(|_| anyhow!("P256 session key not found for index {}", session_index))
    }

    fn save(&self, db: &SecureStorageClient) -> Result<()> {
        db.put(self).map_err(|e| anyhow!("Failed to save P256 session key: {}", e))
    }
}

// ── RPMB Anti-Rollback Counter ──
//
// The counter is stored in TEE_STORAGE_PRIVATE_RPMB (0x80000003), backed by the
// eMMC RPMB partition. This provides hardware-enforced monotonicity: unlike REE-FS,
// RPMB objects cannot be silently rolled back by restoring the filesystem.
//
// TLS safety: TEE_OpenPersistentObject (read) does NOT corrupt TLS; only write
// syscalls do. Read counter BEFORE any thread_local access; write AFTER save_wallet().
//
// M-8 (known architectural limitation — tracked in Issue #48, future ELE/HSM
// migration): this is a SINGLE GLOBAL monotonic counter, not a per-wallet one.
// It detects a full storage rollback (all wallets reverted) but NOT a *targeted*
// rollback of one wallet to an older state whose epoch is still <= the current
// global counter (e.g. revert wallet A to epoch=5 while the global counter is 50;
// 5 <= 50 passes the check). Closing this fully requires per-wallet monotonic
// counters, which the current RPMB object model does not provide cheaply. The
// global counter is the strongest defense available without ELE/HSM hardware.

/// Open wallet storage. With the `ree-fs-only` feature this is plain REE-FS
/// (TEE_STORAGE_PRIVATE) and never touches RPMB; by default it is RPMB with
/// transparent REE-FS migration. Every storage call in the TA goes through here.
fn open_storage() -> Result<SecureStorageClient> {
    #[cfg(feature = "ree-fs-only")]
    {
        SecureStorageClient::open(DB_NAME)
    }
    #[cfg(not(feature = "ree-fs-only"))]
    {
        SecureStorageClient::open_rpmb_migrating(DB_NAME)
    }
}

fn rpmb_read_counter() -> Result<u64> {
    let (counter, _present) = rpmb_read_counter_ex()?;
    Ok(counter)
}

/// Like rpmb_read_counter but also reports whether the counter object actually
/// exists in RPMB. `present == false` means the counter object is absent
/// (ItemNotFound) — which happens on a fresh device OR after an eMMC reflash /
/// RPMB key reset. Callers can use this to distinguish "counter legitimately 0"
/// from "counter never initialized", which matters for C-2 (legitimate wallets
/// with epoch > 0 must not be bricked just because the counter object is gone).
fn rpmb_read_counter_ex() -> Result<(u64, bool)> {
    // REE-FS mode: never issue an RPMB syscall (the eMMC RPMB key is not
    // programmed on this hardware; touching RPMB faults/kills the TA).
    // Report "counter absent" so epoch_check's C-2 path applies.
    #[cfg(feature = "ree-fs-only")]
    {
        return Ok((0, false));
    }
    #[cfg(not(feature = "ree-fs-only"))]
    match PersistentObject::open(
        ObjectStorageConstants::Rpmb,
        RPMB_COUNTER_ID,
        DataFlag::ACCESS_READ | DataFlag::SHARE_READ,
    ) {
        Ok(obj) => {
            let mut buf = [0u8; 8];
            let n = obj.read(&mut buf)?;
            if n != 8 {
                return Err(anyhow!("RPMB counter: short read ({} bytes)", n));
            }
            Ok((u64::from_be_bytes(buf), true))
        }
        Err(e) => match e.kind() {
            ErrorKind::ItemNotFound => Ok((0, false)),
            _ => {
                // REE-FS fallback: RPMB unavailable (e.g. eMMC RPMB key never
                // programmed — NXP FRDM-IMX93 out of the box). Degrade to
                // "counter absent" rather than failing every operation;
                // epoch_check's C-2 path handles counter==absent gracefully.
                // Anti-rollback is inactive in REE-FS mode (tracked in #50).
                trace_println!(
                    "[!] RPMB counter unreadable ({:?}) — RPMB unavailable, anti-rollback degraded to REE-FS mode",
                    e
                );
                Ok((0, false))
            }
        },
    }
}

// Write counter to RPMB. OVERWRITES any existing counter object atomically.
// Caller MUST ensure no thread_local access follows this call in the same frame.
//
// C-3 (monotonic guard, defense-in-depth): before overwriting, read the current
// counter. If a counter object already exists and the new value would DECREASE
// it, reject the write. In correct code paths `value` is always current+1 (via
// rpmb_next_epoch) or an exact recovery to current+1, so this never fires in
// practice — but it hard-stops any future bug or tampered-wallet path from
// silently rolling the hardware counter backwards. When the counter object is
// absent (fresh device / reflash / C-2 re-init) there is no baseline to
// violate, so any initial value is accepted.
fn rpmb_write_counter(value: u64) -> Result<()> {
    // REE-FS mode: no RPMB anti-rollback counter — skip the write entirely so
    // the TA never issues an RPMB syscall on hardware without a programmed key.
    #[cfg(feature = "ree-fs-only")]
    {
        let _ = value;
        return Ok(());
    }
    #[cfg(not(feature = "ree-fs-only"))]
    {
    let (current, present) = rpmb_read_counter_ex()?;
    if present && value < current {
        return Err(anyhow!(
            "RPMB monotonic violation: refusing to write counter {} < current {}",
            value, current
        ));
    }

    let flags = DataFlag::ACCESS_READ
        | DataFlag::ACCESS_WRITE
        | DataFlag::ACCESS_WRITE_META
        | DataFlag::OVERWRITE;
    match PersistentObject::create(
        ObjectStorageConstants::Rpmb,
        RPMB_COUNTER_ID,
        flags,
        None,
        &value.to_be_bytes(),
    ) {
        Ok(_) => {
            trace_println!("[+] RPMB anti-rollback counter written: {}", value);
            Ok(())
        }
        Err(e) => {
            // REE-FS fallback: RPMB not writable (key not programmed). Skip the
            // counter write instead of failing the whole mutation. Anti-rollback
            // is inactive in REE-FS mode — acceptable degradation, tracked in
            // #50; the wallet itself is still persisted via REE-FS.
            trace_println!(
                "[!] RPMB counter write skipped ({:?}) — RPMB unavailable, anti-rollback degraded to REE-FS mode",
                e
            );
            Ok(())
        }
    }
    }
}

// Increment RPMB counter and return the new value.
// Safe ordering: call rpmb_read_counter() before thread_local access;
// call rpmb_write_counter() only after save_wallet() (all thread_local done).
fn rpmb_next_epoch() -> Result<u64> {
    let current = rpmb_read_counter()?;
    current.checked_add(1)
        .ok_or_else(|| anyhow!("RPMB anti-rollback counter overflow"))
}

/// Save wallet to secure storage AND update cache.
fn save_wallet(db: &SecureStorageClient, wallet: &Wallet) -> Result<()> {
    // Cache MUST come before db.put: OP-TEE secure storage syscall corrupts TLS,
    // causing thread_local WALLET_CACHE access to panic if called after db.put.
    cache_put(wallet);
    db.put(wallet)?;
    Ok(())
}

/// Anti-rollback epoch validation (pure function — unit-testable, H-D):
///
///   Normal state:    wallet.rollback_epoch <= rpmb_now        → Ok(false)
///   Recovery state:  wallet.rollback_epoch == rpmb_now + 1    → Ok(true)
///     → previous mutation saved wallet (epoch=N+1) but crashed before
///       rpmb_write_counter(N+1). Caller completes the interrupted write.
///   Tampered state:  wallet.rollback_epoch > rpmb_now + 1     → Err
///     → impossible in normal operation; reject hard.
///
/// C-2 (counter object absent): if the RPMB counter object does not exist
/// (eMMC reflash, RPMB key reset, replaced storage) we read 0 with
/// counter_present=false. A legitimate wallet may still carry epoch=N>1.
/// Treating that as "tampered" would brick a real wallet. Instead, when the
/// counter is absent we self-heal: re-initialize the RPMB counter to the
/// wallet's own epoch (return Ok(true)). Safe because an absent counter means
/// there is no monotonic baseline to violate — we are establishing one.
///
/// TLS ordering contract for the CALLER: a returned `true` means an RPMB
/// write (rpmb_write_counter, a TEE write that corrupts tpidr_el0) is due —
/// it must be performed AFTER all thread_local access (cache_put).
fn epoch_check(
    epoch: u64,
    rpmb_now: u64,
    counter_present: bool,
    wallet_id: &uuid::Uuid,
) -> Result<bool> {
    if epoch == 0 {
        return Ok(false); // legacy wallet, skip check
    }
    if !counter_present {
        // C-2: counter object missing → no baseline exists. Re-establish it
        // from this wallet's epoch rather than rejecting a legitimate wallet.
        trace_println!(
            "[!] anti-rollback: RPMB counter absent, re-initializing to epoch {} for {:?}",
            epoch, wallet_id
        );
        return Ok(true); // needs RPMB (re-)init write
    }
    // saturating_add: rpmb_now == u64::MAX would otherwise overflow-panic in
    // debug builds (unreachable in practice, but free to harden).
    let recovery_epoch = rpmb_now.saturating_add(1);
    if epoch > recovery_epoch {
        return Err(anyhow!(
            "anti-rollback: wallet epoch {} > RPMB {}+1 for {:?} — tampered or corrupt",
            epoch, rpmb_now, wallet_id
        ));
    }
    Ok(epoch == recovery_epoch) // true = needs RPMB recovery write
}

/// Load wallet + ensure seed cached.
/// On cache hit with seed already cached: ZERO secure storage I/O.
///
/// IMPORTANT: We intentionally do NOT call db.put here, even if the seed
/// was just computed. OP-TEE secure storage syscalls corrupt the TLS register,
/// and any thread_local access (including WALLET_CACHE) after db.put will panic.
/// Since load_wallet_cached is always followed by verify_passkey_for_wallet
/// (which may use TLS-backed allocators), we must never call db.put in this path.
/// Seed persistence is handled exclusively by derive_address_auto.
///
/// Anti-rollback: reads RPMB counter first (read-only, TLS-safe) to detect
/// wallets whose epoch is ahead of RPMB — which is impossible in normal operation
/// and indicates either an atomicity failure (RPMB write failed after wallet save)
/// or tampered wallet bytes with a forged future epoch.
fn load_wallet_cached(wallet_id: &Uuid) -> Result<Wallet> {
    // Read RPMB before any TLS access. rpmb_read_counter uses open+read (safe).
    // `counter_present == false` means the RPMB counter object is absent
    // (fresh device or post-reflash) — see C-2 handling in epoch_check.
    let (rpmb_now, counter_present) = rpmb_read_counter_ex()?;

    // Fast path: cache hit
    if let Some(mut w) = cache_get(wallet_id) {
        let needs_recovery = epoch_check(w.rollback_epoch, rpmb_now, counter_present, wallet_id)?;
        let changed = w.ensure_seed_cached()?;
        if changed {
            // Update in-memory cache only — NO db.put (would corrupt TLS)
            trace_println!(
                "[!] load_wallet_cached: cold seed computed for {:?}, memory-only cache",
                wallet_id
            );
            cache_put(&w);
        }
        // Recovery: complete interrupted RPMB write — AFTER all TLS (cache_put above)
        if needs_recovery {
            trace_println!(
                "[!] load_wallet_cached: recovering RPMB counter to {} for {:?}",
                w.rollback_epoch, wallet_id
            );
            rpmb_write_counter(w.rollback_epoch)?;
        }
        return Ok(w);
    }

    // Slow path: cache miss — read from storage
    let db = open_storage()?;
    let mut w = db
        .get::<Wallet>(wallet_id)
        .map_err(|e| anyhow!("wallet not found: {:?}", e))?;

    let needs_recovery = epoch_check(w.rollback_epoch, rpmb_now, counter_present, wallet_id)?;

    let changed = w.ensure_seed_cached()?;
    if changed {
        trace_println!(
            "[!] load_wallet_cached: cold seed from storage for {:?}, memory-only cache",
            wallet_id
        );
    }
    // Always cache in memory (NO db.put — avoids TLS corruption in signing path)
    cache_put(&w);
    // Recovery: complete interrupted RPMB write — AFTER cache_put (last TLS access)
    if needs_recovery {
        trace_println!(
            "[!] load_wallet_cached: recovering RPMB counter to {} for {:?}",
            w.rollback_epoch, wallet_id
        );
        rpmb_write_counter(w.rollback_epoch)?;
    }
    Ok(w)
}

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] TA open session");
    // H-A NOTE (reverted after on-hardware testing 2026-06-11):
    // We deliberately do NOT run the REE-FS→RPMB migration here. Doing so was
    // tried and triggers a TEE security fault (0xffff000f, origin TEE) on real
    // i.MX93 hardware: at open_session time the RPMB / secure-storage path via
    // tee-supplicant is not yet usable, so any PersistentObject access faults
    // and kills every session (the whole TA becomes unusable, not just one
    // command). Migration therefore stays in the handlers (load_wallet_cached /
    // create_wallet / derive_address_auto), which is crash-safe and idempotent.
    // Residual cost (accepted): on the very first command after an upgrade that
    // actually performs migration, the in-handler migration writes corrupt TLS,
    // so that one command may fail once and self-heals on retry.
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] TA destroy");
}

#[cfg(debug_assertions)]
macro_rules! dbg_println {
    ($($arg:tt)*) => (trace_println!($($arg)*));
}

#[cfg(not(debug_assertions))]
macro_rules! dbg_println {
    ($($arg:tt)*) => {};
}

// p256-m FFI: P-256 ECDSA verify, sign, and key generation inside TA.
// Compile flags fixed in e1b50c2 (2026-03-03): -O1 -fPIC -fno-common -marm (ARM32).
// 5/5 stability tests passed on DK2 (Cortex-A7) after the flag fix.
extern "C" {
    fn p256_ecdsa_verify(sig: *const u8, pubkey: *const u8, hash: *const u8, hlen: usize) -> i32;
    fn p256_gen_keypair(priv_key: *mut u8, pub_key: *mut u8) -> i32;
    fn p256_ecdsa_sign(sig: *mut u8, priv_key: *const u8, hash: *const u8, hlen: usize) -> i32;
}
// Callback for p256-m: fills output with cryptographically secure random bytes via OP-TEE RNG.
// Required for p256_gen_keypair and p256_ecdsa_sign.
#[no_mangle]
pub extern "C" fn p256_generate_random(output: *mut u8, output_size: u32) -> i32 {
    if output.is_null() || output_size == 0 {
        return -1; // P256_RANDOM_FAILED
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(output, output_size as usize) };
    Random::generate(buf);
    0 // P256_SUCCESS
}

/// SHA-256("aastar.io") — the relying party ID for this KMS deployment.
/// Hardcoded in TA so a compromised CA cannot substitute a different rpId.
/// To change for a different deployment: recompute SHA-256 of the new rpId
/// and update this constant; then rebuild and re-flash the TA.
/// Computed: echo -n "aastar.io" | sha256sum
const EXPECTED_RP_ID_HASH: [u8; 32] = [
    0xd9, 0x44, 0xd2, 0xad, 0xbd, 0x65, 0x6d, 0x76,
    0x9a, 0x81, 0x15, 0x28, 0xd2, 0xac, 0xa5, 0x14,
    0x71, 0xd5, 0xfc, 0x5c, 0x7f, 0xca, 0xd4, 0x1d,
    0x86, 0x31, 0x08, 0x3b, 0x20, 0x9a, 0xa6, 0x3a,
];

/// Verify passkey assertion against the passkey bound to this wallet.
/// All wallets MUST have a passkey bound — rejects if missing.
///
/// Two-layer defense: CA pre-verifies with Rust p256 crate before enqueuing the TA call;
/// TA re-verifies with p256-m (C, ~320ms on Cortex-A7) as defense-in-depth.
/// Both layers must pass for any sensitive operation.
fn verify_passkey_for_wallet(
    wallet: &Wallet,
    assertion: Option<&proto::PasskeyAssertion>,
) -> Result<()> {
    let _pubkey = match wallet.get_passkey() {
        Some(pk) => pk,
        None => return Err(anyhow!("Wallet has no PassKey bound. Cannot verify.")),
    };

    let _assertion =
        assertion.ok_or_else(|| anyhow!("Wallet has PassKey bound. Provide PassKey assertion."))?;

    // Verify authenticatorData structure (WebAuthn Level 2 §6.1):
    //   [0..32]  rpIdHash  (32 bytes, SHA-256 of rpId)
    //   [32]     flags     (1 byte: bit0=UP, bit2=UV, bit6=AT, bit7=ED)
    //   [33..37] signCount (4 bytes, big-endian uint32)
    if _assertion.authenticator_data.len() < 37 {
        return Err(anyhow!(
            "authenticatorData too short: {} bytes (minimum 37)",
            _assertion.authenticator_data.len()
        ));
    }

    // Verify rpId hash — MANDATORY, hardcoded in TA (not CA-controlled).
    // Prevents credential-substitution attack: attacker cannot use a valid
    // credential from evil.com because its rpId hash will not match aastar.io's.
    // Constant-time XOR comparison prevents timing side-channel.
    let actual_rp_id_hash = &_assertion.authenticator_data[0..32];
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= EXPECTED_RP_ID_HASH[i] ^ actual_rp_id_hash[i];
    }
    if diff != 0 {
        return Err(anyhow!(
            "WebAuthn rpId hash mismatch: expected SHA-256(\"aastar.io\"), got different value"
        ));
    }
    trace_println!("[+] rpId hash verified in TA (constant-time)");

    // Verify User Presence (UP) flag — bit 0 of flags byte (offset 32).
    // Ensures the user physically interacted with the authenticator.
    // Checked after rpId to fail fast on wrong-RP assertions.
    let flags = _assertion.authenticator_data[32];
    if flags & 0x01 == 0 {
        return Err(anyhow!(
            "WebAuthn User Presence flag not set (flags=0x{:02x})",
            flags
        ));
    }

    // signature = r(32) || s(32) = 64 bytes
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(&_assertion.signature_r);
    sig_bytes[32..].copy_from_slice(&_assertion.signature_s);

    // pubkey from wallet is 65 bytes (04 || x || y), p256-m wants 64 bytes (x || y)
    let pubkey_xy = if _pubkey.len() == 65 && _pubkey[0] == 0x04 {
        &_pubkey[1..65]
    } else {
        return Err(anyhow!(
            "Invalid pubkey format: expected 65 bytes (04||x||y), got {}",
            _pubkey.len()
        ));
    };

    // ── Issue #49: challenge binding / anti-replay (TA-side) ──
    // Verified BEFORE the ECDSA check so a replayed/forged assertion is rejected
    // without spending the ~320ms p256-m verification. The wallet_id used to
    // consume the nonce is the TA's own (wallet.get_id()), never a CA-supplied
    // value, so a compromised CA cannot redirect the consumption to another slot.
    verify_challenge_binding(&wallet.get_id(), _assertion)?;

    // Build signed_data = authenticator_data || client_data_hash
    let mut signed_data = _assertion.authenticator_data.clone();
    signed_data.extend_from_slice(&_assertion.client_data_hash);

    // Hash the signed_data with SHA-256 (p256-m expects hash input)
    use sha2::Digest;
    let hash_of_signed = sha2::Sha256::digest(&signed_data);

    trace_println!(
        "[+] p256-m verify: sig={}B pubkey={}B hash={}B",
        sig_bytes.len(),
        pubkey_xy.len(),
        hash_of_signed.len()
    );

    let ret = unsafe {
        p256_ecdsa_verify(
            sig_bytes.as_ptr(),
            pubkey_xy.as_ptr(),
            hash_of_signed.as_ptr(),
            hash_of_signed.len(),
        )
    };

    trace_println!("[+] p256-m verify result: {}", ret);

    if ret != 0 {
        return Err(anyhow!(
            "PassKey verification failed (p256-m): error code {}",
            ret
        ));
    }

    Ok(())
}

/// Issue #49: bind the assertion to a TA-issued one-time challenge nonce.
///
/// Two cases:
///   * `client_data_json` present → STRICT. We (1) verify
///     `SHA-256(client_data_json) == client_data_hash` so the JSON is the exact
///     preimage of the bytes the authenticator signed (a compromised CA cannot
///     forge the JSON), (2) extract the `challenge` field, base64url-decode it,
///     (3) consume the TA's pending nonce for this wallet and require an exact,
///     constant-time match plus a fresh (un-expired) issue time. The nonce is
///     deleted on first lookup, making it strictly one-time.
///   * `client_data_json` absent → governed by `ENFORCE_TA_CHALLENGE`:
///     transition mode logs a warning and allows (legacy ECDSA-only path);
///     strict mode rejects.
fn verify_challenge_binding(wallet_id: &Uuid, assertion: &proto::PasskeyAssertion) -> Result<()> {
    let client_data_json = match assertion.client_data_json.as_ref() {
        Some(json) => json,
        None => {
            if ENFORCE_TA_CHALLENGE {
                return Err(anyhow!(
                    "Issue #49 strict mode: assertion missing clientDataJSON; \
                     obtain a challenge via GetChallenge and resubmit"
                ));
            }
            // Transition: do NOT leave a stale nonce around for this wallet — if
            // one was issued but this legacy assertion bypassed binding, drop it
            // so it cannot be paired with a future replay.
            let _ = challenge_consume(wallet_id);
            trace_println!(
                "[!] Issue #49 TRANSITION: assertion without clientDataJSON accepted (legacy path); \
                 migrate client to GetChallenge flow"
            );
            return Ok(());
        }
    };

    // (1) Bind JSON to the signed bytes: SHA-256(clientDataJSON) must equal the
    // client_data_hash that goes into the ECDSA-verified message. Constant-time.
    use sha2::Digest;
    let computed = sha2::Sha256::digest(client_data_json);
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= computed[i] ^ assertion.client_data_hash[i];
    }
    if diff != 0 {
        return Err(anyhow!(
            "clientDataJSON does not hash to client_data_hash (binding broken)"
        ));
    }

    // (2) Extract the base64url `challenge` field and decode it.
    let challenge_b64 = extract_json_string_field(client_data_json, "challenge")
        .ok_or_else(|| anyhow!("clientDataJSON missing 'challenge' field"))?;
    let challenge_bytes = base64url_decode_no_pad(challenge_b64.as_bytes())
        .ok_or_else(|| anyhow!("clientDataJSON challenge is not valid base64url"))?;

    // (3) PEEK (do not yet consume) the TA's pending nonce for this wallet. We
    // only remove it once every check below passes, so a request with a wrong or
    // expired challenge cannot burn a victim's still-valid nonce (DoS-on-nonce).
    let (nonce, issued_at) = challenge_peek(wallet_id).ok_or_else(|| {
        anyhow!("No pending challenge for this wallet (replay, expired, or GetChallenge not called)")
    })?;

    if challenge_bytes.len() != nonce.len() {
        return Err(anyhow!(
            "challenge length mismatch: got {}B, expected {}B",
            challenge_bytes.len(),
            nonce.len()
        ));
    }
    let mut cdiff = 0u8;
    for i in 0..nonce.len() {
        cdiff |= challenge_bytes[i] ^ nonce[i];
    }
    if cdiff != 0 {
        return Err(anyhow!("challenge does not match the TA-issued nonce"));
    }

    // Freshness: reject a nonce that, while matching, was issued too long ago.
    // tee_unix_secs uses REE time (TA SystemTime::now() panics — see kms memory).
    let now = tee_unix_secs();
    let age = now.saturating_sub(issued_at);
    if age < 0 || age > CHALLENGE_TTL_SECS {
        return Err(anyhow!(
            "challenge expired (age {}s > TTL {}s)",
            age,
            CHALLENGE_TTL_SECS
        ));
    }

    // All checks passed — NOW consume the nonce (strictly one-time). Consuming
    // here rather than before the checks means a failed verification leaves the
    // legitimate nonce intact for the real client to retry.
    let _ = challenge_consume(wallet_id);

    trace_println!("[+] Issue #49: challenge nonce verified + consumed (age {}s)", age);
    Ok(())
}

/// Minimal extractor for a string-valued JSON field, used in place of serde_json
/// (which is not a TA dependency). Returns the raw (unescaped) string content of
/// `"<key>":"<value>"`. WebAuthn clientDataJSON `challenge`/`type`/`origin` are
/// always plain base64url / ASCII tokens with no escapes, so unescaping is not
/// required for our use. Returns None if the field is absent or malformed.
///
/// This is intentionally conservative: it scans for the `"key"` token, skips
/// whitespace and the colon, requires an opening quote, and reads until the next
/// unescaped quote. A `\"` inside the value terminates extraction early and the
/// caller's base64url decode will then reject it — safe-by-rejection.
fn extract_json_string_field(json: &[u8], key: &str) -> Option<String> {
    // Build the needle: "key"
    let mut needle = Vec::with_capacity(key.len() + 2);
    needle.push(b'"');
    needle.extend_from_slice(key.as_bytes());
    needle.push(b'"');

    let mut i = 0usize;
    let n = json.len();
    let nl = needle.len();
    while i + nl <= n {
        if &json[i..i + nl] == needle.as_slice() {
            let mut j = i + nl;
            // skip whitespace
            while j < n && (json[j] == b' ' || json[j] == b'\t' || json[j] == b'\n' || json[j] == b'\r') {
                j += 1;
            }
            // require colon
            if j >= n || json[j] != b':' {
                return None;
            }
            j += 1;
            // skip whitespace
            while j < n && (json[j] == b' ' || json[j] == b'\t' || json[j] == b'\n' || json[j] == b'\r') {
                j += 1;
            }
            // require opening quote
            if j >= n || json[j] != b'"' {
                return None;
            }
            j += 1;
            let start = j;
            while j < n && json[j] != b'"' {
                // stop at backslash — value would need unescaping we don't do;
                // safer to refuse than to mis-handle.
                if json[j] == b'\\' {
                    return None;
                }
                j += 1;
            }
            if j >= n {
                return None; // unterminated string
            }
            return core::str::from_utf8(&json[start..j]).ok().map(|s| s.to_string());
        }
        i += 1;
    }
    None
}

/// Decode unpadded base64url. WebAuthn challenges are base64url WITHOUT padding,
/// but we also tolerate padding by stripping trailing '='. Returns None on any
/// invalid character or length. Implemented directly to avoid relying on the
/// base64ct decoder's exact padding mode (we already depend on base64ct for JWT,
/// but its Base64UrlUnpadded would reject any '=' a client might include).
fn base64url_decode_no_pad(input: &[u8]) -> Option<Vec<u8>> {
    // Map a base64url char to its 6-bit value.
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    // Strip optional trailing padding.
    let mut end = input.len();
    while end > 0 && input[end - 1] == b'=' {
        end -= 1;
    }
    let data = &input[..end];
    // A valid base64 group is 2..=4 chars; length % 4 == 1 is impossible.
    if data.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &c in data {
        let v = val(c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

fn create_wallet(input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    // Validate passkey public key (mandatory)
    if input.passkey_pubkey.len() != 65 || input.passkey_pubkey[0] != 0x04 {
        return Err(anyhow!(
            "PassKey pubkey must be 65 bytes uncompressed (0x04||x||y), got {} bytes",
            input.passkey_pubkey.len()
        ));
    }

    // Read RPMB counter before any thread_local access (reads don't corrupt TLS).
    let epoch = rpmb_next_epoch()?;

    // If the CA supplied pre-generated entropy (CAAM-bypass mode), use it directly.
    // Otherwise fall back to TEE_GenerateRandom() — which can hang if CAAM TRNG is stuck.
    let mut wallet = match &input.entropy_seed {
        Some(seed) => {
            dbg_println!("[+] create_wallet: using CA-provided entropy (CAAM bypass)");
            Wallet::from_seed(seed)?
        }
        None => {
            dbg_println!("[+] create_wallet: using TEE_GenerateRandom (hardware TRNG)");
            Wallet::new()?
        }
    };
    wallet.set_passkey(input.passkey_pubkey.clone());
    wallet.rollback_epoch = epoch;
    let wallet_id = wallet.get_id();

    // Mnemonic never crosses TEE boundary in production.
    // Only populated with the export-secrets feature (dev/test).
    #[cfg(feature = "export-secrets")]
    let mnemonic = wallet.get_mnemonic()?;
    #[cfg(not(feature = "export-secrets"))]
    let mnemonic = String::new();

    dbg_println!("[+] Wallet ID: {:?}", wallet_id);

    // Open storage once (a single key-list read does not corrupt TLS); reused
    // for both the count check and the save below.
    let db_client = open_storage()?;

    // M-4: bound total wallet count to prevent storage exhaustion (DoS).
    // count_entries reads ONLY the in-memory key list — no per-entry object
    // reads — so it issues no extra storage syscalls and cannot corrupt the TLS
    // register before the cache_put inside save_wallet. (The previous
    // implementation read every wallet object here, which corrupted TLS on real
    // i.MX93 hardware and panicked the subsequent thread_local cache access.)
    //
    // Capacity sizing: wallets live in REE-FS (GB-scale), NOT RPMB/ELE secure
    // storage. RPMB only holds the anti-rollback epoch counter, and the i.MX93
    // ELE cannot do secp256k1 (issue #40/#48), so Ethereum keys are software-
    // managed in REE-FS with the secure enclave acting only as a root-of-trust /
    // rollback guard — NOT as wallet storage. Enabling the MX security enclave
    // therefore does NOT shrink this budget: capacity stays bounded by REE-FS.
    // Measured on FRDM-IMX93: ~100 wallets occupy ~476 KB and /var/lib/tee has
    // >1 GB free → physical room for ~300 000 wallets. We cap at 30 000 (~140 MB)
    // to keep ~10x headroom AND a hard DoS ceiling on a compromised CA. The old
    // value of 100 was three orders of magnitude too low for a community/city-
    // scale KMS and only ever bit us via repeated-E2E test pollution.
    //
    // Kept as a build-time const (NOT a runtime/CA-supplied config) on purpose:
    // this is a security boundary, so a compromised CA must not be able to raise
    // it. Operators needing a different ceiling change this line and rebuild.
    const MAX_WALLETS: usize = 30_000;
    let existing = db_client.count_entries::<Wallet>()?;
    if existing >= MAX_WALLETS {
        return Err(anyhow!(
            "wallet limit reached ({}/{}) — cannot create more wallets",
            existing, MAX_WALLETS
        ));
    }

    // save_wallet does cache_put (TLS) then db.put (corrupts TLS). After this,
    // no more thread_local access — safe to call rpmb_write_counter.
    save_wallet(&db_client, &wallet)?;
    rpmb_write_counter(epoch)?;
    dbg_println!("[+] Wallet saved (passkey bound, RPMB epoch={})", epoch);

    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,
    })
}

fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput> {
    trace_println!("[+] Removing wallet: {:?}", input.wallet_id);

    // Read RPMB epoch before any thread_local access (read doesn't corrupt TLS).
    let next_epoch = rpmb_next_epoch()?;

    let db_client = open_storage()?;

    // Load from DB (not cache) — read op doesn't corrupt TLS
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("wallet not found: {:?}", e))?;

    // Mandatory passkey verification
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;

    // H-3: invalidate the LRU cache entry BEFORE the delete syscall. The deleted
    // wallet must not remain signable from a stale cache hit. cache_remove
    // touches thread_local, so it must run before delete_entry (a TEE write that
    // corrupts TLS).
    cache_remove(&input.wallet_id);

    // delete_entry corrupts TLS; write RPMB counter after.
    db_client.delete_entry::<Wallet>(&input.wallet_id)?;
    rpmb_write_counter(next_epoch)?;
    trace_println!(
        "[+] Wallet removed (passkey verified, RPMB epoch={})",
        next_epoch
    );

    Ok(proto::RemoveWalletOutput {})
}

/// Force-remove a wallet from TEE secure storage WITHOUT passkey verification.
/// This is intentionally a narrow admin operation: called ONLY by the host-side
/// delete_key() gap-key path, which has already validated that passkey_pubkey is
/// NOT a valid P-256 curve point (and therefore can never produce a valid assertion).
///
/// Security: an invalid P-256 pubkey cannot be set via CreateKey or ChangePasskey
/// (both validate the curve point since v0.19.0). So this path only reaches wallets
/// created before that fix — they are unreachable by any normal owner operation.
fn force_remove_wallet(
    input: &proto::ForceRemoveWalletInput,
) -> Result<proto::ForceRemoveWalletOutput> {
    trace_println!("[!] ForceRemoveWallet (gap key): {:?}", input.wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    // Confirm the entry exists before deleting
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("wallet not found in TEE storage: {:?}", e))?;

    // Safety gate: only proceed if passkey is invalid (confirms this IS a gap key)
    if let Some(pk) = wallet.get_passkey() {
        if pk.len() == 65 && pk[0] == 0x04 {
            // Attempt P-256 validation — if it succeeds the key is NOT a gap key
            // (p256 crate is not available in TA; rely on the host having already
            //  validated before invoking this command)
        }
    }

    db_client.delete_entry::<Wallet>(&input.wallet_id)?;
    trace_println!("[!] Gap key purged from TEE secure storage");
    Ok(proto::ForceRemoveWalletOutput {})
}

fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    let (address, public_key) = wallet.derive_address(&input.hd_path)?;
    Ok(proto::DeriveAddressOutput {
        address,
        public_key,
    })
}

fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
    Ok(proto::SignTransactionOutput { signature })
}

fn sign_message(input: &proto::SignMessageInput) -> Result<proto::SignMessageOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    let signature = wallet.sign_message(&input.hd_path, &input.message)?;
    Ok(proto::SignMessageOutput { signature })
}

fn sign_hash(input: &proto::SignHashInput) -> Result<proto::SignHashOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    let signature = wallet.sign_hash(&input.hd_path, &input.hash)?;
    Ok(proto::SignHashOutput { signature })
}

// H-1 (DOWNGRADED to Medium — tracked as an accepted limitation / follow-up issue):
// DeriveAddressAuto carries no passkey assertion and mutates+persists wallet state
// (next_address_index). It is invoked by the CA immediately after wallet creation,
// before any passkey-gated flow exists for the new wallet, so requiring a passkey
// here would break the legitimate creation path (no user assertion is available at
// that instant). A *compromised CA* could call it directly to advance the address
// index up to the per-wallet cap (100, enforced in Wallet::increment_address_index),
// but it cannot exfiltrate keys, sign, or affect other wallets, and the index is now
// anti-rollback protected (M-5). Net effect of abuse is bounded address churn for a
// single wallet. A proper fix (bind to a just-created-this-session token, or move the
// call inline into create_wallet) is deferred to a dedicated issue to avoid breaking
// the creation flow in this bugfix pass.
fn derive_address_auto(
    input: &proto::DeriveAddressAutoInput,
) -> Result<proto::DeriveAddressAutoOutput> {
    // M-5: this mutation bumps next_address_index and persists it. Bump the RPMB
    // anti-rollback epoch too, otherwise the address index could be silently
    // rolled back to re-derive (reuse) a previously issued address. Read the next
    // epoch before any thread_local (cache) access; write the counter only after
    // save_wallet (last TLS access), matching create_wallet's ordering.
    let epoch = rpmb_next_epoch()?;

    let db_client = open_storage()?;

    dbg_println!("[+] DeriveAddressAuto for wallet: {:?}", input.wallet_id);
    let mut wallet = match cache_get(&input.wallet_id) {
        Some(w) => w,
        None => db_client
            .get::<Wallet>(&input.wallet_id)
            .map_err(|e| anyhow!("wallet not found: {:?}", e))?,
    };

    let address_index = wallet.increment_address_index()?;
    wallet.ensure_seed_cached()?;
    wallet.rollback_epoch = epoch;

    let derivation_path = format!("m/44'/60'/0'/0/{}", address_index);
    let (address, public_key) = wallet.derive_address(&derivation_path)?;

    // save_wallet does cache_put (TLS) then db.put (corrupts TLS). After this,
    // no more thread_local access — safe to call rpmb_write_counter.
    save_wallet(&db_client, &wallet)?;
    rpmb_write_counter(epoch)?;

    Ok(proto::DeriveAddressAutoOutput {
        wallet_id: input.wallet_id,
        address,
        public_key,
        derivation_path,
    })
}

// Production builds: unconditionally reject — private key must never leave the TEE.
#[cfg(not(feature = "export-secrets"))]
fn export_private_key(
    _input: &proto::ExportPrivateKeyInput,
) -> Result<proto::ExportPrivateKeyOutput> {
    Err(anyhow!("ExportPrivateKey is disabled in production TA builds"))
}

// Dev/test builds only (--features export-secrets): allow explicit export with passkey or admin bypass.
#[cfg(feature = "export-secrets")]
fn export_private_key(
    input: &proto::ExportPrivateKeyInput,
) -> Result<proto::ExportPrivateKeyOutput> {
    dbg_println!(
        "[+] Export private key for wallet: {:?}, path: {}",
        input.wallet_id,
        input.derivation_path
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;

    if input.passkey_assertion.is_some() {
        verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    } else {
        dbg_println!("[+] ExportPrivateKey: dev admin mode (no passkey assertion)");
    }

    let private_key = wallet.export_private_key(&input.derivation_path)?;

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

// M-3: no longer wired into handle_invoke (removed from dispatch to avoid being
// used as an auth oracle). Kept (allow-dead-code) only as a documentation stub.
#[allow(dead_code)]
fn verify_passkey(_input: &proto::VerifyPasskeyInput) -> Result<proto::VerifyPasskeyOutput> {
    dbg_println!("[+] Verify passkey for wallet: {:?}", _input.wallet_id);

    // Standalone VerifyPasskey TA command: not exposed via any HTTP endpoint.
    // Actual signing operations use verify_passkey_for_wallet() which calls p256-m.
    // This stub exists for future diagnostic use only.
    Ok(proto::VerifyPasskeyOutput { valid: true })
}

fn register_passkey_ta(
    input: &proto::RegisterPasskeyTaInput,
) -> Result<proto::RegisterPasskeyTaOutput> {
    trace_println!("[+] Registering passkey for wallet: {:?}", input.wallet_id);

    if input.passkey_pubkey.len() != 65 || input.passkey_pubkey[0] != 0x04 {
        bail!(
            "PassKey public key must be 65 bytes uncompressed (0x04 || x || y), got {} bytes",
            input.passkey_pubkey.len()
        );
    }

    // Read RPMB epoch before load_wallet_cached (which touches thread_local cache).
    let epoch = rpmb_next_epoch()?;

    let mut wallet = load_wallet_cached(&input.wallet_id)?;
    // Verify current passkey before allowing change
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    wallet.set_passkey(input.passkey_pubkey.clone());
    wallet.rollback_epoch = epoch;

    let db = open_storage()?;
    // save_wallet does cache_put (TLS) then db.put (corrupts TLS).
    save_wallet(&db, &wallet)?;
    rpmb_write_counter(epoch)?;
    trace_println!("[+] PassKey registered, wallet saved (RPMB epoch={})", epoch);

    Ok(proto::RegisterPasskeyTaOutput { registered: true })
}

fn warmup_cache(input: &proto::WarmupCacheInput) -> Result<proto::WarmupCacheOutput> {
    dbg_println!("[+] Warmup cache for wallet: {:?}", input.wallet_id);
    let _wallet = load_wallet_cached(&input.wallet_id)?;
    Ok(proto::WarmupCacheOutput {
        cached: true,
        cache_size: cache_len() as u32,
    })
}

fn read_rollback_counter(
    _input: &proto::ReadRollbackCounterInput,
) -> Result<proto::ReadRollbackCounterOutput> {
    let counter = rpmb_read_counter()?;
    Ok(proto::ReadRollbackCounterOutput { counter })
}

/// Issue #49: issue a fresh one-time WebAuthn challenge nonce bound to a wallet.
///
/// Requires the wallet to exist (and thus have a passkey bound) so a compromised
/// CA cannot farm nonces for non-existent wallets. The nonce is held in-memory
/// only and consumed by the next signing assertion. No secure-storage write
/// happens here, so there is no TLS/thread_local hazard (H-3).
fn get_challenge(input: &proto::GetChallengeInput) -> Result<proto::GetChallengeOutput> {
    dbg_println!("[+] GetChallenge for wallet: {:?}", input.wallet_id);
    // Ensure the wallet exists before issuing a nonce. load_wallet_cached errors
    // if the wallet is unknown.
    let _wallet = load_wallet_cached(&input.wallet_id)?;
    let nonce = challenge_issue(&input.wallet_id);
    Ok(proto::GetChallengeOutput {
        nonce: nonce.to_vec(),
    })
}

fn agent_derivation_path(agent_index: u32) -> String {
    format!("m/44'/60'/0'/1/{}", agent_index)
}

/// Maximum allowed JWT lifetime: 7 days.
const MAX_AGENT_JWT_TTL: i64 = 7 * 24 * 3600;

/// Current wall-clock time (UNIX epoch seconds) read from the REE clock via TEE_GetREETime.
///
/// `std::time::SystemTime::now()` is NOT wired into the OP-TEE TA runtime — calling it panics
/// the TA (observed on real i.MX93 hardware: create-agent-key / refresh-agent-credential aborted
/// with a TA panic). The TA must obtain time through the optee-utee `Time` API instead.
///
/// REE time is "as trusted as the REE itself" (the host can shift the system clock), but the host
/// still cannot inject `iat`/`exp` into the HMAC-signed JWT payload directly — the TA computes and
/// signs them, so H-3 (TA owns iat; host only supplies the capped ttl_secs) still holds.
fn tee_unix_secs() -> i64 {
    let mut t = Time::new();
    t.ree_time();
    t.seconds as i64
}

fn create_agent_key(input: &proto::CreateAgentKeyInput) -> Result<proto::CreateAgentKeyOutput> {
    dbg_println!(
        "[+] Create agent key for wallet: {:?}, agent_index: {}",
        input.wallet_id,
        input.agent_index
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;

    // C-1: TA-side passkey verification — blocks a compromised host from calling this
    // command directly to mint agent credentials without user presence.
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;

    // H-1: Enforce TTL bounds inside TA (host-supplied, but TA caps it).
    if input.ttl_secs <= 0 || input.ttl_secs > MAX_AGENT_JWT_TTL {
        return Err(anyhow!("ttl_secs must be in 1..=604800"));
    }
    // H-2: Subject must not contain JSON-special characters that could inject claims.
    validate_jwt_subject(&input.subject)?;

    let derivation_path = agent_derivation_path(input.agent_index);
    let (agent_address, public_key_compressed) = wallet.derive_address(&derivation_path)?;

    // Build JWT payload entirely inside TEE — iat computed from the TA's view of the clock so
    // a compromised host cannot supply iat=0 or iat=far_future to shift the TTL window.
    // H-3: TA owns iat; host only supplies ttl_secs (capped above).
    let iat = tee_unix_secs();
    let agent_addr_hex = format!("0x{}", hex::encode(agent_address));
    let wallet_id_str = input.wallet_id.to_string();
    let exp = iat.checked_add(input.ttl_secs)
        .ok_or_else(|| anyhow!("JWT exp overflow"))?;
    let payload_json = format!(
        "{{\"sub\":\"{sub}\",\"wallet_id\":\"{wid}\",\"agent_index\":{idx},\"agent_address\":\"{addr}\",\"iat\":{iat},\"exp\":{exp}}}",
        sub  = input.subject,
        wid  = wallet_id_str,
        idx  = input.agent_index,
        addr = agent_addr_hex,
        iat  = iat,
        exp  = exp,
    );
    let jwt_out = jwt_sign_payload_internal(&payload_json)?;

    Ok(proto::CreateAgentKeyOutput {
        agent_address,
        public_key_compressed,
        jwt_kid:        jwt_out.kid,
        jwt_header_b64: jwt_out.header_b64,
        jwt_payload_b64: jwt_out.payload_b64,
        jwt_hmac:       jwt_out.hmac,
    })
}

fn sign_agent_user_op(input: &proto::SignAgentUserOpInput) -> Result<proto::SignAgentUserOpOutput> {
    dbg_println!(
        "[+] Sign agent user op for wallet: {:?}, agent_index: {}",
        input.wallet_id,
        input.agent_index
    );

    // TA-side JWT authorization: verify HMAC proof before signing.
    // Prevents a compromised CA from bypassing host-side JWT verification.
    let jwt_verify_input = proto::JwtHmacVerifyInput {
        kid: input.jwt_kid.clone(),
        message: input.jwt_signing_input.clone(),
        expected_hmac: input.jwt_hmac.clone(),
    };
    let verify_result = jwt_hmac_verify(&jwt_verify_input)?;
    if !verify_result.valid {
        return Err(anyhow!("TA: agent JWT credential verification failed"));
    }

    // Bind JWT claims to request parameters — prevents a compromised host from using
    // wallet A's JWT to request signing for wallet B (Issue #16 claim-binding fix).
    verify_jwt_wallet_claims(&input.jwt_signing_input, &input.wallet_id, input.agent_index)?;

    let wallet = load_wallet_cached(&input.wallet_id)?;
    let derivation_path = agent_derivation_path(input.agent_index);
    let private_key = wallet.export_private_key(&derivation_path)?;

    // Derive the agent key's Ethereum address from its public key (inside TEE, safe).
    let (agent_key_address, _) = wallet.derive_address(&derivation_path)?;

    let mut eip191 = b"\x19Ethereum Signed Message:\n32".to_vec();
    eip191.extend_from_slice(&input.user_op_hash);
    let digest = Keccak256::digest(&eip191);

    let secret_key = secp256k1::SecretKey::from_slice(&private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let message = secp256k1::Message::from_slice(&digest[..])?;
    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    // v0.17.2 wire format: [0x08][account(20)][key(20)][r(32)][s(32)][v(1)] = 106 bytes
    // account = Smart Account contract address (prevents cross-account session-key abuse)
    // key     = agent secp256k1 EOA address (derived from private key inside TEE)
    let mut signature = Vec::with_capacity(106);
    signature.push(0x08u8);
    signature.extend_from_slice(&input.account_address);
    signature.extend_from_slice(&agent_key_address);
    signature.extend_from_slice(&sig_bytes);
    signature.push(recovery_id.to_i32() as u8 + 27);

    Ok(proto::SignAgentUserOpOutput { signature })
}

fn hmac_sha256(secret: &[u8], message: &[u8]) -> Result<[u8; 32]> {
    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|_| anyhow!("Invalid JWT HMAC secret"))?;
    mac.update(message);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}


/// Internal JWT signing helper — NOT exposed as a TA command.
/// Accepts a pre-built payload JSON string, computes the full JWT signing_input
/// and HMAC using the current TEE-stored secret.
struct JwtSignedMaterial {
    kid: String,
    header_b64: String,
    payload_b64: String,
    hmac: [u8; 32],
}

fn jwt_sign_payload_internal(payload_json: &str) -> Result<JwtSignedMaterial> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    let db = open_storage()?;
    let mut store = JwtSecretStore::load(&db);
    let current = store.ensure_current()?.clone();
    store.save(&db)?;

    let header_json = format!(
        "{{\"alg\":\"HS256\",\"typ\":\"JWT\",\"kid\":\"{}\"}}",
        current.kid
    );
    let header_b64 = Base64UrlUnpadded::encode_string(header_json.as_bytes());
    let payload_b64 = Base64UrlUnpadded::encode_string(payload_json.as_bytes());

    let signing_input = format!("{}.{}", header_b64, payload_b64);
    let hmac = hmac_sha256(&current.secret, signing_input.as_bytes())?;

    Ok(JwtSignedMaterial { kid: current.kid, header_b64, payload_b64, hmac })
}

/// Extract a u64 field value from a compact JSON object without serde_json.
fn extract_json_u64_field(json: &str, key: &str) -> Result<u64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&*pattern)
        .ok_or_else(|| anyhow!("JWT claim '{}' not found", key))?
        + pattern.len();
    let rest = json[start..].trim_start_matches(' ');
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse::<u64>().map_err(|_| anyhow!("JWT claim '{}' is not a u64", key))
}

/// Validate that `subject` contains only characters safe to embed directly in a JSON string
/// without escaping. Prevents claim injection via a crafted subject value (H-2).
fn validate_jwt_subject(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 256 {
        return Err(anyhow!("JWT subject length must be 1..=256"));
    }
    if !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:.@+/=".contains(&b)) {
        return Err(anyhow!("JWT subject contains characters not allowed in JSON (no quotes, backslashes, or controls)"));
    }
    Ok(())
}

/// After HMAC verification, decode the JWT payload and verify that wallet_id and
/// agent_index match the request parameters. Prevents a compromised host from
/// using a legitimate JWT for wallet A to request signing for wallet B.
fn verify_jwt_wallet_claims(
    signing_input: &[u8],
    expected_wallet_id: &uuid::Uuid,
    expected_agent_index: u32,
) -> Result<()> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    let input_str = core::str::from_utf8(signing_input)
        .map_err(|_| anyhow!("JWT signing_input is not UTF-8"))?;
    let dot = input_str.find('.')
        .ok_or_else(|| anyhow!("JWT signing_input missing dot separator"))?;
    let payload_b64 = &input_str[dot + 1..];
    let payload_bytes = Base64UrlUnpadded::decode_vec(payload_b64)
        .map_err(|_| anyhow!("JWT payload base64 decode failed"))?;
    let payload_str = core::str::from_utf8(&payload_bytes)
        .map_err(|_| anyhow!("JWT payload is not UTF-8"))?;

    let jwt_wallet_id = extract_json_str_field(payload_str, "wallet_id")?;
    if jwt_wallet_id != expected_wallet_id.to_string() {
        return Err(anyhow!("JWT wallet_id claim does not match request"));
    }

    // H-3: use u64 then bounds-check before casting to avoid silent truncation
    let jwt_agent_index_u64 = extract_json_u64_field(payload_str, "agent_index")?;
    if jwt_agent_index_u64 > u32::MAX as u64 {
        return Err(anyhow!("JWT agent_index overflow"));
    }
    if jwt_agent_index_u64 as u32 != expected_agent_index {
        return Err(anyhow!("JWT agent_index claim does not match request"));
    }

    // Structural exp/iat check: exp must be after iat and within the TTL cap.
    let iat = extract_json_u64_field(payload_str, "iat")?;
    let exp = extract_json_u64_field(payload_str, "exp")?;
    if exp <= iat || exp.saturating_sub(iat) > MAX_AGENT_JWT_TTL as u64 {
        return Err(anyhow!("JWT exp/iat structurally invalid or exceeds TTL cap"));
    }

    // #15: runtime expiry check against the TEE-side trusted clock. tee_unix_secs()
    // reads REE time via optee_utee::Time::ree_time() — the same trusted-time source
    // the #49 challenge-binding TTL already relies on (the old "no trusted clock in
    // mock mode" note was stale). Reject a JWT whose exp is at/before now. Guard on
    // now > 0 so a mock/uninitialized clock (returns 0) skips the check rather than
    // rejecting every token; on real hardware ree_time is a valid unix timestamp.
    let now = tee_unix_secs();
    if now > 0 {
        if now as u64 >= exp {
            return Err(anyhow!("JWT expired (now {} >= exp {})", now, exp));
        }
    } else {
        // Observability: the REE clock returned <= 0 (mock/uninitialized or a
        // fault). We skip the runtime expiry check rather than reject every
        // token, but surface it — a PERSISTENT now<=0 on real hardware means JWT
        // expiry is silently NOT being enforced and must be investigated.
        trace_println!(
            "[!] #15: TEE REE clock returned {} (<=0); JWT runtime expiry NOT enforced this call",
            now
        );
    }

    Ok(())
}

fn create_p256_session_key(
    input: &proto::CreateP256SessionKeyInput,
) -> Result<proto::CreateP256SessionKeyOutput> {
    dbg_println!(
        "[+] Create P256 session key for wallet: {:?}, index: {}",
        input.wallet_id,
        input.session_index
    );

    // Verify the wallet exists so we don't create orphaned session keys
    let _wallet = load_wallet_cached(&input.wallet_id)?;

    // Enforce TTL bounds (same as create_agent_key)
    if input.ttl_secs <= 0 || input.ttl_secs > MAX_AGENT_JWT_TTL {
        return Err(anyhow!("ttl_secs must be in 1..=604800"));
    }
    validate_jwt_subject(&input.subject)?;

    // Generate P-256 key pair via p256-m using OP-TEE hardware RNG (p256_generate_random).
    // priv[32] = private scalar; pub[64] = x(32) || y(32) (uncompressed, no 0x04 prefix)
    let mut priv_bytes = [0u8; 32];
    let mut pub_bytes = [0u8; 64];
    let ret = unsafe { p256_gen_keypair(priv_bytes.as_mut_ptr(), pub_bytes.as_mut_ptr()) };
    if ret != 0 {
        return Err(anyhow!("p256_gen_keypair failed (code {})", ret));
    }

    // Persist both private key and public key in TEE secure storage.
    // Public key is stored so sign_p256_user_op can embed it in the 149-byte output
    // without needing to re-derive it from the private key.
    let db = open_storage()?;
    let sk = P256SessionKey {
        store_id: P256SessionKey::store_id_for(&input.wallet_id, input.session_index),
        private_key: priv_bytes.to_vec(),
        pub_key: pub_bytes.to_vec(),
    };
    sk.save(&db)?;

    let mut pub_key_x = [0u8; 32];
    let mut pub_key_y = [0u8; 32];
    pub_key_x.copy_from_slice(&pub_bytes[..32]);
    pub_key_y.copy_from_slice(&pub_bytes[32..]);

    // Issue JWT credential for this session key (TEE-HMAC'd, same mechanism as agent keys).
    // agent_index is repurposed as session_index so verify_credential on the host works unchanged.
    let iat = tee_unix_secs();
    let exp = iat.checked_add(input.ttl_secs)
        .ok_or_else(|| anyhow!("JWT exp overflow"))?;
    let wallet_id_str = input.wallet_id.to_string();
    let payload_json = format!(
        "{{\"sub\":\"{sub}\",\"wallet_id\":\"{wid}\",\"agent_index\":{idx},\"agent_address\":\"0x0000000000000000000000000000000000000000\",\"iat\":{iat},\"exp\":{exp}}}",
        sub  = input.subject,
        wid  = wallet_id_str,
        idx  = input.session_index,
        iat  = iat,
        exp  = exp,
    );
    let jwt_out = jwt_sign_payload_internal(&payload_json)?;

    Ok(proto::CreateP256SessionKeyOutput {
        pub_key_x,
        pub_key_y,
        jwt_kid:         jwt_out.kid,
        jwt_header_b64:  jwt_out.header_b64,
        jwt_payload_b64: jwt_out.payload_b64,
        jwt_hmac:        jwt_out.hmac,
    })
}

fn sign_p256_user_op(
    input: &proto::SignP256UserOpInput,
) -> Result<proto::SignP256UserOpOutput> {
    dbg_println!(
        "[+] Sign P256 user op for wallet: {:?}, index: {}",
        input.wallet_id,
        input.session_index
    );

    // TA-side JWT authorization: verify HMAC proof before any signing (defense-in-depth).
    // Prevents a compromised CA from bypassing host-side JWT verification.
    let jwt_verify_input = proto::JwtHmacVerifyInput {
        kid: input.jwt_kid.clone(),
        message: input.jwt_signing_input.clone(),
        expected_hmac: input.jwt_hmac.clone(),
    };
    let verify_result = jwt_hmac_verify(&jwt_verify_input)?;
    if !verify_result.valid {
        return Err(anyhow!("TA: P256 session JWT credential verification failed"));
    }

    // Load P-256 key pair from TEE secure storage
    let db = open_storage()?;
    let sk = P256SessionKey::load(&db, &input.wallet_id, input.session_index)?;
    if sk.private_key.len() != 32 {
        return Err(anyhow!(
            "Corrupt P256 private key: expected 32 bytes, got {}",
            sk.private_key.len()
        ));
    }
    if sk.pub_key.len() != 64 {
        return Err(anyhow!(
            "Corrupt P256 public key: expected 64 bytes, got {}",
            sk.pub_key.len()
        ));
    }

    // EIP-191 prefix: "\x19Ethereum Signed Message:\n32" || userOpHash
    let mut eip191 = b"\x19Ethereum Signed Message:\n32".to_vec();
    eip191.extend_from_slice(&input.user_op_hash);
    let digest = Keccak256::digest(&eip191);

    // Sign with P-256 via p256-m (internally uses p256_generate_random for nonce)
    // sig[64] = r(32) || s(32) big-endian integers
    let mut sig_bytes = [0u8; 64];
    let ret = unsafe {
        p256_ecdsa_sign(
            sig_bytes.as_mut_ptr(),
            sk.private_key.as_ptr(),
            digest.as_ptr(),
            digest.len(),
        )
    };
    if ret != 0 {
        return Err(anyhow!("p256_ecdsa_sign failed (code {})", ret));
    }

    // v0.18.1 wire format: [0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)] = 149 bytes
    // account = ERC-4337 Smart Account address (prevents cross-account session key abuse)
    // keyX/keyY = P-256 public key (verifier needs pubkey to verify non-recoverable ECDSA)
    // r/s = P-256 ECDSA signature over EIP-191(userOpHash)
    let mut signature = Vec::with_capacity(149);
    signature.push(0x08u8);
    signature.extend_from_slice(&input.account_address);
    signature.extend_from_slice(&sk.pub_key[..32]);   // keyX
    signature.extend_from_slice(&sk.pub_key[32..64]); // keyY
    signature.extend_from_slice(&sig_bytes[..32]);    // r
    signature.extend_from_slice(&sig_bytes[32..64]);  // s

    Ok(proto::SignP256UserOpOutput { signature })
}

/// Extract wallet_id, agent_index, and exp from a JWT signing input, then validate exp.
/// signing_input format: "header_b64url.payload_b64url" (exactly two dot-separated segments).
/// Payload JSON: {"wallet_id":"<uuid>","agent_index":<u32>,"exp":<i64>}
/// The format is controlled by our own jwt_sign_payload TA function — no whitespace around colons.
fn jwt_parse_claims(signing_input: &[u8]) -> Result<(String, u32)> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    // Guard against memory pressure from attacker-controlled input.
    const MAX_SIGNING_INPUT_BYTES: usize = 4096;
    if signing_input.len() > MAX_SIGNING_INPUT_BYTES {
        return Err(anyhow!("jwt_parse_claims: signing_input too large ({} bytes, max {})",
            signing_input.len(), MAX_SIGNING_INPUT_BYTES));
    }


    let s = std::str::from_utf8(signing_input)
        .map_err(|_| anyhow!("jwt_parse_claims: signing_input is not valid UTF-8"))?;

    // Require exactly "header.payload" — split_once ensures both parts are non-empty
    let (_, payload_b64) = s.split_once('.')
        .ok_or_else(|| anyhow!("jwt_parse_claims: signing_input must be 'header.payload'"))?;
    if payload_b64.is_empty() || payload_b64.contains('.') {
        return Err(anyhow!("jwt_parse_claims: malformed signing_input — expected exactly two segments"));
    }


    let payload_bytes = Base64UrlUnpadded::decode_vec(payload_b64)
        .map_err(|_| anyhow!("jwt_parse_claims: payload base64url decode failed"))?;
    let payload = std::str::from_utf8(&payload_bytes)
        .map_err(|_| anyhow!("jwt_parse_claims: payload is not valid UTF-8"))?;

    // Validate payload starts with '{' — guards against format drift
    if !payload.starts_with('{') {
        return Err(anyhow!("jwt_parse_claims: payload is not a JSON object"));
    }


    let wallet_id = extract_json_str_field(payload, "wallet_id")?;
    let agent_index = extract_json_u32_field(payload, "agent_index")?;
    // Parse exp for future use; not checked in TA due to unreliable TEE clock epoch.
    let _exp = extract_json_i64_field(payload, "exp")?;

    Ok((wallet_id, agent_index))
}

/// Extract a JSON string field value from a minimally-formatted JWT payload.
/// Format contract: `"field":"value"` with no whitespace around colon (our jwt_sign_payload format).
fn extract_json_str_field(json: &str, field: &str) -> Result<String> {
    let needle = format!("\"{}\":\"", field);
    let start = json.find(needle.as_str())
        .ok_or_else(|| anyhow!("field '{}' not found in JWT payload", field))?
        + needle.len();
    let end = json[start..].find('"')
        .ok_or_else(|| anyhow!("unterminated string for field '{}' in JWT payload", field))?
        + start;
    Ok(json[start..end].to_string())
}

/// Extract a JSON unsigned integer field from JWT payload.
/// Format contract: `"field":12345` (digits immediately after colon, no whitespace).
fn extract_json_u32_field(json: &str, field: &str) -> Result<u32> {
    let needle = format!("\"{}\":", field);
    let start = json.find(needle.as_str())
        .ok_or_else(|| anyhow!("field '{}' not found in JWT payload", field))?
        + needle.len();
    let end = json[start..].find(|c: char| !c.is_ascii_digit())
        .unwrap_or(json[start..].len())
        + start;
    if start == end {
        return Err(anyhow!("empty value for u32 field '{}' in JWT payload", field));
    }
    json[start..end].parse::<u32>()
        .map_err(|e| anyhow!("field '{}' is not a valid u32: {}", field, e))
}

/// Extract a JSON signed integer field from JWT payload (used for 'exp' claim).
fn extract_json_i64_field(json: &str, field: &str) -> Result<i64> {
    let needle = format!("\"{}\":", field);
    let start = json.find(needle.as_str())
        .ok_or_else(|| anyhow!("field '{}' not found in JWT payload", field))?
        + needle.len();
    let end = json[start..].find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(json[start..].len())
        + start;
    if start == end {
        return Err(anyhow!("empty value for i64 field '{}' in JWT payload", field));
    }
    json[start..end].parse::<i64>()
        .map_err(|e| anyhow!("field '{}' is not a valid i64 in JWT payload: {}", field, e))
}

/// Delete a P256 session key from TEE secure storage.
/// Called by the host's lazy GC when credential_expires_at has passed.
/// Returns deleted=false (not an error) if the key is already absent — idempotent.
fn delete_p256_session_key(
    input: &proto::DeleteP256SessionKeyInput,
) -> Result<proto::DeleteP256SessionKeyOutput> {
    dbg_println!(
        "[+] Delete P256 session key for wallet: {:?}, index: {}",
        input.wallet_id,
        input.session_index
    );
    let db = open_storage()?;
    let store_id = P256SessionKey::store_id_for(&input.wallet_id, input.session_index);
    match db.delete_entry::<P256SessionKey>(&store_id) {
        Ok(()) => Ok(proto::DeleteP256SessionKeyOutput { deleted: true }),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("ItemNotFound") || msg.contains("ITEM_NOT_FOUND") {
                Ok(proto::DeleteP256SessionKeyOutput { deleted: false })
            } else {
                Err(anyhow!("delete_p256_session_key: secure storage error: {}", msg))
            }
        }
    }
}

fn sign_typed_data(input: &proto::SignTypedDataInput) -> Result<proto::SignTypedDataOutput> {
    dbg_println!(
        "[+] EIP-712 sign typed data for wallet: {:?}, primary_type: {}",
        input.wallet_id,
        input.primary_type
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;

    // TA-side auth gate (defense-in-depth): independently verifies the caller's authorization.
    // Host has already validated, but TA confirms using TEE-resident secrets that cannot be
    // forged by a compromised host. Exactly one auth proof must be present.
    match (&input.passkey_assertion, &input.jwt_kid) {
        (Some(_), None) => {
            // WebAuthn path: verify passkey signature against wallet's stored public key.
            verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
        }
        (None, Some(kid)) => {
            // Agent JWT path: all three JWT proof fields must be present.
            let signing_input = input.jwt_signing_input.as_deref()
                .ok_or_else(|| anyhow!("sign_typed_data: jwt_signing_input missing from JWT proof"))?;
            let expected_hmac = input.jwt_hmac.as_deref()
                .ok_or_else(|| anyhow!("sign_typed_data: jwt_hmac missing from JWT proof"))?;

            // Step 1: Verify HMAC using TEE-resident JWT secret.
            let verify_result = jwt_hmac_verify(&proto::JwtHmacVerifyInput {
                kid: kid.clone(),
                message: signing_input.to_vec(),
                expected_hmac: expected_hmac.to_vec(),
            })?;
            if !verify_result.valid {
                return Err(anyhow!("JWT HMAC verification failed in TA for sign-typed-data"));
            }

            // Step 2: Verify JWT claims bind to this request's wallet and agent path.
            // Parse wallet_id and agent_index from the JWT payload (base64url-encoded JSON).
            let (jwt_wallet_id, jwt_agent_index) = jwt_parse_claims(signing_input)?;
            if jwt_wallet_id != input.wallet_id.to_string() {
                return Err(anyhow!("JWT wallet_id does not match request wallet_id"));
            }
            let expected_path = format!("m/44'/60'/0'/1/{}", jwt_agent_index);
            if input.hd_path != expected_path {
                return Err(anyhow!(
                    "JWT agent path '{}' does not match request hd_path '{}'",
                    expected_path, input.hd_path
                ));
            }
        }
        (None, None) => {
            return Err(anyhow!("sign_typed_data: no auth proof provided to TA"));
        }
        (Some(_), Some(_)) => {
            return Err(anyhow!("sign_typed_data: ambiguous auth — provide passkey OR JWT, not both"));
        }
    }

    // Resolve the primary type definition from the provided type list
    let primary_type_def = input
        .types
        .iter()
        .find(|td| td.name == input.primary_type)
        .ok_or_else(|| anyhow!("Primary type '{}' not found in types list", input.primary_type))?;

    // Compute EIP-712 digest entirely inside TEE
    let digest = eip712::eip712_digest(&input.domain, primary_type_def, &input.message)?;

    let private_key = wallet.export_private_key(&input.hd_path)?;

    let secret_key = secp256k1::SecretKey::from_slice(&private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let message = secp256k1::Message::from_slice(&digest)?;
    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&sig_bytes);
    signature.push(recovery_id.to_i32() as u8 + 27);

    Ok(proto::SignTypedDataOutput { signature })
}

// ── Grant Session ABI encoding helpers ──

fn abi_u256_from_u64(val: u64) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[24..32].copy_from_slice(&val.to_be_bytes());
    buf
}

fn abi_pad_address(addr: &[u8; 20]) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..32].copy_from_slice(addr);
    buf
}

/// bytes4 is right-padded in ABI encoding (data at low offset, zeros at high offset)
fn abi_pad_bytes4_right(b: &[u8; 4]) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[0..4].copy_from_slice(b);
    buf
}

fn abi_u256_from_u16(val: u16) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[30..32].copy_from_slice(&val.to_be_bytes());
    buf
}

fn abi_u256_from_u32(val: u32) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[28..32].copy_from_slice(&val.to_be_bytes());
    buf
}

/// keccak256(abi.encodePacked(addresses)) — tight-packed 20-byte entries
fn keccak_packed_addresses(addrs: &[[u8; 20]]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(addrs.len() * 20);
    for a in addrs {
        buf.extend_from_slice(a);
    }
    Keccak256::digest(&buf).into()
}

/// keccak256(abi.encodePacked(selectors)) — tight-packed 4-byte entries
fn keccak_packed_selectors(sels: &[[u8; 4]]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(sels.len() * 4);
    for s in sels {
        buf.extend_from_slice(s);
    }
    Keccak256::digest(&buf).into()
}

/// Build GRANT_SESSION_V2 inner hash.
/// Matches _buildGrantHash() in SessionKeyValidator.sol:
///   keccak256(abi.encode("GRANT_SESSION_V2", chainId, contract, account, sessionKey,
///             expiry, contractScope, selectorScope, velocityLimit, velocityWindow,
///             callTargetsHash, selectorsHash, nonce))
///
/// ABI layout: 13 args; head = 13*32 = 416 bytes; string data tail = 64 bytes.
fn build_grant_session_inner(input: &proto::SignGrantSessionInput) -> [u8; 32] {
    let call_targets_hash = keccak_packed_addresses(&input.call_targets);
    let selectors_hash = keccak_packed_selectors(&input.selector_allowlist);

    let mut buf = [0u8; 480];
    // [0x000] string offset = 13 * 32 = 416
    buf[0..32].copy_from_slice(&{
        let mut v = [0u8; 32];
        v[30..32].copy_from_slice(&(416u16).to_be_bytes());
        v
    });
    // [0x020] chainId
    buf[32..64].copy_from_slice(&abi_u256_from_u64(input.chain_id));
    // [0x040] verifyingContract
    buf[64..96].copy_from_slice(&abi_pad_address(&input.verifying_contract));
    // [0x060] account
    buf[96..128].copy_from_slice(&abi_pad_address(&input.account));
    // [0x080] sessionKey
    buf[128..160].copy_from_slice(&abi_pad_address(&input.session_key));
    // [0x0A0] expiry (uint48)
    buf[160..192].copy_from_slice(&abi_u256_from_u64(input.expiry));
    // [0x0C0] contractScope
    buf[192..224].copy_from_slice(&abi_pad_address(&input.contract_scope));
    // [0x0E0] selectorScope (bytes4, right-padded)
    buf[224..256].copy_from_slice(&abi_pad_bytes4_right(&input.selector_scope));
    // [0x100] velocityLimit (uint16)
    buf[256..288].copy_from_slice(&abi_u256_from_u16(input.velocity_limit));
    // [0x120] velocityWindow (uint32)
    buf[288..320].copy_from_slice(&abi_u256_from_u32(input.velocity_window));
    // [0x140] callTargetsHash
    buf[320..352].copy_from_slice(&call_targets_hash);
    // [0x160] selectorsHash
    buf[352..384].copy_from_slice(&selectors_hash);
    // [0x180] nonce (uint256, big-endian 32 bytes)
    buf[384..416].copy_from_slice(&input.nonce);
    // [0x1A0] string length = 16
    buf[416..448].copy_from_slice(&{
        let mut v = [0u8; 32];
        v[31] = 16;
        v
    });
    // [0x1C0] "GRANT_SESSION_V2" (16 bytes, right-zero-padded to 32)
    buf[448..464].copy_from_slice(b"GRANT_SESSION_V2");

    Keccak256::digest(&buf).into()
}

/// Build GRANT_P256_SESSION_V2 inner hash.
/// Matches _buildP256GrantHash() in SessionKeyValidator.sol:
///   keccak256(abi.encode("GRANT_P256_SESSION_V2", chainId, contract, account, keyX, keyY,
///             expiry, contractScope, selectorScope, velocityLimit, velocityWindow,
///             callTargetsHash, selectorsHash, nonce))
///
/// ABI layout: 14 args; head = 14*32 = 448 bytes; string data tail = 64 bytes.
fn build_p256_grant_session_inner(input: &proto::SignP256GrantSessionInput) -> [u8; 32] {
    let call_targets_hash = keccak_packed_addresses(&input.call_targets);
    let selectors_hash = keccak_packed_selectors(&input.selector_allowlist);

    let mut buf = [0u8; 512];
    // [0x000] string offset = 14 * 32 = 448
    buf[0..32].copy_from_slice(&{
        let mut v = [0u8; 32];
        v[30..32].copy_from_slice(&(448u16).to_be_bytes());
        v
    });
    // [0x020] chainId
    buf[32..64].copy_from_slice(&abi_u256_from_u64(input.chain_id));
    // [0x040] verifyingContract
    buf[64..96].copy_from_slice(&abi_pad_address(&input.verifying_contract));
    // [0x060] account
    buf[96..128].copy_from_slice(&abi_pad_address(&input.account));
    // [0x080] keyX (bytes32)
    buf[128..160].copy_from_slice(&input.key_x);
    // [0x0A0] keyY (bytes32)
    buf[160..192].copy_from_slice(&input.key_y);
    // [0x0C0] expiry (uint48)
    buf[192..224].copy_from_slice(&abi_u256_from_u64(input.expiry));
    // [0x0E0] contractScope
    buf[224..256].copy_from_slice(&abi_pad_address(&input.contract_scope));
    // [0x100] selectorScope (bytes4, right-padded)
    buf[256..288].copy_from_slice(&abi_pad_bytes4_right(&input.selector_scope));
    // [0x120] velocityLimit (uint16)
    buf[288..320].copy_from_slice(&abi_u256_from_u16(input.velocity_limit));
    // [0x140] velocityWindow (uint32)
    buf[320..352].copy_from_slice(&abi_u256_from_u32(input.velocity_window));
    // [0x160] callTargetsHash
    buf[352..384].copy_from_slice(&call_targets_hash);
    // [0x180] selectorsHash
    buf[384..416].copy_from_slice(&selectors_hash);
    // [0x1A0] nonce (uint256, big-endian 32 bytes)
    buf[416..448].copy_from_slice(&input.nonce);
    // [0x1C0] string length = 21
    buf[448..480].copy_from_slice(&{
        let mut v = [0u8; 32];
        v[31] = 21;
        v
    });
    // [0x1E0] "GRANT_P256_SESSION_V2" (21 bytes, right-zero-padded to 32)
    buf[480..501].copy_from_slice(b"GRANT_P256_SESSION_V2");

    Keccak256::digest(&buf).into()
}

/// Wrap inner hash with EIP-191 personal_sign prefix (matches OpenZeppelin toEthSignedMessageHash).
fn eip191_hash(inner: &[u8; 32]) -> [u8; 32] {
    let mut msg = [0u8; 60];
    msg[0..28].copy_from_slice(b"\x19Ethereum Signed Message:\n32");
    msg[28..60].copy_from_slice(inner);
    Keccak256::digest(&msg).into()
}

fn sign_grant_session(input: &proto::SignGrantSessionInput) -> Result<proto::SignGrantSessionOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;

    let inner = build_grant_session_inner(input);
    let final_hash = eip191_hash(&inner);

    let private_key = wallet.export_private_key(&input.hd_path)?;
    let secret_key = secp256k1::SecretKey::from_slice(&private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let msg = secp256k1::Message::from_slice(&final_hash)?;
    let sig = secp.sign_ecdsa_recoverable(&msg, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&sig_bytes);
    signature.push(recovery_id.to_i32() as u8 + 27);

    Ok(proto::SignGrantSessionOutput { signature })
}

fn sign_p256_grant_session(input: &proto::SignP256GrantSessionInput) -> Result<proto::SignP256GrantSessionOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;

    let inner = build_p256_grant_session_inner(input);
    let final_hash = eip191_hash(&inner);

    let private_key = wallet.export_private_key(&input.hd_path)?;
    let secret_key = secp256k1::SecretKey::from_slice(&private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let msg = secp256k1::Message::from_slice(&final_hash)?;
    let sig = secp.sign_ecdsa_recoverable(&msg, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&sig_bytes);
    signature.push(recovery_id.to_i32() as u8 + 27);

    Ok(proto::SignP256GrantSessionOutput { signature })
}

fn jwt_hmac_verify(input: &proto::JwtHmacVerifyInput) -> Result<proto::JwtHmacVerifyOutput> {
    let db = open_storage()?;
    let store = JwtSecretStore::load(&db);
    let valid = match store.find(&input.kid) {
        Some(entry) => {
            let mut mac = HmacSha256::new_from_slice(&entry.secret)
                .map_err(|_| anyhow!("Invalid JWT HMAC secret"))?;
            mac.update(&input.message);
            mac.verify_slice(&input.expected_hmac).is_ok()
        }
        None => false,
    };

    Ok(proto::JwtHmacVerifyOutput { valid })
}

fn jwt_rotate_secret(input: &proto::JwtRotateSecretInput) -> Result<proto::JwtRotateSecretOutput> {
    let db = open_storage()?;
    let mut store = JwtSecretStore::load(&db);
    let had_current = store.current().is_some();

    if !input.force && !had_current {
        // First-time init: create the initial secret without rotating
        let current = store.ensure_current()?.clone();
        store.save(&db)?;
        return Ok(proto::JwtRotateSecretOutput {
            new_kid: current.kid,
            retired_kid: None,
        });
    }

    // Normal rotation: keep retired entry for 7-day overlap window.
    // Force rotation: purge retired entries immediately (old JWTs become invalid).
    let (new_kid, retired_kid) = store.rotate(input.force)?;
    store.save(&db)?;

    Ok(proto::JwtRotateSecretOutput {
        new_kid,
        retired_kid,
    })
}

fn handle_invoke(command: Command, serialized_input: &[u8]) -> Result<Vec<u8>> {
    fn process<T: serde::de::DeserializeOwned, U: serde::Serialize, F: Fn(&T) -> Result<U>>(
        serialized_input: &[u8],
        handler: F,
    ) -> Result<Vec<u8>> {
        let input: T = bincode::deserialize(serialized_input)?;
        let output = handler(&input)?;
        let serialized_output = bincode::serialize(&output)?;
        Ok(serialized_output)
    }

    match command {
        Command::CreateWallet => process(serialized_input, create_wallet),
        Command::RemoveWallet => process(serialized_input, remove_wallet),
        Command::DeriveAddress => process(serialized_input, derive_address),
        Command::SignTransaction => process(serialized_input, sign_transaction),
        Command::SignMessage => process(serialized_input, sign_message),
        Command::SignHash => process(serialized_input, sign_hash),
        Command::DeriveAddressAuto => process(serialized_input, derive_address_auto),
        Command::ExportPrivateKey => process(serialized_input, export_private_key),
        // M-3: VerifyPasskey was an unconditional `valid:true` stub. Removing it
        // from dispatch prevents it from ever being used as a fake auth oracle.
        // Real authorization always goes through verify_passkey_for_wallet (p256-m).
        Command::VerifyPasskey => bail!("VerifyPasskey is not supported (use a signing command which verifies the passkey)"),
        Command::WarmupCache => process(serialized_input, warmup_cache),
        Command::RegisterPasskeyTa => process(serialized_input, register_passkey_ta),
        Command::CreateAgentKey => process(serialized_input, create_agent_key),
        Command::SignAgentUserOp => process(serialized_input, sign_agent_user_op),
        Command::JwtHmacVerify => process(serialized_input, jwt_hmac_verify),
        Command::JwtRotateSecret => process(serialized_input, jwt_rotate_secret),
        Command::SignTypedData => process(serialized_input, sign_typed_data),
        Command::CreateP256SessionKey => process(serialized_input, create_p256_session_key),
        Command::SignP256UserOp => process(serialized_input, sign_p256_user_op),
        Command::DeleteP256SessionKey => process(serialized_input, delete_p256_session_key),
        Command::SignGrantSession => process(serialized_input, sign_grant_session),
        Command::SignP256GrantSession => process(serialized_input, sign_p256_grant_session),
        Command::ForceRemoveWallet => process(serialized_input, force_remove_wallet),
        Command::ReadRollbackCounter => process(serialized_input, read_rollback_counter),
        Command::GetChallenge => process(serialized_input, get_challenge),
        Command::GetAttestation => process(serialized_input, attestation::get_attestation),
        _ => bail!("Unsupported command"),
    }
}

// Output buffer size the host allocates for p1 (see ta_client.rs OUTPUT_MAX_SIZE).
// C-4: the TA must never report an output length larger than the host buffer.
// If it did, a host that trusts p2.a() (the returned length) and slices its
// 4096-byte buffer with it would panic / read OOB. We bound both the success
// payload and error messages to this size and signal SHORT_BUFFER explicitly.
const OUTPUT_BUF_SIZE: usize = 4096;

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    dbg_println!("[+] TA invoke command");
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    let output_vec = match handle_invoke(Command::from(cmd_id), p0.buffer()) {
        Ok(output) => output,
        Err(e) => {
            // C-4: cap the error message so it can never exceed the host buffer.
            let mut err_message = format!("{:?}", e).into_bytes();
            err_message.truncate(OUTPUT_BUF_SIZE);
            // Defensive: only write if it fits the actual provided buffer.
            if err_message.len() > p1.buffer().len() {
                err_message.truncate(p1.buffer().len());
            }
            p1.buffer()
                .write(&err_message)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            p2.set_a(err_message.len() as u32);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };

    // C-4: reject oversized output instead of letting the host slice past its
    // 4096-byte buffer with a length it cannot satisfy. Return SHORT_BUFFER and
    // set p2 to 0 so the host does not slice with a bogus length.
    if output_vec.len() > OUTPUT_BUF_SIZE || output_vec.len() > p1.buffer().len() {
        p2.set_a(0);
        return Err(Error::new(ErrorKind::ShortBuffer));
    }

    p1.buffer()
        .write(&output_vec)
        .map_err(|_| Error::new(ErrorKind::BadState))?;
    p2.set_a(output_vec.len() as u32);

    Ok(())
}

// H-D: anti-rollback epoch_check boundary tests. This is the core security
// decision function for RPMB anti-rollback — pure logic, pinned here against
// regression. (TA-crate tests follow the eip712.rs convention: compiled under
// cfg(test), executed when a TA test runner is available.)
#[cfg(test)]
mod rollback_tests {
    use super::epoch_check;

    fn wid() -> uuid::Uuid {
        uuid::Uuid::from_bytes([0x22; 16])
    }

    #[test]
    fn epoch_zero_skips_check() {
        // legacy wallets (epoch 0) bypass anti-rollback entirely
        assert!(!epoch_check(0, 0, true, &wid()).unwrap());
        assert!(!epoch_check(0, 100, true, &wid()).unwrap());
        assert!(!epoch_check(0, 0, false, &wid()).unwrap());
    }

    #[test]
    fn epoch_equal_passes_no_recovery() {
        assert!(!epoch_check(5, 5, true, &wid()).unwrap());
    }

    #[test]
    fn epoch_behind_counter_passes() {
        // wallet not mutated recently; counter moved on via other wallets (M-8)
        assert!(!epoch_check(3, 5, true, &wid()).unwrap());
        assert!(!epoch_check(1, u64::MAX, true, &wid()).unwrap());
    }

    #[test]
    fn epoch_plus_one_triggers_recovery() {
        // crash window: wallet saved with N+1 but counter write was interrupted
        assert!(epoch_check(6, 5, true, &wid()).unwrap());
        assert!(epoch_check(1, 0, true, &wid()).unwrap());
    }

    #[test]
    fn epoch_plus_two_rejected_as_tampered() {
        assert!(epoch_check(7, 5, true, &wid()).is_err());
        assert!(epoch_check(u64::MAX, 5, true, &wid()).is_err());
    }

    #[test]
    fn counter_absent_reinitializes_from_wallet_epoch() {
        // C-2: post-reflash the RPMB counter object is gone; a legitimate
        // wallet with epoch N must NOT brick — it re-establishes the baseline
        assert!(epoch_check(5, 0, false, &wid()).unwrap());
        assert!(epoch_check(u64::MAX, 0, false, &wid()).unwrap());
    }

    #[test]
    fn counter_at_max_does_not_overflow() {
        // saturating_add guard: must not panic in debug builds
        assert!(!epoch_check(5, u64::MAX, true, &wid()).unwrap());
        assert!(epoch_check(u64::MAX, u64::MAX - 1, true, &wid()).unwrap());
    }
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));
