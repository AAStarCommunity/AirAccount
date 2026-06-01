// KMS API Server
// Real TA integration only - requires OP-TEE environment
// Deploy to QEMU for testing, production-ready architecture

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use warp::Filter;
use hex;
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use p256::EncodedPoint;

// Import from kms library and proto
use kms::ta_client::TeeHandle;
use kms::db::{AgentKeyRow, KmsDb, WalletRow};
use kms::rate_limit::RateLimiter;
use kms::webauthn;
use kms::agent_jwt;
use proto;

/// Estimated seconds per TEE operation with persistent session
const TEE_OP_ESTIMATE_SECS: u64 = 1;

// ========================================
// AWS KMS 兼容的数据结构
// ========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
    #[serde(rename = "Origin")]
    pub origin: String,
    /// P-256 PassKey public key in hex (0x04..., 65 bytes uncompressed) — mandatory
    #[serde(rename = "PasskeyPublicKey")]
    pub passkey_public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
    #[serde(rename = "Mnemonic")]
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DescribeKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DescribeKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListKeysRequest {
    #[serde(rename = "Limit", skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(rename = "Marker", skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListKeysResponse {
    #[serde(rename = "Keys")]
    pub keys: Vec<KeyListEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyListEntry {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "KeyArn")]
    pub key_arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "PublicKey", skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none")]
    pub derivation_path: Option<String>,
    #[serde(rename = "Arn")]
    pub arn: String,
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
    #[serde(rename = "Origin")]
    pub origin: String,
    #[serde(rename = "PasskeyPublicKey", skip_serializing_if = "Option::is_none")]
    pub passkey_public_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "DerivationPath")]
    pub derivation_path: String,
    /// Legacy: raw PassKey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveAddressResponse {
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignRequest {
    // New: Address-based lookup (priority)
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none", default)]
    pub address: Option<String>,
    // Old: KeyId + DerivationPath (backward compatibility)
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none", default)]
    pub derivation_path: Option<String>,
    // Transaction signing mode (original)
    #[serde(rename = "Transaction", skip_serializing_if = "Option::is_none", default)]
    pub transaction: Option<EthereumTransaction>,
    // Message signing mode (new)
    #[serde(rename = "Message", skip_serializing_if = "Option::is_none", default)]
    pub message: Option<String>,
    #[serde(rename = "SigningAlgorithm", skip_serializing_if = "Option::is_none", default)]
    pub signing_algorithm: Option<String>,
    /// Legacy: raw PassKey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignResponse {
    #[serde(rename = "Signature")]
    pub signature: String,
    #[serde(rename = "TransactionHash")]
    pub transaction_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignHashRequest {
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none", default)]
    pub address: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none", default)]
    pub derivation_path: Option<String>,
    #[serde(rename = "Hash")]
    pub hash: String,
    #[serde(rename = "SigningAlgorithm", skip_serializing_if = "Option::is_none", default)]
    pub signing_algorithm: Option<String>,
    /// Legacy: raw PassKey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignHashResponse {
    #[serde(rename = "Signature")]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "PendingWindowInDays", skip_serializing_if = "Option::is_none")]
    pub pending_window_in_days: Option<i32>,
    /// Legacy: raw PassKey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "DeletionDate")]
    pub deletion_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: String,
    #[serde(rename = "KeySpec")]
    pub key_spec: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthereumTransaction {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    pub nonce: u64,
    pub to: String,
    pub value: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    pub gas: u64,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyStatusResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Status")]
    pub status: String,  // "creating" | "deriving" | "ready" | "error"
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "PublicKey", skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none")]
    pub derivation_path: Option<String>,
    #[serde(rename = "Error", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueStatusResponse {
    pub queue_depth: usize,
    pub estimated_wait_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circuit_breaker_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_failures: Option<usize>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasskeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    /// New P-256 public key in uncompressed hex (0x04...)
    #[serde(rename = "PasskeyPublicKey")]
    pub passkey_public_key: String,
    /// Legacy: current passkey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasskeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Changed")]
    pub changed: bool,
}

/// WebAuthn assertion data attached to Sign/SignHash requests
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PasskeyAssertion {
    /// authenticatorData in hex
    #[serde(rename = "AuthenticatorData")]
    pub authenticator_data: String,
    /// SHA-256(clientDataJSON) in hex
    #[serde(rename = "ClientDataHash")]
    pub client_data_hash: String,
    /// ECDSA signature in hex (DER or r||s 64 bytes)
    #[serde(rename = "Signature")]
    pub signature: String,
}

/// WebAuthn ceremony-based assertion (from BeginAuthentication flow)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebAuthnAssertion {
    #[serde(rename = "ChallengeId")]
    pub challenge_id: String,
    #[serde(rename = "Credential")]
    pub credential: webauthn::AuthenticationResponseJSON,
}

// ========================================
// Agent Key Request/Response Structs
// ========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAgentKeyRequest {
    #[serde(rename = "humanKeyId")]
    pub human_key_id: String,
    #[serde(rename = "label", default)]
    pub label: String,
    #[serde(rename = "passkeyAssertion", skip_serializing_if = "Option::is_none")]
    pub passkey_assertion: Option<PasskeyAssertion>,
    #[serde(rename = "webAuthnAssertion", skip_serializing_if = "Option::is_none")]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAgentKeyResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "agentAddress")]
    pub agent_address: String,
    #[serde(rename = "derivationPath")]
    pub derivation_path: String,
    #[serde(rename = "agentCredential")]
    pub agent_credential: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignAgentRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "payload")]
    pub payload: String,
    #[serde(rename = "algorithm", default = "default_secp256k1")]
    pub algorithm: String,
    /// Smart Account contract address bound to this session key (v0.17.2+).
    /// Embedded in the 106-byte signature: [0x08][account(20)][key(20)][ECDSA(65)].
    /// Must be the ERC-4337 account that will call SessionKeyValidator.validateUserOp.
    #[serde(rename = "accountAddress")]
    pub account_address: String,
}

fn default_secp256k1() -> String { "secp256k1".to_string() }

#[derive(Debug, Serialize, Deserialize)]
pub struct SignAgentResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "agentAddress")]
    pub agent_address: String,
    #[serde(rename = "signature")]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshAgentCredentialRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "passkeyAssertion", skip_serializing_if = "Option::is_none")]
    pub passkey_assertion: Option<PasskeyAssertion>,
    #[serde(rename = "webAuthnAssertion", skip_serializing_if = "Option::is_none")]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeAgentCredentialRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "passkeyAssertion", skip_serializing_if = "Option::is_none")]
    pub passkey_assertion: Option<PasskeyAssertion>,
    #[serde(rename = "webAuthnAssertion", skip_serializing_if = "Option::is_none")]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeAgentCredentialResponse {
    pub success: bool,
    #[serde(rename = "revokedAt")]
    pub revoked_at: i64,
}

// ── EIP-712 SignTypedData ──

/// JSON representation of an EIP-712 domain separator.
/// All fields are optional per EIP-712 spec; only present fields contribute to the domain hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEip712Domain {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "chainId")]
    pub chain_id: Option<u64>,
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: Option<String>, // "0x..." 20-byte hex
}

/// JSON representation of a single field type definition.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEip712TypeField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

/// JSON representation of a type definition (name + ordered field list).
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEip712TypeDef {
    pub name: String,
    pub fields: Vec<JsonEip712TypeField>,
}

/// JSON representation of a typed field value.
/// The `value` carries a JSON value; its interpretation is driven by the declared field type.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonEip712FieldValue {
    pub name: String,
    pub value: serde_json::Value,
}

/// Request body for `POST /kms/SignTypedData`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataRequest {
    /// KMS key ID (wallet UUID)
    #[serde(rename = "keyId")]
    pub key_id: String,
    /// BIP-44 derivation path
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    /// EIP-712 domain separator
    pub domain: JsonEip712Domain,
    /// Name of the primary struct type being signed
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    /// All referenced type definitions
    pub types: Vec<JsonEip712TypeDef>,
    /// Field values for the primary type
    pub message: Vec<JsonEip712FieldValue>,
    #[serde(rename = "passkeyAssertion", default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

fn default_hd_path() -> String { "m/44'/60'/0'/0/0".to_string() }

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    /// Hex-encoded 65-byte ECDSA signature: R(32) || S(32) || V(1), V=27/28
    pub signature: String,
}

// ── P256 Session Key (v0.18.1) ──

/// POST /kms/create-p256-session-key
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateP256SessionKeyRequest {
    #[serde(rename = "humanKeyId")]
    pub human_key_id: String,
    #[serde(rename = "label", default)]
    pub label: String,
    #[serde(rename = "webAuthnAssertion", skip_serializing_if = "Option::is_none")]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateP256SessionKeyResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "pubKeyX")]
    pub pub_key_x: String,
    #[serde(rename = "pubKeyY")]
    pub pub_key_y: String,
    pub algorithm: String,
    #[serde(rename = "agentCredential")]
    pub agent_credential: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
}

/// POST /kms/revoke-p256-session-key
#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeP256SessionKeyRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "webAuthnAssertion", skip_serializing_if = "Option::is_none")]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeP256SessionKeyResponse {
    pub success: bool,
    #[serde(rename = "revokedAt")]
    pub revoked_at: i64,
}

/// POST /kms/sign-p256-user-op
#[derive(Debug, Serialize, Deserialize)]
pub struct SignP256UserOpRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "payload")]
    pub payload: String,
    /// ERC-4337 Smart Account contract address (embedded in the 149-byte signature)
    #[serde(rename = "accountAddress")]
    pub account_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignP256UserOpResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "pubKeyX")]
    pub pub_key_x: String,
    #[serde(rename = "pubKeyY")]
    pub pub_key_y: String,
    /// Hex-encoded 149-byte wire format: [0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)]
    pub signature: String,
}

/// Parse compound agent keyId "wallet_uuid:agent_index"
fn parse_agent_key_id(key_id: &str) -> Result<(Uuid, u32)> {
    let parts: Vec<&str> = key_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid agent keyId format (expected wallet_id:index): {}", key_id));
    }
    let wallet_id = Uuid::parse_str(parts[0])
        .map_err(|_| anyhow!("Invalid wallet_id in agent keyId: {}", parts[0]))?;
    let agent_index: u32 = parts[1].parse()
        .map_err(|_| anyhow!("Invalid agent_index in keyId: {}", parts[1]))?;
    Ok((wallet_id, agent_index))
}

