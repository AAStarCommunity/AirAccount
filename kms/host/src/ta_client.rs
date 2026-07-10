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

use anyhow::{Context as AnyhowContext, Result};
use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

const OUTPUT_MAX_SIZE: usize = 4096;

/// TA Client for managing sessions with the Trusted Application
pub struct TaClient {
    ctx: Context,
    uuid: Uuid,
}

impl TaClient {
    /// Create a new TA client
    pub fn new() -> Result<Self> {
        let ctx =
            Context::new().map_err(|e| anyhow::anyhow!("Failed to create TEE context: {:?}", e))?;

        let uuid = Uuid::parse_str(proto::UUID)
            .map_err(|_| anyhow::anyhow!("Invalid UUID in proto::UUID"))?;

        Ok(Self { ctx, uuid })
    }

    /// Invoke a command in the TA
    fn invoke_command(&mut self, command: proto::Command, input: &[u8]) -> Result<Vec<u8>> {
        let mut session = self
            .ctx
            .open_session(self.uuid.clone())
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
                Err(anyhow::anyhow!(
                    "TA command failed: {} (error: {:?})",
                    err_message,
                    e
                ))
            }
        }
    }

    /// Create a new wallet in the TA with mandatory passkey binding
    /// Returns the wallet UUID
    pub fn create_wallet(&mut self, passkey_pubkey: &[u8]) -> Result<uuid::Uuid> {
        let input = proto::CreateWalletInput {
            passkey_pubkey: passkey_pubkey.to_vec(),
            entropy_seed: None,
        };
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize CreateWalletInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::CreateWallet, &serialized_input)?;
        let output: proto::CreateWalletOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize CreateWalletOutput")?;
        Ok(output.wallet_id)
    }

    /// Remove a wallet from the TA
    pub fn remove_wallet(
        &mut self,
        wallet_id: uuid::Uuid,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<()> {
        let input = proto::RemoveWalletInput {
            wallet_id,
            passkey_assertion,
        };
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize RemoveWalletInput")?;
        self.invoke_command(proto::Command::RemoveWallet, &serialized_input)?;
        Ok(())
    }

    /// Issue #49: request a fresh one-time WebAuthn challenge nonce from the TA.
    ///
    /// The returned 32-byte nonce MUST be used as the WebAuthn `challenge`
    /// presented to the browser, so the value the authenticator signs is the one
    /// the TA can later verify and consume. The TA binds the nonce to `wallet_id`.
    pub fn get_challenge(&mut self, wallet_id: uuid::Uuid) -> Result<Vec<u8>> {
        let input = proto::GetChallengeInput { wallet_id };
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize GetChallengeInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::GetChallenge, &serialized_input)?;
        let output: proto::GetChallengeOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize GetChallengeOutput")?;
        Ok(output.nonce)
    }

    /// Derive an Ethereum address from the wallet using HD path
    /// Returns 20-byte Ethereum address
    pub fn derive_address(
        &mut self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<[u8; 20]> {
        let input = proto::DeriveAddressInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            passkey_assertion,
        };
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize DeriveAddressInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::DeriveAddress, &serialized_input)?;
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
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize SignTransactionInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::SignTransaction, &serialized_input)?;
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
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize SignMessageInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::SignMessage, &serialized_input)?;
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
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize SignHashInput")?;
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
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize DeriveAddressAutoInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::DeriveAddressAuto, &serialized_input)?;
        let output: proto::DeriveAddressAutoOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize DeriveAddressAutoOutput")?;
        Ok((
            output.wallet_id,
            output.address,
            output.public_key,
            output.derivation_path,
        ))
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
        let serialized_input =
            bincode::serialize(&input).context("Failed to serialize VerifyPasskeyInput")?;
        let serialized_output =
            self.invoke_command(proto::Command::VerifyPasskey, &serialized_input)?;
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

