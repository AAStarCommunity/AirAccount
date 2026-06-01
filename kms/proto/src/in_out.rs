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

/// WebAuthn PassKey assertion data — attached to sign/export/delete requests
/// for TA-level mandatory verification when a passkey is bound to the wallet.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PasskeyAssertion {
    /// authenticatorData from WebAuthn assertion
    pub authenticator_data: Vec<u8>,
    /// SHA-256(clientDataJSON) - 32 bytes
    pub client_data_hash: [u8; 32],
    /// ECDSA signature r component (32 bytes)
    pub signature_r: [u8; 32],
    /// ECDSA signature s component (32 bytes)
    pub signature_s: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateWalletInput {
    /// P-256 public key in uncompressed format (65 bytes: 0x04 || x || y)
    pub passkey_pubkey: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateWalletOutput {
    pub wallet_id: Uuid,
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RemoveWalletInput {
    pub wallet_id: Uuid,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RemoveWalletOutput {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DeriveAddressInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignTransactionInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub transaction: EthTransaction,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignMessageInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub message: Vec<u8>,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignMessageOutput {
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignHashInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    pub hash: [u8; 32],
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignHashOutput {
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DeriveAddressAutoInput {
    pub wallet_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DeriveAddressAutoOutput {
    pub wallet_id: Uuid,
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
    pub derivation_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ExportPrivateKeyInput {
    pub wallet_id: Uuid,
    pub derivation_path: String,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ExportPrivateKeyOutput {
    pub private_key: Vec<u8>, // 32 bytes
}

/// WebAuthn PassKey (P-256/secp256r1) ECDSA verification
/// TA verifies the passkey signature before allowing private key operations
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VerifyPasskeyInput {
    /// The wallet being accessed (for audit logging)
    pub wallet_id: Uuid,
    /// P-256 public key in uncompressed format (65 bytes: 0x04 || x || y)
    pub public_key: Vec<u8>,
    /// authenticatorData from WebAuthn assertion
    pub authenticator_data: Vec<u8>,
    /// SHA-256(clientDataJSON) - 32 bytes
    pub client_data_hash: [u8; 32],
    /// ECDSA signature r component (32 bytes)
    pub signature_r: [u8; 32],
    /// ECDSA signature s component (32 bytes)
    pub signature_s: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VerifyPasskeyOutput {
    pub valid: bool,
}

/// Register a PassKey public key to a wallet (stored in TEE secure storage).
/// Once registered, all sensitive operations require PassKey assertion.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RegisterPasskeyTaInput {
    pub wallet_id: Uuid,
    /// New P-256 public key in uncompressed format (65 bytes: 0x04 || x || y)
    pub passkey_pubkey: Vec<u8>,
    /// Current passkey assertion (required to change passkey)
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RegisterPasskeyTaOutput {
    pub registered: bool,
}

/// Pre-load wallet into TA memory cache (no crypto, just storage read + seed cache).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WarmupCacheInput {
    pub wallet_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WarmupCacheOutput {
    pub cached: bool,
    pub cache_size: u32,
}

// Agent Key Commands

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateAgentKeyInput {
    pub wallet_id: Uuid,
    pub agent_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateAgentKeyOutput {
    pub agent_address: [u8; 20],
    pub public_key_compressed: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignAgentUserOpInput {
    pub wallet_id: Uuid,
    pub agent_index: u32,
    pub user_op_hash: [u8; 32],
    /// JWT authorization proof verified inside TEE (defense-in-depth against compromised CA).
    /// Fields extracted from the agent Bearer JWT by the host before calling TA.
    pub jwt_kid: String,
    pub jwt_signing_input: Vec<u8>,  // b64url(header).b64url(payload) bytes
    pub jwt_hmac: Vec<u8>,           // 32 bytes — HMAC-SHA256 from JWT signature field
    /// Smart Account contract address that this session key is bound to.
    /// Embedded in the v0.17.2 signature wire format: [0x08][account(20)][key(20)][ECDSA(65)].
    /// Verified on-chain by SessionKeyValidator to prevent cross-account session-key abuse.
    pub account_address: [u8; 20],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignAgentUserOpOutput {
    pub signature: Vec<u8>,
}

// JWT HMAC Commands

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtHmacSignInput {
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtHmacSignOutput {
    pub hmac: [u8; 32],
    pub kid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtHmacVerifyInput {
    pub kid: String,
    pub message: Vec<u8>,
    pub expected_hmac: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtHmacVerifyOutput {
    pub valid: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtRotateSecretInput {
    pub force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtRotateSecretOutput {
    pub new_kid: String,
    pub retired_kid: Option<String>,
}

// Single-call payload signing — atomically picks current kid, builds header, signs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtSignPayloadInput {
    pub payload_b64: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JwtSignPayloadOutput {
    pub kid: String,
    pub header_b64: String,
    pub hmac: [u8; 32],
}

// EIP-712 Typed Data Signing

/// EIP-712 domain separator fields (all optional per spec)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Eip712Domain {
    pub name: Option<String>,
    pub version: Option<String>,
    pub chain_id: Option<u64>,
    pub verifying_contract: Option<[u8; 20]>,
}

/// A single field definition in an EIP-712 type
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Eip712TypeField {
    pub name: String,
    pub field_type: String,
}

/// A named struct type with its field definitions
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Eip712TypeDef {
    pub name: String,
    pub fields: Vec<Eip712TypeField>,
}

/// A typed value for EIP-712 message fields.
/// v0.18.1 scope: flat primitive types only (no array, no nested struct).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Eip712Value {
    Address([u8; 20]),
    /// Big-endian unsigned integer, 1–32 bytes (uint8 through uint256)
    Uint(Vec<u8>),
    Bytes32([u8; 32]),
    Bool(bool),
    Str(String),
    Bytes(Vec<u8>),
}

/// A named field-value pair in the EIP-712 message
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Eip712FieldValue {
    pub name: String,
    pub value: Eip712Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignTypedDataInput {
    pub wallet_id: Uuid,
    pub hd_path: String,
    /// EIP-712 domain separator
    pub domain: Eip712Domain,
    /// Primary type name (the type being signed)
    pub primary_type: String,
    /// All type definitions referenced (primary type + any referenced types)
    pub types: Vec<Eip712TypeDef>,
    /// The message values for the primary type
    pub message: Vec<Eip712FieldValue>,
    #[serde(default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignTypedDataOutput {
    /// 65 bytes: R(32) || S(32) || V(1), V normalized to 27/28
    pub signature: Vec<u8>,
}

// ── P256 Session Key (v0.18.1) ──
// Wire format: [0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)] = 149 bytes

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateP256SessionKeyInput {
    pub wallet_id: Uuid,
    pub session_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CreateP256SessionKeyOutput {
    /// P-256 public key X coordinate (32 bytes big-endian)
    pub pub_key_x: [u8; 32],
    /// P-256 public key Y coordinate (32 bytes big-endian)
    pub pub_key_y: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignP256UserOpInput {
    pub wallet_id: Uuid,
    pub session_index: u32,
    pub user_op_hash: [u8; 32],
    /// JWT kid for TA-side HMAC authorization check (defense-in-depth)
    pub jwt_kid: String,
    pub jwt_signing_input: Vec<u8>,
    pub jwt_hmac: Vec<u8>,
    /// ERC-4337 Smart Account address embedded in wire format to prevent cross-account abuse
    pub account_address: [u8; 20],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignP256UserOpOutput {
    /// 149 bytes: [0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)]
    pub signature: Vec<u8>,
}
