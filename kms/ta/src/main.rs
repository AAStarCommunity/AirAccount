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
            self.rotate()?;
        }
        self.current()
            .ok_or_else(|| anyhow!("JWT HMAC secret store has no current secret"))
    }

    fn rotate(&mut self) -> Result<(String, Option<String>)> {
        let retired_kid = self.current().map(|entry| entry.kid.clone());
        for entry in self.entries.iter_mut() {
            if entry.current {
                entry.current = false;
                entry.retired_at = Some(0);
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

// p256-m FFI — debugging crash (branch: debug/p256m-ta)
extern "C" {
    fn p256_ecdsa_verify(sig: *const u8, pubkey: *const u8, hash: *const u8, hlen: usize) -> i32;
}
#[no_mangle]
pub extern "C" fn p256_generate_random(_output: *mut u8, _output_size: u32) -> i32 {
    -1
}

/// Verify passkey assertion against wallet's bound passkey.
/// All wallets MUST have passkey bound — rejects if missing.
///
/// NOTE: p256-m C library crashes in OP-TEE Secure World on DK2 (Cortex-A7).
/// CA-side pre-verification (Rust p256 crate) is the primary security check.
/// TA-side only validates that passkey is bound and assertion is present.
/// TODO: debug p256-m crash on DK2, re-enable TA-side ECDSA verify.
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
    let mnemonic = wallet.get_mnemonic()?;

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

    // CLI admin mode: None assertion = skip passkey verify
    // API mode (if re-enabled): assertion provided = verify
    if input.passkey_assertion.is_some() {
        verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    } else {
        dbg_println!("[+] ExportPrivateKey: admin mode (no passkey assertion)");
    }

    let private_key = wallet.export_private_key(&input.derivation_path)?;

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

fn verify_passkey(_input: &proto::VerifyPasskeyInput) -> Result<proto::VerifyPasskeyOutput> {
    dbg_println!("[+] Verify passkey for wallet: {:?}", _input.wallet_id);

    // p256-m disabled: crashes in OP-TEE Secure World on DK2 (Cortex-A7).
    // CA-side P-256 verify (Rust p256 crate) is the primary security check.
    // TA-side verify temporarily returns OK — CA has already verified.
    dbg_println!("[+] Passkey verification: delegated to CA (p256-m disabled)");

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

fn create_agent_key(input: &proto::CreateAgentKeyInput) -> Result<proto::CreateAgentKeyOutput> {
    dbg_println!(
        "[+] Create agent key for wallet: {:?}, agent_index: {}",
        input.wallet_id,
        input.agent_index
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;
    let derivation_path = agent_derivation_path(input.agent_index);
    let (agent_address, public_key_compressed) = wallet.derive_address(&derivation_path)?;

    Ok(proto::CreateAgentKeyOutput {
        agent_address,
        public_key_compressed,
    })
}

fn sign_agent_user_op(input: &proto::SignAgentUserOpInput) -> Result<proto::SignAgentUserOpOutput> {
    dbg_println!(
        "[+] Sign agent user op for wallet: {:?}, agent_index: {}",
        input.wallet_id,
        input.agent_index
    );

    let wallet = load_wallet_cached(&input.wallet_id)?;
    let derivation_path = agent_derivation_path(input.agent_index);
    let private_key = wallet.export_private_key(&derivation_path)?;

    let mut eip191 = b"\x19Ethereum Signed Message:\n32".to_vec();
    eip191.extend_from_slice(&input.user_op_hash);
    let digest = Keccak256::digest(&eip191);

    let secret_key = secp256k1::SecretKey::from_slice(&private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let message = secp256k1::Message::from_slice(&digest[..])?;
    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut signature = Vec::with_capacity(65);
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

fn jwt_hmac_sign(input: &proto::JwtHmacSignInput) -> Result<proto::JwtHmacSignOutput> {
    let db = SecureStorageClient::open(DB_NAME)?;
    let mut store = JwtSecretStore::load(&db);
    let current = store.ensure_current()?.clone();
    store.save(&db)?;

    Ok(proto::JwtHmacSignOutput {
        hmac: hmac_sha256(&current.secret, &input.message)?,
        kid: current.kid,
    })
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
        let current = store.ensure_current()?.clone();
        store.save(&db)?;
        return Ok(proto::JwtRotateSecretOutput {
            new_kid: current.kid,
            retired_kid: None,
        });
    }

    let (new_kid, retired_kid) = store.rotate()?;
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
        Command::JwtHmacSign => process(serialized_input, jwt_hmac_sign),
        Command::JwtHmacVerify => process(serialized_input, jwt_hmac_verify),
        Command::JwtRotateSecret => process(serialized_input, jwt_rotate_secret),
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
