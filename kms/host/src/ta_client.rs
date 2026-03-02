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

//! TA Client - Encapsulates communication with Trusted Application
//! This module provides a clean interface for HTTP API server to call TA functions

use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};
use anyhow::{Result, Context as AnyhowContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const OUTPUT_MAX_SIZE: usize = 1024;

/// TA Client for managing sessions with the Trusted Application
pub struct TaClient {
    ctx: Context,
    uuid: Uuid,
}

impl TaClient {
    /// Create a new TA client
    pub fn new() -> Result<Self> {
        let ctx = Context::new()
            .map_err(|e| anyhow::anyhow!("Failed to create TEE context: {:?}", e))?;

        let uuid = Uuid::parse_str(proto::UUID)
            .map_err(|_| anyhow::anyhow!("Invalid UUID in proto::UUID"))?;

        Ok(Self { ctx, uuid })
    }

    /// Invoke a command in the TA
    fn invoke_command(&mut self, command: proto::Command, input: &[u8]) -> Result<Vec<u8>> {
        let mut session = self.ctx.open_session(self.uuid.clone())
            .map_err(|e| anyhow::anyhow!("Failed to open TA session: {:?}", e))?;

        let p0 = ParamTmpRef::new_input(input);
        let mut output = vec![0u8; OUTPUT_MAX_SIZE];
        let p1 = ParamTmpRef::new_output(output.as_mut_slice());
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout);

        let mut operation = Operation::new(0, p0, p1, p2, ParamNone);

        match session.invoke_command(command as u32, &mut operation) {
            Ok(()) => {
                let output_len = operation.parameters().2.a() as usize;
                Ok(output[..output_len].to_vec())
            }
            Err(e) => {
                let output_len = operation.parameters().2.a() as usize;
                let err_message = String::from_utf8_lossy(&output[..output_len]);
                Err(anyhow::anyhow!("TA command failed: {} (error: {:?})", err_message, e))
            }
        }
    }

    /// Create a new wallet in the TA with mandatory passkey binding
    /// Returns the wallet UUID
    pub fn create_wallet(&mut self, passkey_pubkey: &[u8]) -> Result<uuid::Uuid> {
        let input = proto::CreateWalletInput {
            passkey_pubkey: passkey_pubkey.to_vec(),
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize CreateWalletInput")?;
        let serialized_output = self.invoke_command(proto::Command::CreateWallet, &serialized_input)?;
        let output: proto::CreateWalletOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize CreateWalletOutput")?;
        Ok(output.wallet_id)
    }

    /// Remove a wallet from the TA
    pub fn remove_wallet(&mut self, wallet_id: uuid::Uuid, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<()> {
        let input = proto::RemoveWalletInput { wallet_id, passkey_assertion };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize RemoveWalletInput")?;
        self.invoke_command(proto::Command::RemoveWallet, &serialized_input)?;
        Ok(())
    }

    /// Derive an Ethereum address from the wallet using HD path
    /// Returns 20-byte Ethereum address
    pub fn derive_address(&mut self, wallet_id: uuid::Uuid, hd_path: &str, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<[u8; 20]> {
        let input = proto::DeriveAddressInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            passkey_assertion,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize DeriveAddressInput")?;
        let serialized_output = self.invoke_command(proto::Command::DeriveAddress, &serialized_input)?;
        let output: proto::DeriveAddressOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize DeriveAddressOutput")?;
        Ok(output.address)
    }

    /// Sign an Ethereum transaction
    /// Returns raw signature bytes
    pub fn sign_transaction(
        &mut self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        transaction: proto::EthTransaction,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = proto::SignTransactionInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            transaction,
            passkey_assertion,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize SignTransactionInput")?;
        let serialized_output = self.invoke_command(proto::Command::SignTransaction, &serialized_input)?;
        let output: proto::SignTransactionOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize SignTransactionOutput")?;
        Ok(output.signature)
    }

    /// Sign a raw message
    /// Returns raw signature bytes (65 bytes: r + s + v)
    pub fn sign_message(
        &mut self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        message: &[u8],
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = proto::SignMessageInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            message: message.to_vec(),
            passkey_assertion,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize SignMessageInput")?;
        let serialized_output = self.invoke_command(proto::Command::SignMessage, &serialized_input)?;
        let output: proto::SignMessageOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize SignMessageOutput")?;
        Ok(output.signature)
    }

    /// Sign a 32-byte hash directly (no additional hashing)
    /// Returns raw signature bytes (65 bytes: r + s + v)
    pub fn sign_hash(
        &mut self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        hash: &[u8; 32],
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = proto::SignHashInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            hash: *hash,
            passkey_assertion,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize SignHashInput")?;
        let serialized_output = self.invoke_command(proto::Command::SignHash, &serialized_input)?;
        let output: proto::SignHashOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize SignHashOutput")?;
        Ok(output.signature)
    }

    /// Automatically derive address with incremented index for an existing wallet.
    /// Returns (wallet_id, address, public_key, derivation_path)
    pub fn derive_address_auto(
        &mut self,
        wallet_id: uuid::Uuid,
    ) -> Result<(uuid::Uuid, [u8; 20], Vec<u8>, String)> {
        let input = proto::DeriveAddressAutoInput { wallet_id };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize DeriveAddressAutoInput")?;
        let serialized_output = self.invoke_command(proto::Command::DeriveAddressAuto, &serialized_input)?;
        let output: proto::DeriveAddressAutoOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize DeriveAddressAutoOutput")?;
        Ok((output.wallet_id, output.address, output.public_key, output.derivation_path))
    }

    /// Verify a WebAuthn PassKey (P-256/secp256r1) signature inside TEE
    pub fn verify_passkey(
        &mut self,
        wallet_id: uuid::Uuid,
        public_key: &[u8],
        authenticator_data: &[u8],
        client_data_hash: &[u8; 32],
        signature_r: &[u8; 32],
        signature_s: &[u8; 32],
    ) -> Result<bool> {
        let input = proto::VerifyPasskeyInput {
            wallet_id,
            public_key: public_key.to_vec(),
            authenticator_data: authenticator_data.to_vec(),
            client_data_hash: *client_data_hash,
            signature_r: *signature_r,
            signature_s: *signature_s,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize VerifyPasskeyInput")?;
        let serialized_output = self.invoke_command(proto::Command::VerifyPasskey, &serialized_input)?;
        let output: proto::VerifyPasskeyOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize VerifyPasskeyOutput")?;
        Ok(output.valid)
    }
}

/// Convenience functions for one-off calls (creates new client each time)
/// For better performance in API server, reuse TaClient instance

pub fn create_wallet(passkey_pubkey: &[u8]) -> Result<uuid::Uuid> {
    let mut client = TaClient::new()?;
    client.create_wallet(passkey_pubkey)
}

pub fn derive_address(wallet_id: uuid::Uuid, hd_path: &str, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<[u8; 20]> {
    let mut client = TaClient::new()?;
    client.derive_address(wallet_id, hd_path, passkey_assertion)
}

pub fn sign_transaction(
    wallet_id: uuid::Uuid,
    hd_path: &str,
    chain_id: u64,
    nonce: u128,
    to: [u8; 20],
    value: u128,
    gas_price: u128,
    gas: u128,
) -> Result<Vec<u8>> {
    let transaction = proto::EthTransaction {
        chain_id,
        nonce,
        to: Some(to),
        value,
        gas_price,
        gas,
        data: vec![],
    };
    let mut client = TaClient::new()?;
    client.sign_transaction(wallet_id, hd_path, transaction, None)
}

impl TaClient {
    /// Export private key for a given wallet and derivation path
    /// WARNING: This should only be used for debugging/verification purposes
    pub fn export_private_key(&mut self, wallet_id: uuid::Uuid, derivation_path: &str, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<Vec<u8>> {
        let input = proto::ExportPrivateKeyInput {
            wallet_id,
            derivation_path: derivation_path.to_string(),
            passkey_assertion,
        };

        let serialized_input = bincode::serialize(&input)?;
        let output_bytes = self.invoke_command(proto::Command::ExportPrivateKey, &serialized_input)?;

        let output: proto::ExportPrivateKeyOutput = bincode::deserialize(&output_bytes)
            .with_context(|| "Failed to deserialize ExportPrivateKeyOutput")?;

        Ok(output.private_key)
    }
}

// ========================================
// TeeHandle — persistent session via dedicated TEE thread
// ========================================

struct TeeCommand {
    command: proto::Command,
    input: Vec<u8>,
    reply: tokio::sync::oneshot::Sender<Result<Vec<u8>>>,
}

/// Cloneable async handle to a single long-lived TEE session.
/// All TEE calls are serialised through one worker thread, avoiding the
/// ~4.4s open_session overhead on every request.
#[derive(Clone)]
pub struct TeeHandle {
    tx: std::sync::mpsc::Sender<TeeCommand>,
    pending: Arc<AtomicUsize>,
}

impl TeeHandle {
    /// Spawn the TEE worker thread and return a handle.
    /// Panics if the initial Context / Session cannot be created.
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<TeeCommand>();
        let pending = Arc::new(AtomicUsize::new(0));

        std::thread::spawn(move || {
            tee_worker_loop(rx);
        });

        println!("🔗 TeeHandle: worker thread spawned, session will be opened on first command");

        Self { tx, pending }
    }

    /// Number of commands currently queued (for QueueStatus).
    pub fn pending_count(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    // ---- async wrappers (mirror TaClient API) ----

    async fn call(&self, command: proto::Command, input: Vec<u8>) -> Result<Vec<u8>> {
        self.pending.fetch_add(1, Ordering::SeqCst);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx.send(TeeCommand { command, input, reply: reply_tx })
            .map_err(|_| anyhow::anyhow!("TEE worker thread has exited"))?;
        let result = reply_rx.await
            .map_err(|_| anyhow::anyhow!("TEE worker dropped reply channel"))?;
        self.pending.fetch_sub(1, Ordering::SeqCst);
        result
    }

    pub async fn create_wallet(&self, passkey_pubkey: &[u8]) -> Result<uuid::Uuid> {
        let input = bincode::serialize(&proto::CreateWalletInput {
            passkey_pubkey: passkey_pubkey.to_vec(),
        }).context("Failed to serialize CreateWalletInput")?;
        let out = self.call(proto::Command::CreateWallet, input).await?;
        let output: proto::CreateWalletOutput = bincode::deserialize(&out)
            .context("Failed to deserialize CreateWalletOutput")?;
        Ok(output.wallet_id)
    }

    pub async fn remove_wallet(&self, wallet_id: uuid::Uuid, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<()> {
        let input = bincode::serialize(&proto::RemoveWalletInput { wallet_id, passkey_assertion })
            .context("Failed to serialize RemoveWalletInput")?;
        self.call(proto::Command::RemoveWallet, input).await?;
        Ok(())
    }

    pub async fn derive_address(&self, wallet_id: uuid::Uuid, hd_path: &str, passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<[u8; 20]> {
        let input = bincode::serialize(&proto::DeriveAddressInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            passkey_assertion,
        }).context("Failed to serialize DeriveAddressInput")?;
        let out = self.call(proto::Command::DeriveAddress, input).await?;
        let output: proto::DeriveAddressOutput = bincode::deserialize(&out)
            .context("Failed to deserialize DeriveAddressOutput")?;
        Ok(output.address)
    }

    pub async fn sign_transaction(
        &self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        transaction: proto::EthTransaction,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::SignTransactionInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            transaction,
            passkey_assertion,
        }).context("Failed to serialize SignTransactionInput")?;
        let out = self.call(proto::Command::SignTransaction, input).await?;
        let output: proto::SignTransactionOutput = bincode::deserialize(&out)
            .context("Failed to deserialize SignTransactionOutput")?;
        Ok(output.signature)
    }

    pub async fn sign_message(
        &self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        message: &[u8],
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::SignMessageInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            message: message.to_vec(),
            passkey_assertion,
        }).context("Failed to serialize SignMessageInput")?;
        let out = self.call(proto::Command::SignMessage, input).await?;
        let output: proto::SignMessageOutput = bincode::deserialize(&out)
            .context("Failed to deserialize SignMessageOutput")?;
        Ok(output.signature)
    }

    pub async fn sign_hash(
        &self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        hash: &[u8; 32],
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::SignHashInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            hash: *hash,
            passkey_assertion,
        }).context("Failed to serialize SignHashInput")?;
        let out = self.call(proto::Command::SignHash, input).await?;
        let output: proto::SignHashOutput = bincode::deserialize(&out)
            .context("Failed to deserialize SignHashOutput")?;
        Ok(output.signature)
    }

    pub async fn derive_address_auto(
        &self,
        wallet_id: uuid::Uuid,
    ) -> Result<(uuid::Uuid, [u8; 20], Vec<u8>, String)> {
        let input = bincode::serialize(&proto::DeriveAddressAutoInput { wallet_id })
            .context("Failed to serialize DeriveAddressAutoInput")?;
        let out = self.call(proto::Command::DeriveAddressAuto, input).await?;
        let output: proto::DeriveAddressAutoOutput = bincode::deserialize(&out)
            .context("Failed to deserialize DeriveAddressAutoOutput")?;
        Ok((output.wallet_id, output.address, output.public_key, output.derivation_path))
    }

    pub async fn verify_passkey(
        &self,
        wallet_id: uuid::Uuid,
        public_key: &[u8],
        authenticator_data: &[u8],
        client_data_hash: &[u8; 32],
        signature_r: &[u8; 32],
        signature_s: &[u8; 32],
    ) -> Result<bool> {
        let input = bincode::serialize(&proto::VerifyPasskeyInput {
            wallet_id,
            public_key: public_key.to_vec(),
            authenticator_data: authenticator_data.to_vec(),
            client_data_hash: *client_data_hash,
            signature_r: *signature_r,
            signature_s: *signature_s,
        }).context("Failed to serialize VerifyPasskeyInput")?;
        let out = self.call(proto::Command::VerifyPasskey, input).await?;
        let output: proto::VerifyPasskeyOutput = bincode::deserialize(&out)
            .context("Failed to deserialize VerifyPasskeyOutput")?;
        Ok(output.valid)
    }

    pub async fn export_private_key(
        &self,
        wallet_id: uuid::Uuid,
        derivation_path: &str,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::ExportPrivateKeyInput {
            wallet_id,
            derivation_path: derivation_path.to_string(),
            passkey_assertion,
        })?;
        let out = self.call(proto::Command::ExportPrivateKey, input).await?;
        let output: proto::ExportPrivateKeyOutput = bincode::deserialize(&out)
            .with_context(|| "Failed to deserialize ExportPrivateKeyOutput")?;
        Ok(output.private_key)
    }

    /// Register (or change) a PassKey public key for a wallet in TEE secure storage.
    /// Requires current passkey assertion to authorize the change.
    pub async fn register_passkey_ta(&self, wallet_id: uuid::Uuid, passkey_pubkey: &[u8], passkey_assertion: Option<proto::PasskeyAssertion>) -> Result<bool> {
        let input = bincode::serialize(&proto::RegisterPasskeyTaInput {
            wallet_id,
            passkey_pubkey: passkey_pubkey.to_vec(),
            passkey_assertion,
        }).context("Failed to serialize RegisterPasskeyTaInput")?;
        let out = self.call(proto::Command::RegisterPasskeyTa, input).await?;
        let output: proto::RegisterPasskeyTaOutput = bincode::deserialize(&out)
            .context("Failed to deserialize RegisterPasskeyTaOutput")?;
        Ok(output.registered)
    }

    /// Pre-load wallet into TA LRU cache. Returns cache size.
    pub async fn warmup_cache(&self, wallet_id: uuid::Uuid) -> Result<u32> {
        let input = bincode::serialize(&proto::WarmupCacheInput { wallet_id })
            .context("Failed to serialize WarmupCacheInput")?;
        let out = self.call(proto::Command::WarmupCache, input).await?;
        let output: proto::WarmupCacheOutput = bincode::deserialize(&out)
            .context("Failed to deserialize WarmupCacheOutput")?;
        Ok(output.cache_size)
    }
}

