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

use anyhow::{anyhow, Result};
use bip32::Mnemonic;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use uuid::Uuid;

use crate::bip32_secp::{self, CachedXPrv, DerivedKey};
use crate::hash::keccak_hash_to_bytes;
use ethereum_tx_sign::Transaction;
use optee_utee::Random;
use proto::EthTransaction;
use secure_db::Storable;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Wallet {
    id: Uuid,
    entropy: Vec<u8>,
    next_address_index: u32,
    next_account_index: u32,
    /// Cached BIP39 seed (64 bytes) — avoids re-running PBKDF2 on every operation
    #[serde(default)]
    cached_seed: Option<Vec<u8>>,
    /// Cached m/44'/60'/0' extended key (97 bytes) — skips 3 hardened derivation levels
    #[serde(default)]
    cached_account_root: Option<Vec<u8>>,
    /// P-256 passkey public key (65 bytes uncompressed: 0x04 || x || y)
    passkey_pubkey: Option<Vec<u8>>,
}

impl Storable for Wallet {
    type Key = Uuid;

    fn unique_id(&self) -> Self::Key {
        self.id
    }
}

impl Wallet {
    pub fn new() -> Result<Self> {
        let mut entropy = vec![0u8; 32];
        Random::generate(entropy.as_mut() as _);

        let mut random_bytes = vec![0u8; 16];
        Random::generate(random_bytes.as_mut() as _);
        let uuid = uuid::Builder::from_random_bytes(
            random_bytes
                .try_into()
                .map_err(|_| anyhow!("[-] Wallet::new(): invalid random bytes"))?,
        )
        .into_uuid();

        Ok(Self {
            id: uuid,
            entropy,
            next_address_index: 0,
            next_account_index: 0,
            cached_seed: None,
            cached_account_root: None,
            passkey_pubkey: None,
        })
    }

    pub fn get_next_address_index(&self) -> u32 {
        self.next_address_index
    }