/// Convert a JSON value to a proto::Eip712Value using the declared ABI type for guidance.
///
/// Supported types: address, uint*, int*, bytes32, bytes*, bool, string
fn json_to_eip712_value(json_val: &serde_json::Value, declared_type: &str) -> Result<proto::Eip712Value> {
    let t = declared_type.trim();
    if t == "address" {
        let s = json_val.as_str()
            .ok_or_else(|| anyhow!("address field must be a JSON string"))?;
        let bytes = hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid address hex '{}': {}", s, e))?;
        if bytes.len() != 20 {
            return Err(anyhow!("address must be 20 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&bytes);
        return Ok(proto::Eip712Value::Address(arr));
    }
    if t == "bool" {
        let b = json_val.as_bool()
            .ok_or_else(|| anyhow!("bool field must be a JSON boolean"))?;
        return Ok(proto::Eip712Value::Bool(b));
    }
    if t == "string" {
        let s = json_val.as_str()
            .ok_or_else(|| anyhow!("string field must be a JSON string"))?;
        return Ok(proto::Eip712Value::Str(s.to_string()));
    }
    if t == "bytes32" {
        let s = json_val.as_str()
            .ok_or_else(|| anyhow!("bytes32 field must be a JSON hex string"))?;
        let bytes = hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid bytes32 hex '{}': {}", s, e))?;
        if bytes.len() != 32 {
            return Err(anyhow!("bytes32 must be exactly 32 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(proto::Eip712Value::Bytes32(arr));
    }
    if t.starts_with("bytes") {
        let s = json_val.as_str()
            .ok_or_else(|| anyhow!("bytes field must be a JSON hex string"))?;
        let bytes = hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid bytes hex '{}': {}", s, e))?;
        return Ok(proto::Eip712Value::Bytes(bytes));
    }
    if t.starts_with("uint") || t.starts_with("int") {
        // Accept JSON number or decimal/hex string
        let be_bytes = if let Some(n) = json_val.as_u64() {
            n.to_be_bytes().to_vec()
        } else if let Some(s) = json_val.as_str() {
            let hex_str = s.trim_start_matches("0x");
            if s.starts_with("0x") {
                hex::decode(hex_str)
                    .map_err(|e| anyhow!("Invalid uint hex '{}': {}", s, e))?
            } else {
                // Decimal string — parse as u128 max (covers uint8–uint128)
                let n: u128 = s.parse()
                    .map_err(|_| anyhow!("Invalid uint decimal '{}'", s))?;
                n.to_be_bytes().to_vec()
            }
        } else {
            return Err(anyhow!("uint/int field must be number or string, got {:?}", json_val));
        };
        return Ok(proto::Eip712Value::Uint(be_bytes));
    }
    Err(anyhow!("Unsupported EIP-712 field type '{}' (v0.18.0 supports: address, bool, string, bytes, bytes32, uint*, int*)", t))
}

/// Parse DER-encoded ECDSA signature into (r, s) 32-byte arrays
fn parse_der_signature(der: &[u8]) -> Result<([u8; 32], [u8; 32])> {
    if der.len() < 8 || der[0] != 0x30 {
        return Err(anyhow!("Invalid DER signature: bad header"));
    }
    let total_len = der[1] as usize;
    if total_len > 0x7F {
        return Err(anyhow!("Invalid DER signature: long-form length not supported"));
    }
    if 2 + total_len > der.len() {
        return Err(anyhow!("Invalid DER signature: declared length {} exceeds data {}", total_len, der.len()));
    }
    let mut pos = 2;
    if pos >= der.len() || der[pos] != 0x02 {
        return Err(anyhow!("Invalid DER signature: expected INTEGER tag for r"));
    }
    pos += 1;
    if pos >= der.len() {
        return Err(anyhow!("Invalid DER signature: missing r_len byte"));
    }
    let r_len = der[pos] as usize;
    pos += 1;
    if pos + r_len > der.len() {
        return Err(anyhow!("Invalid DER signature: r overflows buffer (r_len={} pos={} len={})", r_len, pos, der.len()));
    }
    let r_raw = &der[pos..pos + r_len];
    pos += r_len;
    if pos >= der.len() || der[pos] != 0x02 {
        return Err(anyhow!("Invalid DER signature: expected INTEGER tag for s"));
    }
    pos += 1;
    if pos >= der.len() {
        return Err(anyhow!("Invalid DER signature: missing s_len byte"));
    }
    let s_len = der[pos] as usize;
    pos += 1;
    if pos + s_len > der.len() {
        return Err(anyhow!("Invalid DER signature: s overflows buffer (s_len={} pos={} len={})", s_len, pos, der.len()));
    }
    let s_raw = &der[pos..pos + s_len];

    // Pad/trim to 32 bytes (DER integers may have leading zero for sign)
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    if r_raw.len() > 32 {
        r.copy_from_slice(&r_raw[r_raw.len() - 32..]);
    } else {
        r[32 - r_raw.len()..].copy_from_slice(r_raw);
    }
    if s_raw.len() > 32 {
        s.copy_from_slice(&s_raw[s_raw.len() - 32..]);
    } else {
        s[32 - s_raw.len()..].copy_from_slice(s_raw);
    }
    Ok((r, s))
}

// ========================================
// KMS API Server
// ========================================

fn wallet_to_metadata(w: &WalletRow) -> KeyMetadata {
    let creation_date = w.created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now());
    KeyMetadata {
        key_id: w.key_id.clone(),
        address: w.address.clone(),
        public_key: w.public_key.clone(),
        derivation_path: w.derivation_path.clone(),
        arn: format!("arn:aws:kms:region:account:key/{}", w.key_id),
        creation_date,
        enabled: true,
        description: w.description.clone(),
        key_usage: w.key_usage.clone(),
        key_spec: w.key_spec.clone(),
        origin: w.origin.clone(),
        passkey_public_key: w.passkey_pubkey.clone(),
    }
}

pub struct KmsApiServer {
    db: KmsDb,
    tee: TeeHandle,
    rate_limiter: RateLimiter,
    agent_rate_limiter: RateLimiter,
    rp_name: String,
    rp_ids: Vec<String>,
    expected_origins: Vec<String>,
}

impl KmsApiServer {
    pub fn new(db: KmsDb) -> Self {
        let rp_ids: Vec<String> = std::env::var("KMS_RP_ID")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| vec!["aastar.io".to_string()]);
        let rp_name = std::env::var("KMS_RP_NAME").unwrap_or_else(|_| "AirAccount KMS".to_string());
        let expected_origins: Vec<String> = std::env::var("KMS_ORIGIN")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| vec![format!("https://{}", rp_ids[0])]);
        println!("🌐 Allowed origins: {:?}", expected_origins);
        println!("🔑 Allowed rpIds: {:?}", rp_ids);
        let rate_limiter = RateLimiter::from_env();
        println!("⏱️  Rate limiter: {}/min per API key", rate_limiter.limit());
        let agent_rl_limit = std::env::var("KMS_AGENT_RATE_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        let agent_rate_limiter = RateLimiter::new(agent_rl_limit);
        println!("⏱️  Agent rate limiter: {}/min per credential", agent_rl_limit);
        Self {
            db,
            tee: TeeHandle::new(),
            rate_limiter,
            agent_rate_limiter,
            rp_name,
            rp_ids,
            expected_origins,
        }
    }

    // ========================================
    // CA-side input validation (defense-in-depth)
    // Validates BEFORE sending to TA to prevent TA crashes from bad input.
    // ========================================

    /// Validate BIP-44 derivation path format.
    /// Accepted: m/44'/60'/0'/0/N where N is 0-9999.
    fn validate_derivation_path(path: &str) -> Result<()> {
        if path.len() > 64 {
            return Err(anyhow!("Derivation path too long: {} chars (max 64)", path.len()));
        }
        if !path.starts_with("m/") {
            return Err(anyhow!("Derivation path must start with 'm/': {}", path));
        }
        // Validate each component is a number with optional hardened marker
        for part in path[2..].split('/') {
            let num_str = part.trim_end_matches('\'');
            if num_str.parse::<u32>().is_err() {
                return Err(anyhow!("Invalid derivation path component '{}' in: {}", part, path));
            }
        }
        Ok(())
    }

    /// Validate wallet UUID format at CA layer.
    fn validate_key_id(key_id: &str) -> Result<Uuid> {
        Uuid::parse_str(key_id)
            .map_err(|_| anyhow!("Invalid KeyId format (expected UUID): {}", key_id))
    }

    /// Validate hex-encoded hash (must be exactly 32 bytes = 64 hex chars).
    fn parse_address_hex(addr: &str) -> Result<[u8; 20]> {
        let hex_str = addr.trim_start_matches("0x");
        let bytes = hex::decode(hex_str)
            .map_err(|e| anyhow!("Invalid address hex: {}", e))?;
        if bytes.len() != 20 {
            return Err(anyhow!("Address must be exactly 20 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    fn validate_hash_hex(hash: &str) -> Result<[u8; 32]> {
        let hex_str = hash.trim_start_matches("0x");
        let bytes = hex::decode(hex_str)
            .map_err(|e| anyhow!("Invalid hash hex: {}", e))?;
        if bytes.len() != 32 {
            return Err(anyhow!("Hash must be exactly 32 bytes, got {} bytes", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    /// Validate hex-encoded message (reasonable size limit for TA).
    fn validate_message(message: &str) -> Result<()> {
        let max_len = 64 * 1024; // 64KB
        if message.len() > max_len {
            return Err(anyhow!("Message too large: {} bytes (max {})", message.len(), max_len));
        }
        Ok(())
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("📝 KMS CreateKey API called");

        // Decode and validate passkey public key (mandatory)
        let pk_hex = req.passkey_public_key.trim_start_matches("0x");
        let passkey_pubkey = hex::decode(pk_hex)
            .map_err(|e| anyhow!("Invalid PasskeyPublicKey hex: {}", e))?;
        if passkey_pubkey.len() != 65 || passkey_pubkey[0] != 0x04 {
            return Err(anyhow!(
                "PasskeyPublicKey must be 65 bytes uncompressed (0x04||x||y), got {} bytes",
                passkey_pubkey.len()
            ));
        }

        let wallet_id = self.tee.create_wallet(&passkey_pubkey).await?;
        let now = Utc::now();

        let key_metadata = KeyMetadata {
            key_id: wallet_id.to_string(),
            address: None,
            public_key: None,
            derivation_path: None,
            arn: format!("arn:aws:kms:region:account:key/{}", wallet_id),
            creation_date: now,
            enabled: true,
            description: req.description.clone(),
            key_usage: req.key_usage.clone(),
            key_spec: req.key_spec.clone(),
            origin: req.origin.clone(),
            passkey_public_key: Some(req.passkey_public_key.clone()),
        };

        // Persist to DB
        self.db.insert_wallet(&WalletRow {
            key_id: wallet_id.to_string(),
            address: None,
            public_key: None,
            derivation_path: None,
            description: req.description,
            key_usage: req.key_usage,
            key_spec: req.key_spec,
            origin: req.origin,
            passkey_pubkey: Some(req.passkey_public_key),
            credential_id: None,
            sign_count: 0,
            status: "deriving".to_string(),
            error_msg: None,
            created_at: now.to_rfc3339(),
        })?;

        // Spawn background address derivation
        let db = self.db.clone();
        let tee = self.tee.clone();
        tokio::spawn(async move {
            match tee.derive_address_auto(wallet_id).await {
                Ok((_wid, address_bytes, public_key, derivation_path)) => {
                    let address_hex = format!("0x{}", hex::encode(&address_bytes));
                    let pubkey_hex = format!("0x{}", hex::encode(&public_key));
                    println!("✅ Background derivation done for {}: {}", wallet_id, address_hex);

                    let _ = db.update_wallet_derived(
                        &wallet_id.to_string(), &address_hex, &pubkey_hex, &derivation_path, "ready",
                    );
                    let _ = db.upsert_address(&address_hex, &wallet_id.to_string(), &derivation_path, Some(&pubkey_hex));
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    eprintln!("❌ Background derivation failed for {}: {}", wallet_id, err_msg);
                    let _ = db.update_wallet_status(&wallet_id.to_string(), "error", Some(&err_msg));
                }
            }
        });

        Ok(CreateKeyResponse {
            key_metadata,
            mnemonic: "[MNEMONIC_IN_SECURE_WORLD]".to_string(),
        })
    }

    pub async fn describe_key(&self, req: DescribeKeyRequest) -> Result<DescribeKeyResponse> {
        println!("📝 KMS DescribeKey API called for key: {}", req.key_id);

        let w = self.db.get_wallet(&req.key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        Ok(DescribeKeyResponse { key_metadata: wallet_to_metadata(&w) })
    }

    pub async fn list_keys(&self, _req: ListKeysRequest) -> Result<ListKeysResponse> {
        println!("📝 KMS ListKeys API called");

        let wallets = self.db.list_wallets()?;
        let keys = wallets.iter().map(|w| KeyListEntry {
            key_id: w.key_id.clone(),
            key_arn: format!("arn:aws:kms:region:account:key/{}", w.key_id),
        }).collect();

        Ok(ListKeysResponse { keys })
    }

    pub async fn key_status(&self, key_id: &str) -> Result<KeyStatusResponse> {
        let w = self.db.get_wallet(key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

        let (status, error) = if w.status.starts_with("error") {
            ("error", w.error_msg.clone())
        } else {
            (w.status.as_str(), None)
        };

        Ok(KeyStatusResponse {
            key_id: key_id.to_string(),
            status: status.to_string(),
            address: w.address,
            public_key: w.public_key,
            derivation_path: w.derivation_path,
            error,
        })
    }

    pub fn queue_status(&self) -> QueueStatusResponse {
        let depth = self.tee.pending_count();
        let (cb_open, cb_failures) = self.tee.circuit_breaker_status();
        QueueStatusResponse {
            queue_depth: depth,
            estimated_wait_seconds: depth as u64 * TEE_OP_ESTIMATE_SECS,
            circuit_breaker_open: Some(cb_open),
            consecutive_failures: Some(cb_failures),
        }
    }

    pub async fn change_passkey(&self, req: ChangePasskeyRequest) -> Result<ChangePasskeyResponse> {
        println!("📝 KMS ChangePasskey API called for key: {}", req.key_id);

        if !self.db.wallet_exists(&req.key_id)? {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }

        // Decode public key from hex
        let pubkey_hex = req.passkey_public_key.trim_start_matches("0x");
        let pubkey_bytes = hex::decode(pubkey_hex)
            .map_err(|e| anyhow!("Invalid passkey public key hex: {}", e))?;

        if pubkey_bytes.len() != 65 || pubkey_bytes[0] != 0x04 {
            return Err(anyhow!(
                "PassKey public key must be 65 bytes uncompressed (0x04 || x || y), got {} bytes",
                pubkey_bytes.len()
            ));
        }

        // Resolve current passkey assertion (WebAuthn or legacy hex)
        let passkey_assertion = self.resolve_passkey_assertion(
            &req.key_id, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;

        // Change passkey in TEE secure storage (TA verifies current passkey first)
        let wallet_uuid = uuid::Uuid::parse_str(&req.key_id)?;
        self.tee.register_passkey_ta(wallet_uuid, &pubkey_bytes, passkey_assertion).await?;

        // Update DB
        let new_pk = format!("0x{}", pubkey_hex);
        self.db.update_wallet_passkey(&req.key_id, &new_pk, None)?;

        Ok(ChangePasskeyResponse {
            key_id: req.key_id,
            changed: true,
        })
    }

    /// Parse API-layer PasskeyAssertion (hex strings) into proto::PasskeyAssertion (bytes).
    /// Returns None if no assertion provided — TA will decide whether to allow or reject.
    fn parse_passkey_assertion(passkey: Option<&PasskeyAssertion>) -> Result<Option<proto::PasskeyAssertion>> {
        let assertion = match passkey {
            Some(a) => a,
            None => return Ok(None),
        };

        let auth_data = hex::decode(assertion.authenticator_data.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid authenticator_data hex: {}", e))?;
        let cdh_bytes = hex::decode(assertion.client_data_hash.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid client_data_hash hex: {}", e))?;
        if cdh_bytes.len() != 32 {
            return Err(anyhow!("client_data_hash must be 32 bytes"));
        }
        let mut client_data_hash = [0u8; 32];
        client_data_hash.copy_from_slice(&cdh_bytes);

        let sig_bytes = hex::decode(assertion.signature.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid signature hex: {}", e))?;

        let (signature_r, signature_s) = if sig_bytes.len() == 64 {
            let mut r = [0u8; 32];
            let mut s = [0u8; 32];
            r.copy_from_slice(&sig_bytes[..32]);
            s.copy_from_slice(&sig_bytes[32..]);
            (r, s)
        } else {
            parse_der_signature(&sig_bytes)?
        };

        Ok(Some(proto::PasskeyAssertion {
            authenticator_data: auth_data,
            client_data_hash,
            signature_r,
            signature_s,
        }))
    }

    /// CA-side passkey pre-verification (mirrors TA logic).
    /// If pubkey_hex and assertion are both present, verify ECDSA P-256 signature
    /// before forwarding to TA queue. Returns Ok(()) on success or missing data.
    ///
    /// Verification: ECDSA_verify(pubkey, SHA256(auth_data || cdh), signature)
    /// Uses Verifier::verify(msg, sig) which internally hashes msg with SHA-256.
    fn verify_passkey_ca(pubkey_hex: &str, assertion: &proto::PasskeyAssertion) -> Result<()> {
        let pk_hex = pubkey_hex.trim_start_matches("0x");
        let pk_bytes = hex::decode(pk_hex)
            .map_err(|e| anyhow!("Invalid stored passkey pubkey hex: {}", e))?;

        let encoded_point = EncodedPoint::from_bytes(&pk_bytes)
            .map_err(|e| anyhow!("Invalid passkey public key point: {:?}", e))?;
        let verifying_key = VerifyingKey::from_encoded_point(&encoded_point)
            .map_err(|e| anyhow!("Failed to parse passkey verifying key: {:?}", e))?;

        // Concatenate raw message: auth_data || client_data_hash
        // Verifier::verify() internally computes SHA-256(msg) before ECDSA math,
        // matching TA's verify_digest(SHA256(auth_data || cdh), sig).
        let mut msg = Vec::with_capacity(assertion.authenticator_data.len() + 32);
        msg.extend_from_slice(&assertion.authenticator_data);
        msg.extend_from_slice(&assertion.client_data_hash);

        let signature = Signature::from_scalars(assertion.signature_r, assertion.signature_s)
            .map_err(|e| anyhow!("Invalid passkey signature: {:?}", e))?;

        verifying_key.verify(&msg, &signature)
            .map_err(|_| anyhow!("PassKey verification failed (CA pre-check)"))?;

        Ok(())
    }

    /// Pre-verify passkey at CA level if metadata has pubkey and assertion is present.
    /// Rejects bad signatures before they reach TA queue.
    async fn pre_verify_passkey(&self, key_id: &str, assertion: &Option<proto::PasskeyAssertion>) -> Result<()> {
        let pubkey_hex = self.db.get_wallet(key_id)?
            .and_then(|w| w.passkey_pubkey);

        if let (Some(ref pk), Some(ref a)) = (pubkey_hex, assertion) {
            Self::verify_passkey_ca(pk, a)?;
        }
        Ok(())
    }

    /// Resolve passkey assertion from either legacy hex format or WebAuthn ceremony.
    /// WebAuthn path: consume challenge, verify assertion, update sign_count, return proto assertion.
    /// Legacy path: parse hex assertion + CA pre-verify.
    /// Returns None if neither is provided.
    async fn resolve_passkey_assertion(
        &self,
        key_id: &str,
        raw: Option<&PasskeyAssertion>,
        wa: Option<&WebAuthnAssertion>,
    ) -> Result<Option<proto::PasskeyAssertion>> {
        if let Some(wa) = wa {
            // WebAuthn ceremony path
            let challenge_row = self.db.consume_challenge(&wa.challenge_id)?
                .ok_or_else(|| anyhow!("Challenge not found or expired: {}", wa.challenge_id))?;

            // challenge must be bound to this key
            if let Some(ref bound_key) = challenge_row.key_id {
                if bound_key != key_id {
                    return Err(anyhow!("Challenge bound to different key"));
                }
            }

            let w = self.db.get_wallet(key_id)?
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            let pubkey_hex = w.passkey_pubkey
                .ok_or_else(|| anyhow!("Wallet has no passkey public key"))?;
            let pk_bytes = hex::decode(pubkey_hex.trim_start_matches("0x"))
                .map_err(|e| anyhow!("Invalid stored passkey hex: {}", e))?;

            let verified = webauthn::verify_authentication_response(
                &wa.credential,
                &challenge_row.challenge,
                &self.expected_origins,
                &challenge_row.rp_id,
                &pk_bytes,
                w.sign_count,
            )?;

            // Update sign_count in DB
            let _ = self.db.update_wallet_sign_count(key_id, verified.new_counter);

            Ok(Some(verified.proto_assertion))
        } else if raw.is_some() {
            // Legacy hex path: DEPRECATED — raw ECDSA bytes with no challenge or origin binding.
            // Vulnerable to replay if an attacker captures a valid assertion.
            // Retained for backward compatibility only; new clients should use WebAuthn ceremony.
            eprintln!("⚠️  DEPRECATED: legacy passkey assertion (no challenge binding) for key_id={}. Migrate to WebAuthn ceremony.", key_id);
            let assertion = Self::parse_passkey_assertion(raw)?;
            self.pre_verify_passkey(key_id, &assertion).await?;
            Ok(assertion)
        } else {
            Ok(None)
        }
    }

    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("📝 KMS DeriveAddress API called for key: {}", req.key_id);

        // CA-side validation before TA call
        let wallet_uuid = Self::validate_key_id(&req.key_id)?;
        Self::validate_derivation_path(&req.derivation_path)?;

        if !self.db.wallet_exists(&req.key_id)? {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }
        let passkey_assertion = self.resolve_passkey_assertion(
            &req.key_id, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;
        let address_bytes = self.tee.derive_address(wallet_uuid, &req.derivation_path, passkey_assertion).await?;

        let address = format!("0x{}", hex::encode(&address_bytes));

        Ok(DeriveAddressResponse {
            address,
            public_key: "[PUBKEY_FROM_TA]".to_string(),
        })
    }

    pub async fn sign(&self, req: SignRequest) -> Result<SignResponse> {
        // CA-side validation: message size
        if let Some(ref msg) = req.message {
            Self::validate_message(msg)?;
        }

        // Resolve wallet_id and derivation_path (support both Address and KeyId modes)
        let (wallet_uuid, derivation_path) = if let Some(ref address) = req.address {
            println!("📝 KMS Sign API called with Address: {}", address);

            let row = self.db.lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found: {}", address))?;

            (Uuid::parse_str(&row.key_id)?, row.derivation_path)
        } else if let (Some(ref key_id), Some(ref path)) = (req.key_id.as_ref(), req.derivation_path.as_ref()) {
            println!("📝 KMS Sign API called with KeyId: {}, Path: {}", key_id, path);

            if !self.db.wallet_exists(key_id)? {
                return Err(anyhow!("Key not found: {}", key_id));
            }

            (Uuid::parse_str(key_id)?, path.to_string())
        } else {
            return Err(anyhow!(
                "Must provide either Address or (KeyId + DerivationPath)"
            ));
        };

        // Resolve passkey assertion (WebAuthn ceremony or legacy hex)
        let key_id_str = wallet_uuid.to_string();
        let passkey_assertion = self.resolve_passkey_assertion(
            &key_id_str, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;

        // Prepare sign payload
        let signature = if let Some(transaction) = req.transaction {
            println!("  📝 Transaction signing mode");
            let to_bytes = if transaction.to.starts_with("0x") {
                hex::decode(&transaction.to[2..])
            } else {
                hex::decode(&transaction.to)
            }?;
            if to_bytes.len() != 20 {
                return Err(anyhow!("Transaction.to must be 20 bytes (40 hex chars), got {} bytes", to_bytes.len()));
            }
            let mut to_array = [0u8; 20];
            to_array.copy_from_slice(&to_bytes);

            let data = if transaction.data.is_empty() {
                vec![]
            } else {
                hex::decode(&transaction.data.trim_start_matches("0x"))?
            };

            let eth_transaction = proto::EthTransaction {
                chain_id: transaction.chain_id,
                nonce: transaction.nonce as u128,
                to: Some(to_array),
                value: u128::from_str_radix(&transaction.value.trim_start_matches("0x"), 16)?,
                gas_price: u128::from_str_radix(&transaction.gas_price.trim_start_matches("0x"), 16)?,
                gas: transaction.gas as u128,
                data,
            };
            self.tee.sign_transaction(wallet_uuid, &derivation_path, eth_transaction, passkey_assertion.clone()).await?
        } else if let Some(message) = req.message {
            println!("  📝 Message signing mode");
            let message_bytes = if message.starts_with("0x") {
                hex::decode(&message[2..])?
            } else {
                base64::decode(&message).unwrap_or_else(|_| message.as_bytes().to_vec())
            };
            self.tee.sign_message(wallet_uuid, &derivation_path, &message_bytes, passkey_assertion).await?
        } else {
            return Err(anyhow!("Either Transaction or Message must be provided"));
        };

        Ok(SignResponse {
            signature: hex::encode(&signature),
            transaction_hash: "[TX_HASH_OR_MESSAGE_HASH]".to_string(),
        })
    }

    pub async fn sign_hash(&self, req: SignHashRequest) -> Result<SignHashResponse> {
        // CA-side validation: hash format
        let hash_array = Self::validate_hash_hex(&req.hash)?;

        // 支持三种方式:
        // 1. Address (优先级最高,从 DB 查找)
        // 2. KeyId + DerivationPath (手动指定路径)
        // 3. KeyId only (自动使用默认路径)
        let (wallet_uuid, derivation_path) = if let Some(address) = &req.address {
            println!("📝 KMS SignHash API called with Address: {}", address);

            let row = self.db.lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found: {}", address))?;

            (Self::validate_key_id(&row.key_id)?, row.derivation_path)
        } else if let Some(key_id) = &req.key_id {
            println!("📝 KMS SignHash API called with KeyId: {}", key_id);

            let w = self.db.get_wallet(key_id)?
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            let derivation_path = req.derivation_path
                .or(w.derivation_path)
                .ok_or_else(|| anyhow!("No derivation path available for this key"))?;

            (Self::validate_key_id(key_id)?, derivation_path)
        } else {
            return Err(anyhow!("Either KeyId or Address must be provided"));
        };

        // CA-side validation: derivation path
        Self::validate_derivation_path(&derivation_path)?;

        // Resolve passkey assertion (WebAuthn ceremony or legacy hex)
        let key_id_str = wallet_uuid.to_string();
        let passkey_assertion = self.resolve_passkey_assertion(
            &key_id_str, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;

        let signature = self.tee.sign_hash(wallet_uuid, &derivation_path, &hash_array, passkey_assertion).await?;

        Ok(SignHashResponse {
            signature: hex::encode(&signature),
        })
    }

    pub async fn get_public_key(&self, req: GetPublicKeyRequest) -> Result<GetPublicKeyResponse> {
        println!("📝 KMS GetPublicKey API called for key: {}", req.key_id);

        let w = self.db.get_wallet(&req.key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        Ok(GetPublicKeyResponse {
            key_id: req.key_id,
            public_key: w.public_key.unwrap_or_else(|| "[PUBLIC_KEY_PENDING]".to_string()),
            key_usage: w.key_usage,
            key_spec: w.key_spec,
        })
    }

    pub async fn delete_key(&self, req: DeleteKeyRequest) -> Result<DeleteKeyResponse> {
        println!("📝 KMS DeleteKey API called for key: {}", req.key_id);

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        let passkey_assertion = self.resolve_passkey_assertion(
            &req.key_id, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;
        self.tee.remove_wallet(wallet_uuid, passkey_assertion).await?;

        // Remove from DB (CASCADE deletes address_index entries)
        self.db.delete_wallet(&req.key_id)?;

        let days = req.pending_window_in_days.unwrap_or(7);
        let deletion_date = Utc::now() + chrono::Duration::days(days as i64);

        Ok(DeleteKeyResponse {
            key_id: req.key_id,
            deletion_date,
        })
    }

    // ── WebAuthn ceremonies ──

    /// Pick rpId from configured KMS_RP_ID list based on caller's HTTP Origin header.
    /// e.g. origin "http://localhost:5173" → matches "localhost" if in list.
    /// Falls back to first configured rpId.
    fn resolve_rp_id(&self, caller_origin: Option<&str>) -> String {
        if let Some(origin) = caller_origin {
            let host = origin
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .split(':')
                .next()
                .unwrap_or("");
            // Check if any configured rpId matches (exact or suffix)
            for rp in &self.rp_ids {
                if host == rp.as_str() || host.ends_with(&format!(".{}", rp)) {
                    return rp.clone();
                }
            }
        }
        self.rp_ids[0].clone()
    }

    pub async fn begin_registration(&self, req: webauthn::BeginRegistrationRequest, origin_header: Option<&str>) -> Result<webauthn::RegistrationOptionsResponse> {
        let user_name = req.user_name.as_deref().unwrap_or("wallet-user");
        let user_display = req.user_display_name.as_deref().unwrap_or("AirAccount Wallet");
        let rp_id = self.resolve_rp_id(origin_header);
        println!("🔑 WebAuthn rpId resolved: {} (from origin: {:?})", rp_id, req.origin);

        let (challenge_id, challenge_bytes, resp) = webauthn::generate_registration_options(
            &self.rp_name, &rp_id, user_name, user_display, vec![],
        );

        self.db.store_challenge(&challenge_id, &challenge_bytes, None, "registration", &rp_id, 300)?;

        // Stash description/key_usage/etc in challenge metadata (store as JSON in key_id field)
        let meta_json = serde_json::to_string(&serde_json::json!({
            "description": req.description.unwrap_or_default(),
            "key_usage": req.key_usage.unwrap_or_else(|| "SIGN_VERIFY".to_string()),
            "key_spec": req.key_spec.unwrap_or_else(|| "ECC_SECG_P256K1".to_string()),
            "origin": req.origin.unwrap_or_else(|| "EXTERNAL_KMS".to_string()),
        }))?;
        // Re-store with metadata in key_id field
        self.db.store_challenge(
            &format!("{}_meta", challenge_id),
            meta_json.as_bytes(), None, "registration_meta", &rp_id, 300,
        )?;

        println!("📝 WebAuthn BeginRegistration: challenge_id={}", challenge_id);
        Ok(resp)
    }

    pub async fn complete_registration(&self, req: webauthn::CompleteRegistrationRequest) -> Result<webauthn::CompleteRegistrationResponse> {
        // 1. Consume challenge
        let challenge_row = self.db.consume_challenge(&req.challenge_id)?
            .ok_or_else(|| anyhow!("Challenge not found or expired: {}", req.challenge_id))?;

        // 2. Load stashed metadata
        let meta_row = self.db.consume_challenge(&format!("{}_meta", req.challenge_id))?;
        let (description, key_usage, key_spec, origin) = if let Some(mr) = meta_row {
            let v: serde_json::Value = serde_json::from_slice(&mr.challenge).unwrap_or_default();
            (
                v["description"].as_str().unwrap_or("").to_string(),
                v["key_usage"].as_str().unwrap_or("SIGN_VERIFY").to_string(),
                v["key_spec"].as_str().unwrap_or("ECC_SECG_P256K1").to_string(),
                v["origin"].as_str().unwrap_or("EXTERNAL_KMS").to_string(),
            )
        } else {
            (
                req.description.unwrap_or_default(),
                req.key_usage.unwrap_or_else(|| "SIGN_VERIFY".to_string()),
                req.key_spec.unwrap_or_else(|| "ECC_SECG_P256K1".to_string()),
                req.origin.unwrap_or_else(|| "EXTERNAL_KMS".to_string()),
            )
        };

        // 3. Verify attestation (use rpId from stored challenge, not hardcoded)
        let rp_id = &challenge_row.rp_id;
        let verified = webauthn::verify_registration_response(
            &req.credential, &challenge_row.challenge, &self.expected_origins, rp_id,
        )?;

        println!("✅ WebAuthn registration verified, pubkey {} bytes, credential_id {} bytes",
            verified.public_key.len(), verified.credential_id.len());

        // 4. Create wallet in TA with extracted P-256 pubkey
        let wallet_id = self.tee.create_wallet(&verified.public_key).await?;
        let now = Utc::now();
        let credential_id_b64 = webauthn::b64url_encode(&verified.credential_id);
        let passkey_pubkey_hex = format!("0x{}", hex::encode(&verified.public_key));

        // 5. Persist to DB
        self.db.insert_wallet(&WalletRow {
            key_id: wallet_id.to_string(),
            address: None,
            public_key: None,
            derivation_path: None,
            description,
            key_usage,
            key_spec,
            origin,
            passkey_pubkey: Some(passkey_pubkey_hex),
            credential_id: Some(credential_id_b64.clone()),
            sign_count: verified.sign_count,
            status: "deriving".to_string(),
            error_msg: None,
            created_at: now.to_rfc3339(),
        })?;

        // 6. Spawn background address derivation
        let db = self.db.clone();
        let tee = self.tee.clone();
        tokio::spawn(async move {
            match tee.derive_address_auto(wallet_id).await {
                Ok((_wid, address_bytes, public_key, derivation_path)) => {
                    let address_hex = format!("0x{}", hex::encode(&address_bytes));
                    let pubkey_hex = format!("0x{}", hex::encode(&public_key));
                    println!("✅ Background derivation done for {}: {}", wallet_id, address_hex);
                    let _ = db.update_wallet_derived(
                        &wallet_id.to_string(), &address_hex, &pubkey_hex, &derivation_path, "ready",
                    );
                    let _ = db.upsert_address(&address_hex, &wallet_id.to_string(), &derivation_path, Some(&pubkey_hex));
                }
                Err(e) => {
                    eprintln!("❌ Background derivation failed for {}: {}", wallet_id, e);
                    let _ = db.update_wallet_status(&wallet_id.to_string(), "error", Some(&e.to_string()));
                }
            }
        });

        Ok(webauthn::CompleteRegistrationResponse {
            key_id: wallet_id.to_string(),
            credential_id: credential_id_b64,
            status: "deriving".to_string(),
        })
    }

    pub async fn begin_authentication(&self, req: webauthn::BeginAuthenticationRequest, origin_header: Option<&str>) -> Result<webauthn::AuthenticationOptionsResponse> {
        // Resolve key_id from KeyId or Address
        let key_id = if let Some(ref kid) = req.key_id {
            kid.clone()
        } else if let Some(ref addr) = req.address {
            let row = self.db.lookup_address(addr)?
                .ok_or_else(|| anyhow!("Address not found: {}", addr))?;
            row.key_id
        } else {
            return Err(anyhow!("Must provide either KeyId or Address"));
        };

        let w = self.db.get_wallet(&key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

        let allow_credentials = if let Some(ref cid) = w.credential_id {
            vec![webauthn::CredentialDescriptor {
                id: cid.clone(),
                type_: "public-key".to_string(),
                transports: Some(vec!["internal".to_string(), "hybrid".to_string()]),
            }]
        } else {
            vec![]
        };

        let rp_id = self.resolve_rp_id(origin_header);
        let (challenge_id, challenge_bytes, resp) = webauthn::generate_authentication_options(
            &rp_id, allow_credentials,
        );

        self.db.store_challenge(&challenge_id, &challenge_bytes, Some(&key_id), "authentication", &rp_id, 300)?;

        println!("📝 WebAuthn BeginAuthentication: challenge_id={}, key_id={}", challenge_id, key_id);
        Ok(resp)
    }

    // ========================================
    // Agent Key methods
    // ========================================

    pub async fn create_agent_key(&self, req: CreateAgentKeyRequest) -> Result<CreateAgentKeyResponse> {
        let wallet_id = Self::validate_key_id(&req.human_key_id)?;

        // Verify human wallet exists
        let _wallet = self.db.get_wallet(&req.human_key_id)?
            .ok_or_else(|| anyhow!("Human wallet not found: {}", req.human_key_id))?;

        // Agent operations MUST use WebAuthn ceremony (challenge-based) to prevent replay attacks.
        // Legacy raw passkey assertions lack challenge/origin binding and can be replayed.
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!("create-agent-key requires WebAuthn ceremony (BeginAuthentication flow). Legacy passkey assertions are not accepted for agent operations."));
        }
        let assertion = self.resolve_passkey_assertion(
            &req.human_key_id,
            None,  // reject legacy path
            req.webauthn_assertion.as_ref(),
        ).await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to create agent key"));
        }

        // Atomically allocate the next agent_index (MAX+1 in a single lock acquire).
        // Avoids the race between count() and insert() that could yield duplicate indices.
        let agent_index = self.db.next_agent_index_for_wallet(&req.human_key_id)?;

        // Derive agent key in TEE (secp256k1, BIP44 m/44'/60'/0'/1/<index>)
        let tee_result = self.tee.create_agent_key(wallet_id, agent_index).await?;
        let agent_address = format!("0x{}", hex::encode(&tee_result.agent_address));
        let pubkey_hex = hex::encode(&tee_result.public_key_compressed);
        let derivation_path = format!("m/44'/60'/0'/1/{}", agent_index);

        // Issue JWT credential (3-day TTL), EIP-191 wrapping done inside TEE
        let (jwt, expires_at) = agent_jwt::issue_credential(
            &self.tee,
            &req.human_key_id,
            wallet_id,
            agent_index,
            &agent_address,
            3 * 24 * 3600,
        ).await?;

        // Store in DB — only credential_hash, never the full JWT
        let now = Utc::now().to_rfc3339();
        let cred_hash = agent_jwt::credential_hash(&jwt);
        self.db.insert_agent_key(&AgentKeyRow {
            wallet_id: req.human_key_id.clone(),
            agent_index,
            human_id: req.human_key_id.clone(),
            agent_address: agent_address.clone(),
            public_key_compressed: pubkey_hex,
            credential_hash: Some(cred_hash),
            credential_jwt: None,
            credential_expires_at: Some(expires_at),
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
            revoked_at: None,
        })?;

        let key_id = format!("{}:{}", req.human_key_id, agent_index);
        println!("✅ CreateAgentKey: wallet={} idx={} addr={}", req.human_key_id, agent_index, agent_address);

        Ok(CreateAgentKeyResponse {
            key_id,
            agent_address,
            derivation_path,
            agent_credential: jwt,
            expires_at,
        })
    }

    pub async fn sign_agent(&self, bearer_jwt: String, req: SignAgentRequest) -> Result<SignAgentResponse> {
        // Verify JWT via TEE HMAC
        let payload = agent_jwt::verify_credential(&self.tee, &bearer_jwt).await
            .map_err(|e| anyhow!("Invalid agent credential: {}", e))?;

        // Validate keyId matches JWT payload
        let (wallet_uuid, agent_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();
        if payload.wallet_id != wallet_id_str || payload.agent_index != agent_index {
            return Err(anyhow!("keyId does not match agent credential"));
        }

        // Per-credential rate limit (design §2.2): prevents single compromised key from
        // flooding TEE signing. Keyed by wallet_id/agent_index — independent of global API key limit.
        let cred_rl_key = format!("{}/{}", wallet_id_str, agent_index);
        self.agent_rate_limiter.check(&cred_rl_key)
            .map_err(|limit| anyhow!("Per-credential rate limit exceeded ({}/min). Retry after 60s.", limit))?;

        // Check agent key is active in DB + credential_hash matches
        let agent_key = self.db.get_agent_key(&wallet_id_str, agent_index)?
            .ok_or_else(|| anyhow!("Agent key not found: {}", req.key_id))?;
        if agent_key.status != "active" {
            return Err(anyhow!("Agent key is revoked"));
        }
        let current_hash = agent_jwt::credential_hash(&bearer_jwt);
        if agent_key.credential_hash.as_deref() != Some(current_hash.as_str()) {
            return Err(anyhow!("Agent credential has been superseded or revoked"));
        }

        // Validate userOpHash (exactly 32 bytes)
        let user_op_hash = Self::validate_hash_hex(&req.payload)?;

        // Parse accountAddress — must be a 20-byte hex string (with or without 0x prefix)
        let account_address = Self::parse_address_hex(&req.account_address)
            .map_err(|e| anyhow!("Invalid accountAddress: {}", e))?;

        // Extract JWT proof for TA-side authorization (defense-in-depth)
        let (jwt_kid, jwt_signing_input, jwt_hmac) = agent_jwt::extract_signing_proof(&bearer_jwt)
            .map_err(|e| anyhow!("Failed to extract JWT proof: {}", e))?;

        // Sign in TEE — TA re-verifies JWT HMAC before signing; EIP-191 inside TEE; V=27/28
        // Returns 106-byte v0.17.2 format: [0x08][account(20)][key(20)][ECDSA(65)]
        let sig_bytes = self.tee.sign_agent_user_op(
            wallet_uuid, agent_index, &user_op_hash,
            jwt_kid, jwt_signing_input, jwt_hmac,
            account_address,
        ).await?;

        println!("✅ SignAgent: wallet={} idx={} addr={}", wallet_id_str, agent_index, agent_key.agent_address);

        Ok(SignAgentResponse {
            key_id: req.key_id,
            agent_address: agent_key.agent_address,
            signature: format!("0x{}", hex::encode(&sig_bytes)),
        })
    }

    pub async fn sign_typed_data(&self, req: SignTypedDataRequest) -> Result<SignTypedDataResponse> {
        let wallet_id = Self::validate_key_id(&req.key_id)?;

        // Passkey verification (optional — wallet may or may not have one bound)
        let passkey_assertion = Self::parse_passkey_assertion(req.passkey_assertion.as_ref())?;

        // Convert domain verifyingContract from hex string to [u8; 20]
        let verifying_contract = match &req.domain.verifying_contract {
            Some(hex_str) => {
                let bytes = hex::decode(hex_str.trim_start_matches("0x"))
                    .map_err(|e| anyhow!("Invalid verifyingContract hex: {}", e))?;
                if bytes.len() != 20 {
                    return Err(anyhow!("verifyingContract must be 20 bytes, got {}", bytes.len()));
                }
                let mut arr = [0u8; 20];
                arr.copy_from_slice(&bytes);
                Some(arr)
            }
            None => None,
        };

        // Convert JSON types to proto types
        let domain = proto::Eip712Domain {
            name: req.domain.name.clone(),
            version: req.domain.version.clone(),
            chain_id: req.domain.chain_id,
            verifying_contract,
        };

        let types: Vec<proto::Eip712TypeDef> = req.types.iter().map(|td| proto::Eip712TypeDef {
            name: td.name.clone(),
            fields: td.fields.iter().map(|f| proto::Eip712TypeField {
                name: f.name.clone(),
                field_type: f.field_type.clone(),
            }).collect(),
        }).collect();

        // Find the primary type definition to help with value conversion
        let primary_type_def = req.types.iter()
            .find(|td| td.name == req.primary_type)
            .ok_or_else(|| anyhow!("Primary type '{}' not in types list", req.primary_type))?;

        // Convert JSON field values to proto Eip712Value using declared field types for guidance
        let message = req.message.iter().map(|fv| {
            let declared_type = primary_type_def.fields.iter()
                .find(|f| f.name == fv.name)
                .map(|f| f.field_type.as_str())
                .unwrap_or("");
            let value = json_to_eip712_value(&fv.value, declared_type)?;
            Ok(proto::Eip712FieldValue { name: fv.name.clone(), value })
        }).collect::<Result<Vec<_>>>()?;

        let ta_input = proto::SignTypedDataInput {
            wallet_id,
            hd_path: req.hd_path.clone(),
            domain,
            primary_type: req.primary_type.clone(),
            types,
            message,
            passkey_assertion,
        };

        let output = self.tee.sign_typed_data(ta_input).await?;

        println!("✅ SignTypedData: keyId={} primaryType={}", req.key_id, req.primary_type);
        Ok(SignTypedDataResponse {
            key_id: req.key_id,
            signature: format!("0x{}", hex::encode(&output.signature)),
        })
    }

    pub async fn refresh_agent_credential(&self, bearer_jwt: String, req: RefreshAgentCredentialRequest) -> Result<CreateAgentKeyResponse> {
        // Verify current JWT is still valid
        let payload = agent_jwt::verify_credential(&self.tee, &bearer_jwt).await
            .map_err(|e| anyhow!("Invalid agent credential: {}", e))?;

        let (wallet_uuid, agent_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();
        if payload.wallet_id != wallet_id_str || payload.agent_index != agent_index {
            return Err(anyhow!("keyId does not match agent credential"));
        }

        // Require WebAuthn ceremony for replay protection (no legacy passkey path)
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!("refresh-agent-credential requires WebAuthn ceremony. Legacy passkey assertions are not accepted for agent operations."));
        }
        let assertion = self.resolve_passkey_assertion(
            &wallet_id_str,
            None,  // reject legacy path
            req.webauthn_assertion.as_ref(),
        ).await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to refresh agent credential"));
        }

        // Check agent key is active
        let agent_key = self.db.get_agent_key(&wallet_id_str, agent_index)?
            .ok_or_else(|| anyhow!("Agent key not found: {}", req.key_id))?;
        if agent_key.status != "active" {
            return Err(anyhow!("Agent key is revoked"));
        }

        // Issue new JWT (old JWT is implicitly superseded via credential_hash update)
        let (new_jwt, expires_at) = agent_jwt::issue_credential(
            &self.tee,
            &wallet_id_str,
            wallet_uuid,
            agent_index,
            &agent_key.agent_address,
            3 * 24 * 3600,
        ).await?;

        let cred_hash = agent_jwt::credential_hash(&new_jwt);
        self.db.update_agent_credential(&wallet_id_str, agent_index, &cred_hash, expires_at)?;

        let derivation_path = format!("m/44'/60'/0'/1/{}", agent_index);
        println!("✅ RefreshAgentCredential: wallet={} idx={}", wallet_id_str, agent_index);

        Ok(CreateAgentKeyResponse {
            key_id: req.key_id,
            agent_address: agent_key.agent_address,
            derivation_path,
            agent_credential: new_jwt,
            expires_at,
        })
    }

    pub async fn revoke_agent_credential(&self, req: RevokeAgentCredentialRequest) -> Result<RevokeAgentCredentialResponse> {
        let (wallet_uuid, agent_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();

        // Require WebAuthn ceremony for replay protection (no legacy passkey path)
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!("revoke-agent-credential requires WebAuthn ceremony. Legacy passkey assertions are not accepted for agent operations."));
        }
        let assertion = self.resolve_passkey_assertion(
            &wallet_id_str,
            None,  // reject legacy path
            req.webauthn_assertion.as_ref(),
        ).await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to revoke agent key"));
        }

        // Revoke in DB
        let revoked = self.db.revoke_agent_key(&wallet_id_str, agent_index)?;
        if !revoked {
            return Err(anyhow!("Agent key not found or already revoked: {}", req.key_id));
        }

        let revoked_at = Utc::now().timestamp();
        println!("✅ RevokeAgentCredential: wallet={} idx={}", wallet_id_str, agent_index);

        Ok(RevokeAgentCredentialResponse { success: true, revoked_at })
    }

    /// Lazy GC: delete expired P256 session keys for a wallet from TEE and DB.
    /// Called silently on create/sign/revoke — errors are logged, never propagated.
    ///
    /// `exclude_session_index`: skip this index (used during sign to avoid GC-ing the key
    /// being signed, in case the JWT expires in the same second as the GC runs).
    ///
    /// Grace window: 60s added to account for clock skew between host and credential issuer,
    /// so keys are not GC'd until at least 60s past credential_expires_at.
    async fn gc_expired_p256_session_keys(
        &self,
        wallet_id_str: &str,
        wallet_uuid: uuid::Uuid,
        exclude_session_index: Option<u32>,
    ) {
        // 60-second grace window guards against host-clock drift causing premature deletion.
        let gc_cutoff = Utc::now().timestamp() - 60;
        let expired = match self.db.list_expired_p256_session_keys(
            wallet_id_str,
            gc_cutoff,
            exclude_session_index,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("⚠️  P256 GC: DB query failed for {}: {}", wallet_id_str, e);
                return;
            }
        };
        for session_index in expired {
            // TEE-first: delete key from secure storage. If TEE returns not-found, treat as
            // already deleted (idempotent). DB update follows only on TEE success.
            match self.tee.delete_p256_session_key(wallet_uuid, session_index).await {
                Ok(deleted) => {
                    if let Err(e) = self.db.mark_p256_session_key_gc(wallet_id_str, session_index) {
                        // DB update failed: ghost-active row remains. Next GC will retry TEE
                        // (returns deleted=false) and retry DB. Self-healing on next trigger.
                        eprintln!(
                            "⚠️  P256 GC: TEE deleted={} but DB update failed for {}:{}: {}",
                            deleted, wallet_id_str, session_index, e
                        );
                    } else {
                        println!(
                            "🗑️  P256 GC: cleaned expired key {}:{} (tee_deleted={})",
                            wallet_id_str, session_index, deleted
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  P256 GC: TEE delete failed for {}:{}: {}",
                        wallet_id_str, session_index, e
                    );
                }
            }
        }
    }

    pub async fn create_p256_session_key(
        &self,
        req: CreateP256SessionKeyRequest,
    ) -> Result<CreateP256SessionKeyResponse> {
        let wallet_id = Self::validate_key_id(&req.human_key_id)?;

        // Verify human wallet exists
        let _wallet = self
            .db
            .get_wallet(&req.human_key_id)?
            .ok_or_else(|| anyhow!("Human wallet not found: {}", req.human_key_id))?;

        // P256 session key creation requires WebAuthn ceremony (replay-protected)
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!(
                "create-p256-session-key requires WebAuthn ceremony. \
                 Legacy passkey assertions are not accepted."
            ));
        }
        let assertion = self
            .resolve_passkey_assertion(&req.human_key_id, None, req.webauthn_assertion.as_ref())
            .await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to create P256 session key"));
        }

        // Lazy GC: clean up expired P256 session keys for this wallet before creating a new one.
        self.gc_expired_p256_session_keys(&req.human_key_id, wallet_id, None).await;

        // Enforce max-2 active P256 session keys per wallet (current + one during rotation).
        // Checked after GC so freshly expired keys are excluded from the count.
        let active_count = self
            .db
            .count_active_p256_session_keys(&req.human_key_id, Utc::now().timestamp())?;
        if active_count >= 2 {
            return Err(anyhow!(
                "Wallet already has {} active P256 session keys (max 2). \
                 Revoke an existing key before creating a new one.",
                active_count
            ));
        }

        // Atomically allocate next session_index via INSERT-SELECT in a single lock.
        // This prevents TOCTOU: the 'pending' row reserves the index before the slow TA call.
        let session_index = self
            .db
            .allocate_p256_session_key_pending(&req.human_key_id, &req.human_key_id)?;

        // Generate P256 key pair in TEE (may take ~seconds on Cortex-A7)
        let tee_result = match self
            .tee
            .create_p256_session_key(wallet_id, session_index)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // TA failed — delete the pending placeholder so the index is not leaked.
                let _ = self.db.delete_p256_session_key_pending(&req.human_key_id, session_index);
                return Err(e);
            }
        };
        let pub_key_x = hex::encode(&tee_result.pub_key_x);
        let pub_key_y = hex::encode(&tee_result.pub_key_y);

        // Issue JWT credential for this P256 session key (reuse agent JWT infrastructure)
        let key_id = format!("{}:{}", req.human_key_id, session_index);
        let (jwt, expires_at) = match agent_jwt::issue_credential(
            &self.tee,
            &req.human_key_id,
            wallet_id,
            session_index,
            &key_id,
            3 * 24 * 3600,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                // JWT issuance failed after TEE key was created: clean up both TEE key and DB
                // row to prevent an orphaned "ghost key" (HIGH-2 fix).
                let _ = self.tee.delete_p256_session_key(wallet_id, session_index).await;
                let _ = self.db.delete_p256_session_key_pending(&req.human_key_id, session_index);
                return Err(e);
            }
        };

        let cred_hash = agent_jwt::credential_hash(&jwt);
        // Activate the pending row: sets pub_key_x/y, credential_hash, status='active'
        if let Err(e) = self.db.activate_p256_session_key(
            &req.human_key_id,
            session_index,
            &pub_key_x,
            &pub_key_y,
            &cred_hash,
            expires_at,
        ) {
            // DB activation failed after TEE key and JWT were created: clean up TEE key to
            // prevent ghost key. DB pending row stays; GC cannot pick it up (NULL expiry)
            // so explicitly delete it here too.
            let _ = self.tee.delete_p256_session_key(wallet_id, session_index).await;
            let _ = self.db.delete_p256_session_key_pending(&req.human_key_id, session_index);
            return Err(e);
        }

        println!(
            "✅ CreateP256SessionKey: wallet={} idx={} x={}...",
            req.human_key_id,
            session_index,
            &pub_key_x[..8]
        );

        Ok(CreateP256SessionKeyResponse {
            key_id,
            pub_key_x,
            pub_key_y,
            algorithm: "p256".to_string(),
            agent_credential: jwt,
            expires_at,
        })
    }

    pub async fn sign_p256_user_op(
        &self,
        bearer_jwt: String,
        req: SignP256UserOpRequest,
    ) -> Result<SignP256UserOpResponse> {
        // Verify JWT via TEE HMAC
        let payload = agent_jwt::verify_credential(&self.tee, &bearer_jwt)
            .await
            .map_err(|e| anyhow!("Invalid P256 session credential: {}", e))?;

        // Validate keyId matches JWT payload
        let (wallet_uuid, session_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();
        if payload.wallet_id != wallet_id_str || payload.agent_index != session_index {
            return Err(anyhow!("keyId does not match P256 session credential"));
        }

        // Lazy GC: clean up other expired P256 session keys for this wallet.
        // Exclude the current session_index to avoid GC-ing the key being signed.
        self.gc_expired_p256_session_keys(&wallet_id_str, wallet_uuid, Some(session_index)).await;

        // Per-credential rate limit
        let cred_rl_key = format!("p256/{}/{}", wallet_id_str, session_index);
        self.agent_rate_limiter
            .check(&cred_rl_key)
            .map_err(|limit| {
                anyhow!(
                    "Per-credential rate limit exceeded ({}/min). Retry after 60s.",
                    limit
                )
            })?;

        // Check session key is active and credential_hash matches
        let session_key = self
            .db
            .get_p256_session_key(&wallet_id_str, session_index)?
            .ok_or_else(|| anyhow!("P256 session key not found: {}", req.key_id))?;
        if session_key.status != "active" {
            return Err(anyhow!("P256 session key is revoked"));
        }
        let current_hash = agent_jwt::credential_hash(&bearer_jwt);
        if session_key.credential_hash.as_deref() != Some(current_hash.as_str()) {
            return Err(anyhow!("P256 session credential has been superseded or revoked"));
        }

        // Validate userOpHash (exactly 32 bytes)
        let user_op_hash = Self::validate_hash_hex(&req.payload)?;

        // Parse accountAddress
        let account_address = Self::parse_address_hex(&req.account_address)
            .map_err(|e| anyhow!("Invalid accountAddress: {}", e))?;

        // Extract JWT proof for TA-side authorization (defense-in-depth)
        let (jwt_kid, jwt_signing_input, jwt_hmac) =
            agent_jwt::extract_signing_proof(&bearer_jwt)
                .map_err(|e| anyhow!("Failed to extract JWT proof: {}", e))?;

        // Sign in TEE — 149-byte P256 format
        let sig_bytes = self
            .tee
            .sign_p256_user_op(
                wallet_uuid,
                session_index,
                &user_op_hash,
                jwt_kid,
                jwt_signing_input,
                jwt_hmac,
                account_address,
            )
            .await?;

        if sig_bytes.len() != 149 {
            return Err(anyhow!(
                "Unexpected P256 signature length: {} (expected 149)",
                sig_bytes.len()
            ));
        }

        println!(
            "✅ SignP256UserOp: wallet={} idx={}",
            wallet_id_str, session_index
        );

        Ok(SignP256UserOpResponse {
            key_id: req.key_id,
            pub_key_x: session_key.pub_key_x,
            pub_key_y: session_key.pub_key_y,
            signature: format!("0x{}", hex::encode(&sig_bytes)),
        })
    }

    pub async fn revoke_p256_session_key(
        &self,
        req: RevokeP256SessionKeyRequest,
    ) -> Result<RevokeP256SessionKeyResponse> {
        let (wallet_uuid, session_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();

        // Require WebAuthn ceremony for replay protection
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!(
                "revoke-p256-session-key requires WebAuthn ceremony. \
                 Legacy passkey assertions are not accepted."
            ));
        }
        let assertion = self
            .resolve_passkey_assertion(&wallet_id_str, None, req.webauthn_assertion.as_ref())
            .await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to revoke P256 session key"));
        }

        // Lazy GC: clean up other expired P256 session keys for this wallet.
        self.gc_expired_p256_session_keys(&wallet_id_str, wallet_uuid, None).await;

        let revoked = self
            .db
            .revoke_p256_session_key(&wallet_id_str, session_index)?;
        if !revoked {
            return Err(anyhow!(
                "P256 session key not found or already revoked: {}",
                req.key_id
            ));
        }

        let revoked_at = Utc::now().timestamp();
        println!(
            "✅ RevokeP256SessionKey: wallet={} idx={}",
            wallet_id_str, session_index
        );

        Ok(RevokeP256SessionKeyResponse {
            success: true,
            revoked_at,
        })
    }
}

// ========================================
// HTTP Server Routes
// ========================================

const KMS_VERSION: &str = "0.17.0";

fn render_stats_page(server: &KmsApiServer) -> String {
    let wallets = server.db.list_wallets().unwrap_or_default();
    let qs = server.queue_status();
    let tx = server.db.get_tx_stats().unwrap_or_default();
    let total = wallets.len();
    let with_addr = wallets.iter().filter(|w| w.address.is_some()).count();
    let with_pk = wallets.iter().filter(|w| w.passkey_pubkey.is_some()).count();
    let enabled = wallets.iter().filter(|w| w.status == "ready").count();
    let api_keys = server.db.list_api_keys().map(|v| v.len()).unwrap_or(0);

    let mut rows = String::new();
    for w in &wallets {
        let addr = if w.address.is_some() { "&#10003;" } else { "-" };
        let addr_cls = if w.address.is_some() { "ok" } else { "dim" };
        let pk = if w.passkey_pubkey.is_some() { "&#10003;" } else { "-" };
        let pk_cls = if w.passkey_pubkey.is_some() { "ok" } else { "dim" };
        let st_cls = if w.status == "ready" { "ok" } else { "warn" };
        let short_id = &w.key_id[..8.min(w.key_id.len())];
        let created = w.created_at.split('T').next().unwrap_or(&w.created_at);
        let masked_desc = if w.description.len() > 8 {
            format!("{}…", &w.description[..8])
        } else {
            w.description.clone()
        };
        rows.push_str(&format!(
            "<tr><td><code>{}&hellip;</code></td><td class=\"{addr_cls}\">{addr}</td><td class=\"{pk_cls}\">{pk}</td><td class=\"{st_cls}\">{}</td><td>{}</td><td>{created}</td><td>{}</td></tr>\n",
            short_id, w.status, w.sign_count, masked_desc
        ));
    }

    let cb = if qs.circuit_breaker_open.unwrap_or(false) { "OPEN" } else { "closed" };
    let cb_cls = if qs.circuit_breaker_open.unwrap_or(false) { "warn" } else { "ok" };
    let fails = qs.consecutive_failures.unwrap_or(0);
    let panic_cls = if tx.panic_count > 0 { "warn" } else { "ok" };
    let error_cls = if tx.error_count > 0 { "warn" } else { "ok" };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>KMS Stats</title>
<style>
  body {{ font-family: 'SF Mono','Menlo',monospace; background:#0d1117; color:#c9d1d9; max-width:960px; margin:0 auto; padding:24px; }}
  h1 {{ color:#58a6ff; font-size:1.3em; margin-bottom:4px; }}
  h2 {{ color:#8b949e; font-size:0.9em; margin:16px 0 8px; text-transform:uppercase; letter-spacing:.05em; }}
  .sub {{ color:#8b949e; font-size:0.85em; margin-bottom:24px; }}
  table {{ width:100%; border-collapse:collapse; margin:12px 0; }}
  th {{ text-align:left; color:#8b949e; font-size:0.8em; border-bottom:1px solid #30363d; padding:6px 10px; }}
  td {{ padding:6px 10px; border-bottom:1px solid #21262d; font-size:0.85em; }}
  .card {{ background:#161b22; border:1px solid #30363d; border-radius:6px; padding:16px; margin:12px 0; }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(130px,1fr)); gap:12px; }}
  .stat {{ text-align:center; }}
  .stat .val {{ font-size:1.5em; font-weight:bold; color:#58a6ff; }}
  .stat .lbl {{ font-size:0.72em; color:#8b949e; margin-top:2px; }}
  .ok {{ color:#3fb950; }}
  .warn {{ color:#d29922; }}
  .dim {{ color:#484f58; }}
  a {{ color:#58a6ff; text-decoration:none; }}
  a:hover {{ text-decoration:underline; }}
  .footer {{ margin-top:32px; color:#484f58; font-size:0.75em; text-align:center; }}
</style>
</head>
<body>
<h1>AirAccount KMS</h1>
<div class="sub">v{version} &middot; TA mode: real &middot; <a href="/test">Test UI</a> &middot; <a href="/health">Health</a></div>

<h2>Keys</h2>
<div class="card grid">
  <div class="stat"><div class="val">{total}</div><div class="lbl">Total Keys</div></div>
  <div class="stat"><div class="val ok">{enabled}</div><div class="lbl">Ready</div></div>
  <div class="stat"><div class="val">{with_addr}</div><div class="lbl">With Address</div></div>
  <div class="stat"><div class="val">{with_pk}</div><div class="lbl">With PassKey</div></div>
  <div class="stat"><div class="val">{api_keys}</div><div class="lbl">API Keys</div></div>
  <div class="stat"><div class="val">{queue}</div><div class="lbl">Queue Depth</div></div>
  <div class="stat"><div class="val {cb_cls}">{cb}</div><div class="lbl">Circuit Breaker</div></div>
  <div class="stat"><div class="val">{fails}</div><div class="lbl">CB Failures</div></div>
</div>

<h2>TX History</h2>
<div class="card grid">
  <div class="stat"><div class="val">{total_sign}</div><div class="lbl">Total Signed</div></div>
  <div class="stat"><div class="val">{daily_sign}</div><div class="lbl">Signed Today</div></div>
  <div class="stat"><div class="val">{total_ops}</div><div class="lbl">Total TEE Ops</div></div>
  <div class="stat"><div class="val">{daily_ops}</div><div class="lbl">TEE Ops Today</div></div>
  <div class="stat"><div class="val">{webauthn}</div><div class="lbl">WebAuthn Signed</div></div>
  <div class="stat"><div class="val">{avg_sign}ms</div><div class="lbl">Avg Sign Latency</div></div>
  <div class="stat"><div class="val">{avg_derive}ms</div><div class="lbl">Avg Derive Latency</div></div>
  <div class="stat"><div class="val {error_cls}">{errors}</div><div class="lbl">Errors</div></div>
  <div class="stat"><div class="val {panic_cls}">{panics}</div><div class="lbl">TA Panics</div></div>
</div>

<h2>Wallets</h2>
<div class="card">
<table>
<tr><th>KeyId</th><th>Addr</th><th>PassKey</th><th>Status</th><th>Signs</th><th>Created</th><th>Description</th></tr>
{rows}
</table>
</div>

<div class="footer">
  OP-TEE Secure World &middot; UUID 4319f351-0b24-4097-b659-80ee4f824cdd
</div>
</body>
</html>"#,
        version = KMS_VERSION,
        total = total,
        enabled = enabled,
        with_addr = with_addr,
        with_pk = with_pk,
        api_keys = api_keys,
        queue = qs.queue_depth,
        cb_cls = cb_cls,
        cb = cb,
        fails = fails,
        total_sign = tx.total_sign,
        daily_sign = tx.daily_sign,
        total_ops = tx.total_ops,
        daily_ops = tx.daily_ops,
        webauthn = tx.webauthn_count,
        avg_sign = tx.avg_sign_ms as u64,
        avg_derive = tx.avg_derive_ms as u64,
        errors = tx.error_count,
        error_cls = error_cls,
        panics = tx.panic_count,
        panic_cls = panic_cls,
        rows = rows,
    )
}

async fn health_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kms-api",
        "version": KMS_VERSION,
        "ta_mode": "real",
        "endpoints": {
            "POST": ["/CreateKey", "/DeleteKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/ChangePasskey", "/BeginRegistration", "/CompleteRegistration", "/BeginAuthentication"],
            "GET": ["/health", "/version", "/KeyStatus?KeyId=xxx", "/QueueStatus"]
        }
    })))
}

async fn version_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "version": KMS_VERSION,
        "build": env!("CARGO_PKG_VERSION"),
    })))
}

async fn handle_create_key(
    body: CreateKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.create_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ CreateKey OK {}ms", elapsed);
            let _ = server.db.record_tx("CreateKey", Some(&response.key_metadata.key_id), None, false, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CreateKey error: {} {}ms", e, elapsed);
            let _ = server.db.record_tx("CreateKey", None, None, false, elapsed as u64, false, false);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_describe_key(
    body: DescribeKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.describe_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DescribeKey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_list_keys(
    body: ListKeysRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.list_keys(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ListKeys error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_derive_address(
    body: DeriveAddressRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.derive_address(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ DeriveAddress OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx("DeriveAddress", Some(&key), None, false, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!("{}DeriveAddress error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" }, msg, key, elapsed);
            let _ = server.db.record_tx("DeriveAddress", Some(&key), None, false, elapsed as u64, false, is_panic);
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_sign(
    body: SignRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    let addr = body.address.clone().unwrap_or_default();
    let path = body.webauthn.is_some();
    let t0 = std::time::Instant::now();
    match server.sign(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ Sign OK addr={} webauthn={} {}ms", addr, path, elapsed);
            let _ = server.db.record_tx("Sign", None, Some(&addr), path, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!("{}Sign error: {} addr={} webauthn={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" }, msg, addr, path, elapsed);
            let _ = server.db.record_tx("Sign", None, Some(&addr), path, elapsed as u64, false, is_panic);
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_sign_hash(
    body: SignHashRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    let addr = body.address.clone().unwrap_or_default();
    let path = body.webauthn.is_some();
    let t0 = std::time::Instant::now();
    match server.sign_hash(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignHash OK addr={} webauthn={} {}ms", addr, path, elapsed);
            let _ = server.db.record_tx("SignHash", None, Some(&addr), path, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!("{}SignHash error: {} addr={} webauthn={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" }, msg, addr, path, elapsed);
            let _ = server.db.record_tx("SignHash", None, Some(&addr), path, elapsed as u64, false, is_panic);
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}


async fn handle_get_public_key(
    body: GetPublicKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.get_public_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("GetPublicKey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_delete_key(
    body: DeleteKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.delete_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ DeleteKey OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx("DeleteKey", Some(&key), None, false, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!("{}DeleteKey error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" }, msg, key, elapsed);
            let _ = server.db.record_tx("DeleteKey", Some(&key), None, false, elapsed as u64, false, is_panic);
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_change_passkey(
    body: ChangePasskeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.change_passkey(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ ChangePasskey OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx("ChangePasskey", Some(&key), None, false, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!("{}ChangePasskey error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" }, msg, key, elapsed);
            let _ = server.db.record_tx("ChangePasskey", Some(&key), None, false, elapsed as u64, false, is_panic);
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_begin_registration(
    body: webauthn::BeginRegistrationRequest,
    server: Arc<KmsApiServer>,
    origin_header: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.begin_registration(body, origin_header.as_deref()).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("BeginRegistration error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_complete_registration(
    body: webauthn::CompleteRegistrationRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.complete_registration(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ CompleteRegistration OK {}ms", elapsed);
            let _ = server.db.record_tx("Registration", Some(&response.key_id), None, true, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CompleteRegistration error: {} {}ms", e, elapsed);
            let _ = server.db.record_tx("Registration", None, None, true, elapsed as u64, false, false);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_begin_authentication(
    body: webauthn::BeginAuthenticationRequest,
    server: Arc<KmsApiServer>,
    origin_header: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.begin_authentication(body, origin_header.as_deref()).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("BeginAuthentication error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_key_status(
    key_id: String,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.key_status(&key_id).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("KeyStatus error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_queue_status(
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&server.queue_status()))
}

async fn handle_create_agent_key(
    body: CreateAgentKeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.create_agent_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ CreateAgentKey OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CreateAgentKey error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_agent(
    auth_header: String,
    body: SignAgentRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let jwt = auth_header.strip_prefix("Bearer ")
        .ok_or_else(|| warp::reject::custom(ApiError("Authorization must be 'Bearer <jwt>'".to_string())))?
        .to_string();
    let t0 = std::time::Instant::now();
    match server.sign_agent(jwt, body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignAgent OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("SignAgent error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_refresh_agent_credential(
    auth_header: String,
    body: RefreshAgentCredentialRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let jwt = auth_header.strip_prefix("Bearer ")
        .ok_or_else(|| warp::reject::custom(ApiError("Authorization must be 'Bearer <jwt>'".to_string())))?
        .to_string();
    let t0 = std::time::Instant::now();
    match server.refresh_agent_credential(jwt, body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ RefreshAgentCredential OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("RefreshAgentCredential error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_revoke_agent_credential(
    body: RevokeAgentCredentialRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.revoke_agent_credential(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ RevokeAgentCredential OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("RevokeAgentCredential error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_typed_data(
    body: SignTypedDataRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.sign_typed_data(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignTypedData OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("SignTypedData error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_create_p256_session_key(
    body: CreateP256SessionKeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.create_p256_session_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ CreateP256SessionKey OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CreateP256SessionKey error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_revoke_p256_session_key(
    body: RevokeP256SessionKeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.revoke_p256_session_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ RevokeP256SessionKey OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("RevokeP256SessionKey error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_p256_user_op(
    auth_header: String,
    body: SignP256UserOpRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let jwt = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            warp::reject::custom(ApiError(
                "Authorization must be 'Bearer <jwt>'".to_string(),
            ))
        })?
        .to_string();
    let t0 = std::time::Instant::now();
    match server.sign_p256_user_op(jwt, body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignP256UserOp OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("SignP256UserOp error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

#[derive(Debug)]
struct ApiError(String);

impl warp::reject::Reject for ApiError {}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    if let Some(rl_error) = err.find::<RateLimitError>() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": format!("Rate limit exceeded: {} requests/minute", rl_error.0)
            })),
            warp::http::StatusCode::TOO_MANY_REQUESTS,
        ));
    }
    if let Some(api_error) = err.find::<ApiError>() {
        let status = if api_error.0.contains("API key") {
            warp::http::StatusCode::UNAUTHORIZED
        } else if api_error.0.contains("circuit breaker") {
            warp::http::StatusCode::SERVICE_UNAVAILABLE
        } else if api_error.0.contains("0xffff") || api_error.0.contains("panicked") || api_error.0.contains("TEE error") {
            // TA / TEE errors are server-side faults, not bad requests
            warp::http::StatusCode::INTERNAL_SERVER_ERROR
        } else {
            warp::http::StatusCode::BAD_REQUEST
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": api_error.0
            })),
            status,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "Internal server error"
            })),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

// ========================================
// Custom body filter for AWS KMS content-type
// ========================================

const MAX_REQUEST_BODY_BYTES: usize = 256 * 1024; // 256 KB

fn aws_kms_body<T: serde::de::DeserializeOwned + Send>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::bytes()
        .or(warp::any().map(|| bytes::Bytes::new()))
        .unify()
        .and_then(|bytes: bytes::Bytes| async move {
            if bytes.len() > MAX_REQUEST_BODY_BYTES {
                return Err(warp::reject::custom(ApiError(format!(
                    "Request body too large: {} bytes (max {}KB)",
                    bytes.len(), MAX_REQUEST_BODY_BYTES / 1024
                ))));
            }
            let data: &[u8] = if bytes.is_empty() { b"{}" } else { &bytes };
            serde_json::from_slice(data)
                .map_err(|e| {
                    eprintln!("JSON parse error: {}", e);
                    warp::reject::custom(ApiError(format!("Invalid JSON: {}", e)))
                })
        })
}

// ========================================
// Rate limit middleware
// ========================================

/// Per-API-key rate limiter. Extracts x-api-key header and checks sliding window.
/// Returns 429 if rate limit exceeded.
fn rate_limit_filter(
    limiter: RateLimiter,
) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("x-api-key")
        .and_then(move |key: Option<String>| {
            let limiter = limiter.clone();
            async move {
                let key = key.unwrap_or_else(|| "anonymous".to_string());
                match limiter.check(&key) {
                    Ok(_remaining) => Ok(()),
                    Err(limit) => Err(warp::reject::custom(RateLimitError(limit))),
                }
            }
        })
        .untuple_one()
}

#[derive(Debug)]
struct RateLimitError(usize);
impl warp::reject::Reject for RateLimitError {}

// ========================================
// API Key middleware
// ========================================

/// API key filter: if DB has any api_keys, require valid x-api-key header.
/// Also accepts KMS_API_KEY env var as a legacy fallback.
fn db_api_key_filter(
    db: KmsDb,
    legacy_key: Option<String>,
    enabled: bool,
) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
    warp::header::optional::<String>("x-api-key")
        .and_then(move |key: Option<String>| {
            let db = db.clone();
            let legacy_key = legacy_key.clone();
            async move {
                if !enabled {
                    return Ok(());
                }
                match key {
                    None => Err(warp::reject::custom(ApiError(
                        "Missing API key".to_string(),
                    ))),
                    Some(k) => {
                        // Check legacy env var first
                        if let Some(ref lk) = legacy_key {
                            if k == *lk { return Ok(()); }
                        }
                        // Check DB
                        match db.validate_api_key(&k) {
                            Ok(true) => Ok(()),
                            _ => Err(warp::reject::custom(ApiError(
                                "Invalid API key".to_string(),
                            ))),
                        }
                    }
                }
            }
        })
        .untuple_one()
}

// ========================================
// Main Server Startup
// ========================================

pub async fn start_kms_server() -> Result<()> {
    // Initialize SQLite DB (default: /data/kms/kms.db, fallback: ./kms.db)
    let db_path = std::env::var("KMS_DB_PATH").unwrap_or_else(|_| {
        if std::path::Path::new("/data/kms").exists() {
            "/data/kms/kms.db".to_string()
        } else {
            "kms.db".to_string()
        }
    });
    let db = KmsDb::open(&db_path)?;
    println!("💾 SQLite DB: {}", db_path);

    let server = Arc::new(KmsApiServer::new(db.clone()));

    // API Key guard.
    // Enabled if DB has keys OR KMS_API_KEY env var is set.
    // If neither is configured, access is open unless KMS_REQUIRE_API_KEY=1 is set.
    // In production, set KMS_REQUIRE_API_KEY=1 to fail-closed even before key provisioning.
    let legacy_key = std::env::var("KMS_API_KEY").ok();
    let has_db_keys = db.has_api_keys().unwrap_or(false);
    let force_required = std::env::var("KMS_REQUIRE_API_KEY").map(|v| v == "1").unwrap_or(false);
    let api_key_enabled = has_db_keys || legacy_key.is_some() || force_required;
    if api_key_enabled {
        let source = match (has_db_keys, legacy_key.is_some(), force_required) {
            (true, true, _) => "DB + env",
            (true, false, _) => "DB",
            (false, true, _) => "env (KMS_API_KEY)",
            (false, false, true) => "KMS_REQUIRE_API_KEY=1 (no keys configured — all requests will be rejected)",
            _ => unreachable!(),
        };
        println!("🔑 API Key authentication: ENABLED (source: {})", source);
    } else {
        println!("⚠️  API Key authentication: DISABLED — all requests are unauthenticated.");
        println!("⚠️  To enable: run `kms-admin api-key generate` or set KMS_API_KEY / KMS_REQUIRE_API_KEY=1");
    }
    let api_key_filter = db_api_key_filter(db, legacy_key, api_key_enabled);
    let rl_filter = rate_limit_filter(server.rate_limiter.clone());

    // Root path - live stats dashboard
    let server_index = server.clone();
    let index = warp::path::end()
        .and(warp::get())
        .map(move || {
            warp::reply::html(render_stats_page(&server_index))
        });

    // Test UI page
    let test_ui = warp::path("test")
        .and(warp::get())
        .map(|| {
            match std::fs::read_to_string("/root/shared/kms-test-page.html") {
                Ok(html) => warp::reply::html(html),
                Err(_) => warp::reply::html("<html><body><h1>Test UI not available</h1><p>Please deploy kms-test-page.html to /root/shared/</p></body></html>".to_string())
            }
        });

    // Health check
    let health = warp::path("health")
        .and(warp::get())
        .and_then(health_check);

    // Version check
    let version = warp::path("version")
        .and(warp::get())
        .and_then(version_check);

    // KeyStatus - GET /KeyStatus?KeyId=xxx
    let server_ks = server.clone();
    let key_status = warp::path("KeyStatus")
        .and(warp::get())
        .and(warp::query::raw().map(|q: String| {
            // Parse KeyId from query string
            q.split('&')
                .find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some("KeyId"), Some(v)) => Some(v.to_string()),
                        _ => None,
                    }
                })
                .unwrap_or_default()
        }))
        .and(warp::any().map(move || server_ks.clone()))
        .and_then(handle_key_status);

    // QueueStatus - GET /QueueStatus
    let server_qs = server.clone();
    let queue_status = warp::path("QueueStatus")
        .and(warp::get())
        .and(warp::any().map(move || server_qs.clone()))
        .and_then(handle_queue_status);

    // ChangePasskey API (TEE)
    let server_cp = server.clone();
    let change_passkey = warp::path("ChangePasskey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_cp.clone()))
        .and_then(handle_change_passkey);

    // Clone server for each route
    let server1 = server.clone();
    let server2 = server.clone();
    let server3 = server.clone();
    let server4 = server.clone();
    let server5 = server.clone();
    let server6 = server.clone();

    // CreateKey API (TEE)
    let create_key = warp::path("CreateKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.CreateKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server1.clone()))
        .and_then(handle_create_key);

    // DescribeKey API
    let describe_key = warp::path("DescribeKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.DescribeKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server2.clone()))
        .and_then(handle_describe_key);

    // ListKeys API
    let list_keys = warp::path("ListKeys")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.ListKeys"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server3.clone()))
        .and_then(handle_list_keys);

    // DeriveAddress API (TEE)
    let derive_address = warp::path("DeriveAddress")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.DeriveAddress"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server4.clone()))
        .and_then(handle_derive_address);

    // Sign API (TEE)
    let sign = warp::path("Sign")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.Sign"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server5.clone()))
        .and_then(handle_sign);

    // SignHash API (TEE)
    let server6_clone = Arc::clone(&server);
    let sign_hash = warp::path("SignHash")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.SignHash"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6_clone.clone()))
        .and_then(handle_sign_hash);

    // GetPublicKey API (TEE)
    let get_public_key = warp::path("GetPublicKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.GetPublicKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6.clone()))
        .and_then(handle_get_public_key);

    // DeleteKey API (TEE)
    let server7 = Arc::clone(&server);
    let delete_key = warp::path("DeleteKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.ScheduleKeyDeletion"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server7.clone()))
        .and_then(handle_delete_key);

    // WebAuthn: BeginRegistration
    let server_br = Arc::clone(&server);
    let begin_registration = warp::path("BeginRegistration")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_br.clone()))
        .and(warp::header::optional::<String>("origin"))
        .and_then(handle_begin_registration);

    // WebAuthn: CompleteRegistration
    let server_cr = Arc::clone(&server);
    let complete_registration = warp::path("CompleteRegistration")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_cr.clone()))
        .and_then(handle_complete_registration);

    // WebAuthn: BeginAuthentication
    let server_ba = Arc::clone(&server);
    let begin_authentication = warp::path("BeginAuthentication")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_ba.clone()))
        .and(warp::header::optional::<String>("origin"))
        .and_then(handle_begin_authentication);

    // Agent Key endpoints (POST /kms/create-agent-key, /kms/sign-agent, etc.)
    let server_cak = server.clone();
    let create_agent_key = warp::path("kms")
        .and(warp::path("create-agent-key"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_cak.clone()))
        .and_then(handle_create_agent_key);

    let server_sa = server.clone();
    let sign_agent = warp::path("kms")
        .and(warp::path("sign-agent"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sa.clone()))
        .and_then(handle_sign_agent);

    let server_rac = server.clone();
    let refresh_agent_credential = warp::path("kms")
        .and(warp::path("refresh-agent-credential"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_rac.clone()))
        .and_then(handle_refresh_agent_credential);

    let server_revoke = server.clone();
    let revoke_agent_credential = warp::path("kms")
        .and(warp::path("revoke-agent-credential"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_revoke.clone()))
        .and_then(handle_revoke_agent_credential);

    let server_std = server.clone();
    let sign_typed_data = warp::path("kms")
        .and(warp::path("SignTypedData"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_std.clone()))
        .and_then(handle_sign_typed_data);

    // P256 Session Key endpoints
    let server_cp256 = server.clone();
    let create_p256_session_key = warp::path("kms")
        .and(warp::path("create-p256-session-key"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_cp256.clone()))
        .and_then(handle_create_p256_session_key);

    let server_sp256 = server.clone();
    let sign_p256_user_op = warp::path("kms")
        .and(warp::path("sign-p256-user-op"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sp256.clone()))
        .and_then(handle_sign_p256_user_op);

    let server_rp256 = server.clone();
    let revoke_p256_session_key = warp::path("kms")
        .and(warp::path("revoke-p256-session-key"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_rp256.clone()))
        .and_then(handle_revoke_p256_session_key);

    // JWT secret auto-rotation background task (runs every 24h)
    let server_rot = server.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
        interval.tick().await; // Skip immediate first tick
        loop {
            interval.tick().await;
            match server_rot.tee.jwt_rotate_secret(false).await {
                Ok(result) => {
                    let now = Utc::now().to_rfc3339();
                    let _ = server_rot.db.upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
                        kid: result.new_kid.clone(),
                        status: "current".to_string(),
                        created_at: now.clone(),
                        retired_at: None,
                        expires_at: None,
                    });
                    if let Some(old_kid) = result.retired_kid {
                        let retire_ts = Utc::now().timestamp() + 7 * 24 * 3600;
                        let _ = server_rot.db.upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
                            kid: old_kid,
                            status: "verify-only".to_string(),
                            created_at: now,
                            retired_at: None,
                            expires_at: Some(retire_ts),
                        });
                    }
                    println!("🔑 JWT secret auto-rotated: new kid={}", result.new_kid);
                }
                Err(e) => eprintln!("JWT rotation error: {}", e),
            }
        }
    });

    let routes = index
        .or(test_ui)
        .or(health)
        .or(version)
        .or(key_status)
        .or(queue_status)
        .or(change_passkey)
        .or(create_key)
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(sign_hash)
        .or(get_public_key)
        .or(delete_key)
        .or(begin_registration)
        .or(complete_registration)
        .or(begin_authentication)
        .or(create_agent_key)
        .or(sign_agent)
        .or(refresh_agent_credential)
        .or(revoke_agent_credential)
        .or(sign_typed_data)
        .or(create_p256_session_key)
        .or(sign_p256_user_op)
        .or(revoke_p256_session_key)
        .recover(handle_rejection);

    println!("🚀 KMS API Server v{} starting on http://0.0.0.0:3000", KMS_VERSION);
    println!("📚 Supported APIs:");
    println!("   GET  /              - Welcome page");
    println!("   GET  /test          - Interactive test UI");
    println!("   POST /CreateKey     - Create new TEE wallet");
    println!("   POST /DescribeKey   - Query wallet metadata");
    println!("   POST /ListKeys      - List all wallets");
    println!("   POST /DeriveAddress - Derive Ethereum address");
    println!("   POST /Sign          - Sign Ethereum transaction or message");
    println!("   POST /SignHash      - Sign 32-byte hash directly");
    println!("   POST /GetPublicKey  - Get public key");
    println!("   POST /DeleteKey     - Delete wallet (requires PassKey)");
    println!("   POST /ChangePasskey         - Change PassKey public key");
    println!("   POST /BeginRegistration     - WebAuthn registration (step 1)");
    println!("   POST /CompleteRegistration  - WebAuthn registration (step 2)");
    println!("   POST /BeginAuthentication   - WebAuthn authentication challenge");
    println!("   GET  /KeyStatus             - Key derivation status (polling)");
    println!("   GET  /QueueStatus           - TEE queue depth");
    println!("   GET  /health                - Health check");
    println!("   POST /kms/create-agent-key       - Create AI agent key (WebAuthn)");
    println!("   POST /kms/sign-agent             - Agent sign userOpHash (Bearer JWT)");
    println!("   POST /kms/refresh-agent-credential - Refresh agent JWT (Bearer + WebAuthn)");
    println!("   POST /kms/revoke-agent-credential  - Revoke agent key (WebAuthn)");
    println!("   POST /kms/SignTypedData             - EIP-712 typed data signing");
    println!("   POST /kms/create-p256-session-key  - Create P256 session key (WebAuthn)");
    println!("   POST /kms/sign-p256-user-op        - P256 sign userOpHash (Bearer JWT)");
    println!("🔐 TA Mode: ✅ Real TA (OP-TEE Secure World required)");
    println!("🆔 TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd");
    println!("🌐 Public URL: https://kms.aastar.io");

    warp::serve(routes)
        .run(([0, 0, 0, 0], 3000))
        .await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    start_kms_server().await
}