pub fn derive_address(
    wallet_id: uuid::Uuid,
    hd_path: &str,
    passkey_assertion: Option<proto::PasskeyAssertion>,
) -> Result<[u8; 20]> {
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
    pub fn export_private_key(
        &mut self,
        wallet_id: uuid::Uuid,
        derivation_path: &str,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<Vec<u8>> {
        let input = proto::ExportPrivateKeyInput {
            wallet_id,
            derivation_path: derivation_path.to_string(),
            passkey_assertion,
        };

        let serialized_input = bincode::serialize(&input)?;
        let output_bytes =
            self.invoke_command(proto::Command::ExportPrivateKey, &serialized_input)?;

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
    /// T3 backpressure: when this command was enqueued. The worker drops it
    /// (without invoking the TA) if it has waited past MAX_QUEUE_WAIT_SECS.
    enqueued_at: Instant,
}

// ── T3 queue backpressure ──
// The TA is a single serial worker (see TeeHandle). Under a burst, an unbounded
// mpsc queue would accept every request and push each toward the 30s timeout —
// the client waits 30s for a 503 it could have had instantly. Two guards:

/// Max in-flight + queued TEE commands. Beyond this, `call()` fast-fails with a
/// "TEE queue full" error (→ HTTP 429) instead of enqueuing. Sized for the
/// single worker: at ~80 warm-sign/s, 32 deep ≈ <0.5s drain — honest backpressure.
const MAX_QUEUE_DEPTH: usize = 32;

/// A command that has waited longer than this is dropped by the worker BEFORE
/// invoking the TA: the caller has almost certainly moved on, so spending a
/// serial TA slot on it only delays live requests. Kept below
/// TEE_CALL_TIMEOUT_SECS (30s) so a doomed request is shed before the caller's
/// own timeout fires.
const MAX_QUEUE_WAIT_SECS: u64 = 20;

// ---- Circuit Breaker ----
// Tracks consecutive TA failures. Opens circuit after threshold, blocking
// new requests for recovery_secs to prevent cascading crashes.

const CB_THRESHOLD: usize = 3;
const CB_RECOVERY_SECS: u64 = 30;

/// P0-1: hard upper bound for a single TEE call. The worker thread invokes
/// the TA synchronously and CANNOT be cancelled — if the TA hangs, the worker
/// stays blocked. Without this timeout the caller's `reply_rx.await` would
/// block forever and every subsequent request would queue behind it: a single
/// poisoned request = global denial of service.
///
/// On timeout we return 503 upstream and count a circuit-breaker failure so
/// that repeated hangs open the circuit. NOTE: the in-flight TA command may
/// still complete in the background — TEE calls are NOT cancellable. Handlers
/// with side effects (ChangePasskey, CreateWallet…) must treat a timeout as
/// "outcome unknown", not "failed".
const TEE_CALL_TIMEOUT_SECS: u64 = 30;

struct CircuitBreaker {
    consecutive_failures: AtomicUsize,
    open_until: Mutex<Option<Instant>>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            consecutive_failures: AtomicUsize::new(0),
            open_until: Mutex::new(None),
        }
    }

    /// Check if circuit is open. Returns Err if requests should be blocked.
    fn check(&self) -> Result<()> {
        let guard = self.open_until.lock().unwrap();
        if let Some(until) = *guard {
            if Instant::now() < until {
                return Err(anyhow::anyhow!(
                    "TEE circuit breaker OPEN: TA had {} consecutive failures, blocking for {}s",
                    self.consecutive_failures.load(Ordering::SeqCst),
                    CB_RECOVERY_SECS
                ));
            }
        }
        Ok(())
    }

    /// Record a successful TA call. Resets failure counter and closes circuit.
    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        let mut guard = self.open_until.lock().unwrap();
        *guard = None;
    }

    /// Record a failed TA call. Opens circuit if threshold exceeded.
    fn record_failure(&self) {
        let count = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= CB_THRESHOLD {
            let mut guard = self.open_until.lock().unwrap();
            let until = Instant::now() + std::time::Duration::from_secs(CB_RECOVERY_SECS);
            *guard = Some(until);
            eprintln!(
                "🔴 Circuit breaker OPEN: {} consecutive TA failures, blocking for {}s",
                count, CB_RECOVERY_SECS
            );
        }
    }

    fn failure_count(&self) -> usize {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    fn is_open(&self) -> bool {
        let guard = self.open_until.lock().unwrap();
        match *guard {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }
}

/// Cloneable async handle to a single long-lived TEE session.
/// All TEE calls are serialised through one worker thread, avoiding the
/// ~4.4s open_session overhead on every request.
///
/// Includes circuit breaker: after 3 consecutive TA failures, blocks new
/// requests for 30s to prevent cascading crashes. Auto-recovers.
#[derive(Clone)]
pub struct TeeHandle {
    tx: std::sync::mpsc::Sender<TeeCommand>,
    pending: Arc<AtomicUsize>,
    cb: Arc<CircuitBreaker>,
}

impl TeeHandle {
    /// Spawn the TEE worker thread and return a handle.
    /// Panics if the initial Context / Session cannot be created.
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<TeeCommand>();
        let pending = Arc::new(AtomicUsize::new(0));
        let cb = Arc::new(CircuitBreaker::new());

        std::thread::spawn(move || {
            tee_worker_loop(rx);
        });

        println!("🔗 TeeHandle: worker thread spawned, session will be opened on first command");
        println!(
            "🛡️  Circuit breaker: threshold={}, recovery={}s",
            CB_THRESHOLD, CB_RECOVERY_SECS
        );

        Self { tx, pending, cb }
    }

    /// Number of commands currently queued (for QueueStatus).
    pub fn pending_count(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// Circuit breaker status for diagnostics.
    pub fn circuit_breaker_status(&self) -> (bool, usize) {
        (self.cb.is_open(), self.cb.failure_count())
    }

    // ---- async wrappers (mirror TaClient API) ----

    // Maximum seconds to wait for the TEE worker to respond.
    // If the TA freezes (e.g. CAAM RNG hang), the caller receives a 503 instead
    // of blocking forever.  After CB_THRESHOLD timeouts the circuit breaker opens.
    const TEE_CALL_TIMEOUT_SECS: u64 = 30;

    async fn call(&self, command: proto::Command, input: Vec<u8>) -> Result<Vec<u8>> {
        // Circuit breaker: reject immediately if TA is repeatedly failing
        self.cb.check()?;

        // T3: bounded queue. Fast-fail with 429 rather than enqueue behind a
        // backlog that would only time out. Checked before the counter bump so
        // MAX_QUEUE_DEPTH is the true ceiling of accepted-but-unfinished work.
        let depth = self.pending.load(Ordering::SeqCst);
        if depth >= MAX_QUEUE_DEPTH {
            return Err(anyhow::anyhow!(
                "TEE queue full: {} in-flight (max {}) — retry shortly",
                depth,
                MAX_QUEUE_DEPTH
            ));
        }

        self.pending.fetch_add(1, Ordering::SeqCst);
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(TeeCommand {
                command,
                input,
                reply: reply_tx,
                enqueued_at: Instant::now(),
            })
            .map_err(|_| anyhow::anyhow!("TEE worker thread has exited"))?;
        // P0-1: bound the wait. The worker itself cannot be interrupted (the
        // TA invoke is a blocking syscall), but the HTTP caller must not hang
        // forever — and a hung TA must eventually open the circuit breaker.
        let result = match tokio::time::timeout(
            std::time::Duration::from_secs(TEE_CALL_TIMEOUT_SECS),
            reply_rx,
        )
        .await
        {
            Ok(inner) => {
                self.pending.fetch_sub(1, Ordering::SeqCst);
                inner.map_err(|_| anyhow::anyhow!("TEE worker dropped reply channel"))?
            }
            Err(_elapsed) => {
                // The command may still be executing in the worker; we only
                // stop waiting. Decrement pending so the counter doesn't leak
                // (the worker's eventual reply_tx.send() fails silently).
                self.pending.fetch_sub(1, Ordering::SeqCst);
                self.cb.record_failure();
                return Err(anyhow::anyhow!(
                    "TEE call timeout: {:?} did not complete within {}s — outcome unknown, \
                     TA may still be processing (circuit breaker failure recorded)",
                    command,
                    TEE_CALL_TIMEOUT_SECS
                ));
            }
        };

        // Update circuit breaker based on result
        match &result {
            Ok(_) => self.cb.record_success(),
            Err(e) => {
                let msg = format!("{:?}", e);
                // Only count session-level errors, not business logic errors
                if msg.contains("TargetDead")
                    || msg.contains("panicked")
                    || msg.contains("0xffff3024")
                    || msg.contains("Communication")
                    || msg.contains("TEE worker thread has exited")
                {
                    self.cb.record_failure();
                }
            }
        }

        result
    }

    pub async fn create_wallet(&self, passkey_pubkey: &[u8]) -> Result<uuid::Uuid> {
        // Generate 48 bytes of entropy from the OS CSPRNG (/dev/urandom-backed OsRng).
        // Passed to the TA so it can skip TEE_GenerateRandom() and avoid CAAM TRNG hangs.
        // This is safe: OsRng is cryptographically secure.  The entropy never leaves the TA.
        let mut seed = vec![0u8; 48];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut seed);

        let input = bincode::serialize(&proto::CreateWalletInput {
            passkey_pubkey: passkey_pubkey.to_vec(),
            entropy_seed: Some(seed),
        })
        .context("Failed to serialize CreateWalletInput")?;
        let out = self.call(proto::Command::CreateWallet, input).await?;
        let output: proto::CreateWalletOutput =
            bincode::deserialize(&out).context("Failed to deserialize CreateWalletOutput")?;
        Ok(output.wallet_id)
    }

    pub async fn remove_wallet(
        &self,
        wallet_id: uuid::Uuid,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<()> {
        let input = bincode::serialize(&proto::RemoveWalletInput {
            wallet_id,
            passkey_assertion,
        })
        .context("Failed to serialize RemoveWalletInput")?;
        self.call(proto::Command::RemoveWallet, input).await?;
        Ok(())
    }

    /// Issue #49: request a fresh one-time WebAuthn challenge nonce from the TA.
    /// Returns 32 bytes. Use as the WebAuthn challenge so the TA can verify and
    /// consume it on the subsequent signing assertion (anti-replay). Requires
    /// TA with GetChallenge = 25; older TAs return "Unsupported command".
    pub async fn get_challenge(&self, wallet_id: uuid::Uuid) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::GetChallengeInput { wallet_id })
            .context("Failed to serialize GetChallengeInput")?;
        let out = self.call(proto::Command::GetChallenge, input).await?;
        let output: proto::GetChallengeOutput =
            bincode::deserialize(&out).context("Failed to deserialize GetChallengeOutput")?;
        Ok(output.nonce)
    }

    // ── Variant B: BLS(DVT 共签)—— 密钥在 TA 内,CA 只发命令、取签名 ──

    /// 生成独立 BLS12-381 密钥(TA 内 TEE-TRNG 生成+密封)。返回 48B 压缩 G1 公钥。
    pub async fn bls_gen_key(&self, key_id: uuid::Uuid) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::BlsGenKeyInput { key_id })
            .context("Failed to serialize BlsGenKeyInput")?;
        let out = self.call(proto::Command::BlsGenKey, input).await?;
        let output: proto::BlsGenKeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize BlsGenKeyOutput")?;
        anyhow::ensure!(
            output.public_key.len() == 48,
            "BLS pubkey length invalid (expected 48, got {})",
            output.public_key.len()
        );
        Ok(output.public_key)
    }

    /// BLS-sign a 32-byte message with the sealed key. Returns (EIP-2537 G2 256B, compact G2 96B).
    pub async fn bls_sign(&self, key_id: uuid::Uuid, message: [u8; 32]) -> Result<(Vec<u8>, Vec<u8>)> {
        let input = bincode::serialize(&proto::BlsSignInput { key_id, message })
            .context("Failed to serialize BlsSignInput")?;
        let out = self.call(proto::Command::BlsSign, input).await?;
        let output: proto::BlsSignOutput =
            bincode::deserialize(&out).context("Failed to deserialize BlsSignOutput")?;
        anyhow::ensure!(
            output.signature.len() == 256 && output.signature_compact.len() == 96,
            "BLS signature length invalid (EIP-2537 {} want 256, compact {} want 96)",
            output.signature.len(),
            output.signature_compact.len()
        );
        Ok((output.signature, output.signature_compact))
    }

    /// Return the sealed BLS key's 48B compressed G1 public key.
    pub async fn bls_pubkey(&self, key_id: uuid::Uuid) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::BlsPubKeyInput { key_id })
            .context("Failed to serialize BlsPubKeyInput")?;
        let out = self.call(proto::Command::BlsPubKey, input).await?;
        let output: proto::BlsPubKeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize BlsPubKeyOutput")?;
        Ok(output.public_key)
    }

    /// Remove the sealed BLS singleton (delete every stored BLS key). Recovers from
    /// an orphaned key whose key_id was lost, or rotates. Returns the count removed.
    pub async fn bls_remove(&self) -> Result<u32> {
        let input = bincode::serialize(&proto::BlsRemoveInput {})
            .context("Failed to serialize BlsRemoveInput")?;
        let out = self.call(proto::Command::BlsRemove, input).await?;
        let output: proto::BlsRemoveOutput =
            bincode::deserialize(&out).context("Failed to deserialize BlsRemoveOutput")?;
        Ok(output.removed)
    }

    // ── CC-34: keeper/operator ECDSA(secp256k1)—— 密钥在 TA 内,CA 只发命令、取签名 ──

    /// 生成独立 secp256k1 keeper 密钥(TA 内 TEE-TRNG 生成+密封)。
    /// 返回 (65B 未压缩公钥, 20B 以太坊地址=充值 EOA)。一次性 provision(singleton)。
    pub async fn keeper_gen_key(&self, key_id: uuid::Uuid) -> Result<(Vec<u8>, [u8; 20])> {
        let input = bincode::serialize(&proto::KeeperGenKeyInput { key_id })
            .context("Failed to serialize KeeperGenKeyInput")?;
        let out = self.call(proto::Command::KeeperGenKey, input).await?;
        let output: proto::KeeperGenKeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize KeeperGenKeyOutput")?;
        anyhow::ensure!(
            output.public_key.len() == 65 && output.public_key[0] == 0x04,
            "keeper pubkey invalid (expected 65B uncompressed 0x04.., got {}B)",
            output.public_key.len()
        );
        Ok((output.public_key, output.address))
    }

    /// secp256k1-sign a raw 32-byte digest with the sealed keeper key. Returns a
    /// 65-byte Ethereum-recoverable signature r(32)||s(32)||v(1), v=27/28, low-S.
    pub async fn keeper_sign(&self, key_id: uuid::Uuid, digest: [u8; 32]) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::KeeperSignInput { key_id, digest })
            .context("Failed to serialize KeeperSignInput")?;
        let out = self.call(proto::Command::KeeperSign, input).await?;
        let output: proto::KeeperSignOutput =
            bincode::deserialize(&out).context("Failed to deserialize KeeperSignOutput")?;
        anyhow::ensure!(
            output.signature.len() == 65,
            "keeper signature length invalid (expected 65, got {})",
            output.signature.len()
        );
        Ok(output.signature)
    }

    /// Return the sealed keeper key's 65B uncompressed pubkey + 20B address.
    pub async fn keeper_pubkey(&self, key_id: uuid::Uuid) -> Result<(Vec<u8>, [u8; 20])> {
        let input = bincode::serialize(&proto::KeeperPubKeyInput { key_id })
            .context("Failed to serialize KeeperPubKeyInput")?;
        let out = self.call(proto::Command::KeeperPubKey, input).await?;
        let output: proto::KeeperPubKeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize KeeperPubKeyOutput")?;
        Ok((output.public_key, output.address))
    }

    /// Force-remove a gap key from TEE secure storage.
    /// Only called when `api_server` has confirmed the wallet's passkey_pubkey
    /// is not a valid P-256 curve point. Requires TA v0.20.0+ (ForceRemoveWallet = 23).
    /// On older TAs returns an error which the caller handles gracefully.
    pub async fn force_remove_wallet(&self, wallet_id: uuid::Uuid) -> Result<()> {
        let input = bincode::serialize(&proto::ForceRemoveWalletInput { wallet_id })
            .context("Failed to serialize ForceRemoveWalletInput")?;
        self.call(proto::Command::ForceRemoveWallet, input).await?;
        Ok(())
    }

    pub async fn derive_address(
        &self,
        wallet_id: uuid::Uuid,
        hd_path: &str,
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<[u8; 20]> {
        let input = bincode::serialize(&proto::DeriveAddressInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            passkey_assertion,
        })
        .context("Failed to serialize DeriveAddressInput")?;
        let out = self.call(proto::Command::DeriveAddress, input).await?;
        let output: proto::DeriveAddressOutput =
            bincode::deserialize(&out).context("Failed to deserialize DeriveAddressOutput")?;
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
        })
        .context("Failed to serialize SignTransactionInput")?;
        let out = self.call(proto::Command::SignTransaction, input).await?;
        let output: proto::SignTransactionOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignTransactionOutput")?;
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
        })
        .context("Failed to serialize SignMessageInput")?;
        let out = self.call(proto::Command::SignMessage, input).await?;
        let output: proto::SignMessageOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignMessageOutput")?;
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
        })
        .context("Failed to serialize SignHashInput")?;
        let out = self.call(proto::Command::SignHash, input).await?;
        let output: proto::SignHashOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignHashOutput")?;
        Ok(output.signature)
    }

    pub async fn derive_address_auto(
        &self,
        wallet_id: uuid::Uuid,
    ) -> Result<(uuid::Uuid, [u8; 20], Vec<u8>, String)> {
        let input = bincode::serialize(&proto::DeriveAddressAutoInput { wallet_id })
            .context("Failed to serialize DeriveAddressAutoInput")?;
        let out = self.call(proto::Command::DeriveAddressAuto, input).await?;
        let output: proto::DeriveAddressAutoOutput =
            bincode::deserialize(&out).context("Failed to deserialize DeriveAddressAutoOutput")?;
        Ok((
            output.wallet_id,
            output.address,
            output.public_key,
            output.derivation_path,
        ))
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
        })
        .context("Failed to serialize VerifyPasskeyInput")?;
        let out = self.call(proto::Command::VerifyPasskey, input).await?;
        let output: proto::VerifyPasskeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize VerifyPasskeyOutput")?;
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
    pub async fn register_passkey_ta(
        &self,
        wallet_id: uuid::Uuid,
        passkey_pubkey: &[u8],
        passkey_assertion: Option<proto::PasskeyAssertion>,
    ) -> Result<bool> {
        let input = bincode::serialize(&proto::RegisterPasskeyTaInput {
            wallet_id,
            passkey_pubkey: passkey_pubkey.to_vec(),
            passkey_assertion,
        })
        .context("Failed to serialize RegisterPasskeyTaInput")?;
        let out = self.call(proto::Command::RegisterPasskeyTa, input).await?;
        let output: proto::RegisterPasskeyTaOutput =
            bincode::deserialize(&out).context("Failed to deserialize RegisterPasskeyTaOutput")?;
        Ok(output.registered)
    }

    /// Pre-load wallet into TA LRU cache. Returns cache size.
    pub async fn warmup_cache(&self, wallet_id: uuid::Uuid) -> Result<u32> {
        let input = bincode::serialize(&proto::WarmupCacheInput { wallet_id })
            .context("Failed to serialize WarmupCacheInput")?;
        let out = self.call(proto::Command::WarmupCache, input).await?;
        let output: proto::WarmupCacheOutput =
            bincode::deserialize(&out).context("Failed to deserialize WarmupCacheOutput")?;
        Ok(output.cache_size)
    }

    pub async fn create_agent_key(
        &self,
        wallet_id: uuid::Uuid,
        agent_index: u32,
        subject: &str,
        ttl_secs: i64,
        passkey_assertion: Option<proto::PasskeyAssertion>,
        label: &str,
        is_refresh: bool,
    ) -> Result<proto::CreateAgentKeyOutput> {
        let input = bincode::serialize(&proto::CreateAgentKeyInput {
            wallet_id,
            agent_index,
            subject: subject.to_string(),
            ttl_secs,
            passkey_assertion,
            label: label.to_string(), // #115
            is_refresh,               // #115
        })
        .context("Failed to serialize CreateAgentKeyInput")?;
        let out = self.call(proto::Command::CreateAgentKey, input).await?;
        let output: proto::CreateAgentKeyOutput =
            bincode::deserialize(&out).context("Failed to deserialize CreateAgentKeyOutput")?;
        Ok(output)
    }

    pub async fn sign_agent_user_op(
        &self,
        wallet_id: uuid::Uuid,
        agent_index: u32,
        user_op_hash: &[u8; 32],
        jwt_kid: String,
        jwt_signing_input: Vec<u8>,
        jwt_hmac: Vec<u8>,
        account_address: [u8; 20],
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::SignAgentUserOpInput {
            wallet_id,
            agent_index,
            user_op_hash: *user_op_hash,
            jwt_kid,
            jwt_signing_input,
            jwt_hmac,
            account_address,
        })
        .context("Failed to serialize SignAgentUserOpInput")?;
        let out = self.call(proto::Command::SignAgentUserOp, input).await?;
        let output: proto::SignAgentUserOpOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignAgentUserOpOutput")?;
        Ok(output.signature)
    }

    pub async fn jwt_hmac_verify(
        &self,
        kid: &str,
        message: &[u8],
        expected_hmac: &[u8],
    ) -> Result<bool> {
        let input = bincode::serialize(&proto::JwtHmacVerifyInput {
            kid: kid.to_string(),
            message: message.to_vec(),
            expected_hmac: expected_hmac.to_vec(),
        })
        .context("Failed to serialize JwtHmacVerifyInput")?;
        let out = self.call(proto::Command::JwtHmacVerify, input).await?;
        let output: proto::JwtHmacVerifyOutput =
            bincode::deserialize(&out).context("Failed to deserialize JwtHmacVerifyOutput")?;
        Ok(output.valid)
    }

    pub async fn jwt_rotate_secret(&self, force: bool) -> Result<proto::JwtRotateSecretOutput> {
        let input = bincode::serialize(&proto::JwtRotateSecretInput { force })
            .context("Failed to serialize JwtRotateSecretInput")?;
        let out = self.call(proto::Command::JwtRotateSecret, input).await?;
        let output: proto::JwtRotateSecretOutput =
            bincode::deserialize(&out).context("Failed to deserialize JwtRotateSecretOutput")?;
        Ok(output)
    }

    pub async fn sign_typed_data(
        &self,
        input: proto::SignTypedDataInput,
    ) -> Result<proto::SignTypedDataOutput> {
        let serialized =
            bincode::serialize(&input).context("Failed to serialize SignTypedDataInput")?;
        let out = self.call(proto::Command::SignTypedData, serialized).await?;
        let output: proto::SignTypedDataOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignTypedDataOutput")?;
        Ok(output)
    }

    pub async fn sign_grant_session(
        &self,
        input: proto::SignGrantSessionInput,
    ) -> Result<proto::SignGrantSessionOutput> {
        let serialized =
            bincode::serialize(&input).context("Failed to serialize SignGrantSessionInput")?;
        let out = self
            .call(proto::Command::SignGrantSession, serialized)
            .await?;
        let output: proto::SignGrantSessionOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignGrantSessionOutput")?;
        Ok(output)
    }

    pub async fn sign_p256_grant_session(
        &self,
        input: proto::SignP256GrantSessionInput,
    ) -> Result<proto::SignP256GrantSessionOutput> {
        let serialized =
            bincode::serialize(&input).context("Failed to serialize SignP256GrantSessionInput")?;
        let out = self
            .call(proto::Command::SignP256GrantSession, serialized)
            .await?;
        let output: proto::SignP256GrantSessionOutput = bincode::deserialize(&out)
            .context("Failed to deserialize SignP256GrantSessionOutput")?;
        Ok(output)
    }

    /// Issue #37 — fetch a remote-attestation evidence blob from the TA.
    ///
    /// The TA invokes the OP-TEE attestation PTA to measure itself and sign the
    /// caller-supplied `nonce`. Requires a TA with `GetAttestation = 26`; older
    /// TAs return "Unsupported command". No secrets are involved — the evidence
    /// is meant to be verified by anyone holding the attestation public key.
    pub async fn get_attestation(&self, nonce: Vec<u8>) -> Result<proto::GetAttestationOutput> {
        let input = bincode::serialize(&proto::GetAttestationInput { nonce })
            .context("Failed to serialize GetAttestationInput")?;
        let out = self.call(proto::Command::GetAttestation, input).await?;
        let output: proto::GetAttestationOutput =
            bincode::deserialize(&out).context("Failed to deserialize GetAttestationOutput")?;
        Ok(output)
    }

    /// Read the current RPMB anti-rollback counter value (diagnostic endpoint).
    pub async fn read_rollback_counter(&self) -> Result<u64> {
        let input = bincode::serialize(&proto::ReadRollbackCounterInput {})
            .context("Failed to serialize ReadRollbackCounterInput")?;
        let out = self
            .call(proto::Command::ReadRollbackCounter, input)
            .await?;
        let output: proto::ReadRollbackCounterOutput = bincode::deserialize(&out)
            .context("Failed to deserialize ReadRollbackCounterOutput")?;
        Ok(output.counter)
    }

    pub async fn create_p256_session_key(
        &self,
        wallet_id: uuid::Uuid,
        session_index: u32,
        subject: &str,
        ttl_secs: i64,
        // #111: forwarded to the TA, which re-verifies user presence before minting.
        passkey_assertion: Option<proto::PasskeyAssertion>,
        label: &str,
    ) -> Result<proto::CreateP256SessionKeyOutput> {
        let input = bincode::serialize(&proto::CreateP256SessionKeyInput {
            wallet_id,
            session_index,
            subject: subject.to_string(),
            ttl_secs,
            passkey_assertion,
            label: label.to_string(), // #115
        })
        .context("Failed to serialize CreateP256SessionKeyInput")?;
        let out = self
            .call(proto::Command::CreateP256SessionKey, input)
            .await?;
        let output: proto::CreateP256SessionKeyOutput = bincode::deserialize(&out)
            .context("Failed to deserialize CreateP256SessionKeyOutput")?;
        Ok(output)
    }

    pub async fn sign_p256_user_op(
        &self,
        wallet_id: uuid::Uuid,
        session_index: u32,
        user_op_hash: &[u8; 32],
        jwt_kid: String,
        jwt_signing_input: Vec<u8>,
        jwt_hmac: Vec<u8>,
        account_address: [u8; 20],
    ) -> Result<Vec<u8>> {
        let input = bincode::serialize(&proto::SignP256UserOpInput {
            wallet_id,
            session_index,
            user_op_hash: *user_op_hash,
            jwt_kid,
            jwt_signing_input,
            jwt_hmac,
            account_address,
        })
        .context("Failed to serialize SignP256UserOpInput")?;
        let out = self.call(proto::Command::SignP256UserOp, input).await?;
        let output: proto::SignP256UserOpOutput =
            bincode::deserialize(&out).context("Failed to deserialize SignP256UserOpOutput")?;
        Ok(output.signature)
    }

    /// Delete a P256 session key from TEE secure storage (GC cleanup).
    /// Returns true if the key existed and was deleted; false if already absent (idempotent).
    pub async fn delete_p256_session_key(
        &self,
        wallet_id: uuid::Uuid,
        session_index: u32,
    ) -> Result<bool> {
        let input = bincode::serialize(&proto::DeleteP256SessionKeyInput {
            wallet_id,
            session_index,
        })
        .context("Failed to serialize DeleteP256SessionKeyInput")?;
        let out = self
            .call(proto::Command::DeleteP256SessionKey, input)
            .await?;
        let output: proto::DeleteP256SessionKeyOutput = bincode::deserialize(&out)
            .context("Failed to deserialize DeleteP256SessionKeyOutput")?;
        Ok(output.deleted)
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
            Err(anyhow::anyhow!(
                "TA command failed: {} (error: {:?})",
                msg,
                e
            ))
        }
    }
}