    pub fn increment_address_index(&mut self) -> Result<u32> {
        const MAX_ADDRESSES_PER_WALLET: u32 = 100;

        if self.next_address_index >= MAX_ADDRESSES_PER_WALLET {
            return Err(anyhow!(
                "Wallet address limit reached ({}/{})",
                self.next_address_index,
                MAX_ADDRESSES_PER_WALLET
            ));
        }

        let current = self.next_address_index;
        self.next_address_index += 1;
        Ok(current)
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_mnemonic(&self) -> Result<String> {
        let mnemonic = Mnemonic::from_entropy(
            self.entropy.as_slice().try_into()?,
            bip32::Language::English,
        );
        Ok(mnemonic.phrase().to_string())
    }

    pub fn get_seed(&self) -> Result<Vec<u8>> {
        if let Some(ref seed) = self.cached_seed {
            return Ok(seed.clone());
        }
        let mnemonic = Mnemonic::from_entropy(
            self.entropy.as_slice().try_into()?,
            bip32::Language::English,
        );
        let seed = mnemonic.to_seed("");
        Ok(seed.as_bytes().to_vec())
    }

    /// Compute seed via PBKDF2 and cache it, plus compute account root (m/44'/60'/0').
    /// Returns `true` if anything was actually cached (caller should persist).
    pub fn ensure_seed_cached(&mut self) -> Result<bool> {
        let mut changed = false;

        if self.cached_seed.is_none() {
            let mnemonic = Mnemonic::from_entropy(
                self.entropy.as_slice().try_into()?,
                bip32::Language::English,
            );
            let seed = mnemonic.to_seed("");
            self.cached_seed = Some(seed.as_bytes().to_vec());
            changed = true;
        }

        // Also cache the account root if not already cached
        if self.cached_account_root.is_none() {
            let seed = self.cached_seed.as_ref().unwrap();
            let root = bip32_secp::compute_account_root(seed)?;
            self.cached_account_root = Some(root.serialize().to_vec());
            changed = true;
        }

        Ok(changed)
    }

    /// Get cached account root, or compute it on the fly.
    fn get_account_root(&self) -> Result<Option<CachedXPrv>> {
        match &self.cached_account_root {
            Some(data) => Ok(Some(CachedXPrv::deserialize(data)?)),
            None => Ok(None),
        }
    }

    /// Derive key using optimized libsecp256k1 path.
    fn derive_key(&self, hd_path: &str) -> Result<DerivedKey> {
        let seed = self.get_seed()?;
        let (account, address) = bip32_secp::parse_eth_path(hd_path)?;
        let cached = self.get_account_root()?;
        bip32_secp::derive_full(&seed, cached.as_ref(), account, address)
    }

    pub fn derive_address(&self, hd_path: &str) -> Result<([u8; 20], Vec<u8>)> {
        let derived = self.derive_key(hd_path)?;

        // Ethereum address: Keccak256(uncompressed_pubkey[1..]) → last 20 bytes
        let uncompressed_no_prefix = &derived.public_key_uncompressed[1..];
        let address = &keccak_hash_to_bytes(uncompressed_no_prefix)[12..];

        Ok((
            address.try_into()?,
            derived.public_key_compressed.to_vec(),
        ))
    }

    pub fn sign_transaction(&self, hd_path: &str, transaction: &EthTransaction) -> Result<Vec<u8>> {
        let derived = self.derive_key(hd_path)?;
        let legacy_transaction = ethereum_tx_sign::LegacyTransaction {
            chain: transaction.chain_id,
            nonce: transaction.nonce,
            gas_price: transaction.gas_price,
            gas: transaction.gas,
            to: transaction.to,
            value: transaction.value,
            data: transaction.data.clone(),
        };
        let ecdsa = legacy_transaction.ecdsa(&derived.private_key.to_vec()).map_err(|e| {
            let ethereum_tx_sign::Error::Secp256k1(inner_error) = e;
            inner_error
        })?;
        let signature = legacy_transaction.sign(&ecdsa);
        Ok(signature)
    }

    pub fn sign_message(&self, hd_path: &str, message: &[u8]) -> Result<Vec<u8>> {
        let derived = self.derive_key(hd_path)?;

        let message_hash = keccak_hash_to_bytes(message);

        let secret_key = secp256k1::SecretKey::from_slice(&derived.private_key)?;
        let secp = secp256k1::Secp256k1::new();

        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(&message_hash[..32]);
        let message_obj = secp256k1::Message::from_slice(&hash_array)?;

        let sig = secp.sign_ecdsa_recoverable(&message_obj, &secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();

        let mut signature = Vec::with_capacity(65);
        signature.extend_from_slice(&sig_bytes);
        signature.push(recovery_id.to_i32() as u8 + 27);

        Ok(signature)
    }

    pub fn sign_hash(&self, hd_path: &str, hash: &[u8; 32]) -> Result<Vec<u8>> {
        let derived = self.derive_key(hd_path)?;

        let secret_key = secp256k1::SecretKey::from_slice(&derived.private_key)?;
        let secp = secp256k1::Secp256k1::new();

        let message_obj = secp256k1::Message::from_slice(hash)?;

        let sig = secp.sign_ecdsa_recoverable(&message_obj, &secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();

        let mut signature = Vec::with_capacity(65);
        signature.extend_from_slice(&sig_bytes);
        signature.push(recovery_id.to_i32() as u8 + 27);

        Ok(signature)
    }

    pub fn export_private_key(&self, hd_path: &str) -> Result<Vec<u8>> {
        let derived = self.derive_key(hd_path)?;
        Ok(derived.private_key.to_vec())
    }

    pub fn set_passkey(&mut self, pubkey: Vec<u8>) {
        self.passkey_pubkey = Some(pubkey);
    }

    pub fn get_passkey(&self) -> Option<&[u8]> {
        self.passkey_pubkey.as_deref()
    }

    pub fn has_passkey(&self) -> bool {
        self.passkey_pubkey.is_some()
    }
}

impl TryFrom<Wallet> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(wallet: Wallet) -> Result<Vec<u8>> {
        bincode::serialize(&wallet).map_err(|e| anyhow!("[-] Wallet::try_into(): {:?}", e))
    }
}

impl TryFrom<Vec<u8>> for Wallet {
    type Error = anyhow::Error;

    fn try_from(data: Vec<u8>) -> Result<Wallet> {
        bincode::deserialize::<Wallet>(&data)
            .map_err(|e| anyhow!("[-] Wallet::try_from(): {:?}", e))
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.entropy.iter_mut().for_each(|x| *x = 0);
        if let Some(ref mut seed) = self.cached_seed {
            seed.iter_mut().for_each(|x| *x = 0);
        }
        if let Some(ref mut root) = self.cached_account_root {
            root.iter_mut().for_each(|x| *x = 0);
        }
        if let Some(ref mut pk) = self.passkey_pubkey {
            pk.iter_mut().for_each(|x| *x = 0);
        }
    }
}
