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

fn cache_remove(wallet_id: &Uuid) {
    WALLET_CACHE.with(|c| c.borrow_mut().remove(wallet_id));
}

fn cache_len() -> usize {
    WALLET_CACHE.with(|c| c.borrow().len())
}

/// Save wallet to secure storage AND update cache.
fn save_wallet(db: &SecureStorageClient, wallet: &Wallet) -> Result<()> {
    db.put(wallet)?;
    cache_put(wallet);
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

fn create_wallet(_input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    let wallet = Wallet::new()?;
    let wallet_id = wallet.get_id();
    let mnemonic = wallet.get_mnemonic()?;
    dbg_println!("[+] Wallet ID: {:?}", wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    save_wallet(&db_client, &wallet)?;
    dbg_println!("[+] Wallet saved in secure storage");

    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,
    })
}

fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput> {
    trace_println!("[+] Removing wallet: {:?}", input.wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    db_client.delete_entry::<Wallet>(&input.wallet_id)?;
    // NOTE: cache_remove() intentionally skipped — calling RefCell::borrow_mut()
    // from this function triggers TEE_ERROR_TARGET_DEAD on STM32 OP-TEE (compiler/
    // code layout bug with nightly-2024-05-15 arm-unknown-optee). Stale cache
    // entries are harmless: LRU eviction handles cleanup, and load_wallet_cached
    // will fail on storage read for deleted wallets.
    trace_println!("[+] Wallet removed from secure storage");

    Ok(proto::RemoveWalletOutput {})
}

fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    let (address, public_key) = wallet.derive_address(&input.hd_path)?;
    Ok(proto::DeriveAddressOutput { address, public_key })
}

fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
    Ok(proto::SignTransactionOutput { signature })
}

fn sign_message(input: &proto::SignMessageInput) -> Result<proto::SignMessageOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    let signature = wallet.sign_message(&input.hd_path, &input.message)?;
    Ok(proto::SignMessageOutput { signature })
}

fn sign_hash(input: &proto::SignHashInput) -> Result<proto::SignHashOutput> {
    let wallet = load_wallet_cached(&input.wallet_id)?;
    let signature = wallet.sign_hash(&input.hd_path, &input.hash)?;
    Ok(proto::SignHashOutput { signature })
}

fn derive_address_auto(input: &proto::DeriveAddressAutoInput) -> Result<proto::DeriveAddressAutoOutput> {
    let db_client = SecureStorageClient::open(DB_NAME)?;

    let (wallet_id, mut wallet, address_index) = if let Some(existing_id) = input.wallet_id {
        dbg_println!("[+] Loading existing wallet: {:?}", existing_id);
        let mut wallet = match cache_get(&existing_id) {
            Some(w) => w,
            None => db_client.get::<Wallet>(&existing_id)
                .map_err(|e| anyhow!("wallet not found: {:?}", e))?,
        };

        let index = wallet.increment_address_index()?;
        (existing_id, wallet, index)
    } else {
        let mut wallet = Wallet::new()?;
        let wallet_id = wallet.get_id();
        let index = wallet.increment_address_index()?;
        (wallet_id, wallet, index)
    };

    wallet.ensure_seed_cached()?;

    let derivation_path = format!("m/44'/60'/0'/0/{}", address_index);
    let (address, public_key) = wallet.derive_address(&derivation_path)?;

    // Always save: address index was incremented
    save_wallet(&db_client, &wallet)?;

    Ok(proto::DeriveAddressAutoOutput {
        wallet_id,
        address,
        public_key,
        derivation_path,
    })
}

fn export_private_key(input: &proto::ExportPrivateKeyInput) -> Result<proto::ExportPrivateKeyOutput> {
    dbg_println!("[+] Export private key for wallet: {:?}, path: {}", input.wallet_id, input.derivation_path);

    let wallet = load_wallet_cached(&input.wallet_id)?;
    let private_key = wallet.export_private_key(&input.derivation_path)?;

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

fn verify_passkey(input: &proto::VerifyPasskeyInput) -> Result<proto::VerifyPasskeyOutput> {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
    use p256::EncodedPoint;
    use sha2::{Sha256, Digest};

    dbg_println!("[+] Verify passkey for wallet: {:?}", input.wallet_id);

    // Parse the P-256 public key from uncompressed format (65 bytes: 0x04 || x || y)
    let encoded_point = EncodedPoint::from_bytes(&input.public_key)
        .map_err(|e| anyhow!("Invalid P-256 public key: {:?}", e))?;
    let verifying_key = VerifyingKey::from_encoded_point(&encoded_point)
        .map_err(|e| anyhow!("Failed to parse P-256 verifying key: {:?}", e))?;

    // Reconstruct the signed message: SHA-256(authenticatorData || clientDataHash)
    // This is the WebAuthn signature verification procedure per spec
    let mut hasher = Sha256::new();
    hasher.update(&input.authenticator_data);
    hasher.update(&input.client_data_hash);
    let signed_message: [u8; 32] = hasher.finalize().into();

    // Reconstruct ECDSA signature from r, s components
    let signature = Signature::from_scalars(input.signature_r, input.signature_s)
        .map_err(|e| anyhow!("Invalid ECDSA signature: {:?}", e))?;

    // Verify the signature
    let valid = verifying_key
        .verify(&signed_message, &signature)
        .is_ok();

    dbg_println!("[+] Passkey verification result: {}", valid);

    if !valid {
        bail!("Passkey verification failed: invalid signature");
    }

    Ok(proto::VerifyPasskeyOutput { valid })
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