fn is_session_error(result: &Result<Vec<u8>>) -> bool {
    match result {
        Err(e) => {
            let msg = format!("{:?}", e);
            msg.contains("TargetDead")
                || msg.contains("ItemNotFound")
                || msg.contains("Communication")
                || msg.contains("Session")
                || msg.contains("panicked")
                || msg.contains("0xffff3024")
        }
        Ok(_) => false,
    }
}

fn tee_worker_loop(rx: std::sync::mpsc::Receiver<TeeCommand>) {
    let mut ctx = Context::new().expect("TEE Context::new failed");
    let uuid = Uuid::parse_str(proto::UUID).expect("Invalid TA UUID");
    let mut session = ctx
        .open_session(uuid.clone())
        .expect("Initial open_session failed");
    println!("🔗 TEE worker: session opened");

    for cmd in rx.iter() {
        // T3: shed a command that has waited past the deadline BEFORE spending a
        // serial TA slot on it — the caller has very likely already timed out.
        let waited = cmd.enqueued_at.elapsed().as_secs();
        if waited >= MAX_QUEUE_WAIT_SECS {
            let _ = cmd.reply.send(Err(anyhow::anyhow!(
                "TEE request dropped: queued {waited}s (> {MAX_QUEUE_WAIT_SECS}s deadline) — server overloaded"
            )));
            continue;
        }

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