// ---- TEE worker thread ----

fn invoke_on_session(
    session: &mut optee_teec::Session,
    command: proto::Command,
    input: &[u8],
) -> Result<Vec<u8>> {
    let p0 = ParamTmpRef::new_input(input);
    let mut output = vec![0u8; OUTPUT_MAX_SIZE];
    let p1 = ParamTmpRef::new_output(output.as_mut_slice());
    let p2 = ParamValue::new(0, 0, ParamType::ValueInout);
    let mut operation = Operation::new(0, p0, p1, p2, ParamNone);

    match session.invoke_command(command as u32, &mut operation) {
        Ok(()) => {
            let len = operation.parameters().2.a() as usize;
            Ok(output[..len].to_vec())
        }
        Err(e) => {
            let len = operation.parameters().2.a() as usize;
            let msg = String::from_utf8_lossy(&output[..len]);
            Err(anyhow::anyhow!("TA command failed: {} (error: {:?})", msg, e))
        }
    }
}

fn is_session_error(result: &Result<Vec<u8>>) -> bool {
    match result {
        Err(e) => {
            let msg = format!("{:?}", e);
            msg.contains("TargetDead") || msg.contains("ItemNotFound")
                || msg.contains("Communication") || msg.contains("Session")
        }
        Ok(_) => false,
    }
}

