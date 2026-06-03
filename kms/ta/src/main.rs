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

mod bip32_secp;
mod eip712;
mod hash;
mod wallet;

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters, Random};
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

    #[allow(dead_code)]
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

// cache_remove disabled: OP-TEE secure storage writes corrupt TLS,
// and remove_wallet does db.delete before this would be called.
// Let cache entries expire naturally via LRU eviction.

fn cache_len() -> usize {
    WALLET_CACHE.with(|c| c.borrow().len())
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

/// Save wallet to secure storage AND update cache.
fn save_wallet(db: &SecureStorageClient, wallet: &Wallet) -> Result<()> {
    // Cache MUST come before db.put: OP-TEE secure storage syscall corrupts TLS,
    // causing thread_local WALLET_CACHE access to panic if called after db.put.
    cache_put(wallet);
    db.put(wallet)?;
    Ok(())
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
fn load_wallet_cached(wallet_id: &Uuid) -> Result<Wallet> {
    // Fast path: cache hit
    if let Some(mut w) = cache_get(wallet_id) {
        let changed = w.ensure_seed_cached()?;
        if changed {
            // Update in-memory cache only — NO db.put (would corrupt TLS)
            trace_println!(
                "[!] load_wallet_cached: cold seed computed for {:?}, memory-only cache",
                wallet_id
            );
            cache_put(&w);
        }
        return Ok(w);
    }

    // Slow path: cache miss — read from storage
    let db = SecureStorageClient::open(DB_NAME)?;
    let mut w = db
        .get::<Wallet>(wallet_id)
        .map_err(|e| anyhow!("wallet not found: {:?}", e))?;
    let changed = w.ensure_seed_cached()?;
    if changed {
        trace_println!(
            "[!] load_wallet_cached: cold seed from storage for {:?}, memory-only cache",
            wallet_id
        );
    }
    // Always cache in memory (NO db.put — avoids TLS corruption in signing path)
    cache_put(&w);
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

fn create_wallet(input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    // Validate passkey public key (mandatory)
    if input.passkey_pubkey.len() != 65 || input.passkey_pubkey[0] != 0x04 {
        return Err(anyhow!(
            "PassKey pubkey must be 65 bytes uncompressed (0x04||x||y), got {} bytes",
            input.passkey_pubkey.len()
        ));
    }

    let mut wallet = Wallet::new()?;
    wallet.set_passkey(input.passkey_pubkey.clone());
    let wallet_id = wallet.get_id();

    // Mnemonic never crosses TEE boundary in production.
    // Only populated with the export-secrets feature (dev/test).
    #[cfg(feature = "export-secrets")]
    let mnemonic = wallet.get_mnemonic()?;
    #[cfg(not(feature = "export-secrets"))]
    let mnemonic = String::new();

    dbg_println!("[+] Wallet ID: {:?}", wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    save_wallet(&db_client, &wallet)?;
    dbg_println!("[+] Wallet saved in secure storage (passkey bound)");

    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,
    })
}

fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput> {
    trace_println!("[+] Removing wallet: {:?}", input.wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;

    // Load from DB (not cache) — read op doesn't corrupt TLS
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("wallet not found: {:?}", e))?;

    // Mandatory passkey verification
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;

    db_client.delete_entry::<Wallet>(&input.wallet_id)?;
    // No cache_remove — borrow_mut panic risk after secure storage write
    trace_println!("[+] Wallet removed from secure storage (passkey verified)");

    Ok(proto::RemoveWalletOutput {})
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

fn derive_address_auto(
    input: &proto::DeriveAddressAutoInput,
) -> Result<proto::DeriveAddressAutoOutput> {
    let db_client = SecureStorageClient::open(DB_NAME)?;

    dbg_println!("[+] DeriveAddressAuto for wallet: {:?}", input.wallet_id);
    let mut wallet = match cache_get(&input.wallet_id) {
        Some(w) => w,
        None => db_client
            .get::<Wallet>(&input.wallet_id)
            .map_err(|e| anyhow!("wallet not found: {:?}", e))?,
    };

    let address_index = wallet.increment_address_index()?;
    wallet.ensure_seed_cached()?;

    let derivation_path = format!("m/44'/60'/0'/0/{}", address_index);
    let (address, public_key) = wallet.derive_address(&derivation_path)?;

    save_wallet(&db_client, &wallet)?;

    Ok(proto::DeriveAddressAutoOutput {
        wallet_id: input.wallet_id,
        address,
        public_key,
        derivation_path,
    })
}

fn export_private_key(
    input: &proto::ExportPrivateKeyInput,
) -> Result<proto::ExportPrivateKeyOutput> {
    dbg_println!(
        "[+] Export private key for wallet: {:?}, path: {}",
        input.wallet_id,
        input.derivation_path
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;

    // In production builds, passkey assertion is ALWAYS required.
    // The passkey-less admin bypass is only allowed with the export-secrets feature (dev/test).
    #[cfg(feature = "export-secrets")]
    {
        if input.passkey_assertion.is_some() {
            verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
        } else {
            dbg_println!("[+] ExportPrivateKey: dev admin mode (no passkey assertion)");
        }
    }
    #[cfg(not(feature = "export-secrets"))]
    {
        // Production: always require passkey, no bypass
        verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    }

    let private_key = wallet.export_private_key(&input.derivation_path)?;

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

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

    let mut wallet = load_wallet_cached(&input.wallet_id)?;
    // Verify current passkey before allowing change
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    wallet.set_passkey(input.passkey_pubkey.clone());

    let db = SecureStorageClient::open(DB_NAME)?;
    save_wallet(&db, &wallet)?;
    trace_println!("[+] PassKey registered and wallet saved");

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

fn agent_derivation_path(agent_index: u32) -> String {
    format!("m/44'/60'/0'/1/{}", agent_index)
}

/// Maximum allowed JWT lifetime: 7 days.
const MAX_AGENT_JWT_TTL: i64 = 7 * 24 * 3600;

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

    // Build JWT payload entirely inside TEE — iat computed from TA system clock so
    // a compromised host cannot supply iat=0 or iat=far_future to shift the TTL window.
    // H-3: TA owns iat; host only supplies ttl_secs (capped above).
    let iat = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
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

    let db = SecureStorageClient::open(DB_NAME)?;
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

    // H-4-lite: structural exp/iat check (no trusted clock available in TA mock mode,
    // but we can reject obviously invalid JWTs: exp must be after iat, within TTL cap).
    let iat = extract_json_u64_field(payload_str, "iat")?;
    let exp = extract_json_u64_field(payload_str, "exp")?;
    if exp <= iat || exp.saturating_sub(iat) > MAX_AGENT_JWT_TTL as u64 {
        return Err(anyhow!("JWT exp/iat structurally invalid or exceeds TTL cap"));
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
    let db = SecureStorageClient::open(DB_NAME)?;
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
    let iat = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
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
    let db = SecureStorageClient::open(DB_NAME)?;
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
    let db = SecureStorageClient::open(DB_NAME)?;
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
    let db = SecureStorageClient::open(DB_NAME)?;
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
    let db = SecureStorageClient::open(DB_NAME)?;
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
        Command::VerifyPasskey => process(serialized_input, verify_passkey),
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
        _ => bail!("Unsupported command"),
    }
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    dbg_println!("[+] TA invoke command");
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    let output_vec = match handle_invoke(Command::from(cmd_id), p0.buffer()) {
        Ok(output) => output,
        Err(e) => {
            let err_message = format!("{:?}", e).as_bytes().to_vec();
            p1.buffer()
                .write(&err_message)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            p2.set_a(err_message.len() as u32);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };
    p1.buffer()
        .write(&output_vec)
        .map_err(|_| Error::new(ErrorKind::BadState))?;
    p2.set_a(output_vec.len() as u32);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));
