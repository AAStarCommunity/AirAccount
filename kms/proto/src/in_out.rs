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

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateWalletInput {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateWalletOutput {
    pub wallet_id: Uuid,
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveWalletInput {
    pub wallet_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveWalletOutput {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeriveAddressInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignTransactionInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub transaction: EthTransaction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
}

// P-256 signature verification test
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestP256VerifyInput {
    pub pubkey_sec1: Vec<u8>,      // SEC1 encoded public key (65 bytes)
    pub message: Vec<u8>,           // Message that was signed
    pub signature_der: Vec<u8>,     // DER encoded signature
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestP256VerifyOutput {
    pub success: bool,
    pub error_msg: String,
}

// Export private key
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExportPrivateKeyInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExportPrivateKeyOutput {
    pub private_key: Vec<u8>,  // 32 bytes secp256k1 private key
}

// Get Challenge for Passkey authentication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetChallengeInput {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetChallengeOutput {
    pub challenge: Vec<u8>,    // 32 bytes random challenge
    pub expires_in: u64,        // seconds until expiration (180 = 3 minutes)
}

// Set Passkey Public Key
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetPasskeyPubkeyInput {
    pub wallet_id: Uuid,
    pub passkey_pubkey: Vec<u8>,  // SEC1 uncompressed P-256 public key (65 bytes)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetPasskeyPubkeyOutput {
    pub success: bool,
}

// Enable/Disable Passkey Authentication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetPasskeyEnabledInput {
    pub wallet_id: Uuid,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SetPasskeyEnabledOutput {
    pub success: bool,
}

// Sign Hash with optional Passkey verification
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignHashInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub hash: Vec<u8>,  // 32 bytes hash to sign
    /// Optional Passkey authentication (required if passkey_enabled = true)
    pub passkey_signature: Option<PasskeySignature>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasskeySignature {
    pub challenge: Vec<u8>,      // 32 bytes challenge from GetChallenge
    pub signature_der: Vec<u8>,  // DER encoded P-256 signature
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignHashOutput {
    pub signature: Vec<u8>,  // secp256k1 signature (65 bytes: r + s + v)
}