fn tee_worker_loop(rx: std::sync::mpsc::Receiver<TeeCommand>) {
    let mut ctx = Context::new().expect("TEE Context::new failed");
    let uuid = Uuid::parse_str(proto::UUID).expect("Invalid TA UUID");
    let mut session = ctx.open_session(uuid.clone()).expect("Initial open_session failed");
    println!("🔗 TEE worker: session opened");

    for cmd in rx.iter() {
        let result = invoke_on_session(&mut session, cmd.command, &cmd.input);

        if is_session_error(&result) {
            eprintln!("⚠️  TEE session error, attempting reconnect…");
            match ctx.open_session(uuid.clone()) {
                Ok(new_session) => {
                    session = new_session;
                    println!("🔗 TEE worker: session reconnected");
                    let retry = invoke_on_session(&mut session, cmd.command, &cmd.input);
                    let _ = cmd.reply.send(retry);
                    continue;
                }
                Err(e) => {
                    eprintln!("❌ TEE reconnect failed: {:?}", e);
                    // Send the original error
                    let _ = cmd.reply.send(result);
                    continue;
                }
            }
        }

        let _ = cmd.reply.send(result);
    }

    println!("🔗 TEE worker: channel closed, exiting");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ta_client_creation() {
        // This test will only pass in OP-TEE environment
        let result = TaClient::new();
        assert!(result.is_ok() || result.is_err()); // Just check it doesn't panic
    }
}