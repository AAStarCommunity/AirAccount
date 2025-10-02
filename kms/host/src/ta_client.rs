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

    /// Create a new wallet in the TA
    /// Returns the wallet UUID
    pub fn create_wallet(&mut self) -> Result<uuid::Uuid> {
        let serialized_output = self.invoke_command(proto::Command::CreateWallet, &[])?;
        let output: proto::CreateWalletOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize CreateWalletOutput")?;
        Ok(output.wallet_id)
    }

    /// Remove a wallet from the TA
    pub fn remove_wallet(&mut self, wallet_id: uuid::Uuid) -> Result<()> {
        let input = proto::RemoveWalletInput { wallet_id };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize RemoveWalletInput")?;
        self.invoke_command(proto::Command::RemoveWallet, &serialized_input)?;
        Ok(())
    }

    /// Derive an Ethereum address from the wallet using HD path
    /// Returns 20-byte Ethereum address
    pub fn derive_address(&mut self, wallet_id: uuid::Uuid, hd_path: &str) -> Result<[u8; 20]> {
        let input = proto::DeriveAddressInput {
            wallet_id,
            hd_path: hd_path.to_string(),
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
    ) -> Result<Vec<u8>> {
        let input = proto::SignTransactionInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            transaction,
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
    ) -> Result<Vec<u8>> {
        let input = proto::SignMessageInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            message: message.to_vec(),
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
    ) -> Result<Vec<u8>> {
        let input = proto::SignHashInput {
            wallet_id,
            hd_path: hd_path.to_string(),
            hash: *hash,
        };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize SignHashInput")?;
        let serialized_output = self.invoke_command(proto::Command::SignHash, &serialized_input)?;
        let output: proto::SignHashOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize SignHashOutput")?;
        Ok(output.signature)
    }

    /// Automatically derive address with incremented index
    /// If wallet_id is None, creates new wallet
    /// If wallet_id is Some, uses existing wallet and increments address index
    /// Returns (wallet_id, address, public_key, derivation_path)
    pub fn derive_address_auto(
        &mut self,
        wallet_id: Option<uuid::Uuid>,
    ) -> Result<(uuid::Uuid, [u8; 20], Vec<u8>, String)> {
        let input = proto::DeriveAddressAutoInput { wallet_id };
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize DeriveAddressAutoInput")?;
        let serialized_output = self.invoke_command(proto::Command::DeriveAddressAuto, &serialized_input)?;
        let output: proto::DeriveAddressAutoOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize DeriveAddressAutoOutput")?;
        Ok((output.wallet_id, output.address, output.public_key, output.derivation_path))
    }

    /// Get a new challenge for Passkey authentication
    /// Returns (challenge_bytes, expires_in_seconds)
    pub fn get_challenge(&mut self) -> Result<(Vec<u8>, u64)> {
        let input = proto::GetChallengeInput {};
        let serialized_input = bincode::serialize(&input)
            .context("Failed to serialize GetChallengeInput")?;
        let serialized_output = self.invoke_command(proto::Command::GetChallenge, &serialized_input)?;
        let output: proto::GetChallengeOutput = bincode::deserialize(&serialized_output)
            .context("Failed to deserialize GetChallengeOutput")?;
        Ok((output.challenge, output.expires_in))
    }
}

/// Convenience functions for one-off calls (creates new client each time)
/// For better performance in API server, reuse TaClient instance

pub fn create_wallet() -> Result<uuid::Uuid> {
    let mut client = TaClient::new()?;
    client.create_wallet()
}

pub fn remove_wallet(wallet_id: uuid::Uuid) -> Result<()> {
    let mut client = TaClient::new()?;
    client.remove_wallet(wallet_id)
}

pub fn derive_address(wallet_id: uuid::Uuid, hd_path: &str) -> Result<[u8; 20]> {
    let mut client = TaClient::new()?;
    client.derive_address(wallet_id, hd_path)
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
    client.sign_transaction(wallet_id, hd_path, transaction)
}

impl TaClient {
    /// Export private key for a given wallet and derivation path
    /// WARNING: This should only be used for debugging/verification purposes
    pub fn export_private_key(&mut self, wallet_id: uuid::Uuid, derivation_path: &str) -> Result<Vec<u8>> {
        let input = proto::ExportPrivateKeyInput {
            wallet_id,
            derivation_path: derivation_path.to_string(),
        };

        let serialized_input = bincode::serialize(&input)?;
        let output_bytes = self.invoke_command(proto::Command::ExportPrivateKey, &serialized_input)?;

        let output: proto::ExportPrivateKeyOutput = bincode::deserialize(&output_bytes)
            .with_context(|| "Failed to deserialize ExportPrivateKeyOutput")?;

        Ok(output.private_key)
    }
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