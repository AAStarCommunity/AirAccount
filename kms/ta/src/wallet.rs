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
use bip32::{Mnemonic, XPrv};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use uuid::Uuid;

use crate::hash::keccak_hash_to_bytes;
use ethereum_tx_sign::Transaction;
use optee_utee::Random;
use proto::EthTransaction;
use secure_db::Storable;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Wallet {
    id: Uuid,
    entropy: Vec<u8>,
    /// Passkey P-256 public key (SEC1 uncompressed format: 65 bytes)
    /// None means passkey is not configured for this wallet
    #[serde(default)]
    passkey_pubkey: Option<Vec<u8>>,
    /// Whether passkey authentication is enabled for critical operations
    #[serde(default)]
    passkey_enabled: bool,
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
            passkey_pubkey: None,
            passkey_enabled: false,
        })
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
        let mnemonic = Mnemonic::from_entropy(
            self.entropy.as_slice().try_into()?,
            bip32::Language::English,
        );
        let seed = mnemonic.to_seed(""); // empty passwords
        Ok(seed.as_bytes().to_vec())
    }

    pub fn derive_prv_key(&self, hd_path: &str) -> Result<Vec<u8>> {
        let path = hd_path.parse()?;
        let child_xprv = XPrv::derive_from_path(self.get_seed()?, &path)?;
        let child_xprv_bytes = child_xprv.to_bytes();
        Ok(child_xprv_bytes.to_vec())
    }

    pub fn derive_pub_key(&self, hd_path: &str) -> Result<Vec<u8>> {
        let path = hd_path.parse()?;
        let child_xprv = XPrv::derive_from_path(self.get_seed()?, &path)?;
        // public key
        let child_xpub_bytes = child_xprv.public_key().to_bytes();
        Ok(child_xpub_bytes.to_vec())
    }

    pub fn derive_address(&self, hd_path: &str) -> Result<([u8; 20], Vec<u8>)> {
        let public_key_bytes = self.derive_pub_key(hd_path)?;
        // uncompress public key
        let public_key = secp256k1::PublicKey::from_slice(&public_key_bytes)?;
        let uncompressed_public_key = &public_key.serialize_uncompressed()[1..];

        // pubkey to address
        let address = &keccak_hash_to_bytes(&uncompressed_public_key)[12..];
        Ok((address.try_into()?, public_key_bytes))
    }

    pub fn sign_transaction(&self, hd_path: &str, transaction: &EthTransaction) -> Result<Vec<u8>> {
        let xprv = self.derive_prv_key(hd_path)?;
        let legacy_transaction = ethereum_tx_sign::LegacyTransaction {
            chain: transaction.chain_id,
            nonce: transaction.nonce,
            gas_price: transaction.gas_price,
            gas: transaction.gas,
            to: transaction.to,
            value: transaction.value,
            data: transaction.data.clone(),
        };
        let ecdsa = legacy_transaction.ecdsa(&xprv).map_err(|e| {
            let ethereum_tx_sign::Error::Secp256k1(inner_error) = e;
            inner_error
        })?;
        let signature = legacy_transaction.sign(&ecdsa);
        Ok(signature)
    }

    /// Sign a raw hash using secp256k1
    ///
    /// # Arguments
    /// * `hd_path` - HD derivation path (e.g., "m/44'/60'/0'/0/0")
    /// * `hash` - 32-byte hash to sign
    ///
    /// # Returns
    /// 65-byte signature (r + s + v)
    pub fn sign_hash(&self, hd_path: &str, hash: &[u8; 32]) -> Result<Vec<u8>> {
        use secp256k1::{ecdsa::RecoverableSignature, Message, Secp256k1, SecretKey};

        if hash.len() != 32 {
            return Err(anyhow!("Hash must be exactly 32 bytes"));
        }

        // Derive private key
        let xprv_bytes = self.derive_prv_key(hd_path)?;
        let secret_key = SecretKey::from_slice(&xprv_bytes[..32])?;

        // Sign the hash
        let secp = Secp256k1::new();
        // In secp256k1 0.27, Message::from_slice expects exactly 32 bytes
        let message = Message::from_slice(hash)
            .map_err(|e| anyhow!("Failed to create message from hash: {:?}", e))?;
        let sig: RecoverableSignature = secp.sign_ecdsa_recoverable(&message, &secret_key);
        let (recovery_id, sig_bytes) = sig.serialize_compact();

        // Combine signature + recovery_id into 65 bytes
        let mut signature = Vec::with_capacity(65);
        signature.extend_from_slice(&sig_bytes);
        signature.push(recovery_id.to_i32() as u8);

        Ok(signature)
    }

    // ===== Passkey-related methods =====

    /// Configure passkey public key for this wallet
    ///
    /// # Arguments
    /// * `pubkey_sec1` - P-256 public key in SEC1 uncompressed format (65 bytes: 0x04 + x + y)
    ///
    /// # Security
    /// This permanently associates a passkey with this wallet.
    /// Once set, passkey verification can be enabled for critical operations.
    pub fn set_passkey_pubkey(&mut self, pubkey_sec1: Vec<u8>) -> Result<()> {
        if pubkey_sec1.len() != 65 {
            return Err(anyhow!(
                "Invalid passkey public key length: expected 65 bytes, got {}",
                pubkey_sec1.len()
            ));
        }
        if pubkey_sec1[0] != 0x04 {
            return Err(anyhow!(
                "Invalid passkey public key format: must be uncompressed (start with 0x04)"
            ));
        }

        self.passkey_pubkey = Some(pubkey_sec1);
        Ok(())
    }

    /// Get the configured passkey public key
    pub fn get_passkey_pubkey(&self) -> Option<&[u8]> {
        self.passkey_pubkey.as_deref()
    }

    /// Enable or disable passkey authentication
    ///
    /// # Errors
    /// Returns error if trying to enable passkey when no public key is configured
    pub fn set_passkey_enabled(&mut self, enabled: bool) -> Result<()> {
        if enabled && self.passkey_pubkey.is_none() {
            return Err(anyhow!(
                "Cannot enable passkey: no passkey public key configured"
            ));
        }
        self.passkey_enabled = enabled;
        Ok(())
    }

    /// Check if passkey authentication is enabled
    pub fn is_passkey_enabled(&self) -> bool {
        self.passkey_enabled
    }

    /// Check if passkey is configured (has public key)
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
        bincode::deserialize(&data).map_err(|e| anyhow!("[-] Wallet::try_from(): {:?}", e))
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        // Zero out sensitive data
        self.entropy.iter_mut().for_each(|x| *x = 0);
        if let Some(ref mut pubkey) = self.passkey_pubkey {
            pubkey.iter_mut().for_each(|x| *x = 0);
        }
    }
}
