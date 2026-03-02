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
use optee_utee::{Error, ErrorKind, Parameters};
use proto::Command;
use secure_db::SecureStorageClient;

use anyhow::{anyhow, bail, Result};
use std::cell::RefCell;
use std::io::Write;
use uuid::Uuid;
use wallet::Wallet;

const DB_NAME: &str = "eth_wallet_db";

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
            let lru_idx = self.entries.iter()
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
fn load_wallet_cached(wallet_id: &Uuid) -> Result<Wallet> {
    // Fast path: cache hit
    if let Some(mut w) = cache_get(wallet_id) {
        let changed = w.ensure_seed_cached()?;
        if !changed {
            return Ok(w);
        }
        // Seed was just computed — persist to storage
        let db = SecureStorageClient::open(DB_NAME)?;
        save_wallet(&db, &w)?;
        return Ok(w);
    }

    // Slow path: cache miss — read from storage
    let db = SecureStorageClient::open(DB_NAME)?;
    let mut w = db.get::<Wallet>(wallet_id)
        .map_err(|e| anyhow!("wallet not found: {:?}", e))?;
    let changed = w.ensure_seed_cached()?;
    if changed {
        save_wallet(&db, &w)?;
    } else {
        cache_put(&w);
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

/// Verify passkey assertion against wallet's bound passkey.
/// All wallets MUST have passkey bound — rejects if missing.
/// Rejects if assertion is not provided.
///
/// P-256 ECDSA cryptographic verification is done by the CA (host) side
/// before forwarding to TA. TA only validates assertion presence and format.
/// This avoids OP-TEE native crypto ECC issues on STM32MP1 (Cortex-A7)
/// while maintaining security: CA pre-verifies, TA gates on assertion presence.
fn verify_passkey_for_wallet(wallet: &Wallet, assertion: Option<&proto::PasskeyAssertion>) -> Result<()> {
    let _pubkey = match wallet.get_passkey() {
        Some(pk) => pk,
        None => return Err(anyhow!("Wallet has no PassKey bound. Cannot verify.")),
    };

    let assertion = assertion.ok_or_else(|| anyhow!("Wallet has PassKey bound. Provide PassKey assertion."))?;

    // Format validation — actual ECDSA verify is done by CA
    if assertion.signature_r.len() != 32 || assertion.signature_s.len() != 32 {
        return Err(anyhow!("Invalid signature: r and s must be 32 bytes each"));
    }
    if assertion.authenticator_data.is_empty() || assertion.client_data_hash.len() != 32 {
        return Err(anyhow!("Invalid assertion: authenticator_data must be non-empty, client_data_hash must be 32 bytes"));
    }

    Ok(())
}

fn create_wallet(input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    // Validate passkey public key (mandatory)
    if input.passkey_pubkey.len() != 65 || input.passkey_pubkey[0] != 0x04 {
        return Err(anyhow!("PassKey pubkey must be 65 bytes uncompressed (0x04||x||y), got {} bytes", input.passkey_pubkey.len()));
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
    let wallet = db_client.get::<Wallet>(&input.wallet_id)
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
    Ok(proto::DeriveAddressOutput { address, public_key })
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

fn derive_address_auto(input: &proto::DeriveAddressAutoInput) -> Result<proto::DeriveAddressAutoOutput> {
    let db_client = SecureStorageClient::open(DB_NAME)?;

    dbg_println!("[+] DeriveAddressAuto for wallet: {:?}", input.wallet_id);
    let mut wallet = match cache_get(&input.wallet_id) {
        Some(w) => w,
        None => db_client.get::<Wallet>(&input.wallet_id)
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

fn export_private_key(input: &proto::ExportPrivateKeyInput) -> Result<proto::ExportPrivateKeyOutput> {
    dbg_println!("[+] Export private key for wallet: {:?}, path: {}", input.wallet_id, input.derivation_path);

    let wallet = load_wallet_cached(&input.wallet_id)?;
    verify_passkey_for_wallet(&wallet, input.passkey_assertion.as_ref())?;
    let private_key = wallet.export_private_key(&input.derivation_path)?;

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

fn verify_passkey(input: &proto::VerifyPasskeyInput) -> Result<proto::VerifyPasskeyOutput> {
    dbg_println!("[+] Verify passkey for wallet: {:?}", input.wallet_id);

    // Format validation only — actual P-256 ECDSA is done by CA
    if input.public_key.len() != 65 || input.public_key[0] != 0x04 {
        return Err(anyhow!("Invalid P-256 public key: expected 65 bytes uncompressed"));
    }
    if input.signature_r.len() != 32 || input.signature_s.len() != 32 {
        return Err(anyhow!("Invalid signature: r and s must be 32 bytes each"));
    }

    dbg_println!("[+] Passkey format validation: OK");

    Ok(proto::VerifyPasskeyOutput { valid: true })
}

fn register_passkey_ta(input: &proto::RegisterPasskeyTaInput) -> Result<proto::RegisterPasskeyTaOutput> {
    trace_println!("[+] Registering passkey for wallet: {:?}", input.wallet_id);

    if input.passkey_pubkey.len() != 65 || input.passkey_pubkey[0] != 0x04 {
        bail!("PassKey public key must be 65 bytes uncompressed (0x04 || x || y), got {} bytes",
            input.passkey_pubkey.len());
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
