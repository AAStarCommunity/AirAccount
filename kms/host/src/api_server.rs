// KMS API Server
// Real TA integration only - requires OP-TEE environment
// Deploy to QEMU for testing, production-ready architecture

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use hex;
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::EncodedPoint;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use warp::Filter;

// Import from kms library and proto
use kms::agent_jwt;
use kms::db::{AgentKeyRow, KmsDb, WalletRow};
use kms::rate_limit::RateLimiter;
use kms::ta_client::TeeHandle;
use kms::webauthn;
use proto;

/// Estimated seconds per TEE operation with persistent session
const TEE_OP_ESTIMATE_SECS: u64 = 1;

/// Issue #42: a key with no successful Sign/Derive activity for longer than this
/// is automatically moved to lifecycle_status='frozen' by the background sweep.
/// Freezing is a soft host-side gate (extra verification door for dormant keys),
/// NOT theft prevention or storage reclamation — the TEE key material is untouched.
/// Default: 365 days. Override with KMS_INACTIVITY_FREEZE_SECS for testing.
const INACTIVITY_FREEZE_SECS: i64 = 365 * 24 * 60 * 60;

/// How often the dormant-key freeze sweep runs. 6 hours is ample given the
/// 365-day default threshold; short enough to act promptly when the threshold
/// is lowered for testing via KMS_INACTIVITY_FREEZE_SECS.
const FREEZE_SWEEP_INTERVAL_SECS: u64 = 6 * 60 * 60;

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
    /// Issue #42: RFC3339 timestamp of the last successful operation for this key,
    /// derived from tx_log. None if the key has never had a successful operation.
    #[serde(rename = "LastUsedAt", skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    /// Issue #42: key lifecycle gate — "active" or "frozen". Frozen keys reject
    /// signing until unfrozen via passkey (POST /UnfreezeKey).
    #[serde(rename = "LifecycleStatus")]
    pub lifecycle_status: String,
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
    #[serde(
        rename = "DerivationPath",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub derivation_path: Option<String>,
    // Transaction signing mode (original)
    #[serde(
        rename = "Transaction",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub transaction: Option<EthereumTransaction>,
    // Message signing mode (new)
    #[serde(rename = "Message", skip_serializing_if = "Option::is_none", default)]
    pub message: Option<String>,
    #[serde(
        rename = "SigningAlgorithm",
        skip_serializing_if = "Option::is_none",
        default
    )]
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
    #[serde(
        rename = "DerivationPath",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub derivation_path: Option<String>,
    #[serde(rename = "Hash")]
    pub hash: String,
    #[serde(
        rename = "SigningAlgorithm",
        skip_serializing_if = "Option::is_none",
        default
    )]
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
    #[serde(
        rename = "PendingWindowInDays",
        skip_serializing_if = "Option::is_none"
    )]
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

/// Issue #42: POST /UnfreezeKey — owner WebAuthn-gated unfreeze of a dormant key.
/// Same passkey authentication shape as DeleteKey (strict assertion required).
#[derive(Debug, Serialize, Deserialize)]
pub struct UnfreezeKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    /// Legacy: raw PassKey assertion (hex)
    #[serde(rename = "Passkey", skip_serializing_if = "Option::is_none", default)]
    pub passkey: Option<PasskeyAssertion>,
    /// WebAuthn ceremony assertion (from BeginAuthentication)
    #[serde(rename = "WebAuthn", skip_serializing_if = "Option::is_none", default)]
    pub webauthn: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnfreezeKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "LifecycleStatus")]
    pub lifecycle_status: String,
}

/// Admin force-purge request — bypasses passkey, deletes from TEE + SQLite.
/// Requires Authorization: Bearer $KMS_ADMIN_TOKEN header.
///
/// DEV/TEST ONLY — compiled in only under the `admin-purge` feature. Release
/// builds (no feature) contain no admin surface at all.
#[cfg(feature = "admin-purge")]
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminPurgeKeyRequest {
    pub key_id: String,
    /// Human-readable reason for audit log (e.g. "orphan cleanup", "test key")
    #[serde(default)]
    pub reason: String,
}

#[cfg(feature = "admin-purge")]
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminPurgeKeyResponse {
    pub key_id: String,
    pub tee_purged: bool,
    pub sqlite_deleted: bool,
    pub message: String,
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
    pub status: String, // "creating" | "deriving" | "ready" | "error"
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "PublicKey", skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(rename = "DerivationPath", skip_serializing_if = "Option::is_none")]
    pub derivation_path: Option<String>,
    #[serde(rename = "Error", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Issue #42: RFC3339 of last successful op (from tx_log), None if never used.
    #[serde(rename = "LastUsedAt", skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    /// Issue #42: "active" or "frozen".
    #[serde(rename = "LifecycleStatus")]
    pub lifecycle_status: String,
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

fn default_secp256k1() -> String {
    "secp256k1".to_string()
}

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
    /// WebAuthn ceremony assertion (challenge-based, replay-protected). Required if no Bearer JWT.
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
    /// Deprecated: legacy raw passkey assertion. NOT accepted by sign-typed-data (no replay protection).
    /// Field kept for JSON parse compatibility; the server rejects requests that rely on it.
    #[serde(rename = "passkeyAssertion", default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
}

fn default_hd_path() -> String {
    "m/44'/60'/0'/0/0".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    /// Hex-encoded 65-byte ECDSA signature: R(32) || S(32) || V(1), V=27/28
    pub signature: String,
}

// ── P2 SuperPaymaster convenience signers (v0.18.1) ──
// Each builds a fixed EIP-712 domain+type in the host and signs via the existing
// SignTypedData TA command — no new TA command/Command-ID is introduced.

/// `POST /kms/SignMicropaymentVoucher`
#[derive(Debug, Serialize, Deserialize)]
pub struct SignMicropaymentVoucherRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    /// MicroPaymentChannel contract address (0x…, 20 bytes)
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
    /// Payment channel ID (0x…, 32 bytes)
    #[serde(rename = "channelId")]
    pub channel_id: String,
    /// Cumulative amount (decimal or 0x… hex, up to uint256)
    #[serde(rename = "cumulativeAmount")]
    pub cumulative_amount: String,
    /// WebAuthn ceremony assertion (challenge-bound, replay-protected). Required if no Bearer JWT.
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

/// `POST /kms/SignGTokenAuthorization`
/// Builds GToken EIP-3009 TransferWithAuthorization domain + type and signs.
#[derive(Debug, Serialize, Deserialize)]
pub struct SignGTokenAuthorizationRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    /// GToken ERC-20 contract address (0x…, 20 bytes)
    #[serde(rename = "gTokenAddress")]
    pub gtoken_address: String,
    /// Transfer sender — MUST equal the Ethereum address derived from keyId+hdPath.
    /// On-chain EIP-3009 verifies ecrecover(hash,sig) == from; a mismatch causes revert.
    pub from: String,
    pub to: String,
    /// Token amount (decimal or 0x… hex)
    pub value: String,
    #[serde(rename = "validAfter")]
    pub valid_after: String,
    #[serde(rename = "validBefore")]
    pub valid_before: String,
    /// 32-byte random nonce (0x…)
    pub nonce: String,
    /// WebAuthn ceremony assertion (challenge-bound, replay-protected). Required if no Bearer JWT.
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

/// `POST /kms/SignX402Payment`
/// Builds SuperPaymaster x402 PaymentPayload EIP-712 struct and signs.
#[derive(Debug, Serialize, Deserialize)]
pub struct SignX402PaymentRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    /// SuperPaymaster contract address (0x…, 20 bytes)
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
    /// Unique payment ID (0x…, 32 bytes)
    #[serde(rename = "paymentId")]
    pub payment_id: String,
    /// Amount to settle (decimal or 0x… hex)
    pub amount: String,
    /// Recipient address (0x…)
    pub recipient: String,
    /// Deadline Unix timestamp (decimal or 0x… hex)
    pub deadline: String,
    /// WebAuthn ceremony assertion (challenge-bound, replay-protected). Required if no Bearer JWT.
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

// P2 responses share the same shape as SignTypedDataResponse
pub type SignMicropaymentVoucherResponse = SignTypedDataResponse;
pub type SignGTokenAuthorizationResponse = SignTypedDataResponse;
pub type SignX402PaymentResponse = SignTypedDataResponse;

// ── Grant Session Signing ──

/// Parse bytes4 hex string ("0x..." or bare 8 hex chars) into [u8; 4].
fn parse_bytes4_hex(s: &str) -> Result<[u8; 4]> {
    let bytes = hex::decode(s.trim_start_matches("0x"))
        .map_err(|e| anyhow!("Invalid bytes4 hex '{}': {}", s, e))?;
    if bytes.len() != 4 {
        return Err(anyhow!(
            "bytes4 must be exactly 4 bytes, got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 4];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignGrantSessionRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    /// SessionKeyValidator contract address ("0x..." 20 bytes)
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
    /// Smart Account address ("0x..." 20 bytes)
    pub account: String,
    /// secp256k1 session key address ("0x..." 20 bytes)
    #[serde(rename = "sessionKey")]
    pub session_key: String,
    /// Session expiry as Unix timestamp (uint48)
    pub expiry: u64,
    /// Allowed contract address or zero for any
    #[serde(rename = "contractScope")]
    pub contract_scope: String,
    /// Allowed 4-byte selector or zero for any
    #[serde(rename = "selectorScope")]
    pub selector_scope: String,
    #[serde(rename = "velocityLimit")]
    pub velocity_limit: u16,
    #[serde(rename = "velocityWindow")]
    pub velocity_window: u32,
    /// Allowed call target addresses (empty = any)
    #[serde(rename = "callTargets", default)]
    pub call_targets: Vec<String>,
    /// Allowed 4-byte function selectors (empty = any)
    #[serde(rename = "selectorAllowlist", default)]
    pub selector_allowlist: Vec<String>,
    /// grantNonces[account][sessionKey] read from chain (decimal u64)
    pub nonce: u64,
    #[serde(rename = "passkeyAssertion", default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignGrantSessionResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    /// Hex-encoded 65-byte ECDSA signature: R(32) || S(32) || V(1), V=27/28
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignP256GrantSessionRequest {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "hdPath", default = "default_hd_path")]
    pub hd_path: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
    pub account: String,
    /// P256 session key X coordinate (hex, 32 bytes)
    #[serde(rename = "keyX")]
    pub key_x: String,
    /// P256 session key Y coordinate (hex, 32 bytes)
    #[serde(rename = "keyY")]
    pub key_y: String,
    pub expiry: u64,
    #[serde(rename = "contractScope")]
    pub contract_scope: String,
    #[serde(rename = "selectorScope")]
    pub selector_scope: String,
    #[serde(rename = "velocityLimit")]
    pub velocity_limit: u16,
    #[serde(rename = "velocityWindow")]
    pub velocity_window: u32,
    #[serde(rename = "callTargets", default)]
    pub call_targets: Vec<String>,
    #[serde(rename = "selectorAllowlist", default)]
    pub selector_allowlist: Vec<String>,
    /// grantNonces_p256[account][keyHash] read from chain (decimal u64)
    pub nonce: u64,
    #[serde(rename = "passkeyAssertion", default)]
    pub passkey_assertion: Option<PasskeyAssertion>,
    #[serde(rename = "webAuthnAssertion", default)]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignP256GrantSessionResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
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
        return Err(anyhow!(
            "Invalid agent keyId format (expected wallet_id:index): {}",
            key_id
        ));
    }
    let wallet_id = Uuid::parse_str(parts[0])
        .map_err(|_| anyhow!("Invalid wallet_id in agent keyId: {}", parts[0]))?;
    let agent_index: u32 = parts[1]
        .parse()
        .map_err(|_| anyhow!("Invalid agent_index in keyId: {}", parts[1]))?;
    Ok((wallet_id, agent_index))
}

/// Convert a JSON value to a proto::Eip712Value using the declared ABI type for guidance.
///
/// Supported types: address, uint*, int*, bytes32, bytes*, bool, string
fn json_to_eip712_value(
    json_val: &serde_json::Value,
    declared_type: &str,
) -> Result<proto::Eip712Value> {
    let t = declared_type.trim();
    if t == "address" {
        let s = json_val
            .as_str()
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
        let b = json_val
            .as_bool()
            .ok_or_else(|| anyhow!("bool field must be a JSON boolean"))?;
        return Ok(proto::Eip712Value::Bool(b));
    }
    if t == "string" {
        let s = json_val
            .as_str()
            .ok_or_else(|| anyhow!("string field must be a JSON string"))?;
        return Ok(proto::Eip712Value::Str(s.to_string()));
    }
    if t == "bytes32" {
        let s = json_val
            .as_str()
            .ok_or_else(|| anyhow!("bytes32 field must be a JSON hex string"))?;
        let bytes = hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid bytes32 hex '{}': {}", s, e))?;
        if bytes.len() != 32 {
            return Err(anyhow!(
                "bytes32 must be exactly 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(proto::Eip712Value::Bytes32(arr));
    }
    if t.starts_with("bytes") {
        let s = json_val
            .as_str()
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
                hex::decode(hex_str).map_err(|e| anyhow!("Invalid uint hex '{}': {}", s, e))?
            } else {
                // Decimal string — parse as u128 max (covers uint8–uint128)
                let n: u128 = s
                    .parse()
                    .map_err(|_| anyhow!("Invalid uint decimal '{}'", s))?;
                n.to_be_bytes().to_vec()
            }
        } else {
            return Err(anyhow!(
                "uint/int field must be number or string, got {:?}",
                json_val
            ));
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
        return Err(anyhow!(
            "Invalid DER signature: long-form length not supported"
        ));
    }
    if 2 + total_len > der.len() {
        return Err(anyhow!(
            "Invalid DER signature: declared length {} exceeds data {}",
            total_len,
            der.len()
        ));
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
        return Err(anyhow!(
            "Invalid DER signature: r overflows buffer (r_len={} pos={} len={})",
            r_len,
            pos,
            der.len()
        ));
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
        return Err(anyhow!(
            "Invalid DER signature: s overflows buffer (s_len={} pos={} len={})",
            s_len,
            pos,
            der.len()
        ));
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
    let creation_date = w
        .created_at
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());
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
        // Issue #42: populated by the caller (describe_key) via DB lookups, since
        // WalletRow intentionally does not carry tx_log-derived / lifecycle data.
        last_used_at: None,
        lifecycle_status: "active".to_string(),
    }
}

/// Issue #73: minimum seconds between `/health` attestation probes while
/// capability is not yet confirmed. Bounds TEE load from frequent /health
/// polling against an older/incapable TA (or during the startup window).
const ATTESTATION_PROBE_MIN_INTERVAL_SECS: i64 = 30;

pub struct KmsApiServer {
    db: KmsDb,
    tee: TeeHandle,
    rate_limiter: RateLimiter,
    agent_rate_limiter: RateLimiter,
    rp_name: String,
    rp_ids: Vec<String>,
    expected_origins: Vec<String>,
    /// Issue #73 — attestation capability for `/health`, replacing a hardcoded
    /// `true`. `attestation_capable` is a **monotonic latch**: the first probe
    /// that proves the deployed TA supports GetAttestation (=26) latches it
    /// `true` for the process lifetime (a TA that gains/loses the command needs a
    /// redeploy, which restarts the process and resets this). While unconfirmed,
    /// `attestation_probe_at` (unix secs of the last probe) rate-limits re-probes
    /// so a transient startup window OR an older TA cannot trigger a TEE call on
    /// every `/health`.
    attestation_capable: std::sync::atomic::AtomicBool,
    attestation_probe_at: std::sync::atomic::AtomicI64,
}

impl KmsApiServer {
    pub fn new(db: KmsDb) -> Self {
        // DEV/TEST builds (feature dev-rpid) bake localhost into the defaults so
        // a test image is self-contained; production builds default to aastar.io
        // only. KMS_RP_ID / KMS_ORIGIN env always override either default.
        // NOTE: the host default/env only governs what the CA advertises and
        // pre-checks; the TA's compiled-in rpId allow-list is the binding gate
        // (the TA must also be a dev-rpid build to accept localhost assertions).
        let rp_ids: Vec<String> = std::env::var("KMS_RP_ID")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| {
                if cfg!(feature = "dev-rpid") {
                    vec!["aastar.io".to_string(), "localhost".to_string()]
                } else {
                    vec!["aastar.io".to_string()]
                }
            });
        let rp_name = std::env::var("KMS_RP_NAME").unwrap_or_else(|_| "AirAccount KMS".to_string());
        let expected_origins: Vec<String> = std::env::var("KMS_ORIGIN")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| {
                if cfg!(feature = "dev-rpid") {
                    vec![
                        "https://aastar.io".to_string(),
                        "https://*.aastar.io".to_string(),
                        "http://localhost:*".to_string(),
                    ]
                } else {
                    vec![format!("https://{}", rp_ids[0])]
                }
            });
        #[cfg(feature = "dev-rpid")]
        println!("⚠️  DEV-RPID build: localhost rpId/origin accepted — NOT a production image");
        println!("🌐 Allowed origins: {:?}", expected_origins);
        println!("🔑 Allowed rpIds: {:?}", rp_ids);
        let rate_limiter = RateLimiter::from_env();
        println!("⏱️  Rate limiter: {}/min per API key", rate_limiter.limit());
        let agent_rl_limit = std::env::var("KMS_AGENT_RATE_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        let agent_rl_max_keys = std::env::var("KMS_RATE_LIMIT_MAX_KEYS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);
        let agent_rate_limiter = RateLimiter::new(agent_rl_limit, agent_rl_max_keys);
        println!(
            "⏱️  Agent rate limiter: {}/min per credential (max {} tracked keys)",
            agent_rl_limit, agent_rl_max_keys
        );
        Self {
            db,
            tee: TeeHandle::new(),
            rate_limiter,
            agent_rate_limiter,
            rp_name,
            rp_ids,
            expected_origins,
            attestation_capable: std::sync::atomic::AtomicBool::new(false),
            attestation_probe_at: std::sync::atomic::AtomicI64::new(0),
        }
    }

    /// Issue #73 — real attestation capability for `/health`, replacing a
    /// hardcoded `true`. Capability is a **monotonic latch**: the first probe
    /// that succeeds (GetAttestation with a fixed, non-secret dummy nonce; the
    /// evidence is discarded) latches `true` for the process lifetime. While
    /// unconfirmed, probes are rate-limited (>= `ATTESTATION_PROBE_MIN_INTERVAL_SECS`
    /// apart) so a transient startup window OR an older TA without cmd 26 cannot
    /// trigger a TEE call on every `/health`.
    ///
    /// There is deliberately **no coupling to any error wording**: any error
    /// just means "not capable right now", and the next probe (after the
    /// interval) flips it to `true` the moment a capable TA is ready. This is
    /// both robust (a reworded TA error can't mislead it) and fail-safe (worst
    /// case is an extra probe per interval, never a wrong permanent verdict).
    pub async fn attestation_capable(&self) -> bool {
        use std::sync::atomic::Ordering;
        // Monotonic: a TA cannot lose cmd 26 under a running host (that needs a
        // redeploy, which restarts the process and resets this latch).
        if self.attestation_capable.load(Ordering::Relaxed) {
            return true;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let last = self.attestation_probe_at.load(Ordering::Relaxed);
        // Skip only when the clock has genuinely advanced but less than the
        // interval. `now < last` means wall time moved backward (NTP / tz jump);
        // treat that as probe-due rather than freezing re-probes until wall time
        // catches up to a stale future timestamp.
        if now >= last && now.saturating_sub(last) < ATTESTATION_PROBE_MIN_INTERVAL_SECS {
            // Probed recently and still not capable — don't hammer the TEE.
            return false;
        }
        self.attestation_probe_at.store(now, Ordering::Relaxed);
        match self.get_attestation(b"health-probe".to_vec()).await {
            Ok(_) => {
                self.attestation_capable.store(true, Ordering::Relaxed);
                true
            }
            // Transient (worker not ready) OR unsupported (older TA): both mean
            // "not capable now". Re-probed after the interval — no error-string match.
            Err(_) => false,
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
            return Err(anyhow!(
                "Derivation path too long: {} chars (max 64)",
                path.len()
            ));
        }
        if !path.starts_with("m/") {
            return Err(anyhow!("Derivation path must start with 'm/': {}", path));
        }
        // Validate each component is a number with optional hardened marker
        for part in path[2..].split('/') {
            let num_str = part.trim_end_matches('\'');
            if num_str.parse::<u32>().is_err() {
                return Err(anyhow!(
                    "Invalid derivation path component '{}' in: {}",
                    part,
                    path
                ));
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
        let bytes = hex::decode(hex_str).map_err(|e| anyhow!("Invalid address hex: {}", e))?;
        if bytes.len() != 20 {
            return Err(anyhow!(
                "Address must be exactly 20 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    fn validate_hash_hex(hash: &str) -> Result<[u8; 32]> {
        let hex_str = hash.trim_start_matches("0x");
        let bytes = hex::decode(hex_str).map_err(|e| anyhow!("Invalid hash hex: {}", e))?;
        if bytes.len() != 32 {
            return Err(anyhow!(
                "Hash must be exactly 32 bytes, got {} bytes",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    /// Validate hex-encoded message (reasonable size limit for TA).
    fn validate_message(message: &str) -> Result<()> {
        let max_len = 64 * 1024; // 64KB
        if message.len() > max_len {
            return Err(anyhow!(
                "Message too large: {} bytes (max {})",
                message.len(),
                max_len
            ));
        }
        Ok(())
    }

    pub async fn create_key(&self, req: CreateKeyRequest) -> Result<CreateKeyResponse> {
        println!("📝 KMS CreateKey API called");

        // Decode and validate passkey public key (mandatory)
        let pk_hex = req.passkey_public_key.trim_start_matches("0x");
        let passkey_pubkey =
            hex::decode(pk_hex).map_err(|e| anyhow!("Invalid PasskeyPublicKey hex: {}", e))?;
        if passkey_pubkey.len() != 65 || passkey_pubkey[0] != 0x04 {
            return Err(anyhow!(
                "PasskeyPublicKey must be 65 bytes uncompressed (0x04||x||y), got {} bytes",
                passkey_pubkey.len()
            ));
        }
        // Validate the point is actually on the P-256 curve (prevents gap keys)
        if p256::PublicKey::from_sec1_bytes(&passkey_pubkey).is_err() {
            return Err(anyhow!(
                "PasskeyPublicKey is not a valid point on the P-256 curve"
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
            // Issue #42: a just-created key is active and has no usage history yet.
            last_used_at: None,
            lifecycle_status: "active".to_string(),
        };

        // Persist to DB.
        // H-C: if this insert fails the TA wallet becomes an invisible orphan
        // (occupies an RPMB slot, unreachable via API). A host-side
        // compensating delete is impossible — the TA mandates passkey
        // verification for removal. So: retry the (usually transient) SQLite
        // failure, and if it still fails, log CRITICAL with the orphan id for
        // ForceRemoveWallet cleanup (admin command, PR #35).
        let row = WalletRow {
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
        };
        let mut insert_result = self.db.insert_wallet(&row);
        for attempt in 1..=3u64 {
            if insert_result.is_ok() {
                break;
            }
            eprintln!(
                "⚠️  CreateKey: DB insert attempt {}/4 failed for {}: {:?}",
                attempt, wallet_id, insert_result
            );
            tokio::time::sleep(std::time::Duration::from_millis(100 * attempt)).await;
            insert_result = self.db.insert_wallet(&row);
        }
        if let Err(e) = insert_result {
            eprintln!(
                "🔴 CRITICAL: TA wallet {} created but DB insert failed after retries — \
                 ORPHAN in TEE storage (no DB row). Clean up via ForceRemoveWallet. \
                 Error: {:?}",
                wallet_id, e
            );
            return Err(anyhow!(
                "CreateKey: metadata persistence failed (TEE wallet {} orphaned, \
                 operator notified): {}",
                wallet_id,
                e
            ));
        }

        // Spawn background address derivation
        let db = self.db.clone();
        let tee = self.tee.clone();
        tokio::spawn(async move {
            match tee.derive_address_auto(wallet_id).await {
                Ok((_wid, address_bytes, public_key, derivation_path)) => {
                    let address_hex = format!("0x{}", hex::encode(&address_bytes));
                    let pubkey_hex = format!("0x{}", hex::encode(&public_key));
                    println!(
                        "✅ Background derivation done for {}: {}",
                        wallet_id, address_hex
                    );

                    let _ = db.update_wallet_derived(
                        &wallet_id.to_string(),
                        &address_hex,
                        &pubkey_hex,
                        &derivation_path,
                        "ready",
                    );
                    let _ = db.upsert_address(
                        &address_hex,
                        &wallet_id.to_string(),
                        &derivation_path,
                        Some(&pubkey_hex),
                    );
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    eprintln!(
                        "❌ Background derivation failed for {}: {}",
                        wallet_id, err_msg
                    );
                    let _ =
                        db.update_wallet_status(&wallet_id.to_string(), "error", Some(&err_msg));
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

        let w = self
            .db
            .get_wallet(&req.key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        let mut key_metadata = wallet_to_metadata(&w);
        // Issue #42: enrich with tx_log-derived last-used and lifecycle gate.
        key_metadata.last_used_at = self.db.last_used_at(&req.key_id)?;
        if let Some(ls) = self.db.get_lifecycle_status(&req.key_id)? {
            key_metadata.lifecycle_status = ls;
        }

        Ok(DescribeKeyResponse { key_metadata })
    }

    pub async fn list_keys(&self, _req: ListKeysRequest) -> Result<ListKeysResponse> {
        println!("📝 KMS ListKeys API called");

        let wallets = self.db.list_wallets()?;
        let keys = wallets
            .iter()
            .map(|w| KeyListEntry {
                key_id: w.key_id.clone(),
                key_arn: format!("arn:aws:kms:region:account:key/{}", w.key_id),
            })
            .collect();

        Ok(ListKeysResponse { keys })
    }

    pub async fn key_status(&self, key_id: &str) -> Result<KeyStatusResponse> {
        let w = self
            .db
            .get_wallet(key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

        let (status, error) = if w.status.starts_with("error") {
            ("error", w.error_msg.clone())
        } else {
            (w.status.as_str(), None)
        };

        // Issue #42: surface dormancy/lifecycle alongside derivation status.
        let last_used_at = self.db.last_used_at(key_id)?;
        let lifecycle_status = self
            .db
            .get_lifecycle_status(key_id)?
            .unwrap_or_else(|| "active".to_string());

        Ok(KeyStatusResponse {
            key_id: key_id.to_string(),
            status: status.to_string(),
            address: w.address,
            public_key: w.public_key,
            derivation_path: w.derivation_path,
            error,
            last_used_at,
            lifecycle_status,
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

    pub async fn read_rollback_counter(&self) -> Result<u64> {
        self.tee.read_rollback_counter().await
    }

    /// Issue #37 — produce a remote-attestation evidence blob bound to `nonce`.
    pub async fn get_attestation(&self, nonce: Vec<u8>) -> Result<proto::GetAttestationOutput> {
        self.tee.get_attestation(nonce).await
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
        let passkey_assertion = self
            .resolve_passkey_assertion_strict(
                &req.key_id,
                req.passkey.as_ref(),
                req.webauthn.as_ref(),
                false, // #110: nonce-only op — TA enforces challenge==nonce; host stays strict
            )
            .await?;

        // Change passkey in TEE secure storage (TA verifies current passkey first)
        let wallet_uuid = uuid::Uuid::parse_str(&req.key_id)?;
        self.tee
            .register_passkey_ta(wallet_uuid, &pubkey_bytes, passkey_assertion)
            .await?;

        // H-B: the TA has now committed the NEW passkey. If the DB update
        // below is lost, the DB keeps the OLD pubkey and every subsequent
        // WebAuthn verification for this wallet fails against the wrong key —
        // the wallet is effectively locked out. Retry with backoff and log
        // CRITICAL with the exact recovery SQL if all retries fail.
        let new_pk = format!("0x{}", pubkey_hex);
        let mut db_result = Ok(());
        for attempt in 1..=3 {
            db_result = self
                .db
                .update_wallet_passkey(&req.key_id, &new_pk, None)
                .map(|_| ());
            if db_result.is_ok() {
                break;
            }
            eprintln!(
                "⚠️  ChangePasskey: DB update attempt {}/3 failed for key {}: {:?}",
                attempt, req.key_id, db_result
            );
            tokio::time::sleep(std::time::Duration::from_millis(100 * attempt)).await;
        }
        if let Err(e) = db_result {
            eprintln!(
                "🔴 CRITICAL: TA passkey changed but DB update FAILED for key {} — \
                 WebAuthn for this wallet will verify against a STALE pubkey. \
                 Manual recovery: UPDATE wallets SET passkey_pubkey='{}' WHERE key_id='{}'; \
                 error: {:?}",
                req.key_id, new_pk, req.key_id, e
            );
            return Err(anyhow!(
                "Passkey changed in TEE but metadata update failed — contact operator \
                 (wallet may not authenticate until DB is repaired): {}",
                e
            ));
        }

        Ok(ChangePasskeyResponse {
            key_id: req.key_id,
            changed: true,
        })
    }

    /// Parse API-layer PasskeyAssertion (hex strings) into proto::PasskeyAssertion (bytes).
    /// Returns None if no assertion provided — TA will decide whether to allow or reject.
    fn parse_passkey_assertion(
        passkey: Option<&PasskeyAssertion>,
    ) -> Result<Option<proto::PasskeyAssertion>> {
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
            // Legacy hex path carries no clientDataJSON. The TA treats this as the
            // transition/legacy case (issue #49): no challenge binding. This path is
            // already DEPRECATED + gated elsewhere; the WebAuthn ceremony path
            // (verify_authentication_response) is the one that gets challenge binding.
            client_data_json: None,
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
        let pk_bytes =
            hex::decode(pk_hex).map_err(|e| anyhow!("Invalid stored passkey pubkey hex: {}", e))?;

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

        verifying_key
            .verify(&msg, &signature)
            .map_err(|_| anyhow!("PassKey verification failed (CA pre-check)"))?;

        Ok(())
    }

    /// Pre-verify passkey at CA level if metadata has pubkey and assertion is present.
    /// Rejects bad signatures before they reach TA queue.
    async fn pre_verify_passkey(
        &self,
        key_id: &str,
        assertion: &Option<proto::PasskeyAssertion>,
    ) -> Result<()> {
        let pubkey_hex = self.db.get_wallet(key_id)?.and_then(|w| w.passkey_pubkey);

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
        // Issue #110: true when this op is re-verified inside the TA (signing/
        // mutating paths) → host delegates the challenge-value binding to the TA
        // (accepts a payload-commitment challenge, not just the bare nonce). false
        // for host-authoritative paths → keep strict `challenge == nonce`.
        delegate_challenge_to_ta: bool,
    ) -> Result<Option<proto::PasskeyAssertion>> {
        if let Some(wa) = wa {
            // WebAuthn ceremony path
            let challenge_row = self
                .db
                .consume_challenge(&wa.challenge_id)?
                .ok_or_else(|| anyhow!("Challenge not found or expired: {}", wa.challenge_id))?;

            // Reject operation-specific challenges (e.g. "grant-session") to prevent
            // cross-purpose replay. This resolver is for generic authentication only.
            if challenge_row.purpose != "authentication" {
                return Err(anyhow!(
                    "Challenge purpose '{}' cannot be used for this operation",
                    challenge_row.purpose
                ));
            }

            // challenge must be bound to this key
            if let Some(ref bound_key) = challenge_row.key_id {
                if bound_key != key_id {
                    return Err(anyhow!("Challenge bound to different key"));
                }
            }

            let w = self
                .db
                .get_wallet(key_id)?
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            let pubkey_hex = w
                .passkey_pubkey
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
                delegate_challenge_to_ta,
            )?;

            // Update sign_count in DB
            let _ = self
                .db
                .update_wallet_sign_count(key_id, verified.new_counter);

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

    /// P0-2: strict resolver for the signing / mutating endpoints
    /// (Sign, SignHash, DeriveAddress, DeleteKey, ChangePasskey).
    ///
    /// Two hardenings over `resolve_passkey_assertion`:
    /// 1. The legacy raw-hex path carries NO challenge binding — a captured
    ///    assertion is replayable forever. It is rejected here unless
    ///    `KMS_ALLOW_LEGACY_PASSKEY=1` is set (test environments only).
    /// 2. If the wallet has a passkey bound, an assertion is REQUIRED at the
    ///    CA layer (defence in depth). Previously we passed `None` through
    ///    and relied on the TA alone; every agent/p256/grant endpoint already
    ///    enforces presence host-side — the core KMS paths now match.
    ///
    /// Issue #42: host-side gate that rejects signing on a frozen (dormant) key
    /// before any TEE call. Returns Err("key is frozen") when the wallet's
    /// lifecycle_status is 'frozen'. Unknown/missing keys and 'active' keys pass
    /// through (callers do their own existence checks). This is a soft CA-layer
    /// gate, not a TEE-enforced lock — the private key material is untouched.
    ///
    /// TOCTOU note: this check and the subsequent TEE call are not atomic with the
    /// background dormant-sweep. The success tx_log row (which advances
    /// last_used_at) is written only AFTER sign() returns, so during an in-flight
    /// sign the sweep can still read a stale last_used_at, pass its own dormancy
    /// test, and freeze the key after this precheck but before the signature
    /// completes. This race is accepted by design and deliberately not locked
    /// against: (a) freeze is a dormancy re-verification gate, not key-security
    /// enforcement, and this sign is already authorized by a live WebAuthn
    /// ceremony; (b) the TEE has already produced the signature by the time any
    /// post-check could run, so it cannot be rolled back anyway. The only
    /// observable effect of losing the race is that the key ends up frozen right
    /// after this one signature, and the next operation needs an UnfreezeKey.
    fn ensure_not_frozen(&self, key_id: &str) -> Result<()> {
        if let Some(status) = self.db.get_lifecycle_status(key_id)? {
            if status == "frozen" {
                return Err(anyhow!("key is frozen"));
            }
        }
        Ok(())
    }

    /// Resolve a caller-supplied `account` to a wallet key_id. Accepts either the key_id
    /// (UUID) directly or a wallet **address** — the latter resolved via address_index, the
    /// same way the Sign/SignHash endpoints accept an address. This lets DVT (which has the
    /// userOp sender address) and the SDK (which uses the account Address) call the contact /
    /// confirm-verify endpoints without tracking the KMS UUID. Falls back to the original
    /// string when neither resolves, so downstream not-found handling is unchanged.
    fn resolve_account_key_id(&self, account: &str) -> Result<String> {
        if self.db.wallet_exists(account)? {
            return Ok(account.to_string());
        }
        if let Some(row) = self.db.lookup_address(account)? {
            return Ok(row.key_id);
        }
        Ok(account.to_string())
    }

    /// Wallets with no passkey bound (legacy/pre-passkey wallets) still pass
    /// `None` through; the TA applies its own policy for those.
    async fn resolve_passkey_assertion_strict(
        &self,
        key_id: &str,
        raw: Option<&PasskeyAssertion>,
        wa: Option<&WebAuthnAssertion>,
        // #110: true ONLY for ops the TA re-binds with a payload digest (Sign/
        // SignHash). The host then accepts a payload-commitment challenge and lets
        // the TA bind it. For nonce-only ops (DeriveAddress/DeleteKey/ChangePasskey
        // — TA enforces challenge==nonce) and HOST-ONLY ops that never reach the TA
        // (UnfreezeKey — flips lifecycle in the DB, no TA call), pass false so the
        // host keeps the authoritative `challenge == nonce` check. ⚠️ Passing true
        // for a host-only op (UnfreezeKey) would drop challenge binding entirely
        // (host skips it, TA never sees it) → replay of a captured assertion.
        delegate_challenge_to_ta: bool,
    ) -> Result<Option<proto::PasskeyAssertion>> {
        if raw.is_some() && wa.is_none() {
            let legacy_allowed =
                std::env::var("KMS_ALLOW_LEGACY_PASSKEY").ok().as_deref() == Some("1");
            if !legacy_allowed {
                return Err(anyhow!(
                    "Legacy raw passkey assertions are not accepted for signing/mutating \
                     operations (no challenge binding — replayable). Use the WebAuthn \
                     ceremony via /webauthn/begin-authentication. \
                     (KMS_ALLOW_LEGACY_PASSKEY=1 re-enables it for test environments only.)"
                ));
            }
            eprintln!(
                "⚠️  KMS_ALLOW_LEGACY_PASSKEY=1: accepting replayable legacy assertion \
                 for key_id={} — NEVER enable this in production",
                key_id
            );
        }

        // #110: delegate decision is per-op (see param doc) — true only for the
        // payload-commitment signers; false keeps the host as the authoritative
        // challenge==nonce check for nonce-only / host-only ops.
        let resolved = self
            .resolve_passkey_assertion(key_id, raw, wa, delegate_challenge_to_ta)
            .await?;

        if resolved.is_none() {
            // No assertion supplied at all. Only acceptable when the wallet
            // genuinely has no passkey bound.
            if let Some(w) = self.db.get_wallet(key_id)? {
                let has_passkey = w
                    .passkey_pubkey
                    .as_deref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if has_passkey {
                    return Err(anyhow!(
                        "Passkey authorization required: key {} has a passkey bound. \
                         Provide a WebAuthn assertion (begin-authentication ceremony).",
                        key_id
                    ));
                }
            }
        }

        Ok(resolved)
    }

    /// WebAuthn assertion resolver with operation-specific purpose check.
    /// Ensures the challenge was created specifically for the given purpose
    /// (e.g., "grant-session"), preventing cross-operation challenge replay.
    async fn resolve_grant_passkey_assertion(
        &self,
        key_id: &str,
        wa: &WebAuthnAssertion,
        required_purpose: &str,
    ) -> Result<proto::PasskeyAssertion> {
        let challenge_row = self
            .db
            .consume_challenge(&wa.challenge_id)?
            .ok_or_else(|| anyhow!("Challenge not found or expired: {}", wa.challenge_id))?;

        if challenge_row.purpose != required_purpose {
            return Err(anyhow!(
                "Challenge purpose '{}' is not valid for this operation (expected '{}')",
                challenge_row.purpose,
                required_purpose
            ));
        }

        if let Some(ref bound_key) = challenge_row.key_id {
            if bound_key != key_id {
                return Err(anyhow!("Challenge bound to different key"));
            }
        }

        let w = self
            .db
            .get_wallet(key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;
        let pubkey_hex = w
            .passkey_pubkey
            .ok_or_else(|| anyhow!("Wallet has no passkey public key"))?;
        let pk_bytes = hex::decode(pubkey_hex.trim_start_matches("0x"))
            .map_err(|e| anyhow!("Invalid stored passkey hex: {}", e))?;

        // #112: grant-session now uses a TA-issued nonce (begin_grant_session_auth →
        // GetChallenge), and the TA re-binds it at sign time (sign_grant_session /
        // sign_p256_grant_session call verify_passkey_for_wallet with Some(final_hash)
        // → payload commitment). So the host delegates the challenge-value check to
        // the TA (true) — exactly like the regular signing path — accepting a
        // payload-commitment challenge in strict, and the bare nonce in transition.
        // (Host still verifies signature + origin + rpId + one-time challenge_id.)
        let verified = webauthn::verify_authentication_response(
            &wa.credential,
            &challenge_row.challenge,
            &self.expected_origins,
            &challenge_row.rp_id,
            &pk_bytes,
            w.sign_count,
            true,
        )?;

        let _ = self
            .db
            .update_wallet_sign_count(key_id, verified.new_counter);

        // #112: DO NOT strip client_data_json anymore. The TA now holds the nonce
        // (GetChallenge) and is the authoritative binder — forward the assertion so
        // the TA verifies challenge↔nonce↔payload. (Pre-#112 the grant challenge was
        // host-random, absent from the TA pending table, so we had to strip; that is
        // no longer the case.)
        Ok(verified.proto_assertion)
    }

    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("📝 KMS DeriveAddress API called for key: {}", req.key_id);

        // CA-side validation before TA call
        let wallet_uuid = Self::validate_key_id(&req.key_id)?;
        Self::validate_derivation_path(&req.derivation_path)?;

        if !self.db.wallet_exists(&req.key_id)? {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }
        // Issue #42: reject dormant/frozen keys before any TEE call.
        self.ensure_not_frozen(&req.key_id)?;
        let passkey_assertion = self
            .resolve_passkey_assertion_strict(
                &req.key_id,
                req.passkey.as_ref(),
                req.webauthn.as_ref(),
                false, // #110: nonce-only op — TA enforces challenge==nonce; host stays strict
            )
            .await?;
        let address_bytes = self
            .tee
            .derive_address(wallet_uuid, &req.derivation_path, passkey_assertion)
            .await?;

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

            let row = self
                .db
                .lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found: {}", address))?;

            (Uuid::parse_str(&row.key_id)?, row.derivation_path)
        } else if let (Some(ref key_id), Some(ref path)) =
            (req.key_id.as_ref(), req.derivation_path.as_ref())
        {
            println!(
                "📝 KMS Sign API called with KeyId: {}, Path: {}",
                key_id, path
            );

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
        // Issue #42: reject dormant/frozen keys before any TEE call.
        self.ensure_not_frozen(&key_id_str)?;
        let passkey_assertion = self
            .resolve_passkey_assertion_strict(
                &key_id_str,
                req.passkey.as_ref(),
                req.webauthn.as_ref(),
                true, // #110: TA binds Some(tx/msg digest) — accept payload-commitment challenge
            )
            .await?;

        // Prepare sign payload
        let signature = if let Some(transaction) = req.transaction {
            println!("  📝 Transaction signing mode");
            let to_bytes = if transaction.to.starts_with("0x") {
                hex::decode(&transaction.to[2..])
            } else {
                hex::decode(&transaction.to)
            }?;
            if to_bytes.len() != 20 {
                return Err(anyhow!(
                    "Transaction.to must be 20 bytes (40 hex chars), got {} bytes",
                    to_bytes.len()
                ));
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
                gas_price: u128::from_str_radix(
                    &transaction.gas_price.trim_start_matches("0x"),
                    16,
                )?,
                gas: transaction.gas as u128,
                data,
            };
            self.tee
                .sign_transaction(
                    wallet_uuid,
                    &derivation_path,
                    eth_transaction,
                    passkey_assertion.clone(),
                )
                .await?
        } else if let Some(message) = req.message {
            println!("  📝 Message signing mode");
            let message_bytes = if message.starts_with("0x") {
                hex::decode(&message[2..])?
            } else {
                base64::decode(&message).unwrap_or_else(|_| message.as_bytes().to_vec())
            };
            self.tee
                .sign_message(
                    wallet_uuid,
                    &derivation_path,
                    &message_bytes,
                    passkey_assertion,
                )
                .await?
        } else {
            return Err(anyhow!("Either Transaction or Message must be provided"));
        };

        Ok(SignResponse {
            signature: hex::encode(&signature),
            transaction_hash: "[TX_HASH_OR_MESSAGE_HASH]".to_string(),
        })
    }

    /// #124 (DVT path-2): RP-verify a WebAuthn confirm-assertion. The account owner's
    /// passkey signs `challenge = userOpHash` (WYSIWYS) in YAA; a DVT node forwards the
    /// assertion here. Stateless + idempotent: no KMS nonce, sign_count=0 (counter check
    /// skipped + never updated) so the SAME assertion can be verified by every quorum
    /// node. Replay is bounded by userOpHash uniqueness + the node's single-use pending.
    /// A leaked secp256k1 owner key cannot produce a P256 WebAuthn assertion → this is
    /// what genuinely defends against owner-key theft (per Validator#124).
    pub async fn verify_confirm_assertion(
        &self,
        req: VerifyConfirmAssertionRequest,
    ) -> Result<bool> {
        // (opus review) Validate the request STRUCTURE first — independent of account
        // existence — so the only Err (→ 400) is genuinely-malformed caller input and does
        // NOT leak whether `account` exists. Every account-dependent outcome below
        // (not-found / no-passkey / dormant-frozen / bad stored key / bad signature)
        // returns Ok(false) → uniform 200 {verified:false}, no enumeration oracle.
        let uoh = hex::decode(req.user_op_hash.trim_start_matches("0x"))
            .map_err(|e| anyhow!("invalid userOpHash hex: {}", e))?;
        if uoh.len() != 32 {
            return Err(anyhow!("userOpHash must be 32 bytes"));
        }
        // account may be the wallet key_id OR an address (DVT has the userOp sender address).
        let key_id = self.resolve_account_key_id(&req.account)?;
        let wallet = match self.db.get_wallet(&key_id)? {
            Some(w) => w,
            None => return Ok(false),
        };
        // Defense-in-depth (opus review): a dormant/frozen wallet (#42) must not produce a
        // co-signable confirmation. The op may not re-route through this KMS's sign_hash
        // (path-2: final sig is owner/YAA-produced), so this is NOT redundant. Frozen →
        // Ok(false) (uniform, not an error → no oracle).
        if self.ensure_not_frozen(&key_id).is_err() {
            return Ok(false);
        }
        let pk_hex = match wallet.passkey_pubkey {
            Some(h) => h,
            None => return Ok(false),
        };
        let pk = match hex::decode(pk_hex.trim_start_matches("0x")) {
            Ok(b) => b,
            Err(_) => return Ok(false), // corrupt stored key = not verifiable, not a caller error
        };
        // TODO(multi-passkey): with >1 passkey per wallet, resolve the assertion's
        // credential_id to the SPECIFIC bound pubkey before verifying — do not accept any
        // of the account's keys. Inert today (single passkey_pubkey), load-bearing then.
        //
        // Try each configured rpId (prod = aastar.io only → strict; dev board also
        // localhost). delegate=false → host enforces challenge == userOpHash (WYSIWYS).
        // sign_count=0 → counter monotonicity check skipped and never updated, so this is
        // idempotent across the quorum (each node verifies the same assertion).
        let mut last_err = None;
        for rp_id in &self.rp_ids {
            match webauthn::verify_authentication_response(
                &req.passkey,
                &uoh,
                &self.expected_origins,
                rp_id,
                &pk,
                0,
                false,
            ) {
                Ok(_) => return Ok(true),
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            println!(
                "⚠️ verify_confirm_assertion: not verified for account={}: {}",
                req.account, e
            );
        }
        Ok(false)
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

            let row = self
                .db
                .lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found: {}", address))?;

            (Self::validate_key_id(&row.key_id)?, row.derivation_path)
        } else if let Some(key_id) = &req.key_id {
            println!("📝 KMS SignHash API called with KeyId: {}", key_id);

            let w = self
                .db
                .get_wallet(key_id)?
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            let derivation_path = req
                .derivation_path
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
        // Issue #42: reject dormant/frozen keys before any TEE call.
        self.ensure_not_frozen(&key_id_str)?;
        let passkey_assertion = self
            .resolve_passkey_assertion_strict(
                &key_id_str,
                req.passkey.as_ref(),
                req.webauthn.as_ref(),
                true, // #110: TA binds Some(hash) — accept payload-commitment challenge
            )
            .await?;

        let signature = self
            .tee
            .sign_hash(
                wallet_uuid,
                &derivation_path,
                &hash_array,
                passkey_assertion,
            )
            .await?;

        Ok(SignHashResponse {
            signature: hex::encode(&signature),
        })
    }

    pub async fn get_public_key(&self, req: GetPublicKeyRequest) -> Result<GetPublicKeyResponse> {
        println!("📝 KMS GetPublicKey API called for key: {}", req.key_id);

        let w = self
            .db
            .get_wallet(&req.key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        Ok(GetPublicKeyResponse {
            key_id: req.key_id,
            public_key: w
                .public_key
                .unwrap_or_else(|| "[PUBLIC_KEY_PENDING]".to_string()),
            key_usage: w.key_usage,
            key_spec: w.key_spec,
        })
    }

    pub async fn delete_key(&self, req: DeleteKeyRequest) -> Result<DeleteKeyResponse> {
        println!("📝 KMS DeleteKey API called for key: {}", req.key_id);

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
        // Check whether the stored passkey is a valid P-256 curve point.
        // If it isn't (a "gap key" created before the CreateKey validation was
        // tightened), skip passkey verification and TEE removal — the TEE has
        // no valid key material to protect, so the DB record is all that remains.
        let is_gap_key = self
            .db
            .get_wallet(&req.key_id)?
            .and_then(|w| w.passkey_pubkey)
            .and_then(|hex| hex::decode(hex.trim_start_matches("0x")).ok())
            .map(|bytes| p256::PublicKey::from_sec1_bytes(&bytes).is_err())
            .unwrap_or(false);

        if is_gap_key {
            // Gap key: passkey_pubkey is not a valid P-256 curve point.
            // Attempt TEE force-removal (ForceRemoveWallet = cmd 23, added in TA v0.20.0).
            // On older TA binaries this call returns "Unsupported command" — we log and
            // continue so the SQLite row is still cleaned up regardless.
            match self.tee.force_remove_wallet(wallet_uuid).await {
                Ok(()) => {
                    println!("✅ Gap key TEE entry purged (ForceRemoveWallet succeeded)");
                }
                Err(e) => {
                    // TA older than v0.20.0 — TEE orphan remains (~1-2 KB, inaccessible).
                    // SQLite cleanup still proceeds. Rebuild TA to fully resolve.
                    eprintln!(
                        "⚠️  Gap key TEE purge failed (TA may need rebuild): {}. \
                        SQLite row will still be deleted. TEE orphan is inaccessible.",
                        e
                    );
                }
            }
        } else {
            // Normal key: strict passkey/WebAuthn verification (audit-hardened) before removal.
            let passkey_assertion = self
                .resolve_passkey_assertion_strict(
                    &req.key_id,
                    req.passkey.as_ref(),
                    req.webauthn.as_ref(),
                    false, // #110: nonce-only op — TA enforces challenge==nonce; host stays strict
                )
                .await?;
            self.tee
                .remove_wallet(wallet_uuid, passkey_assertion)
                .await?;
        }

        // Remove from DB (CASCADE deletes address_index entries).
        // H-C: the TA wallet is already gone. If this DB delete is lost the
        // row becomes a "ghost" — later operations on it fail with confusing
        // TEE errors. Retry, and on persistent failure mark the row instead
        // of leaving it looking alive.
        let mut del_result = self.db.delete_wallet(&req.key_id).map(|_| ());
        for attempt in 1..=3u64 {
            if del_result.is_ok() {
                break;
            }
            eprintln!(
                "⚠️  DeleteKey: DB delete attempt {}/4 failed for {}: {:?}",
                attempt, req.key_id, del_result
            );
            tokio::time::sleep(std::time::Duration::from_millis(100 * attempt)).await;
            del_result = self.db.delete_wallet(&req.key_id).map(|_| ());
        }
        if let Err(e) = del_result {
            // Best effort: flag the ghost row so it doesn't pose as a live key.
            let _ = self.db.update_wallet_status(
                &req.key_id,
                "error",
                Some("TEE wallet deleted but DB row could not be removed — ghost row"),
            );
            eprintln!(
                "🔴 CRITICAL: TEE wallet {} deleted but DB row removal failed — \
                 ghost row flagged as 'error'. Manual cleanup: \
                 DELETE FROM wallets WHERE key_id='{}'; Error: {:?}",
                req.key_id, req.key_id, e
            );
        }

        let days = req.pending_window_in_days.unwrap_or(7);
        let deletion_date = Utc::now() + chrono::Duration::days(days as i64);

        Ok(DeleteKeyResponse {
            key_id: req.key_id,
            deletion_date,
        })
    }

    /// Issue #42: owner-authorized unfreeze. Verifies owner via WebAuthn (same
    /// strict passkey resolution as DeleteKey), then flips lifecycle_status
    /// frozen→active. No TEE call — this only touches host SQLite metadata.
    /// Idempotent: unfreezing an already-active key succeeds and returns 'active'.
    pub async fn unfreeze_key(&self, req: UnfreezeKeyRequest) -> Result<UnfreezeKeyResponse> {
        println!("📝 KMS UnfreezeKey API called for key: {}", req.key_id);

        // Existence + UUID validation (mirrors delete_key's parse).
        let _wallet_uuid = Uuid::parse_str(&req.key_id)?;
        let current = self
            .db
            .get_lifecycle_status(&req.key_id)?
            .ok_or_else(|| anyhow!("Key not found: {}", req.key_id))?;

        // Owner authentication — identical strict assertion path as DeleteKey.
        // Verifying ownership is required even when the key is already active so
        // the endpoint cannot be used as an unauthenticated key-state probe.
        // #110 (codex Q2): UnfreezeKey is HOST-ONLY — it flips lifecycle_status in
        // the DB and never calls the TA, so the host's challenge==nonce check is the
        // ONLY binding. MUST pass false; true would let a captured assertion be
        // replayed with a fresh challenge_id (host skips value, TA never sees it).
        self.resolve_passkey_assertion_strict(
            &req.key_id,
            req.passkey.as_ref(),
            req.webauthn.as_ref(),
            false,
        )
        .await?;

        if current != "frozen" {
            // Already active (or some future state): nothing to flip, report as-is.
            return Ok(UnfreezeKeyResponse {
                key_id: req.key_id,
                lifecycle_status: current,
            });
        }

        self.db.set_lifecycle_status(&req.key_id, "active")?;
        println!("✅ Key unfrozen: {}", req.key_id);

        Ok(UnfreezeKeyResponse {
            key_id: req.key_id,
            lifecycle_status: "active".to_string(),
        })
    }

    /// Admin force-purge: removes a key from TEE + SQLite without passkey verification.
    /// Used for: TEE orphans (SQLite row gone), test keys, gap keys.
    /// Requires KMS_ADMIN_TOKEN to be set in the environment.
    /// Returns (tee_purged, sqlite_deleted).
    ///
    /// DEV/TEST ONLY — compiled in only under the `admin-purge` feature.
    #[cfg(feature = "admin-purge")]
    pub async fn admin_purge_key(&self, key_id: &str, reason: &str) -> Result<(bool, bool)> {
        let wallet_uuid = Uuid::parse_str(key_id)?;

        println!("🔑 AdminPurgeKey: {} reason={}", key_id, reason);

        // Try TEE removal (ForceRemoveWallet = cmd 23).
        // Succeeds only if the entry exists in TEE and TA supports cmd 23.
        let tee_ok = match self.tee.force_remove_wallet(wallet_uuid).await {
            Ok(()) => {
                println!("  ✅ TEE entry purged");
                true
            }
            Err(e) => {
                eprintln!("  ⚠️  TEE purge failed (orphan or old TA): {}", e);
                false
            }
        };

        // Delete from SQLite (ignore if already gone).
        let sqlite_ok = match self.db.delete_wallet(key_id) {
            Ok(()) => {
                println!("  ✅ SQLite row deleted");
                true
            }
            Err(e) => {
                eprintln!("  ⚠️  SQLite delete failed (row may not exist): {}", e);
                false
            }
        };

        Ok((tee_ok, sqlite_ok))
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

    pub async fn begin_registration(
        &self,
        req: webauthn::BeginRegistrationRequest,
        origin_header: Option<&str>,
    ) -> Result<webauthn::RegistrationOptionsResponse> {
        let user_name = req.user_name.as_deref().unwrap_or("wallet-user");
        let user_display = req
            .user_display_name
            .as_deref()
            .unwrap_or("AirAccount Wallet");
        let rp_id = self.resolve_rp_id(origin_header);
        println!(
            "🔑 WebAuthn rpId resolved: {} (from origin: {:?})",
            rp_id, req.origin
        );

        let (challenge_id, challenge_bytes, resp) = webauthn::generate_registration_options(
            &self.rp_name,
            &rp_id,
            user_name,
            user_display,
            vec![],
        );

        self.db.store_challenge(
            &challenge_id,
            &challenge_bytes,
            None,
            "registration",
            &rp_id,
            300,
        )?;

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
            meta_json.as_bytes(),
            None,
            "registration_meta",
            &rp_id,
            300,
        )?;

        println!(
            "📝 WebAuthn BeginRegistration: challenge_id={}",
            challenge_id
        );
        Ok(resp)
    }

    pub async fn complete_registration(
        &self,
        req: webauthn::CompleteRegistrationRequest,
    ) -> Result<webauthn::CompleteRegistrationResponse> {
        // 1. Consume challenge
        let challenge_row = self
            .db
            .consume_challenge(&req.challenge_id)?
            .ok_or_else(|| anyhow!("Challenge not found or expired: {}", req.challenge_id))?;

        // 2. Load stashed metadata
        let meta_row = self
            .db
            .consume_challenge(&format!("{}_meta", req.challenge_id))?;
        let (description, key_usage, key_spec, origin) = if let Some(mr) = meta_row {
            let v: serde_json::Value = serde_json::from_slice(&mr.challenge).unwrap_or_default();
            (
                v["description"].as_str().unwrap_or("").to_string(),
                v["key_usage"].as_str().unwrap_or("SIGN_VERIFY").to_string(),
                v["key_spec"]
                    .as_str()
                    .unwrap_or("ECC_SECG_P256K1")
                    .to_string(),
                v["origin"].as_str().unwrap_or("EXTERNAL_KMS").to_string(),
            )
        } else {
            (
                req.description.unwrap_or_default(),
                req.key_usage.unwrap_or_else(|| "SIGN_VERIFY".to_string()),
                req.key_spec
                    .unwrap_or_else(|| "ECC_SECG_P256K1".to_string()),
                req.origin.unwrap_or_else(|| "EXTERNAL_KMS".to_string()),
            )
        };

        // 3. Verify attestation (use rpId from stored challenge, not hardcoded)
        let rp_id = &challenge_row.rp_id;
        let verified = webauthn::verify_registration_response(
            &req.credential,
            &challenge_row.challenge,
            &self.expected_origins,
            rp_id,
        )?;

        println!(
            "✅ WebAuthn registration verified, pubkey {} bytes, credential_id {} bytes",
            verified.public_key.len(),
            verified.credential_id.len()
        );

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
                    println!(
                        "✅ Background derivation done for {}: {}",
                        wallet_id, address_hex
                    );
                    let _ = db.update_wallet_derived(
                        &wallet_id.to_string(),
                        &address_hex,
                        &pubkey_hex,
                        &derivation_path,
                        "ready",
                    );
                    let _ = db.upsert_address(
                        &address_hex,
                        &wallet_id.to_string(),
                        &derivation_path,
                        Some(&pubkey_hex),
                    );
                }
                Err(e) => {
                    eprintln!("❌ Background derivation failed for {}: {}", wallet_id, e);
                    let _ = db.update_wallet_status(
                        &wallet_id.to_string(),
                        "error",
                        Some(&e.to_string()),
                    );
                }
            }
        });

        Ok(webauthn::CompleteRegistrationResponse {
            key_id: wallet_id.to_string(),
            credential_id: credential_id_b64,
            status: "deriving".to_string(),
        })
    }

    pub async fn begin_authentication(
        &self,
        req: webauthn::BeginAuthenticationRequest,
        origin_header: Option<&str>,
    ) -> Result<webauthn::AuthenticationOptionsResponse> {
        // Resolve key_id from KeyId or Address
        let key_id = if let Some(ref kid) = req.key_id {
            kid.clone()
        } else if let Some(ref addr) = req.address {
            let row = self
                .db
                .lookup_address(addr)?
                .ok_or_else(|| anyhow!("Address not found: {}", addr))?;
            row.key_id
        } else {
            return Err(anyhow!("Must provide either KeyId or Address"));
        };

        let w = self
            .db
            .get_wallet(&key_id)?
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

        // Issue #49: source the challenge from the TA so the authenticator signs
        // the exact nonce the TA will later verify + consume (anti-replay).
        // key_id is the TA wallet UUID string (see Self::validate_key_id / sign path).
        // Fallback: if the TA is older (no GetChallenge = 25) or transiently
        // unavailable, fall back to a host-generated random challenge so the
        // existing host-side binding still works (transition compatibility).
        //
        // Issue #68: the TA returns a plain random nonce. For a signing op the
        // client must use challenge = SHA-256(nonce || payload_digest) in the
        // WebAuthn ceremony; the TA recomputes + verifies that commitment at
        // signing time. The challenge issuance itself is payload-free.
        let (challenge_id, challenge_bytes, resp) = match uuid::Uuid::parse_str(&key_id) {
            Ok(wallet_uuid) => match self.tee.get_challenge(wallet_uuid).await {
                Ok(nonce) => {
                    println!(
                        "🔐 Issue #49: using TA-issued challenge nonce for key_id={}",
                        key_id
                    );
                    webauthn::generate_authentication_options_with_challenge(
                        &rp_id,
                        allow_credentials,
                        nonce,
                    )
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  Issue #49: TA GetChallenge unavailable ({}); falling back to \
                         host-random challenge (TA will use legacy/transition path)",
                        e
                    );
                    webauthn::generate_authentication_options(&rp_id, allow_credentials)
                }
            },
            Err(_) => {
                // key_id is not a UUID (should not happen for TA wallets) — keep legacy behavior.
                webauthn::generate_authentication_options(&rp_id, allow_credentials)
            }
        };

        self.db.store_challenge(
            &challenge_id,
            &challenge_bytes,
            Some(&key_id),
            "authentication",
            &rp_id,
            300,
        )?;

        println!(
            "📝 WebAuthn BeginAuthentication: challenge_id={}, key_id={}",
            challenge_id, key_id
        );
        Ok(resp)
    }

    /// Start a purpose-bound WebAuthn challenge for grant-session signing.
    /// The stored challenge has purpose="grant-session", which sign_grant_session
    /// and sign_p256_grant_session verify before accepting the assertion.
    pub async fn begin_grant_session_auth(
        &self,
        key_id: &str,
        origin_header: Option<&str>,
    ) -> Result<webauthn::AuthenticationOptionsResponse> {
        let w = self
            .db
            .get_wallet(key_id)?
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

        // #112: source the grant-session challenge from the TA (GetChallenge) so it
        // lands in the TA's pending-nonce table and the TA can bind it at sign time
        // (sign_grant_session / sign_p256_grant_session pass Some(final_hash) → payload
        // commitment). This mirrors the regular BeginAuthentication path and lets the
        // resolver stop stripping client_data_json (so the TA — not just the host —
        // verifies the challenge). Fallback to a host-random challenge only if the
        // TA GetChallenge is unavailable (older TA / transient).
        let (challenge_id, challenge_bytes, resp) = match uuid::Uuid::parse_str(key_id) {
            Ok(wallet_uuid) => match self.tee.get_challenge(wallet_uuid).await {
                Ok(nonce) => {
                    println!(
                        "🔐 #112: using TA-issued nonce for grant-session key_id={}",
                        key_id
                    );
                    webauthn::generate_authentication_options_with_challenge(
                        &rp_id,
                        allow_credentials,
                        nonce,
                    )
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  #112: TA GetChallenge unavailable ({}); grant-session falls back to \
                         host-random challenge (TA legacy path)",
                        e
                    );
                    webauthn::generate_authentication_options(&rp_id, allow_credentials)
                }
            },
            Err(_) => webauthn::generate_authentication_options(&rp_id, allow_credentials),
        };

        self.db.store_challenge(
            &challenge_id,
            &challenge_bytes,
            Some(key_id),
            "grant-session",
            &rp_id,
            300,
        )?;

        println!(
            "📝 WebAuthn BeginGrantSessionAuth: challenge_id={}, key_id={}",
            challenge_id, key_id
        );
        Ok(resp)
    }

    // ========================================
    // Agent Key methods
    // ========================================

    pub async fn create_agent_key(
        &self,
        req: CreateAgentKeyRequest,
    ) -> Result<CreateAgentKeyResponse> {
        let wallet_id = Self::validate_key_id(&req.human_key_id)?;

        // Verify human wallet exists
        let _wallet = self
            .db
            .get_wallet(&req.human_key_id)?
            .ok_or_else(|| anyhow!("Human wallet not found: {}", req.human_key_id))?;

        // Agent operations MUST use WebAuthn ceremony (challenge-based) to prevent replay attacks.
        // Legacy raw passkey assertions lack challenge/origin binding and can be replayed.
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!("create-agent-key requires WebAuthn ceremony (BeginAuthentication flow). Legacy passkey assertions are not accepted for agent operations."));
        }
        let assertion = self
            .resolve_passkey_assertion(
                &req.human_key_id,
                None, // reject legacy path
                req.webauthn_assertion.as_ref(),
                // #115: the TA now re-binds the mint challenge to the label
                // (Some(mint_label_digest)), so the host must DELEGATE the
                // challenge-value check (true) — otherwise it rejects the
                // SHA-256(nonce‖digest) commitment as "not the bare nonce" before
                // it reaches the TA. (host still verifies sig+origin+rpId+one-time.)
                true,
            )
            .await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to create agent key"));
        }

        // Atomically allocate the next agent_index (MAX+1 in a single lock acquire).
        // Avoids the race between count() and insert() that could yield duplicate indices.
        let agent_index = self.db.next_agent_index_for_wallet(&req.human_key_id)?;

        // Derive agent key in TEE; TA constructs JWT payload internally (no oracle exposure).
        // TA computes iat from its own clock — host no longer supplies iat.
        let tee_result = self
            .tee
            .create_agent_key(
                wallet_id,
                agent_index,
                &req.human_key_id,
                24 * 3600, // #115: 24h cap (was 3d)
                assertion,
                &req.label, // #115: bound into mint commitment
                false,      // #115: CREATE (binds label)
            )
            .await?;
        let agent_address = format!("0x{}", hex::encode(&tee_result.agent_address));
        let pubkey_hex = hex::encode(&tee_result.public_key_compressed);
        let derivation_path = format!("m/44'/60'/0'/1/{}", agent_index);

        // Assemble JWT from TEE-produced material (no host-side signing)
        let (jwt, expires_at) = agent_jwt::assemble_jwt(&tee_result)?;

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
        println!(
            "✅ CreateAgentKey: wallet={} idx={} addr={}",
            req.human_key_id, agent_index, agent_address
        );

        Ok(CreateAgentKeyResponse {
            key_id,
            agent_address,
            derivation_path,
            agent_credential: jwt,
            expires_at,
        })
    }

    pub async fn sign_agent(
        &self,
        bearer_jwt: String,
        req: SignAgentRequest,
    ) -> Result<SignAgentResponse> {
        // Verify JWT via TEE HMAC
        let payload = agent_jwt::verify_credential(&self.tee, &bearer_jwt)
            .await
            .map_err(|e| anyhow!("Invalid agent credential: {}", e))?;

        // Validate keyId matches JWT payload
        let (wallet_uuid, agent_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();
        if payload.wallet_id != wallet_id_str || payload.agent_index != agent_index {
            return Err(anyhow!("keyId does not match agent credential"));
        }
        // Issue #42: reject dormant/frozen parent wallet before any TEE signing.
        self.ensure_not_frozen(&wallet_id_str)?;

        // Per-credential rate limit (design §2.2): prevents single compromised key from
        // flooding TEE signing. Keyed by wallet_id/agent_index — independent of global API key limit.
        let cred_rl_key = format!("{}/{}", wallet_id_str, agent_index);
        self.agent_rate_limiter
            .check(&cred_rl_key)
            .map_err(|limit| {
                anyhow!(
                    "Per-credential rate limit exceeded ({}/min). Retry after 60s.",
                    limit
                )
            })?;

        // Check agent key is active in DB + credential_hash matches
        let agent_key = self
            .db
            .get_agent_key(&wallet_id_str, agent_index)?
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
        let sig_bytes = self
            .tee
            .sign_agent_user_op(
                wallet_uuid,
                agent_index,
                &user_op_hash,
                jwt_kid,
                jwt_signing_input,
                jwt_hmac,
                account_address,
            )
            .await?;

        println!(
            "✅ SignAgent: wallet={} idx={} addr={}",
            wallet_id_str, agent_index, agent_key.agent_address
        );

        Ok(SignAgentResponse {
            key_id: req.key_id,
            agent_address: agent_key.agent_address,
            signature: format!("0x{}", hex::encode(&sig_bytes)),
        })
    }

    pub async fn sign_typed_data(
        &self,
        bearer: Option<String>,
        req: SignTypedDataRequest,
    ) -> Result<SignTypedDataResponse> {
        let wallet_id = Self::validate_key_id(&req.key_id)?;
        let wallet_id_str = wallet_id.to_string();
        // Issue #42: reject dormant/frozen keys before any TEE call. Covers the
        // EIP-712 family (voucher / gtoken / x402 all route through this method).
        self.ensure_not_frozen(&wallet_id_str)?;

        // Auth gate: require one of two paths.
        // Path A — Bearer JWT (agent key): user previously authorized via WebAuthn; checked against
        //           DB active-status, credential_hash, and per-credential rate limit. JWT path is
        //           locked to the agent's own derivation path — it cannot escalate to the owner root key.
        // Path B — WebAuthn ceremony assertion: live, challenge-bound proof of ownership; replay-resistant.
        // Legacy raw passkeyAssertion is NOT accepted for sign-typed-data (no challenge binding → replay risk).
        // No auth at all → reject.
        let passkey_assertion = match (&bearer, &req.webauthn_assertion) {
            (Some(jwt), _) => {
                // Path A: agent key JWT
                let payload = agent_jwt::verify_credential(&self.tee, jwt)
                    .await
                    .map_err(|e| anyhow!("Invalid agent credential for sign-typed-data: {}", e))?;
                if payload.wallet_id != wallet_id_str {
                    return Err(anyhow!("Agent credential wallet does not match keyId"));
                }

                // Enforce: JWT can only sign on its own agent derivation path, not the owner root key.
                let expected_path = format!("m/44'/60'/0'/1/{}", payload.agent_index);
                if req.hd_path != expected_path {
                    return Err(anyhow!(
                        "Agent credential may only sign typed-data on path '{}' (requested '{}'). \
                         Use WebAuthn to sign on other paths.",
                        expected_path,
                        req.hd_path
                    ));
                }

                // DB checks: active status + credential_hash match (same pattern as sign_agent)
                let agent_key = self
                    .db
                    .get_agent_key(&wallet_id_str, payload.agent_index)?
                    .ok_or_else(|| {
                        anyhow!(
                            "Agent key not found: {}:{}",
                            wallet_id_str,
                            payload.agent_index
                        )
                    })?;
                if agent_key.status != "active" {
                    return Err(anyhow!("Agent key is revoked"));
                }
                let current_hash = agent_jwt::credential_hash(jwt);
                if agent_key.credential_hash.as_deref() != Some(current_hash.as_str()) {
                    return Err(anyhow!("Agent credential has been superseded or revoked"));
                }

                // Per-credential rate limit (shared key with sign_agent — same credential budget)
                let cred_rl_key = format!("{}/{}", wallet_id_str, payload.agent_index);
                self.agent_rate_limiter
                    .check(&cred_rl_key)
                    .map_err(|limit| {
                        anyhow!(
                            "Per-credential rate limit exceeded ({}/min). Retry after 60s.",
                            limit
                        )
                    })?;

                None // no passkey forwarded to TA; JWT auth is host-enforced
            }
            (None, Some(_)) => {
                // Path B: WebAuthn ceremony (preferred, replay-protected, no hdPath restriction)
                // #110: the TA's sign_typed_data binds the challenge to the EIP-712
                // digest — verify_passkey_for_wallet(..., Some(&digest)) — so the signed
                // challenge is the payload COMMITMENT, not the bare nonce. Delegate the
                // challenge-value check to the TA (true), exactly like SignHash/Sign;
                // otherwise the host's bare-nonce check blocks the commitment (liveness)
                // and, in transition, the typed-data payload stays unbound (V4 CA-swap
                // hole). Covers the convenience signers (micropayment / GToken / x402)
                // that route through sign_typed_data.
                let assertion = self
                    .resolve_passkey_assertion(
                        &req.key_id,
                        None,
                        req.webauthn_assertion.as_ref(),
                        true,
                    )
                    .await?;
                if assertion.is_none() {
                    return Err(anyhow!(
                        "WebAuthn assertion verification failed for sign-typed-data"
                    ));
                }
                assertion
            }
            (None, None) => {
                return Err(anyhow!(
                    "sign-typed-data requires authentication: \
                     provide Authorization: Bearer <agent-jwt> OR webAuthnAssertion. \
                     Legacy passkeyAssertion is not accepted for this endpoint."
                ));
            }
        };

        // Convert domain verifyingContract from hex string to [u8; 20]
        let verifying_contract = match &req.domain.verifying_contract {
            Some(hex_str) => {
                let bytes = hex::decode(hex_str.trim_start_matches("0x"))
                    .map_err(|e| anyhow!("Invalid verifyingContract hex: {}", e))?;
                if bytes.len() != 20 {
                    return Err(anyhow!(
                        "verifyingContract must be 20 bytes, got {}",
                        bytes.len()
                    ));
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

        let types: Vec<proto::Eip712TypeDef> = req
            .types
            .iter()
            .map(|td| proto::Eip712TypeDef {
                name: td.name.clone(),
                fields: td
                    .fields
                    .iter()
                    .map(|f| proto::Eip712TypeField {
                        name: f.name.clone(),
                        field_type: f.field_type.clone(),
                    })
                    .collect(),
            })
            .collect();

        // Find the primary type definition to help with value conversion
        let primary_type_def = req
            .types
            .iter()
            .find(|td| td.name == req.primary_type)
            .ok_or_else(|| anyhow!("Primary type '{}' not in types list", req.primary_type))?;

        // Convert JSON field values to proto Eip712Value using declared field types for guidance
        let message = req
            .message
            .iter()
            .map(|fv| {
                let declared_type = primary_type_def
                    .fields
                    .iter()
                    .find(|f| f.name == fv.name)
                    .map(|f| f.field_type.as_str())
                    .unwrap_or("");
                let value = json_to_eip712_value(&fv.value, declared_type)?;
                Ok(proto::Eip712FieldValue {
                    name: fv.name.clone(),
                    value,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Extract JWT proof for TA-side verification on JWT path (defense-in-depth).
        let (jwt_kid, jwt_signing_input, jwt_hmac) = if let Some(ref jwt) = bearer {
            let (kid, si, hmac) = agent_jwt::extract_signing_proof(jwt)
                .map_err(|e| anyhow!("Failed to extract JWT proof: {}", e))?;
            (Some(kid), Some(si), Some(hmac))
        } else {
            (None, None, None)
        };

        let ta_input = proto::SignTypedDataInput {
            wallet_id,
            hd_path: req.hd_path.clone(),
            domain,
            primary_type: req.primary_type.clone(),
            types,
            message,
            passkey_assertion,
            jwt_kid,
            jwt_signing_input,
            jwt_hmac,
        };

        let output = self.tee.sign_typed_data(ta_input).await?;

        println!(
            "✅ SignTypedData: keyId={} primaryType={}",
            req.key_id, req.primary_type
        );
        Ok(SignTypedDataResponse {
            key_id: req.key_id,
            signature: format!("0x{}", hex::encode(&output.signature)),
        })
    }

    // P2 convenience signers build a fixed EIP-712 struct in the host and delegate to
    // sign_typed_data(), inheriting its auth gate (Bearer agent-JWT OR replay-protected
    // WebAuthn ceremony; legacy raw passkey is rejected). They add no new TA command.

    pub async fn sign_micropayment_voucher(
        &self,
        bearer: Option<String>,
        req: SignMicropaymentVoucherRequest,
    ) -> Result<SignTypedDataResponse> {
        let std_req = SignTypedDataRequest {
            key_id: req.key_id,
            hd_path: req.hd_path,
            domain: JsonEip712Domain {
                name: Some("MicroPaymentChannel".to_string()),
                // EIP-712 version must match the contract's _domainNameAndVersion()
                // (MicroPaymentChannel.sol returns "1.0.0"); "1" would make the
                // domainSeparator differ and voucher signatures fail on-chain (#21).
                version: Some("1.0.0".to_string()),
                chain_id: Some(req.chain_id),
                verifying_contract: Some(req.verifying_contract),
            },
            primary_type: "Voucher".to_string(),
            types: vec![JsonEip712TypeDef {
                name: "Voucher".to_string(),
                fields: vec![
                    JsonEip712TypeField {
                        name: "channelId".to_string(),
                        field_type: "bytes32".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "cumulativeAmount".to_string(),
                        field_type: "uint256".to_string(),
                    },
                ],
            }],
            message: vec![
                JsonEip712FieldValue {
                    name: "channelId".to_string(),
                    value: serde_json::Value::String(req.channel_id),
                },
                JsonEip712FieldValue {
                    name: "cumulativeAmount".to_string(),
                    value: serde_json::Value::String(req.cumulative_amount),
                },
            ],
            webauthn_assertion: req.webauthn_assertion,
            passkey_assertion: None,
        };
        self.sign_typed_data(bearer, std_req).await
    }

    pub async fn sign_gtoken_authorization(
        &self,
        bearer: Option<String>,
        req: SignGTokenAuthorizationRequest,
    ) -> Result<SignTypedDataResponse> {
        // #52: verify `from` equals the address actually derived from keyId+hdPath.
        // EIP-3009 TransferWithAuthorization is checked on-chain by
        // ecrecover(hash, sig) == from; signing with a key whose address != from
        // makes the transfer revert and wastes the user's gas. Catch it here with a
        // clear error. The address must already be cached (DeriveAddress called at
        // least once for this key+path); if not, refuse rather than risk a revert.
        match self.db.address_for_key_path(&req.key_id, &req.hd_path)? {
            Some(addr) => {
                // Normalize both sides — strip an optional 0x/0X prefix and
                // lower-case — so a mere format difference ("0xABCD" vs "abcd")
                // is not treated as a from-mismatch. Ethereum addresses are
                // case-insensitive at the byte level (EIP-55 checksum is
                // display-only), and callers may or may not include the prefix.
                let norm = |s: &str| {
                    s.strip_prefix("0x")
                        .or_else(|| s.strip_prefix("0X"))
                        .unwrap_or(s)
                        .to_ascii_lowercase()
                };
                if norm(&addr) != norm(&req.from) {
                    return Err(anyhow!(
                        "from {} does not match the address derived from keyId+hdPath \
                         ({}); EIP-3009 would revert on-chain",
                        req.from,
                        addr
                    ));
                }
            }
            None => {
                return Err(anyhow!(
                    "no cached address for this keyId+hdPath — call DeriveAddress first \
                     so the signer can verify `from` (prevents an on-chain EIP-3009 revert)"
                ));
            }
        }

        let std_req = SignTypedDataRequest {
            key_id: req.key_id,
            hd_path: req.hd_path,
            domain: JsonEip712Domain {
                name: Some("GToken".to_string()),
                version: Some("1".to_string()),
                chain_id: Some(req.chain_id),
                verifying_contract: Some(req.gtoken_address),
            },
            primary_type: "TransferWithAuthorization".to_string(),
            types: vec![JsonEip712TypeDef {
                name: "TransferWithAuthorization".to_string(),
                fields: vec![
                    JsonEip712TypeField {
                        name: "from".to_string(),
                        field_type: "address".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "to".to_string(),
                        field_type: "address".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "value".to_string(),
                        field_type: "uint256".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "validAfter".to_string(),
                        field_type: "uint256".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "validBefore".to_string(),
                        field_type: "uint256".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "nonce".to_string(),
                        field_type: "bytes32".to_string(),
                    },
                ],
            }],
            message: vec![
                JsonEip712FieldValue {
                    name: "from".to_string(),
                    value: serde_json::Value::String(req.from),
                },
                JsonEip712FieldValue {
                    name: "to".to_string(),
                    value: serde_json::Value::String(req.to),
                },
                JsonEip712FieldValue {
                    name: "value".to_string(),
                    value: serde_json::Value::String(req.value),
                },
                JsonEip712FieldValue {
                    name: "validAfter".to_string(),
                    value: serde_json::Value::String(req.valid_after),
                },
                JsonEip712FieldValue {
                    name: "validBefore".to_string(),
                    value: serde_json::Value::String(req.valid_before),
                },
                JsonEip712FieldValue {
                    name: "nonce".to_string(),
                    value: serde_json::Value::String(req.nonce),
                },
            ],
            webauthn_assertion: req.webauthn_assertion,
            passkey_assertion: None,
        };
        self.sign_typed_data(bearer, std_req).await
    }

    pub async fn sign_x402_payment(
        &self,
        bearer: Option<String>,
        req: SignX402PaymentRequest,
    ) -> Result<SignTypedDataResponse> {
        let std_req = SignTypedDataRequest {
            key_id: req.key_id,
            hd_path: req.hd_path,
            domain: JsonEip712Domain {
                name: Some("SuperPaymaster".to_string()),
                version: Some("1".to_string()),
                chain_id: Some(req.chain_id),
                verifying_contract: Some(req.verifying_contract),
            },
            primary_type: "PaymentPayload".to_string(),
            types: vec![JsonEip712TypeDef {
                name: "PaymentPayload".to_string(),
                fields: vec![
                    JsonEip712TypeField {
                        name: "paymentId".to_string(),
                        field_type: "bytes32".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "amount".to_string(),
                        field_type: "uint256".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "recipient".to_string(),
                        field_type: "address".to_string(),
                    },
                    JsonEip712TypeField {
                        name: "deadline".to_string(),
                        field_type: "uint256".to_string(),
                    },
                ],
            }],
            message: vec![
                JsonEip712FieldValue {
                    name: "paymentId".to_string(),
                    value: serde_json::Value::String(req.payment_id),
                },
                JsonEip712FieldValue {
                    name: "amount".to_string(),
                    value: serde_json::Value::String(req.amount),
                },
                JsonEip712FieldValue {
                    name: "recipient".to_string(),
                    value: serde_json::Value::String(req.recipient),
                },
                JsonEip712FieldValue {
                    name: "deadline".to_string(),
                    value: serde_json::Value::String(req.deadline),
                },
            ],
            webauthn_assertion: req.webauthn_assertion,
            passkey_assertion: None,
        };
        self.sign_typed_data(bearer, std_req).await
    }

    pub async fn sign_grant_session(
        &self,
        req: SignGrantSessionRequest,
    ) -> Result<SignGrantSessionResponse> {
        let wallet_id = Self::validate_key_id(&req.key_id)?;
        let key_id_str = wallet_id.to_string();
        // Issue #42: reject dormant/frozen keys before any TEE call.
        self.ensure_not_frozen(&key_id_str)?;

        // Grant signing requires purpose-bound WebAuthn challenge to prevent
        // cross-operation replay. Challenge must have been created by begin-grant-session-auth.
        let wa = req.webauthn_assertion.as_ref().ok_or_else(|| {
            anyhow!("sign-grant-session requires WebAuthn ceremony started via /kms/begin-grant-session-auth")
        })?;
        let passkey_assertion = Some(
            self.resolve_grant_passkey_assertion(&key_id_str, wa, "grant-session")
                .await?,
        );

        // expiry is uint48 in the contract — reject out-of-range values to keep hash match
        const UINT48_MAX: u64 = (1u64 << 48) - 1;
        if req.expiry > UINT48_MAX {
            return Err(anyhow!(
                "expiry {} exceeds uint48 max ({})",
                req.expiry,
                UINT48_MAX
            ));
        }

        let verifying_contract = Self::parse_address_hex(&req.verifying_contract)?;
        let account = Self::parse_address_hex(&req.account)?;
        let session_key = Self::parse_address_hex(&req.session_key)?;
        let contract_scope = Self::parse_address_hex(&req.contract_scope)?;
        let selector_scope = parse_bytes4_hex(&req.selector_scope)?;

        let mut call_targets = Vec::with_capacity(req.call_targets.len());
        for s in &req.call_targets {
            call_targets.push(Self::parse_address_hex(s)?);
        }
        let mut selector_allowlist = Vec::with_capacity(req.selector_allowlist.len());
        for s in &req.selector_allowlist {
            selector_allowlist.push(parse_bytes4_hex(s)?);
        }

        let mut nonce = [0u8; 32];
        nonce[24..32].copy_from_slice(&req.nonce.to_be_bytes());

        let ta_input = proto::SignGrantSessionInput {
            wallet_id,
            hd_path: req.hd_path,
            chain_id: req.chain_id,
            verifying_contract,
            account,
            session_key,
            expiry: req.expiry,
            contract_scope,
            selector_scope,
            velocity_limit: req.velocity_limit,
            velocity_window: req.velocity_window,
            call_targets,
            selector_allowlist,
            nonce,
            passkey_assertion,
        };

        let output = self.tee.sign_grant_session(ta_input).await?;
        println!("✅ SignGrantSession: keyId={}", req.key_id);
        Ok(SignGrantSessionResponse {
            key_id: req.key_id,
            signature: format!("0x{}", hex::encode(&output.signature)),
        })
    }

    pub async fn sign_p256_grant_session(
        &self,
        req: SignP256GrantSessionRequest,
    ) -> Result<SignP256GrantSessionResponse> {
        let wallet_id = Self::validate_key_id(&req.key_id)?;
        let key_id_str = wallet_id.to_string();
        // Issue #42: reject dormant/frozen keys before any TEE call.
        self.ensure_not_frozen(&key_id_str)?;

        let wa = req.webauthn_assertion.as_ref().ok_or_else(|| {
            anyhow!("sign-p256-grant-session requires WebAuthn ceremony started via /kms/begin-grant-session-auth")
        })?;
        let passkey_assertion = Some(
            self.resolve_grant_passkey_assertion(&key_id_str, wa, "grant-session")
                .await?,
        );

        const UINT48_MAX: u64 = (1u64 << 48) - 1;
        if req.expiry > UINT48_MAX {
            return Err(anyhow!(
                "expiry {} exceeds uint48 max ({})",
                req.expiry,
                UINT48_MAX
            ));
        }

        let verifying_contract = Self::parse_address_hex(&req.verifying_contract)?;
        let account = Self::parse_address_hex(&req.account)?;
        let key_x = Self::validate_hash_hex(&req.key_x)?;
        let key_y = Self::validate_hash_hex(&req.key_y)?;
        let contract_scope = Self::parse_address_hex(&req.contract_scope)?;
        let selector_scope = parse_bytes4_hex(&req.selector_scope)?;

        let mut call_targets = Vec::with_capacity(req.call_targets.len());
        for s in &req.call_targets {
            call_targets.push(Self::parse_address_hex(s)?);
        }
        let mut selector_allowlist = Vec::with_capacity(req.selector_allowlist.len());
        for s in &req.selector_allowlist {
            selector_allowlist.push(parse_bytes4_hex(s)?);
        }

        let mut nonce = [0u8; 32];
        nonce[24..32].copy_from_slice(&req.nonce.to_be_bytes());

        let ta_input = proto::SignP256GrantSessionInput {
            wallet_id,
            hd_path: req.hd_path,
            chain_id: req.chain_id,
            verifying_contract,
            account,
            key_x,
            key_y,
            expiry: req.expiry,
            contract_scope,
            selector_scope,
            velocity_limit: req.velocity_limit,
            velocity_window: req.velocity_window,
            call_targets,
            selector_allowlist,
            nonce,
            passkey_assertion,
        };

        let output = self.tee.sign_p256_grant_session(ta_input).await?;
        println!("✅ SignP256GrantSession: keyId={}", req.key_id);
        Ok(SignP256GrantSessionResponse {
            key_id: req.key_id,
            signature: format!("0x{}", hex::encode(&output.signature)),
        })
    }

    pub async fn refresh_agent_credential(
        &self,
        bearer_jwt: String,
        req: RefreshAgentCredentialRequest,
    ) -> Result<CreateAgentKeyResponse> {
        // Verify current JWT is still valid
        let payload = agent_jwt::verify_credential(&self.tee, &bearer_jwt)
            .await
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
        let assertion = self
            .resolve_passkey_assertion(
                &wallet_id_str,
                None, // reject legacy path
                req.webauthn_assertion.as_ref(),
                // #115: refresh re-mints via CreateAgentKey, which the TA now binds to
                // the (empty) label digest → delegate the challenge-value check to the TA.
                true,
            )
            .await?;
        if assertion.is_none() {
            return Err(anyhow!(
                "Passkey assertion required to refresh agent credential"
            ));
        }

        // Check agent key is active
        let agent_key = self
            .db
            .get_agent_key(&wallet_id_str, agent_index)?
            .ok_or_else(|| anyhow!("Agent key not found: {}", req.key_id))?;
        if agent_key.status != "active" {
            return Err(anyhow!("Agent key is revoked"));
        }

        // Re-derive agent key in TEE with fresh JWT (same key, new TTL — idempotent derivation).
        // TA computes iat from its own clock — host no longer supplies iat.
        // #115: refresh does not (re)set a label — the key identity is fixed by key_id —
        // so bind the empty label (digest still binds wallet_id). The refreshing client
        // commits to the empty label likewise.
        let tee_result = self
            .tee
            .create_agent_key(
                wallet_uuid,
                agent_index,
                &wallet_id_str,
                24 * 3600, // #115: 24h cap (was 3d)
                assertion,
                "",   // #115: refresh binds the index, not a label
                true, // #115: REFRESH (binds agent_index under a distinct tag)
            )
            .await?;
        let (new_jwt, expires_at) = agent_jwt::assemble_jwt(&tee_result)?;

        let cred_hash = agent_jwt::credential_hash(&new_jwt);
        self.db
            .update_agent_credential(&wallet_id_str, agent_index, &cred_hash, expires_at)?;

        let derivation_path = format!("m/44'/60'/0'/1/{}", agent_index);
        println!(
            "✅ RefreshAgentCredential: wallet={} idx={}",
            wallet_id_str, agent_index
        );

        Ok(CreateAgentKeyResponse {
            key_id: req.key_id,
            agent_address: agent_key.agent_address,
            derivation_path,
            agent_credential: new_jwt,
            expires_at,
        })
    }

    pub async fn revoke_agent_credential(
        &self,
        req: RevokeAgentCredentialRequest,
    ) -> Result<RevokeAgentCredentialResponse> {
        let (wallet_uuid, agent_index) = parse_agent_key_id(&req.key_id)?;
        let wallet_id_str = wallet_uuid.to_string();

        // Require WebAuthn ceremony for replay protection (no legacy passkey path)
        if req.webauthn_assertion.is_none() {
            return Err(anyhow!("revoke-agent-credential requires WebAuthn ceremony. Legacy passkey assertions are not accepted for agent operations."));
        }
        let assertion = self
            .resolve_passkey_assertion(
                &wallet_id_str,
                None, // reject legacy path
                req.webauthn_assertion.as_ref(),
                false, // #110: agent path is host-authoritative → keep challenge==nonce
            )
            .await?;
        if assertion.is_none() {
            return Err(anyhow!("Passkey assertion required to revoke agent key"));
        }

        // Revoke in DB
        let revoked = self.db.revoke_agent_key(&wallet_id_str, agent_index)?;
        if !revoked {
            return Err(anyhow!(
                "Agent key not found or already revoked: {}",
                req.key_id
            ));
        }

        let revoked_at = Utc::now().timestamp();
        println!(
            "✅ RevokeAgentCredential: wallet={} idx={}",
            wallet_id_str, agent_index
        );

        Ok(RevokeAgentCredentialResponse {
            success: true,
            revoked_at,
        })
    }

    /// Lazy GC: delete expired P256 session keys for a wallet from DB and TEE.
    /// Called silently on create/sign/revoke — errors are logged, never propagated.
    ///
    /// Ordering: DB-first (atomic claim) → then TEE delete → then tee_deleted=1.
    /// Pass 0 (retry): rows with status='revoked' AND tee_deleted=0 from a prior failed
    /// TEE delete are retried immediately. This closes the ghost-TEE gap where a key
    /// was claimed in DB but TEE delete failed — such rows are excluded from list_expired
    /// (status='revoked'), but list_unconfirmed_tee_deletes catches them for retry.
    ///
    /// `exclude_session_index`: skip this index during sign (prevents GC-ing the key in use).
    /// Grace window: gc_cutoff = now - 60s so keys are GC-eligible 60s after credential_expires_at.
    async fn gc_expired_p256_session_keys(
        &self,
        wallet_id_str: &str,
        wallet_uuid: uuid::Uuid,
        exclude_session_index: Option<u32>,
    ) {
        // Pass 0: Retry TEE deletes that previously failed (DB=revoked, tee_deleted=0).
        let unconfirmed = match self.db.list_unconfirmed_tee_deletes(wallet_id_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "⚠️  P256 GC: list_unconfirmed_tee_deletes failed for {}: {}",
                    wallet_id_str, e
                );
                vec![]
            }
        };
        for session_index in unconfirmed {
            match self
                .tee
                .delete_p256_session_key(wallet_uuid, session_index)
                .await
            {
                Ok(_) => {
                    let _ = self.db.mark_p256_tee_deleted(wallet_id_str, session_index);
                    println!(
                        "🗑️  P256 GC retry: confirmed TEE delete for {}:{}",
                        wallet_id_str, session_index
                    );
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  P256 GC retry: TEE delete still failing for {}:{}: {}",
                        wallet_id_str, session_index, e
                    );
                }
            }
        }

        // Pass 1: GC newly expired/stuck-pending keys.
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
            // Step 1: Atomically claim the key in DB (status → 'revoked', tee_deleted=0).
            // If claim fails (0 rows: already revoked), skip — another path handled it.
            let claimed = match self
                .db
                .mark_p256_session_key_gc(wallet_id_str, session_index)
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "⚠️  P256 GC: DB claim failed for {}:{}: {}",
                        wallet_id_str, session_index, e
                    );
                    continue;
                }
            };
            if !claimed {
                continue; // already revoked by concurrent path
            }

            // Step 2: Delete TEE key (idempotent — stuck-pending rows may have no TEE key).
            match self
                .tee
                .delete_p256_session_key(wallet_uuid, session_index)
                .await
            {
                Ok(tee_deleted) => {
                    let _ = self.db.mark_p256_tee_deleted(wallet_id_str, session_index);
                    println!(
                        "🗑️  P256 GC: cleaned {}:{} (tee_deleted={})",
                        wallet_id_str, session_index, tee_deleted
                    );
                }
                Err(e) => {
                    // TEE delete failed; DB row is already 'revoked' (key inaccessible via API).
                    // tee_deleted remains 0 so Pass 0 retries it on the next GC trigger.
                    eprintln!(
                        "⚠️  P256 GC: DB claimed but TEE delete failed for {}:{}: {} \
                         (will retry via unconfirmed-tee-delete pass)",
                        wallet_id_str, session_index, e
                    );
                }
            }
        }

        // Pass 2: Physically delete rows that are fully cleaned up (revoked + tee_deleted=1)
        // and were revoked more than 24 hours ago. Prevents unbounded DB row accumulation.
        let phys_cutoff = Utc::now().timestamp() - 86400;
        match self
            .db
            .delete_confirmed_revoked_p256_session_keys(wallet_id_str, phys_cutoff)
        {
            Ok(0) => {}
            Ok(n) => println!(
                "🗑️  P256 GC Pass 2: physically deleted {} rows for {}",
                n, wallet_id_str
            ),
            Err(e) => eprintln!(
                "⚠️  P256 GC Pass 2: physical delete failed for {}: {}",
                wallet_id_str, e
            ),
        }
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
        // Issue #42: reject dormant/frozen parent wallet before any TEE signing.
        self.ensure_not_frozen(&wallet_id_str)?;

        // Lazy GC: clean up other expired P256 session keys for this wallet.
        // Exclude the current session_index to avoid GC-ing the key being signed.
        self.gc_expired_p256_session_keys(&wallet_id_str, wallet_uuid, Some(session_index))
            .await;

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
            return Err(anyhow!(
                "P256 session credential has been superseded or revoked"
            ));
        }

        // Validate userOpHash (exactly 32 bytes)
        let user_op_hash = Self::validate_hash_hex(&req.payload)?;

        // Parse accountAddress
        let account_address = Self::parse_address_hex(&req.account_address)
            .map_err(|e| anyhow!("Invalid accountAddress: {}", e))?;

        // Extract JWT proof for TA-side authorization (defense-in-depth)
        let (jwt_kid, jwt_signing_input, jwt_hmac) = agent_jwt::extract_signing_proof(&bearer_jwt)
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

        // Post-check: guard against concurrent revocation between the DB active-check above
        // and the TEE sign. The TEE client uses a single global worker loop, so all TEE
        // commands are globally serialized — a queued delete cannot physically interleave
        // with this sign call. However, DB revocation (status → 'revoked') is a plain
        // SQL UPDATE that can commit any time before the TEE delete is dispatched.
        // Discard the produced signature if the key was revoked during the TEE call window.
        // Guarantee: at the time of this post-check query the key was not yet revoked.
        // Note: revocation could still commit between this SELECT and the HTTP response,
        // so this is a best-effort defense, not a strict serialization barrier.
        if self
            .db
            .p256_session_key_is_revoked(&wallet_id_str, session_index)?
        {
            return Err(anyhow!(
                "P256 session key was revoked concurrently during signing"
            ));
        }

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
            .resolve_passkey_assertion(&wallet_id_str, None, req.webauthn_assertion.as_ref(), false)
            .await?;
        if assertion.is_none() {
            return Err(anyhow!(
                "Passkey assertion required to revoke P256 session key"
            ));
        }

        // Lazy GC: clean up other expired P256 session keys for this wallet.
        self.gc_expired_p256_session_keys(&wallet_id_str, wallet_uuid, None)
            .await;

        // Atomically mark the target key as revoked in DB.
        let claimed = self
            .db
            .mark_p256_session_key_gc(&wallet_id_str, session_index)?;
        if !claimed {
            // mark_p256_session_key_gc returned 0 rows — either already revoked or not found.
            let already_revoked = self
                .db
                .p256_session_key_is_revoked(&wallet_id_str, session_index)?;
            if already_revoked {
                // Idempotent: key is already revoked. Retry TEE delete in case it failed before,
                // then confirm tee_deleted so GC's Pass 0 stops retrying this row.
                match self
                    .tee
                    .delete_p256_session_key(wallet_uuid, session_index)
                    .await
                {
                    Ok(_) => {
                        let _ = self.db.mark_p256_tee_deleted(&wallet_id_str, session_index);
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️  RevokeP256SessionKey (idempotent): TEE delete retry failed {}:{}: {}",
                            wallet_id_str, session_index, e
                        );
                    }
                }
                let revoked_at = Utc::now().timestamp();
                println!(
                    "✅ RevokeP256SessionKey (idempotent): wallet={} idx={}",
                    wallet_id_str, session_index
                );
                return Ok(RevokeP256SessionKeyResponse {
                    success: true,
                    revoked_at,
                });
            }
            return Err(anyhow!("P256 session key not found: {}", req.key_id));
        }

        // Delete TEE key material — best-effort, idempotent.
        // If this fails, tee_deleted stays 0; GC's Pass 0 retries on the next trigger.
        match self
            .tee
            .delete_p256_session_key(wallet_uuid, session_index)
            .await
        {
            Ok(_) => {
                let _ = self.db.mark_p256_tee_deleted(&wallet_id_str, session_index);
            }
            Err(e) => {
                eprintln!(
                    "⚠️  RevokeP256SessionKey: TEE delete failed (DB=revoked, GC will retry) {}:{}: {}",
                    wallet_id_str, session_index, e
                );
            }
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

    /// POST /contact/begin-binding — owner starts a contact binding (#129). Q4 gate
    /// (codex/opus checkpoint review): the owner ceremony challenge is consumed + the
    /// owner's passkey verified BEFORE the binding row is touched — begin's upsert can
    /// overwrite a verified row, so this ceremony is the ENTIRE protection for a verified
    /// binding. delegate=false → host-authoritative bare-nonce (no TA; binding is host
    /// side). Telegram only for now; email is unwired until begin_email_binding exists.
    /// ({account,channel} commitment binding is a follow-up — needs host-side payload
    /// support in resolve_passkey_assertion.)
    pub async fn begin_contact_binding(
        &self,
        req: BeginBindingRequest,
    ) -> Result<BeginBindingResponse> {
        // codex BLOCKER fix: resolve_passkey_assertion returns Ok(None) when no webauthn
        // assertion is supplied — so the ceremony MUST be required explicitly, else a caller
        // (incl. a compromised bot holding bindingCode+verifyToken) could omit `webauthn` and
        // reach the DB write with NO owner ceremony, defeating the whole owner-gate.
        // account may be the wallet key_id OR an address (SDK uses Address) — resolve to the
        // canonical key_id so the binding row + ceremony key the same value.
        let key_id = self.resolve_account_key_id(&req.account)?;
        self.resolve_passkey_assertion(&key_id, None, req.webauthn_assertion.as_ref(), false)
            .await?
            .ok_or_else(|| anyhow!("owner WebAuthn ceremony required"))?;
        if req.channel != "telegram" {
            return Err(anyhow!(
                "channel '{}' not supported yet (telegram only; email pending begin_email_binding)",
                req.channel
            ));
        }
        // High-entropy one-time bindingCode (256-bit, OS CSPRNG) — DB matches on it.
        use rand::RngCore;
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let binding_code = hex::encode(buf);
        let ttl_secs: i64 = 600;
        self.db
            .begin_contact_binding(&key_id, &req.channel, &binding_code, None, ttl_secs)?;
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            + ttl_secs;
        Ok(BeginBindingResponse {
            binding_code,
            expires_at,
        })
    }

    /// POST /contact/claim-binding (bot api-key) — the shared bot reports a /bind. Issues a
    /// one-time verify_token (256-bit) the bot delivers to the chat; the binding only
    /// completes when the OWNER returns the token via confirm (owner ceremony). #129.
    pub async fn claim_contact_binding(
        &self,
        req: ClaimBindingRequest,
    ) -> Result<ClaimBindingResponse> {
        use rand::RngCore;
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let verify_token = hex::encode(buf);
        let ttl_secs: i64 = 600;
        let claimed = self.db.claim_contact_binding(
            &req.binding_code,
            &req.telegram_chat_id,
            req.telegram_username.as_deref(),
            &verify_token,
            req.bot_id.as_deref(),
            ttl_secs,
        )?;
        if !claimed {
            return Err(anyhow!("invalid, expired, or already-claimed binding code"));
        }
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            + ttl_secs;
        Ok(ClaimBindingResponse {
            verify_token,
            expires_at,
        })
    }

    /// POST /contact/confirm-binding — OWNER (app passkey ceremony) returns the verify_token.
    /// The owner ceremony is REQUIRED because the bot knows verify_token (it delivered it),
    /// so a confirm gated only by api-key would let a compromised bot self-complete the
    /// binding. The DB also matches the ceremony'd account (no cross-account confirm). #129.
    pub async fn confirm_contact_binding(
        &self,
        req: ConfirmBindingRequest,
    ) -> Result<ConfirmBindingResponse> {
        // codex BLOCKER fix: resolve_passkey_assertion returns Ok(None) when no webauthn
        // assertion is supplied — so the ceremony MUST be required explicitly, else a caller
        // (incl. a compromised bot holding bindingCode+verifyToken) could omit `webauthn` and
        // reach the DB write with NO owner ceremony, defeating the whole owner-gate.
        // account may be the wallet key_id OR an address — resolve to key_id (the binding row
        // was stored under key_id at begin, so the account-match must compare key_ids).
        let key_id = self.resolve_account_key_id(&req.account)?;
        self.resolve_passkey_assertion(&key_id, None, req.webauthn_assertion.as_ref(), false)
            .await?
            .ok_or_else(|| anyhow!("owner WebAuthn ceremony required"))?;
        let ok = self
            .db
            .confirm_contact_binding(&key_id, &req.binding_code, &req.verify_token)?;
        if !ok {
            return Err(anyhow!(
                "binding not confirmable (bad token, wrong account, not claimed, or expired)"
            ));
        }
        Ok(ConfirmBindingResponse {
            status: "verified".to_string(),
        })
    }

    /// GET /contact/{account} (api-key; DVT node) — verified contacts only, no secrets.
    /// PII: api-key authed, never public (#129 §3.5). Owner-ceremony read is a follow-up.
    pub async fn get_contacts(&self, account: &str) -> Result<Vec<ContactView>> {
        // account may be the wallet key_id OR an address (DVT has the userOp sender address).
        let key_id = self.resolve_account_key_id(account)?;
        let rows = self.db.get_verified_contacts(&key_id)?;
        Ok(rows
            .into_iter()
            .map(|c| ContactView {
                channel: c.channel,
                contact_ref: c.contact_ref,
                display_hint: c.display_hint,
                status: c.status,
                verified_at: c.verified_at,
            })
            .collect())
    }

    /// POST /contact/unbind — OWNER (app passkey ceremony) revokes a binding. #129.
    pub async fn unbind_contact(&self, req: UnbindRequest) -> Result<UnbindResponse> {
        // codex BLOCKER fix: resolve_passkey_assertion returns Ok(None) when no webauthn
        // assertion is supplied — so the ceremony MUST be required explicitly, else a caller
        // (incl. a compromised bot holding bindingCode+verifyToken) could omit `webauthn` and
        // reach the DB write with NO owner ceremony, defeating the whole owner-gate.
        // account may be the wallet key_id OR an address — resolve to the canonical key_id.
        let key_id = self.resolve_account_key_id(&req.account)?;
        self.resolve_passkey_assertion(&key_id, None, req.webauthn_assertion.as_ref(), false)
            .await?
            .ok_or_else(|| anyhow!("owner WebAuthn ceremony required"))?;
        let removed = self.db.unbind_contact(&key_id, &req.channel)?;
        Ok(UnbindResponse {
            status: if removed { "revoked" } else { "not_found" }.to_string(),
        })
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
            // #115: TA binds the mint challenge to the label → delegate to TA (true).
            .resolve_passkey_assertion(
                &req.human_key_id,
                None,
                req.webauthn_assertion.as_ref(),
                true,
            )
            .await?;
        if assertion.is_none() {
            return Err(anyhow!(
                "Passkey assertion required to create P256 session key"
            ));
        }

        // Lazy GC: clean up expired P256 session keys for this wallet before creating a new one.
        self.gc_expired_p256_session_keys(&req.human_key_id, wallet_id, None)
            .await;

        // Atomically check active/pending count and allocate next session_index.
        let session_index = self.db.allocate_p256_session_key_pending(
            &req.human_key_id,
            &req.human_key_id,
            Utc::now().timestamp(),
            2,
        )?;

        // Generate P256 key pair in TEE (may take ~seconds on Cortex-A7)
        let tee_result = match self
            .tee
            .create_p256_session_key(
                wallet_id,
                session_index,
                &req.human_key_id,
                24 * 3600,
                assertion,
                &req.label,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                match self
                    .tee
                    .delete_p256_session_key(wallet_id, session_index)
                    .await
                {
                    Ok(_) => {
                        let _ = self
                            .db
                            .delete_p256_session_key_pending(&req.human_key_id, session_index);
                    }
                    Err(tee_del_err) => {
                        eprintln!(
                            "⚠️  CreateP256SessionKey: TEE create+delete both failed for {}:{}: \
                             create={} delete={} (DB pending row kept for GC retry)",
                            req.human_key_id, session_index, e, tee_del_err
                        );
                    }
                }
                return Err(e);
            }
        };
        let pub_key_x = hex::encode(&tee_result.pub_key_x);
        let pub_key_y = hex::encode(&tee_result.pub_key_y);

        // Assemble JWT credential from TEE-generated material (HMAC signed inside TEE)
        let key_id = format!("{}:{}", req.human_key_id, session_index);
        let (jwt, expires_at) = agent_jwt::assemble_p256_session_jwt(&tee_result)?;

        let cred_hash = agent_jwt::credential_hash(&jwt);
        if let Err(e) = self.db.activate_p256_session_key(
            &req.human_key_id,
            session_index,
            &pub_key_x,
            &pub_key_y,
            &cred_hash,
            expires_at,
            2,
        ) {
            match self
                .tee
                .delete_p256_session_key(wallet_id, session_index)
                .await
            {
                Ok(_) => {
                    let _ = self
                        .db
                        .delete_p256_session_key_pending(&req.human_key_id, session_index);
                }
                Err(tee_err) => {
                    eprintln!(
                        "⚠️  CreateP256SessionKey cleanup: TEE delete failed for {}:{}: {} \
                         (DB pending row kept for GC retry)",
                        req.human_key_id, session_index, tee_err
                    );
                }
            }
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
}

// ========================================
// HTTP Server Routes
// ========================================

const KMS_VERSION: &str = "0.28.1";

/// Minimal HTML-escaping for user-controlled strings interpolated into the
/// (unauthenticated) stats dashboard. Fields like `description` come straight
/// from CreateKey with no sanitization, so `&<>"'` must be neutralized to
/// prevent stored XSS on a page any anonymous visitor can load.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

fn render_stats_page(server: &KmsApiServer) -> String {
    let wallets = server.db.list_wallets().unwrap_or_default();
    let qs = server.queue_status();
    let tx = server.db.get_tx_stats().unwrap_or_default();
    let total = wallets.len();
    let with_addr = wallets.iter().filter(|w| w.address.is_some()).count();
    let with_pk = wallets
        .iter()
        .filter(|w| w.passkey_pubkey.is_some())
        .count();
    let enabled = wallets.iter().filter(|w| w.status == "ready").count();
    let api_keys = server.db.list_api_keys().map(|v| v.len()).unwrap_or(0);

    let mut rows = String::new();
    for w in &wallets {
        let addr = if w.address.is_some() { "&#10003;" } else { "-" };
        let addr_cls = if w.address.is_some() { "ok" } else { "dim" };
        let pk = if w.passkey_pubkey.is_some() {
            "&#10003;"
        } else {
            "-"
        };
        let pk_cls = if w.passkey_pubkey.is_some() {
            "ok"
        } else {
            "dim"
        };
        let st_cls = if w.status == "ready" { "ok" } else { "warn" };
        let short_id = &w.key_id[..8.min(w.key_id.len())];
        let created = w.created_at.split('T').next().unwrap_or(&w.created_at);
        // Truncate by characters, not bytes: `&s[..8]` panics if byte 8 lands
        // mid-UTF-8-codepoint (e.g. 7 ASCII + a multibyte char). Description is
        // user-controlled (CreateKey), so a crafted value would panic every
        // render of this page — a DoS on the stats dashboard.
        let desc_trunc = if w.description.chars().count() > 8 {
            format!("{}…", w.description.chars().take(8).collect::<String>())
        } else {
            w.description.clone()
        };
        // This dashboard is served unauthenticated and `description` is fully
        // user-controlled via CreateKey, so escape it (and status) before
        // splicing into HTML to prevent stored XSS.
        let masked_desc = html_escape(&desc_trunc);
        let status_disp = html_escape(&w.status);
        rows.push_str(&format!(
            "<tr><td><code>{}&hellip;</code></td><td class=\"{addr_cls}\">{addr}</td><td class=\"{pk_cls}\">{pk}</td><td class=\"{st_cls}\">{}</td><td>{}</td><td>{created}</td><td>{}</td></tr>\n",
            short_id, status_disp, w.sign_count, masked_desc
        ));
    }

    let cb = if qs.circuit_breaker_open.unwrap_or(false) {
        "OPEN"
    } else {
        "closed"
    };
    let cb_cls = if qs.circuit_breaker_open.unwrap_or(false) {
        "warn"
    } else {
        "ok"
    };
    let fails = qs.consecutive_failures.unwrap_or(0);
    let panic_cls = if tx.panic_count > 0 { "warn" } else { "ok" };
    let error_cls = if tx.error_count > 0 { "warn" } else { "ok" };

    format!(
        r#"<!DOCTYPE html>
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
<div class="sub">v{version} &middot; TA mode: real &middot; <a href="/docs">📖 API Docs</a> &middot; <a href="/test">Test UI</a> &middot; <a href="/health">Health</a></div>

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

async fn health_check(server: Arc<KmsApiServer>) -> Result<impl warp::Reply, warp::Rejection> {
    // Issue #73: report the *real* capability instead of a hardcoded `true`.
    // The route is always wired in this build, but whether the deployed TA
    // revision supports GetAttestation (=26) is probed once and cached.
    let attestation_available = server.attestation_capable().await;
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kms-api",
        "version": KMS_VERSION,
        "ta_mode": "real",
        "attestation_available": attestation_available,
        "endpoints": {
            "POST": ["/CreateKey", "/DeleteKey", "/UnfreezeKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/ChangePasskey", "/BeginRegistration", "/CompleteRegistration", "/BeginAuthentication", "/verify-confirm-assertion", "/contact/begin-binding", "/contact/claim-binding", "/contact/confirm-binding", "/contact/unbind"],
            "GET": ["/health", "/version", "/KeyStatus?KeyId=xxx", "/QueueStatus", "/stats", "/RollbackCounter", "/attestation?nonce=<hex>", "/contact/{account}"]
        }
    })))
}

async fn version_check() -> Result<impl warp::Reply, warp::Rejection> {
    // `profile` lets ops tell a production board (rpId aastar.io only) from a
    // test board (also accepts localhost) at a glance. Driven by the CA
    // dev-rpid feature; pair with a dev-rpid TA for localhost to actually work.
    let profile = if cfg!(feature = "dev-rpid") {
        "dev"
    } else {
        "prod"
    };
    // `challenge_mode` lets ops tell a STRICT board (rejects bare nonce / no-clientDataJSON;
    // requires payload-commitment ceremony, #63) from a TRANSITION board at a glance.
    // The CA strict-challenge feature is set by the same MX93_STRICT_CHALLENGE build flag
    // as the (authoritative) TA strict-challenge feature, so they stay in sync — the CA
    // flag is purely for this report; the TA is what actually enforces it.
    let challenge_mode = if cfg!(feature = "strict-challenge") {
        "strict"
    } else {
        "transition"
    };
    Ok(warp::reply::json(&serde_json::json!({
        "version": KMS_VERSION,
        "build": env!("CARGO_PKG_VERSION"),
        "profile": profile,
        "challenge_mode": challenge_mode,
    })))
}

async fn handle_create_key(
    body: CreateKeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.create_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ CreateKey OK {}ms", elapsed);
            let _ = server.db.record_tx(
                "CreateKey",
                Some(&response.key_metadata.key_id),
                None,
                false,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CreateKey error: {} {}ms", e, elapsed);
            let _ =
                server
                    .db
                    .record_tx("CreateKey", None, None, false, elapsed as u64, false, false);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_describe_key(
    body: DescribeKeyRequest,
    server: Arc<KmsApiServer>,
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
    server: Arc<KmsApiServer>,
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
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.derive_address(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ DeriveAddress OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx(
                "DeriveAddress",
                Some(&key),
                None,
                false,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!(
                "{}DeriveAddress error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" },
                msg,
                key,
                elapsed
            );
            let _ = server.db.record_tx(
                "DeriveAddress",
                Some(&key),
                None,
                false,
                elapsed as u64,
                false,
                is_panic,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_sign(
    body: SignRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let addr = body.address.clone().unwrap_or_default();
    let path = body.webauthn.is_some();
    let t0 = std::time::Instant::now();
    match server.sign(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ Sign OK addr={} webauthn={} {}ms", addr, path, elapsed);
            let _ =
                server
                    .db
                    .record_tx("Sign", None, Some(&addr), path, elapsed as u64, true, false);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!(
                "{}Sign error: {} addr={} webauthn={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" },
                msg,
                addr,
                path,
                elapsed
            );
            let _ = server.db.record_tx(
                "Sign",
                None,
                Some(&addr),
                path,
                elapsed as u64,
                false,
                is_panic,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_sign_hash(
    body: SignHashRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let addr = body.address.clone().unwrap_or_default();
    let path = body.webauthn.is_some();
    let t0 = std::time::Instant::now();
    match server.sign_hash(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!(
                "✅ SignHash OK addr={} webauthn={} {}ms",
                addr, path, elapsed
            );
            let _ = server.db.record_tx(
                "SignHash",
                None,
                Some(&addr),
                path,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!(
                "{}SignHash error: {} addr={} webauthn={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" },
                msg,
                addr,
                path,
                elapsed
            );
            let _ = server.db.record_tx(
                "SignHash",
                None,
                Some(&addr),
                path,
                elapsed as u64,
                false,
                is_panic,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

/// #124 (DVT path-2 out-of-band confirm): a WebAuthn assertion the account owner
/// produced over `challenge = userOpHash`. `passkey` is the standard browser
/// AuthenticationResponseJSON (base64url; {authenticatorData, clientDataJSON,
/// signature} live under `.response`).
#[derive(Debug, serde::Deserialize)]
pub struct VerifyConfirmAssertionRequest {
    pub account: String,
    #[serde(rename = "userOpHash")]
    pub user_op_hash: String,
    pub passkey: webauthn::AuthenticationResponseJSON,
}

#[derive(Debug, serde::Serialize)]
struct VerifyConfirmAssertionResponse {
    verified: bool,
}

/// POST /verify-confirm-assertion — RP-verify a DVT out-of-band confirm assertion
/// (Validator#124). Authed (DVT node x-api-key). The node does its own local binding
/// check (challenge == userOpHash) and delegates the cryptographic RP verify here.
async fn handle_verify_confirm_assertion(
    body: VerifyConfirmAssertionRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.verify_confirm_assertion(body).await {
        Ok(verified) => Ok(warp::reply::json(&VerifyConfirmAssertionResponse {
            verified,
        })),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

async fn handle_get_public_key(
    body: GetPublicKeyRequest,
    server: Arc<KmsApiServer>,
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
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.delete_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ DeleteKey OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx(
                "DeleteKey",
                Some(&key),
                None,
                false,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!(
                "{}DeleteKey error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" },
                msg,
                key,
                elapsed
            );
            let _ = server.db.record_tx(
                "DeleteKey",
                Some(&key),
                None,
                false,
                elapsed as u64,
                false,
                is_panic,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

/// POST /UnfreezeKey — issue #42 owner WebAuthn-gated unfreeze of a dormant key.
async fn handle_unfreeze_key(
    body: UnfreezeKeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let key = body.key_id.clone();
    let t0 = std::time::Instant::now();
    match server.unfreeze_key(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ UnfreezeKey OK key={} {}ms", key, elapsed);
            let _ = server.db.record_tx(
                "UnfreezeKey",
                Some(&key),
                None,
                true,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            eprintln!("UnfreezeKey error: {} key={} {}ms", msg, key, elapsed);
            let _ = server.db.record_tx(
                "UnfreezeKey",
                Some(&key),
                None,
                true,
                elapsed as u64,
                false,
                false,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

/// POST /admin/purge-key — admin force-delete from TEE + SQLite (no passkey needed).
/// Requires Authorization: Bearer $KMS_ADMIN_TOKEN.
/// Used for: TEE orphans, test keys, gap keys whose SQLite row is already deleted.
///
/// DEV/TEST ONLY — compiled in only under the `admin-purge` feature. Release
/// builds (no feature) do not contain this handler or its route.
#[cfg(feature = "admin-purge")]
async fn handle_admin_purge_key(
    body: AdminPurgeKeyRequest,
    admin_token: String,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Validate admin token
    let expected = std::env::var("KMS_ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty() {
        return Err(warp::reject::custom(ApiError(
            "KMS_ADMIN_TOKEN not configured — admin endpoints disabled".into(),
        )));
    }
    if admin_token != expected {
        return Err(warp::reject::custom(ApiError("Invalid admin token".into())));
    }

    let reason = if body.reason.is_empty() {
        "unspecified".to_string()
    } else {
        body.reason.clone()
    };
    match server.admin_purge_key(&body.key_id, &reason).await {
        Ok((tee_ok, sqlite_ok)) => {
            let msg = format!(
                "tee_purged={} sqlite_deleted={} reason={}",
                tee_ok, sqlite_ok, reason
            );
            println!("✅ AdminPurgeKey OK key={} {}", body.key_id, msg);
            Ok(warp::reply::json(&AdminPurgeKeyResponse {
                key_id: body.key_id,
                tee_purged: tee_ok,
                sqlite_deleted: sqlite_ok,
                message: msg,
            }))
        }
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
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
            let _ = server.db.record_tx(
                "ChangePasskey",
                Some(&key),
                None,
                false,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            let msg = e.to_string();
            let is_panic = msg.contains("panicked") || msg.contains("0xffff3024");
            eprintln!(
                "{}ChangePasskey error: {} key={} {}ms",
                if is_panic { "💀 TA PANIC — " } else { "" },
                msg,
                key,
                elapsed
            );
            let _ = server.db.record_tx(
                "ChangePasskey",
                Some(&key),
                None,
                false,
                elapsed as u64,
                false,
                is_panic,
            );
            Err(warp::reject::custom(ApiError(msg)))
        }
    }
}

async fn handle_begin_registration(
    body: webauthn::BeginRegistrationRequest,
    server: Arc<KmsApiServer>,
    origin_header: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server
        .begin_registration(body, origin_header.as_deref())
        .await
    {
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
            let _ = server.db.record_tx(
                "Registration",
                Some(&response.key_id),
                None,
                true,
                elapsed as u64,
                true,
                false,
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("CompleteRegistration error: {} {}ms", e, elapsed);
            let _ = server.db.record_tx(
                "Registration",
                None,
                None,
                true,
                elapsed as u64,
                false,
                false,
            );
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_begin_authentication(
    body: webauthn::BeginAuthenticationRequest,
    server: Arc<KmsApiServer>,
    origin_header: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server
        .begin_authentication(body, origin_header.as_deref())
        .await
    {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("BeginAuthentication error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_begin_grant_session_auth(
    key_id: String,
    server: Arc<KmsApiServer>,
    origin_header: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server
        .begin_grant_session_auth(&key_id, origin_header.as_deref())
        .await
    {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("BeginGrantSessionAuth error: {}", e);
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

/// Query params for /stats
#[derive(serde::Deserialize, Default)]
struct StatsQuery {
    /// ?pretty=1 or ?pretty=true → human-readable indented JSON
    #[serde(default)]
    pretty: Option<String>,
}

/// GET /stats — JSON stats for internal monitoring / health dashboards.
/// Add ?pretty=1 for human-readable indented output.
async fn handle_get_stats(
    query: StatsQuery,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let pretty = query
        .pretty
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    let wallets = server.db.list_wallets().unwrap_or_default();
    let qs = server.queue_status();
    let tx = server.db.get_tx_stats().unwrap_or_default();
    let api_keys = server.db.list_api_keys().map(|v| v.len()).unwrap_or(0);

    let mut warnings: Vec<serde_json::Value> = Vec::new();
    if api_keys == 0 {
        warnings.push(serde_json::json!({
            "code": "NO_API_KEYS",
            "en": "No API keys registered — server is in open mode, all requests allowed. Add an API key before production.",
            "zh": "未注册 API Key，服务处于开放模式，所有请求均放行。生产上线前必须添加 API Key。"
        }));
    }
    if qs.circuit_breaker_open.unwrap_or(false) {
        warnings.push(serde_json::json!({
            "code": "CIRCUIT_BREAKER_OPEN",
            "en": "TEE call queue circuit breaker is OPEN — TA may be unresponsive.",
            "zh": "TEE 调用队列熔断器已断开，TA 可能无响应。"
        }));
    }

    let resp = serde_json::json!({
        "service": "kms-api",
        "version": env!("CARGO_PKG_VERSION"),
        "ta_mode": "real",
        "keys": {
            "total": wallets.len(),
            "active": wallets.iter().filter(|w| w.status == "ready").count(),
            "with_address": wallets.iter().filter(|w| w.address.is_some()).count(),
            "with_passkey": wallets.iter().filter(|w| w.passkey_pubkey.is_some()).count()
        },
        "operations": {
            "total_signs": tx.total_sign,
            "daily_signs": tx.daily_sign,
            "total_ops": tx.total_ops,
            "daily_ops": tx.daily_ops,
            "avg_sign_ms": tx.avg_sign_ms,
            "avg_derive_ms": tx.avg_derive_ms,
            "errors": tx.error_count,
            "panics": tx.panic_count,
            "webauthn": tx.webauthn_count
        },
        "queue": {
            "circuit_breaker": if qs.circuit_breaker_open.unwrap_or(false) { "open" } else { "closed" },
            "consecutive_failures": qs.consecutive_failures.unwrap_or(0)
        },
        "api_keys": api_keys,
        "warnings": warnings,
        "_explain": {
            "service":    { "en": "Service name",                                          "zh": "服务名称" },
            "version":    { "en": "Binary version (semver)",                               "zh": "二进制版本号" },
            "ta_mode":    { "en": "'real' = real OP-TEE hardware; 'mock' = software-only", "zh": "'real'=真实 OP-TEE 硬件；'mock'=纯软件模拟" },
            "api_keys":   { "en": "Registered API keys count. 0 = open mode (dev only!)", "zh": "已注册 API Key 数量。0 = 开放模式（仅限开发！）" },
            "warnings":   { "en": "Active configuration warnings",                        "zh": "当前配置警告列表" },
            "keys": {
                "_":             { "en": "TEE-protected wallet summary",                          "zh": "TEE 保护的钱包汇总" },
                "total":         { "en": "All wallet records in DB (including test keys)",        "zh": "数据库中钱包总数（含测试 key）" },
                "active":        { "en": "Wallets with status='ready' (key derived)",            "zh": "status='ready'（已完成密钥派生）的钱包数" },
                "with_address":  { "en": "Wallets that have an Ethereum address derived",        "zh": "已派生以太坊地址的钱包数" },
                "with_passkey":  { "en": "Wallets bound to a P-256 passkey public key",          "zh": "已绑定 P-256 passkey 公钥的钱包数" }
            },
            "operations": {
                "_":             { "en": "Cumulative operation counters (since service start)",  "zh": "累计操作计数（服务启动以来）" },
                "total_ops":     { "en": "All operations (CreateKey, Sign, etc.)",               "zh": "所有操作总次数（含 CreateKey/Sign 等）" },
                "daily_ops":     { "en": "Operations today (UTC day boundary)",                  "zh": "今日操作次数（UTC 日边界）" },
                "total_signs":   { "en": "Total secp256k1 signing operations",                   "zh": "累计 secp256k1 签名次数" },
                "daily_signs":   { "en": "Signing operations today",                             "zh": "今日签名次数" },
                "avg_sign_ms":   { "en": "Average TEE signing latency (ms). ~39ms = pure-SW k256; target <1ms with CAAM (Issue #40)", "zh": "平均 TEE 签名耗时(ms)。~39ms=纯软件 k256；接 CAAM 后目标 <1ms（Issue #40）" },
                "avg_derive_ms": { "en": "Average BIP-32 key derivation latency (ms)",          "zh": "平均 BIP-32 密钥派生耗时(ms)" },
                "errors":        { "en": "Total error count (includes intentional security-test rejections)", "zh": "累计错误次数（含安全测试的主动拒绝，非生产故障）" },
                "panics":        { "en": "TA panic count. Non-zero = critical, investigate immediately", "zh": "TA panic 次数。非零 = 严重问题，立即排查" },
                "webauthn":      { "en": "WebAuthn authentication operations count",             "zh": "WebAuthn 认证操作次数" }
            },
            "queue": {
                "_":                    { "en": "TEE call queue health",           "zh": "TEE 调用队列健康状态" },
                "circuit_breaker":      { "en": "'closed'=normal; 'open'=TA unresponsive, calls failing", "zh": "'closed'=正常；'open'=TA 无响应，调用失败" },
                "consecutive_failures": { "en": "Consecutive TEE failures before circuit opens", "zh": "熔断前连续失败次数" }
            }
        }
    });
    let body = if pretty {
        serde_json::to_string_pretty(&resp)
    } else {
        serde_json::to_string(&resp)
    }
    .unwrap_or_else(|_| "{}".to_string());
    Ok(warp::reply::with_header(
        warp::reply::Response::new(body.into()),
        "content-type",
        "application/json; charset=utf-8",
    ))
}

async fn handle_rollback_counter(
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    #[derive(serde::Serialize)]
    struct RollbackCounterResponse {
        counter: u64,
    }
    match server.read_rollback_counter().await {
        Ok(counter) => Ok(warp::reply::json(&RollbackCounterResponse { counter })),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

/// Query string for GET /attestation. The caller supplies a fresh random
/// `nonce` (hex) to bind the evidence and defeat replay.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)] // Issue #73: reject unexpected query params (schema validation)
struct AttestationQuery {
    nonce: Option<String>,
}

/// Issue #73 — upper bound on the attestation nonce. The nonce is a random
/// freshness challenge (32 bytes is the conventional size); anything past this
/// is rejected so an oversized input can't waste decode/compute. Hex input is
/// capped first (≤ 2× the byte cap) to avoid decoding a huge string at all.
const MAX_ATTESTATION_NONCE_BYTES: usize = 64;

/// Issue #37 — GET /attestation?nonce=<hex>
///
/// Returns a TEE attestation evidence blob. All binary fields are hex-encoded
/// for transport. A verifier holding the (TOFU-registered) attestation public
/// key checks: echoed `nonce` == sent nonce; `signature` is a valid RSA-PSS
/// (SHA-256, salt 32) signature over `SHA256(nonce | ta_measurement)`; and
/// `ta_measurement` equals the published `kms_ta_measurement` reference value.
async fn handle_get_attestation(
    query: AttestationQuery,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let nonce_hex = query.nonce.ok_or_else(|| {
        warp::reject::custom(ApiError(
            "missing required query parameter: nonce (hex-encoded random challenge)".to_string(),
        ))
    })?;
    let nonce_hex = nonce_hex.trim();
    // Issue #73: cap raw hex length before decoding (≤ 2 hex chars per byte).
    if nonce_hex.len() > MAX_ATTESTATION_NONCE_BYTES * 2 {
        return Err(warp::reject::custom(ApiError(format!(
            "nonce too long: max {} bytes ({} hex chars)",
            MAX_ATTESTATION_NONCE_BYTES,
            MAX_ATTESTATION_NONCE_BYTES * 2
        ))));
    }
    let nonce = hex::decode(nonce_hex)
        .map_err(|_| warp::reject::custom(ApiError("nonce must be valid hex".to_string())))?;
    if nonce.is_empty() {
        return Err(warp::reject::custom(ApiError(
            "nonce must be non-empty".to_string(),
        )));
    }
    // Issue #73: enforce the byte-length upper bound (defends against odd-length
    // hex that slips under the char cap but decodes within range anyway).
    if nonce.len() > MAX_ATTESTATION_NONCE_BYTES {
        return Err(warp::reject::custom(ApiError(format!(
            "nonce too long: max {} bytes",
            MAX_ATTESTATION_NONCE_BYTES
        ))));
    }

    #[derive(serde::Serialize)]
    struct AttestationResponse {
        /// Evidence schema version (bump on layout changes).
        schema: &'static str,
        nonce: String,
        ta_uuid: String,
        ta_measurement: String,
        signature: String,
        attest_pubkey_exp: String,
        attest_pubkey_mod: String,
        /// Signature algorithm id (TEE_ALG_*). 0x70414930 = RSASSA_PKCS1_PSS_MGF1_SHA256.
        sig_alg: u32,
        ree_time_secs: u64,
        /// Honest trust-root disclosure (see design doc §9 / R-1).
        trust_root: &'static str,
    }

    match server.get_attestation(nonce).await {
        Ok(ev) => Ok(warp::reply::json(&AttestationResponse {
            schema: "airaccount.attestation.v1",
            nonce: hex::encode(&ev.nonce),
            ta_uuid: hex::encode(&ev.ta_uuid),
            ta_measurement: hex::encode(&ev.ta_measurement),
            signature: hex::encode(&ev.signature),
            attest_pubkey_exp: hex::encode(&ev.attest_pubkey_exp),
            attest_pubkey_mod: hex::encode(&ev.attest_pubkey_mod),
            sig_alg: ev.sig_alg,
            ree_time_secs: ev.ree_time_secs,
            trust_root: "tofu-self-signed-optee-key (no NXP chain; see issue #37 R-1)",
        })),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
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
    let jwt = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            warp::reject::custom(ApiError("Authorization must be 'Bearer <jwt>'".to_string()))
        })?
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
    let jwt = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            warp::reject::custom(ApiError("Authorization must be 'Bearer <jwt>'".to_string()))
        })?
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
    auth_header: Option<String>,
    body: SignTypedDataRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // If Authorization header is present it must be "Bearer <token>"; any other format is rejected
    // immediately — silent fallback to body-auth would allow header-stripping downgrade attacks.
    let bearer = match auth_header {
        Some(h) => {
            let token = h.strip_prefix("Bearer ").ok_or_else(|| {
                warp::reject::custom(ApiError(
                    "Authorization header must use 'Bearer <token>' format".to_string(),
                ))
            })?;
            Some(token.to_string())
        }
        None => None,
    };
    let t0 = std::time::Instant::now();
    match server.sign_typed_data(bearer, body).await {
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

fn strip_bearer(auth_header: Option<String>) -> Option<String> {
    auth_header.and_then(|h| h.strip_prefix("Bearer ").map(|s| s.to_string()))
}

async fn handle_sign_micropayment_voucher(
    auth_header: Option<String>,
    body: SignMicropaymentVoucherRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server
        .sign_micropayment_voucher(strip_bearer(auth_header), body)
        .await
    {
        Ok(r) => {
            println!(
                "✅ SignMicropaymentVoucher OK {}ms",
                t0.elapsed().as_millis()
            );
            Ok(warp::reply::json(&r))
        }
        Err(e) => {
            eprintln!(
                "SignMicropaymentVoucher error: {} {}ms",
                e,
                t0.elapsed().as_millis()
            );
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_gtoken_authorization(
    auth_header: Option<String>,
    body: SignGTokenAuthorizationRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server
        .sign_gtoken_authorization(strip_bearer(auth_header), body)
        .await
    {
        Ok(r) => {
            println!(
                "✅ SignGTokenAuthorization OK {}ms",
                t0.elapsed().as_millis()
            );
            Ok(warp::reply::json(&r))
        }
        Err(e) => {
            eprintln!(
                "SignGTokenAuthorization error: {} {}ms",
                e,
                t0.elapsed().as_millis()
            );
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_x402_payment(
    auth_header: Option<String>,
    body: SignX402PaymentRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server
        .sign_x402_payment(strip_bearer(auth_header), body)
        .await
    {
        Ok(r) => {
            println!("✅ SignX402Payment OK {}ms", t0.elapsed().as_millis());
            Ok(warp::reply::json(&r))
        }
        Err(e) => {
            eprintln!(
                "SignX402Payment error: {} {}ms",
                e,
                t0.elapsed().as_millis()
            );
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_grant_session(
    body: SignGrantSessionRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.sign_grant_session(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignGrantSession OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("SignGrantSession error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_p256_grant_session(
    body: SignP256GrantSessionRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let t0 = std::time::Instant::now();
    match server.sign_p256_grant_session(body).await {
        Ok(response) => {
            let elapsed = t0.elapsed().as_millis();
            println!("✅ SignP256GrantSession OK {}ms", elapsed);
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let elapsed = t0.elapsed().as_millis();
            eprintln!("SignP256GrantSession error: {} {}ms", e, elapsed);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

/// POST /contact/begin-binding — owner ceremony + the channel to bind (#129).
#[derive(Debug, serde::Deserialize)]
pub struct BeginBindingRequest {
    pub account: String,
    pub channel: String, // 'telegram' (email pending begin_email_binding)
    // Match the existing KMS API field name `WebAuthn` (what the SDK sends) so a real
    // ceremony isn't silently dropped to None; keep lowercase aliases for flexibility.
    #[serde(
        rename = "WebAuthn",
        alias = "webauthn",
        alias = "webauthn_assertion",
        default
    )]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, serde::Serialize)]
struct BeginBindingResponse {
    #[serde(rename = "bindingCode")]
    binding_code: String,
    #[serde(rename = "expiresAt")]
    expires_at: i64,
}

async fn handle_begin_binding(
    body: BeginBindingRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.begin_contact_binding(body).await {
        Ok(resp) => Ok(warp::reply::json(&resp)),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ClaimBindingRequest {
    #[serde(rename = "bindingCode", alias = "binding_code")]
    pub binding_code: String,
    #[serde(rename = "telegramChatId", alias = "telegram_chat_id")]
    pub telegram_chat_id: String,
    #[serde(rename = "telegramUsername", alias = "telegram_username")]
    pub telegram_username: Option<String>,
    #[serde(rename = "botId", alias = "bot_id")]
    pub bot_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct ClaimBindingResponse {
    #[serde(rename = "verifyToken")]
    verify_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct ConfirmBindingRequest {
    pub account: String,
    #[serde(rename = "bindingCode", alias = "binding_code")]
    pub binding_code: String,
    #[serde(rename = "verifyToken", alias = "verify_token")]
    pub verify_token: String,
    // Match the existing KMS API field name `WebAuthn` (what the SDK sends) so a real
    // ceremony isn't silently dropped to None; keep lowercase aliases for flexibility.
    #[serde(
        rename = "WebAuthn",
        alias = "webauthn",
        alias = "webauthn_assertion",
        default
    )]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, serde::Serialize)]
struct ConfirmBindingResponse {
    status: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UnbindRequest {
    pub account: String,
    pub channel: String,
    // Match the existing KMS API field name `WebAuthn` (what the SDK sends) so a real
    // ceremony isn't silently dropped to None; keep lowercase aliases for flexibility.
    #[serde(
        rename = "WebAuthn",
        alias = "webauthn",
        alias = "webauthn_assertion",
        default
    )]
    pub webauthn_assertion: Option<WebAuthnAssertion>,
}

#[derive(Debug, serde::Serialize)]
struct UnbindResponse {
    status: String,
}

#[derive(Debug, serde::Serialize)]
struct ContactView {
    channel: String,
    #[serde(rename = "contactRef")]
    contact_ref: Option<String>,
    #[serde(rename = "displayHint")]
    display_hint: Option<String>,
    status: String,
    #[serde(rename = "verifiedAt")]
    verified_at: Option<i64>,
}

async fn handle_claim_binding(
    body: ClaimBindingRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.claim_contact_binding(body).await {
        Ok(resp) => Ok(warp::reply::json(&resp)),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

async fn handle_confirm_binding(
    body: ConfirmBindingRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.confirm_contact_binding(body).await {
        Ok(resp) => Ok(warp::reply::json(&resp)),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

async fn handle_get_contacts(
    account: String,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.get_contacts(&account).await {
        Ok(contacts) => Ok(warp::reply::json(
            &serde_json::json!({ "contacts": contacts }),
        )),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
    }
}

async fn handle_unbind_contact(
    body: UnbindRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.unbind_contact(body).await {
        Ok(resp) => Ok(warp::reply::json(&resp)),
        Err(e) => Err(warp::reject::custom(ApiError(e.to_string()))),
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
            warp::reject::custom(ApiError("Authorization must be 'Bearer <jwt>'".to_string()))
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

async fn handle_rejection(
    err: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    // Unmatched path → 404, not 500. warp surfaces these as a plain not_found
    // rejection; without this they fall through to the 500 catch-all below, which
    // is misleading. In particular a compile-gated-out /admin/purge-key (release
    // build, no `admin-purge` feature) must read as "no such endpoint", not
    // "internal server error".
    if err.is_not_found() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "Not found" })),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }
    // (opus/codex review) Malformed JSON / oversized body must read as 400/413, not 500.
    if err
        .find::<warp::filters::body::BodyDeserializeError>()
        .is_some()
    {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "Malformed request body" })),
            warp::http::StatusCode::BAD_REQUEST,
        ));
    }
    if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "error": "Payload too large" })),
            warp::http::StatusCode::PAYLOAD_TOO_LARGE,
        ));
    }
    if let Some(rl_error) = err.find::<RateLimitError>() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": format!("Rate limit exceeded: {} requests/minute", rl_error.0)
            })),
            warp::http::StatusCode::TOO_MANY_REQUESTS,
        ));
    }
    // Issue #73: a malformed query string (an unexpected parameter rejected by
    // AttestationQuery's deny_unknown_fields, or a wrong-typed field) is a CLIENT
    // error → 400 with a clear message, not a 500 "Internal server error".
    if err.find::<warp::reject::InvalidQuery>().is_some() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "invalid query parameters: unexpected or malformed field"
            })),
            warp::http::StatusCode::BAD_REQUEST,
        ));
    }
    if let Some(api_error) = err.find::<ApiError>() {
        let status = if api_error.0.contains("API key") {
            warp::http::StatusCode::UNAUTHORIZED
        } else if api_error.0.contains("TEE queue full") {
            // T3: bounded-queue fast-fail — honest backpressure, client should retry.
            warp::http::StatusCode::TOO_MANY_REQUESTS
        } else if api_error.0.contains("TEE request dropped") {
            // T3: shed past the queue deadline — server overloaded.
            warp::http::StatusCode::SERVICE_UNAVAILABLE
        } else if api_error.0.contains("circuit breaker") {
            warp::http::StatusCode::SERVICE_UNAVAILABLE
        } else if api_error.0.contains("TEE call timeout") {
            // P0-1: hung TA call — outcome unknown, server-side fault
            warp::http::StatusCode::GATEWAY_TIMEOUT
        } else if api_error.0.contains("0xffff")
            || api_error.0.contains("panicked")
            || api_error.0.contains("TEE error")
        {
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
                    bytes.len(),
                    MAX_REQUEST_BODY_BYTES / 1024
                ))));
            }
            let data: &[u8] = if bytes.is_empty() { b"{}" } else { &bytes };
            serde_json::from_slice(data).map_err(|e| {
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
                            if k == *lk {
                                return Ok(());
                            }
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

// Variant B: internal BLS signer for DVT — mirrors the aastar-bls-signer contract
// (POST /sign {node_id, user_op_hash} → {signature, signature_compact, public_key}).
// Bound to 127.0.0.1:3100 ONLY (never via the Cloudflare tunnel). DVT points its
// RUST_SIGNER_URL here; the BLS private key stays sealed in the TA and never leaves.
#[derive(serde::Deserialize)]
struct BlsSignReq {
    #[allow(dead_code)]
    node_id: String,
    user_op_hash: String,
}

#[derive(serde::Serialize)]
struct BlsSignResp {
    signature: String,
    signature_compact: String,
    public_key: String,
}

#[derive(serde::Serialize)]
struct BlsGenResp {
    key_id: String,
    public_key: String,
}

// CC-24 staked registration: BLS proof-of-possession. DVT's register-node.mjs POSTs the
// OPERATOR address (not a caller-chosen point); the TA signs sk·hashToG2(operator, POP_DST),
// so /pop is operator-bound and can never be used as a signing oracle. Loopback + token.
#[derive(serde::Deserialize)]
struct PopSignReq {
    #[allow(dead_code)]
    node_id: String,
    /// 20-byte operator EOA (hex, 0x-optional) — msg.sender of registerWithProof.
    operator: String,
}

#[derive(serde::Serialize)]
struct PopSignResp {
    /// EIP-2537 uncompressed G2 PoP signature (256B) — feeds registerWithProof's popSig.
    pop_signature: String,
    /// Compressed G2 (96B).
    pop_signature_compact: String,
    public_key: String,
}

// ── CC-34: keeper/operator ECDSA(secp256k1) signer — mirrors the BLS signer on
// 127.0.0.1:3100. POST /kms/sign {keeper_id, digest} → {signature(65B r||s||v), address};
// POST /kms/gen-keeper-eoa → {key_id, address, public_key}. Keeper key sealed in TA.
#[derive(serde::Deserialize)]
struct KeeperSignReq {
    /// Optional: informational only. The signing key is the board's singleton
    /// keeper key addressed by KMS_KEEPER_KEY_ID (mirrors the BLS key_id-from-env).
    #[allow(dead_code)]
    keeper_id: Option<String>,
    /// 32-byte raw digest (hex, 0x-optional). Signed as-is — DVT hashes.
    digest: String,
}

#[derive(serde::Serialize)]
struct KeeperSignResp {
    /// 65-byte recoverable signature r(32)||s(32)||v(1), v=27/28, low-S.
    signature: String,
    /// 20-byte keeper EOA (0x..) — the funding address.
    address: String,
}

#[derive(serde::Serialize)]
struct KeeperGenResp {
    key_id: String,
    address: String,
    public_key: String,
}

/// Constant-time byte compare (length-checked). Avoids leaking the token via
/// early-return timing on the loopback signer auth.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Gate the keeper signer/provisioning on KMS_KEEPER_SIGNER_TOKEN (X-Signer-Token
/// header), constant-time compared.
///
/// Unlike the BLS signer (which allows a tokenless localhost default), the keeper
/// key is a **funded Ethereum EOA** — a tokenless signer is a money-signing oracle
/// for any co-located process. So this is **fail-closed**: if the token env is
/// unset/empty the endpoint is refused entirely. Set KMS_KEEPER_SIGNER_TOKEN to
/// use keeper signing/provisioning (the node-setup wizard generates it).
fn check_keeper_token(token: &Option<String>) -> Result<(), warp::Rejection> {
    let expected = match std::env::var("KMS_KEEPER_SIGNER_TOKEN") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "keeper signer disabled: KMS_KEEPER_SIGNER_TOKEN not set (fail-closed — \
                 keeper is a funded EOA and must not sign without a token)"
                    .into(),
            )))
        }
    };
    if token
        .as_deref()
        .map(|t| ct_eq(t.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(warp::reject::custom(ApiError(
            "invalid or missing X-Signer-Token".into(),
        )))
    }
}

/// Fail-closed token gate for the DESTRUCTIVE /remove-key (unlike /gen-key which
/// tolerates a tokenless localhost default). Deleting the sealed BLS key must never
/// be doable by an unauthenticated co-located process — so the signer token MUST be
/// set, constant-time compared.
fn check_signer_token_required(token: &Option<String>) -> Result<(), warp::Rejection> {
    let expected = match std::env::var("KMS_BLS_SIGNER_TOKEN") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "BLS remove requires KMS_BLS_SIGNER_TOKEN to be set (fail-closed — \
                 destroying the sealed key must not be tokenless)"
                    .into(),
            )))
        }
    };
    if token
        .as_deref()
        .map(|t| ct_eq(t.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(warp::reject::custom(ApiError(
            "invalid or missing X-Signer-Token".into(),
        )))
    }
}

// Codex High#2/Med#3: :3100 is localhost-only, but a co-located process could still
// reach it. If KMS_BLS_SIGNER_TOKEN is set, require it (X-Signer-Token header) on
// /sign + /gen-key. Unset = localhost-only default (dev / backward-compat). The token
// gates which local process (the DVT node) may sign, not just "any local process".
fn check_signer_token(token: &Option<String>) -> Result<(), warp::Rejection> {
    match std::env::var("KMS_BLS_SIGNER_TOKEN") {
        Ok(expected) if !expected.is_empty() => {
            if token.as_deref() == Some(expected.as_str()) {
                Ok(())
            } else {
                Err(warp::reject::custom(ApiError(
                    "invalid or missing X-Signer-Token".into(),
                )))
            }
        }
        _ => Ok(()),
    }
}

/// Provision: generate the board's single BLS key in the TA (one-time). Returns
/// key_id + pubkey. Operator then sets KMS_BLS_KEY_ID + KMS_BLS_PUBKEY and restarts.
/// Codex Med#3: gated behind KMS_BLS_PROVISIONING=1 (off by default) + token; the TA
/// enforces a singleton so a loop can't fill secure storage.
async fn bls_gen_handler(
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if std::env::var("KMS_BLS_PROVISIONING").ok().as_deref() != Some("1") {
        return Err(warp::reject::custom(ApiError(
            "BLS provisioning disabled (set KMS_BLS_PROVISIONING=1 to enable)".into(),
        )));
    }
    check_signer_token(&token)?;
    let key_id = Uuid::new_v4();
    match server.tee.bls_gen_key(key_id).await {
        Ok(pk) => Ok(warp::reply::json(&BlsGenResp {
            key_id: key_id.to_string(),
            public_key: format!("0x{}", hex::encode(pk)),
        })),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "BLS gen failed: {}",
            e
        )))),
    }
}

/// Remove the sealed BLS singleton — recovery for an orphaned key whose key_id was
/// lost (can't be addressed by id), or rotation. DESTRUCTIVE, so double-gated:
/// KMS_BLS_PROVISIONING=1 (provisioning session) AND KMS_BLS_ALLOW_REMOVE=1 (both
/// off by default) + the signer token. Returns the count removed.
async fn bls_remove_handler(
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if std::env::var("KMS_BLS_PROVISIONING").ok().as_deref() != Some("1") {
        return Err(warp::reject::custom(ApiError(
            "BLS provisioning disabled (set KMS_BLS_PROVISIONING=1 to enable)".into(),
        )));
    }
    if std::env::var("KMS_BLS_ALLOW_REMOVE").ok().as_deref() != Some("1") {
        return Err(warp::reject::custom(ApiError(
            "BLS remove disabled (destructive; set KMS_BLS_ALLOW_REMOVE=1 to enable)".into(),
        )));
    }
    check_signer_token_required(&token)?; // fail-closed (not the tokenless gen-key default)
    match server.tee.bls_remove().await {
        Ok(removed) => Ok(warp::reply::json(&serde_json::json!({ "removed": removed }))),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "BLS remove failed: {}",
            e
        )))),
    }
}

async fn bls_sign_handler(
    req: BlsSignReq,
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_signer_token(&token)?;
    // Single BLS key per board; key_id from env (set at provisioning after BlsGenKey).
    let key_id = match std::env::var("KMS_BLS_KEY_ID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok())
    {
        Some(k) => k,
        None => {
            return Err(warp::reject::custom(ApiError(
                "KMS_BLS_KEY_ID not configured".into(),
            )))
        }
    };
    // Codex Med#4/#5: use the provisioned pubkey from env — avoids a second TA call
    // per sign AND avoids masking a bls_pubkey failure as public_key:"0x".
    let pk_hex = match std::env::var("KMS_BLS_PUBKEY") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "KMS_BLS_PUBKEY not configured".into(),
            )))
        }
    };
    let hh = req.user_op_hash.trim_start_matches("0x");
    let hb = match hex::decode(hh) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "user_op_hash must be 32-byte hex".into(),
            )))
        }
    };
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hb);
    // ta_client.bls_sign validates the 256B/96B lengths (fail-closed on ABI drift).
    match server.tee.bls_sign(key_id, hash).await {
        Ok((sig, compact)) => Ok(warp::reply::json(&BlsSignResp {
            signature: format!("0x{}", hex::encode(sig)),
            signature_compact: hex::encode(compact),
            public_key: pk_hex,
        })),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "BLS sign failed: {}",
            e
        )))),
    }
}

/// CC-24 staked registration: sign a BLS proof-of-possession over the operator address so
/// a KMS-TEE key-less DVT node can call the validator's registerWithProof. Same key_id-from-env
/// + token as /sign. The TA signs the operator under POP_DST (never a caller-supplied point).
async fn pop_sign_handler(
    req: PopSignReq,
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_signer_token(&token)?;
    let key_id = match std::env::var("KMS_BLS_KEY_ID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok())
    {
        Some(k) => k,
        None => {
            return Err(warp::reject::custom(ApiError(
                "KMS_BLS_KEY_ID not configured".into(),
            )))
        }
    };
    let pk_hex = match std::env::var("KMS_BLS_PUBKEY") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "KMS_BLS_PUBKEY not configured".into(),
            )))
        }
    };
    let ob = match hex::decode(req.operator.trim_start_matches("0x")) {
        Ok(b) if b.len() == 20 => b,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "operator must be a 20-byte address hex".into(),
            )))
        }
    };
    let mut operator = [0u8; 20];
    operator.copy_from_slice(&ob);
    match server.tee.bls_pop_sign(key_id, operator).await {
        Ok((sig, compact)) => Ok(warp::reply::json(&PopSignResp {
            pop_signature: format!("0x{}", hex::encode(sig)),
            pop_signature_compact: hex::encode(compact),
            public_key: pk_hex,
        })),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "BLS PoP sign failed: {}",
            e
        )))),
    }
}

// ── CC-34 keeper/operator ECDSA handlers (loopback :3100) ──

/// Provision the board's singleton keeper EOA (TEE-sealed secp256k1). Returns
/// key_id + 20B address + 65B pubkey. Operator then sets KMS_KEEPER_KEY_ID +
/// KMS_KEEPER_ADDRESS and restarts. Gated behind KMS_KEEPER_PROVISIONING=1 (off
/// by default) + token; the TA enforces a singleton so a loop can't fill storage.
async fn keeper_gen_handler(
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if std::env::var("KMS_KEEPER_PROVISIONING").ok().as_deref() != Some("1") {
        return Err(warp::reject::custom(ApiError(
            "keeper provisioning disabled (set KMS_KEEPER_PROVISIONING=1 to enable)".into(),
        )));
    }
    check_keeper_token(&token)?;
    let key_id = Uuid::new_v4();
    match server.tee.keeper_gen_key(key_id).await {
        Ok((pk, addr)) => Ok(warp::reply::json(&KeeperGenResp {
            key_id: key_id.to_string(),
            address: format!("0x{}", hex::encode(addr)),
            public_key: format!("0x{}", hex::encode(pk)),
        })),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "keeper gen failed: {}",
            e
        )))),
    }
}

async fn keeper_sign_handler(
    req: KeeperSignReq,
    token: Option<String>,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_keeper_token(&token)?;
    // Single keeper key per board; key_id from env (set at provisioning).
    let key_id = match std::env::var("KMS_KEEPER_KEY_ID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok())
    {
        Some(k) => k,
        None => {
            return Err(warp::reject::custom(ApiError(
                "KMS_KEEPER_KEY_ID not configured".into(),
            )))
        }
    };
    // Return the provisioned address from env — avoids a second TA call per sign.
    let addr = match std::env::var("KMS_KEEPER_ADDRESS") {
        Ok(a) if !a.is_empty() => a,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "KMS_KEEPER_ADDRESS not configured".into(),
            )))
        }
    };
    let dh = req.digest.trim_start_matches("0x");
    let db = match hex::decode(dh) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(warp::reject::custom(ApiError(
                "digest must be 32-byte hex".into(),
            )))
        }
    };
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&db);
    // keeper_sign validates the 65-byte length (fail-closed on ABI drift).
    match server.tee.keeper_sign(key_id, digest).await {
        Ok(sig) => Ok(warp::reply::json(&KeeperSignResp {
            signature: format!("0x{}", hex::encode(sig)),
            address: addr,
        })),
        Err(e) => Err(warp::reject::custom(ApiError(format!(
            "keeper sign failed: {}",
            e
        )))),
    }
}

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

    // M-c: periodic challenge GC. consume_challenge only deletes the consumed
    // row; unconsumed expired challenges otherwise accumulate forever — the
    // unauthenticated Begin* endpoints write 1-2 rows each, so without this
    // the challenges table is an unbounded-growth DoS vector.
    {
        let gc_db = db.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(600));
            loop {
                tick.tick().await;
                match gc_db.cleanup_expired_challenges() {
                    Ok(n) if n > 0 => println!("🧹 Challenge GC: removed {} expired rows", n),
                    Ok(_) => {}
                    Err(e) => eprintln!("⚠️  Challenge GC failed: {:?}", e),
                }
            }
        });
        println!("🧹 Challenge GC: every 600s");
    }

    // Issue #42: periodic dormant-key freeze sweep. Any 'active' key whose last
    // successful op (tx_log) is older than the inactivity threshold is moved to
    // lifecycle_status='frozen'. Soft host-side gate only — TEE material is never
    // touched. Owner re-enables via POST /UnfreezeKey (WebAuthn). Threshold is
    // overridable via KMS_INACTIVITY_FREEZE_SECS (seconds) for testing.
    {
        let freeze_db = db.clone();
        let threshold_secs = std::env::var("KMS_INACTIVITY_FREEZE_SECS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(INACTIVITY_FREEZE_SECS);
        tokio::spawn(async move {
            let mut tick =
                tokio::time::interval(std::time::Duration::from_secs(FREEZE_SWEEP_INTERVAL_SECS));
            loop {
                tick.tick().await;
                let now = chrono::Utc::now().timestamp();
                match freeze_db.freeze_dormant_keys(now, threshold_secs) {
                    Ok(ids) if !ids.is_empty() => {
                        println!(
                            "🧊 Dormant-key freeze: froze {} key(s): {:?}",
                            ids.len(),
                            ids
                        );
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("⚠️  Dormant-key freeze sweep failed: {:?}", e),
                }
            }
        });
        println!(
            "🧊 Dormant-key freeze: every {}s (threshold {}s)",
            FREEZE_SWEEP_INTERVAL_SECS, threshold_secs
        );
    }

    let server = Arc::new(KmsApiServer::new(db.clone()));

    // API Key guard — FAIL-CLOSED by default.
    // Authentication is REQUIRED unless the operator explicitly opts into open
    // mode with KMS_ALLOW_OPEN_MODE=1 (dev/test only). This inverts the previous
    // fail-open default, where a board with no keys provisioned and no env set
    // would silently accept unauthenticated requests. A public KMS must never
    // default to open.
    let legacy_key = std::env::var("KMS_API_KEY").ok();
    // On a DB error we cannot confirm keys exist; treat as "keys required"
    // (fail-closed), never as open.
    let has_db_keys = db.has_api_keys().unwrap_or(true);
    let allow_open = std::env::var("KMS_ALLOW_OPEN_MODE")
        .map(|v| v == "1")
        .unwrap_or(false);
    let api_key_enabled = !allow_open;
    if allow_open {
        println!("⚠️  API Key authentication: DISABLED (KMS_ALLOW_OPEN_MODE=1) — all requests are unauthenticated. DEV/TEST ONLY.");
    } else {
        println!("🔑 API Key authentication: ENABLED (fail-closed default)");
        if !has_db_keys && legacy_key.is_none() {
            println!("⚠️  No API key provisioned — all requests will be REJECTED until you run `kms-admin api-key generate` or set KMS_API_KEY.");
            println!("⚠️  For an intentionally open dev instance, set KMS_ALLOW_OPEN_MODE=1.");
        }
    }
    let api_key_filter = db_api_key_filter(db, legacy_key, api_key_enabled);
    let rl_filter = rate_limit_filter(server.rate_limiter.clone());

    // Root path - live stats dashboard
    let server_index = server.clone();
    let index = warp::path::end()
        .and(warp::get())
        .map(move || warp::reply::html(render_stats_page(&server_index)));

    // Test UI page
    let test_ui = warp::path("test")
        .and(warp::get())
        .map(|| {
            // Search in priority order: working dir, MX93 deployment path, legacy QEMU path
            let candidates = [
                "kms-test-page.html",
                "/root/AirAccount/kms-test-page.html",
                "/root/shared/kms-test-page.html",
            ];
            let html = candidates.iter()
                .find_map(|p| std::fs::read_to_string(p).ok())
                .unwrap_or_else(|| "<html><body><h1>Test UI not available</h1><p>Deploy kms-test-page.html to the working directory or /root/AirAccount/</p></body></html>".to_string());
            warp::reply::html(html)
        });

    // Community node download portal (Phase 3 onboarding) — compiled in, served at /portal.
    let portal = warp::path("portal")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| warp::reply::html(include_str!("../../portal/index.html")));

    // Public node-identity page (CC-34) — read-only, NO auth. Displays this co-located
    // node's PUBLIC identities: the DVT BLS G1 pubkey and the keeper EOA. Every value is
    // public (on-chain-derivable) — no secrets, no signing keys — so it is intentionally
    // ungated. Fallback surface for a KMS+DVT ("committee node") until dvt.aastar.io
    // exposes them; values come straight from env so it needs no TA/DB round-trip.
    let identities = warp::path("identities")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            // Middle-ellipsis mask for at-a-glance display; full value in a <details>.
            // Slice by chars (not bytes) so a non-ASCII env value can never panic on a
            // UTF-8 boundary — values are operator-set, but this stays fail-safe.
            fn mask(s: &str) -> String {
                let t = s.trim();
                let chars: Vec<char> = t.chars().collect();
                if chars.len() <= 22 {
                    return t.to_string();
                }
                let head: String = chars[..12].iter().collect();
                let tail: String = chars[chars.len() - 8..].iter().collect();
                format!("{head}…{tail}")
            }
            fn esc(s: &str) -> String {
                s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
            }
            let row = |label: &str, val: &str| -> String {
                let v = val.trim();
                if v.is_empty() {
                    format!(
                        "<tr><td>{}</td><td class=\"empty\">— not provisioned —</td></tr>",
                        label
                    )
                } else {
                    format!(
                        "<tr><td>{0}</td><td><code>{1}</code><details><summary>show full</summary><code>{2}</code></details></td></tr>",
                        label,
                        esc(&mask(v)),
                        esc(v)
                    )
                }
            };
            let bls = std::env::var("KMS_BLS_PUBKEY").unwrap_or_default();
            let keeper = std::env::var("KMS_KEEPER_ADDRESS").unwrap_or_default();
            let html = format!(
                "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Node identities</title>\
<style>body{{font-family:system-ui,-apple-system,sans-serif;max-width:760px;margin:2.2rem auto;padding:0 1rem;color:#222}}\
h1{{font-size:1.25rem;margin-bottom:.2rem}}table{{border-collapse:collapse;width:100%;margin-top:1rem}}\
td{{padding:.55rem .6rem;border-bottom:1px solid #eee;vertical-align:top}}\
td:first-child{{font-weight:600;white-space:nowrap;color:#555}}\
code{{font-family:ui-monospace,SFMono-Regular,monospace;word-break:break-all;font-size:.92rem}}\
.empty{{color:#999}}details{{margin-top:.3rem}}summary{{cursor:pointer;color:#06c;font-size:.82rem}}\
.note{{color:#888;font-size:.84rem;margin-top:1.4rem;line-height:1.5}}</style></head>\
<body><h1>Node public identities</h1>\
<p class=\"note\">Co-located KMS&nbsp;+&nbsp;DVT node. Every value below is <b>public</b> (on-chain-derivable) — no secrets, no login.</p>\
<table>{bls_row}{keeper_row}</table>\
<p class=\"note\">The DVT BLS pubkey signs the validator's BLS aggregate; the keeper EOA funds and sends the node's on-chain transactions.</p>\
</body></html>",
                bls_row = row("DVT BLS pubkey (G1)", &bls),
                keeper_row = row("Keeper EOA", &keeper),
            );
            warp::reply::html(html)
        });

    // Health check (Issue #73: probes real attestation capability)
    let server_health = server.clone();
    let health = warp::path("health")
        .and(warp::get())
        .and(warp::any().map(move || server_health.clone()))
        .and_then(health_check);

    // Issue #12 — signed attestation measurement manifest at
    // GET /.well-known/attestation-measurements.json. Compiled in (include_str!)
    // so it always ships with this build. Clients fetch it, verify its Ed25519
    // signature against the pinned publisher key, and use the listed
    // `ta_measurement` values when verifying GET /attestation evidence.
    const ATTESTATION_MEASUREMENTS_MANIFEST: &str =
        include_str!("../attestation-measurements.json");
    let measurements_manifest = warp::path(".well-known")
        .and(warp::path("attestation-measurements.json"))
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            warp::reply::with_header(
                ATTESTATION_MEASUREMENTS_MANIFEST,
                "content-type",
                "application/json; charset=utf-8",
            )
        });

    // Issue #87 (B) — Sigsum transparency proof for the manifest above, at
    // GET /.well-known/attestation-measurements-proof.json. Compiled in so it
    // ships with the manifest. Clients pass this to verifyMeasurementManifest's
    // `transparency` option (Tier-2): it proves the manifest was publicly logged
    // (witness-cosigned), closing the single-publisher-key gap. Static — the host
    // never talks to the log at runtime; it is refreshed at release time.
    const ATTESTATION_MEASUREMENTS_PROOF: &str =
        include_str!("../attestation-measurements-proof.json");
    let measurements_manifest_proof = warp::path(".well-known")
        .and(warp::path("attestation-measurements-proof.json"))
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            warp::reply::with_header(
                ATTESTATION_MEASUREMENTS_PROOF,
                "content-type",
                "application/json; charset=utf-8",
            )
        });

    // Live API docs — Swagger UI at GET /docs, OpenAPI 3.1 spec at GET /openapi.yaml.
    // The spec is compiled into the binary (include_str!) so it always matches this build.
    // Pinned swagger-ui-dist@5.32.6 with SRI integrity hashes (supply-chain hardening).
    const SWAGGER_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>AirAccount KMS API — v0.21.0 (Beta3)</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.32.6/swagger-ui.css"
 integrity="sha384-9Q2fpS+xeS4ffJy6CagnwoUl+4ldAYhOs9pgZuEKxypVModhmZFzeMlvVsAjf7uT" crossorigin="anonymous">
<style>body{margin:0;background:#fafafa}.swagger-ui .topbar{display:none}
#hdr{background:linear-gradient(110deg,#070b1e,#101637);color:#f5f7fa;padding:16px 28px;font-family:-apple-system,Segoe UI,Roboto,sans-serif}
#hdr h1{margin:0;font-size:20px}#hdr .b{display:inline-block;background:#45e0c8;color:#06231d;font-weight:700;border-radius:14px;padding:2px 12px;font-size:13px;margin-right:8px}
#hdr small{color:#8b93b8}
#theme{position:fixed;top:13px;right:20px;z-index:9999;background:#16203f;color:#f5f7fa;border:1px solid #2c3a66;border-radius:20px;padding:6px 13px;cursor:pointer;font-size:16px;line-height:1}
html.dark body{background:#0b1020}
html.dark #swagger-ui{filter:invert(0.92) hue-rotate(180deg)}
html.dark #swagger-ui .microlight,html.dark #swagger-ui img{filter:invert(1) hue-rotate(180deg)}</style></head>
<body><div id="hdr"><h1><span class="b">BETA3 · v0.21.0</span>AirAccount KMS API</h1>
<small>TEE 私钥管理 · WebAuthn · AWS KMS 兼容 · 私钥永不出 TEE</small></div>
<button id="theme" onclick="tgl()" title="切换 dark / light">🌙</button>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5.32.6/swagger-ui-bundle.js"
 integrity="sha384-EYdOaiRwn44zNjrw+Tfs06qYz9BGQVo2f4/pLY5i7VorbjnZNhdplAbTBk8FXHUJ" crossorigin="anonymous"></script>
<script>window.ui=SwaggerUIBundle({url:'/openapi.yaml',dom_id:'#swagger-ui',deepLinking:true,docExpansion:'list',defaultModelsExpandDepth:1,tryItOutEnabled:true,presets:[SwaggerUIBundle.presets.apis]});
function tgl(){var d=document.documentElement.classList.toggle('dark');document.getElementById('theme').textContent=d?'☀️':'🌙';localStorage.setItem('kms-theme',d?'dark':'light')}
(function(){if(localStorage.getItem('kms-theme')==='dark'){document.documentElement.classList.add('dark');document.getElementById('theme').textContent='☀️'}})();</script>
</body></html>"#;
    let api_docs = warp::path("docs")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| warp::reply::html(SWAGGER_UI_HTML));
    let openapi_spec = warp::path("openapi.yaml")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            warp::reply::with_header(
                include_str!("../../docs/api/openapi.yaml"),
                "content-type",
                "application/yaml; charset=utf-8",
            )
        });

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

    // Stats JSON - GET /stats[?pretty=1] (machine-readable, no auth required)
    let server_stats = server.clone();
    let stats_json = warp::path("stats")
        .and(warp::get())
        .and(warp::query::<StatsQuery>())
        .and(warp::any().map(move || server_stats.clone()))
        .and_then(handle_get_stats);
    // RollbackCounter - GET /RollbackCounter
    let server_rc = server.clone();
    let rollback_counter = warp::path("RollbackCounter")
        .and(warp::get())
        .and(warp::any().map(move || server_rc.clone()))
        .and_then(handle_rollback_counter);

    // Attestation (issue #37) - GET /attestation?nonce=<hex> (no auth; no secrets)
    let server_attest = server.clone();
    let attestation = warp::path("attestation")
        .and(warp::get())
        .and(warp::query::<AttestationQuery>())
        .and(warp::any().map(move || server_attest.clone()))
        .and_then(handle_get_attestation);

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
        .and(warp::header::exact(
            "x-amz-target",
            "TrentService.CreateKey",
        ))
        .and(aws_kms_body())
        .and(warp::any().map(move || server1.clone()))
        .and_then(handle_create_key);

    // DescribeKey API
    let describe_key = warp::path("DescribeKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact(
            "x-amz-target",
            "TrentService.DescribeKey",
        ))
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
        .and(warp::header::exact(
            "x-amz-target",
            "TrentService.DeriveAddress",
        ))
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

    // #124 (DVT path-2): RP-verify an out-of-band confirm assertion. Plain JSON POST
    // (not AWS-KMS framed), x-api-key authed (DVT node) + rate-limited.
    let server_vca_clone = Arc::clone(&server);
    let verify_confirm_assertion = warp::path("verify-confirm-assertion")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(warp::any().map(move || server_vca_clone.clone()))
        .and_then(handle_verify_confirm_assertion);

    // GetPublicKey API (TEE)
    let get_public_key = warp::path("GetPublicKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact(
            "x-amz-target",
            "TrentService.GetPublicKey",
        ))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6.clone()))
        .and_then(handle_get_public_key);

    // DeleteKey API (TEE)
    // Accepts both "TrentService.DeleteKey" (canonical) and
    // "TrentService.ScheduleKeyDeletion" (AWS KMS compat alias).
    let server7 = Arc::clone(&server);
    let delete_key_target = warp::header::<String>("x-amz-target")
        .and_then(|t: String| async move {
            if t == "TrentService.DeleteKey" || t == "TrentService.ScheduleKeyDeletion" {
                Ok(())
            } else {
                Err(warp::reject::not_found())
            }
        })
        .untuple_one();
    let delete_key = warp::path("DeleteKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(delete_key_target)
        .and(aws_kms_body())
        .and(warp::any().map(move || server7.clone()))
        .and_then(handle_delete_key);

    // UnfreezeKey API (issue #42) — owner WebAuthn-gated unfreeze.
    let server_unfreeze = Arc::clone(&server);
    let unfreeze_key = warp::path("UnfreezeKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::exact(
            "x-amz-target",
            "TrentService.UnfreezeKey",
        ))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_unfreeze.clone()))
        .and_then(handle_unfreeze_key);

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

    // Grant session auth (GET /kms/begin-grant-session-auth?keyId=...)
    let server_bgsa = server.clone();
    let begin_grant_session_auth = warp::path("kms")
        .and(warp::path("begin-grant-session-auth"))
        .and(warp::get())
        .and(api_key_filter.clone())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::any().map(move || server_bgsa.clone()))
        .and(warp::header::optional::<String>("origin"))
        .and_then(
            |params: std::collections::HashMap<String, String>,
             server: Arc<KmsApiServer>,
             origin: Option<String>| async move {
                let key_id = params.get("keyId").cloned().unwrap_or_default();
                if key_id.is_empty() {
                    return Err(warp::reject::custom(ApiError(
                        "keyId query parameter required".to_string(),
                    )));
                }
                handle_begin_grant_session_auth(key_id, server, origin).await
            },
        );

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
        .and(warp::header::optional::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_std.clone()))
        .and_then(handle_sign_typed_data);

    let server_smv = server.clone();
    let sign_micropayment_voucher = warp::path("kms")
        .and(warp::path("SignMicropaymentVoucher"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::optional::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_smv.clone()))
        .and_then(handle_sign_micropayment_voucher);

    let server_sga = server.clone();
    let sign_gtoken_authorization = warp::path("kms")
        .and(warp::path("SignGTokenAuthorization"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::optional::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sga.clone()))
        .and_then(handle_sign_gtoken_authorization);

    let server_sx4 = server.clone();
    let sign_x402_payment = warp::path("kms")
        .and(warp::path("SignX402Payment"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::header::optional::<String>("authorization"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sx4.clone()))
        .and_then(handle_sign_x402_payment);

    let server_sgs = server.clone();
    let sign_grant_session = warp::path("kms")
        .and(warp::path("sign-grant-session"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sgs.clone()))
        .and_then(handle_sign_grant_session);

    let server_sp256gs = server.clone();
    let sign_p256_grant_session = warp::path("kms")
        .and(warp::path("sign-p256-grant-session"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(aws_kms_body())
        .and(warp::any().map(move || server_sp256gs.clone()))
        .and_then(handle_sign_p256_grant_session);

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

    // #129 contact-binding: POST /contact/begin-binding (owner ceremony, plain JSON).
    let server_begin_bind = server.clone();
    let begin_binding = warp::path("contact")
        .and(warp::path("begin-binding"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(warp::any().map(move || server_begin_bind.clone()))
        .and_then(handle_begin_binding);

    let server_claim_bind = server.clone();
    let claim_binding = warp::path("contact")
        .and(warp::path("claim-binding"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(warp::any().map(move || server_claim_bind.clone()))
        .and_then(handle_claim_binding);

    let server_confirm_bind = server.clone();
    let confirm_binding = warp::path("contact")
        .and(warp::path("confirm-binding"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(warp::any().map(move || server_confirm_bind.clone()))
        .and_then(handle_confirm_binding);

    let server_unbind = server.clone();
    let unbind_contact = warp::path("contact")
        .and(warp::path("unbind"))
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .and(warp::any().map(move || server_unbind.clone()))
        .and_then(handle_unbind_contact);

    // GET /contact/{account} — verified contacts (DVT api-key). param BEFORE method so it
    // only matches a single segment; POST /contact/* routes above are matched first.
    let server_get_contacts = server.clone();
    let get_contacts = warp::path("contact")
        .and(warp::path::param::<String>())
        .and(warp::get())
        .and(api_key_filter.clone())
        .and(rl_filter.clone())
        .and(warp::any().map(move || server_get_contacts.clone()))
        .and_then(handle_get_contacts);

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
                    let _ = server_rot
                        .db
                        .upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
                            kid: result.new_kid.clone(),
                            status: "current".to_string(),
                            created_at: now.clone(),
                            retired_at: None,
                            expires_at: None,
                        });
                    if let Some(old_kid) = result.retired_kid {
                        let retire_ts = Utc::now().timestamp() + 7 * 24 * 3600;
                        let _ = server_rot
                            .db
                            .upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
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

    // Box route groups to break warp's recursive type nesting (>~20 .or() chains overflow).
    let group1 = index
        .or(test_ui)
        .or(portal)
        .or(identities)
        .or(health)
        .or(measurements_manifest)
        .or(measurements_manifest_proof)
        .or(api_docs)
        .or(openapi_spec)
        .or(version)
        .or(key_status)
        .or(queue_status)
        .or(stats_json)
        .or(rollback_counter)
        .or(attestation)
        .or(change_passkey)
        .boxed();
    let group2 = create_key
        .or(describe_key)
        .or(list_keys)
        .or(derive_address)
        .or(sign)
        .or(sign_hash)
        .or(verify_confirm_assertion)
        .or(get_public_key)
        .boxed();
    let group3 = delete_key
        .or(unfreeze_key)
        .or(begin_registration)
        .or(complete_registration)
        .or(begin_authentication)
        .or(create_agent_key)
        .or(sign_agent)
        .or(refresh_agent_credential)
        .boxed();
    let group4 = revoke_agent_credential
        .or(sign_typed_data)
        .or(sign_micropayment_voucher)
        .or(sign_gtoken_authorization)
        .or(sign_x402_payment)
        .or(begin_grant_session_auth)
        .or(sign_grant_session)
        .or(sign_p256_grant_session)
        .or(create_p256_session_key)
        .or(begin_binding)
        .or(claim_binding)
        .or(confirm_binding)
        .or(unbind_contact)
        .or(get_contacts)
        .or(sign_p256_user_op)
        .or(revoke_p256_session_key)
        .boxed();
    // POST /admin/purge-key — admin force-delete (no passkey). Requires KMS_ADMIN_TOKEN.
    //
    // DEV/TEST ONLY — compiled in only under the `admin-purge` feature. In release
    // builds (no feature) this entire block is cfg-d out, so `group4` keeps its
    // original value and the route is never registered. Folding the route into
    // `group4` (re-boxed) keeps the final `routes` chain type-identical across both
    // compile paths, so no `.or(admin_purge)` is needed in the chain below.
    #[cfg(feature = "admin-purge")]
    let group4 = {
        let server_admin = server.clone();
        let admin_purge = warp::path!("admin" / "purge-key")
            .and(warp::post())
            .and(warp::body::json())
            .and(
                warp::header::optional::<String>("authorization").map(|h: Option<String>| {
                    h.unwrap_or_default()
                        .trim_start_matches("Bearer ")
                        .to_string()
                }),
            )
            .and(warp::any().map(move || server_admin.clone()))
            .and_then(handle_admin_purge_key);
        group4.or(admin_purge).boxed()
    };

    // Per-request access log (target "kms::access"): one line per request with
    // method, path, status, and elapsed — emitted via the `log` crate, so it
    // honours RUST_LOG (info shows it). Wraps the recovered routes so the
    // logged status reflects the final reply (incl. 4xx/5xx from rejections).
    // Note: warp::log records only method/path/status/referer/user-agent/elapsed
    // — it does NOT log request headers, so the x-api-key secret never lands here.
    let routes = group1
        .or(group2)
        .or(group3)
        .or(group4)
        .recover(handle_rejection)
        .with(warp::log("kms::access"));

    println!(
        "🚀 KMS API Server v{} starting on http://0.0.0.0:3000",
        KMS_VERSION
    );
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
    println!("   POST /UnfreezeKey   - Unfreeze dormant wallet (requires PassKey)");
    println!("   POST /ChangePasskey         - Change PassKey public key");
    println!("   POST /BeginRegistration     - WebAuthn registration (step 1)");
    println!("   POST /CompleteRegistration  - WebAuthn registration (step 2)");
    println!("   POST /BeginAuthentication   - WebAuthn authentication challenge");
    println!("   GET  /KeyStatus             - Key derivation status (polling)");
    println!("   GET  /QueueStatus           - TEE queue depth");
    println!("   GET  /RollbackCounter       - RPMB anti-rollback counter (diagnostic)");
    println!("   GET  /health                - Health check");
    println!("   POST /kms/create-agent-key       - Create AI agent key (WebAuthn)");
    println!("   POST /kms/sign-agent             - Agent sign userOpHash (Bearer JWT)");
    println!("   POST /kms/refresh-agent-credential - Refresh agent JWT (Bearer + WebAuthn)");
    println!("   POST /kms/revoke-agent-credential  - Revoke agent key (WebAuthn)");
    println!("   POST /kms/SignTypedData             - EIP-712 typed data signing");
    println!("   POST /kms/sign-grant-session        - Sign GRANT_SESSION_V2 (ECDSA session key)");
    println!(
        "   POST /kms/sign-p256-grant-session   - Sign GRANT_P256_SESSION_V2 (P256 session key)"
    );
    println!("   POST /kms/create-p256-session-key  - Create P256 session key (WebAuthn)");
    println!("   POST /kms/sign-p256-user-op        - P256 sign userOpHash (Bearer JWT)");
    println!("🔐 TA Mode: ✅ Real TA (OP-TEE Secure World required)");
    println!("🆔 TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd");
    println!("🌐 Public URL: https://kms.aastar.io");

    // CC-34: if a keeper key is configured, verify KMS_KEEPER_ADDRESS actually
    // matches the sealed key addressed by KMS_KEEPER_KEY_ID at boot — a mismatch
    // would make /kms/sign return a wrong EOA (DVT ecrecover fail / wrong funding
    // target). Fail-closed when an address is ASSERTED: if the operator set
    // KMS_KEEPER_ADDRESS, it must be successfully read from the TA AND match, or
    // startup aborts (a transient TA error is NOT an excuse to run unverified —
    // it's a funded EOA). If no address is asserted, we only log the derived one.
    if let Some(kid) = std::env::var("KMS_KEEPER_KEY_ID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok())
    {
        // Normalize an operator-supplied hex address to 20 raw bytes: trim, strip
        // optional 0x, decode, require exactly 20 bytes. None = not asserted.
        let asserted: Option<[u8; 20]> = std::env::var("KMS_KEEPER_ADDRESS").ok().and_then(|s| {
            let h = s.trim().trim_start_matches("0x").trim_start_matches("0X");
            if h.is_empty() {
                return None;
            }
            hex::decode(h).ok().filter(|b| b.len() == 20).map(|b| {
                let mut a = [0u8; 20];
                a.copy_from_slice(&b);
                a
            })
        });
        let asserted_raw = std::env::var("KMS_KEEPER_ADDRESS").ok().filter(|s| !s.trim().is_empty());
        match server.tee.keeper_pubkey(kid).await {
            Ok((_pk, addr)) => {
                let derived = format!("0x{}", hex::encode(addr));
                match &asserted_raw {
                    // Not asserted → just surface the derived address for the operator.
                    None => println!(
                        "🔑 Keeper EOA (key {}): {} — set KMS_KEEPER_ADDRESS to this value",
                        kid, derived
                    ),
                    // Asserted → must parse to 20 bytes AND match byte-for-byte, else fatal.
                    Some(raw) => {
                        if asserted == Some(addr) {
                            println!("🔑 Keeper EOA verified: {} (key {})", derived, kid);
                        } else {
                            return Err(anyhow::anyhow!(
                                "KMS_KEEPER_ADDRESS ({}) does not match the sealed keeper key {} \
                                 (derived {}) — refusing to start with a mismatched keeper address",
                                raw,
                                kid,
                                derived
                            ));
                        }
                    }
                }
            }
            // Fail-closed: if an address was asserted we MUST verify it — a boot TA
            // read failure cannot be silently ignored for a funded EOA.
            Err(e) => {
                if let Some(raw) = asserted_raw {
                    return Err(anyhow::anyhow!(
                        "keeper key {} configured with KMS_KEEPER_ADDRESS={} but the TA pubkey \
                         read failed at boot ({}) — refusing to start unverified",
                        kid,
                        raw,
                        e
                    ));
                }
                println!(
                    "⚠️  Keeper key {} configured (no KMS_KEEPER_ADDRESS asserted) but pubkey \
                     read failed at boot ({}); /kms/sign will surface errors if unresolved",
                    kid, e
                );
            }
        }
    }

    // Variant B: internal BLS signer for DVT on 127.0.0.1:3100 ONLY (localhost,
    // NOT exposed via the Cloudflare tunnel which only routes :3000). DVT points
    // RUST_SIGNER_URL here so the BLS private key stays sealed in the TA.
    let signer_server = server.clone();
    let bls_sign_route = warp::post()
        .and(warp::path("sign"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || signer_server.clone()))
        .and_then(bls_sign_handler);
    // CC-24 staked registration: BLS proof-of-possession over the operator address.
    let pop_server = server.clone();
    let pop_route = warp::post()
        .and(warp::path("pop"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024)) // node_id + operator is tiny
        .and(warp::body::json())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || pop_server.clone()))
        .and_then(pop_sign_handler);
    let gen_server = server.clone();
    let bls_gen_route = warp::post()
        .and(warp::path("gen-key"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || gen_server.clone()))
        .and_then(bls_gen_handler);
    // Remove the BLS singleton (orphan recovery / rotation) — double-gated, destructive.
    let remove_server = server.clone();
    let bls_remove_route = warp::post()
        .and(warp::path("remove-key"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || remove_server.clone()))
        .and_then(bls_remove_handler);
    // CC-34: keeper/operator ECDSA on the same loopback signer (distinct /kms/* paths).
    let keeper_sign_server = server.clone();
    let keeper_sign_route = warp::post()
        .and(warp::path("kms"))
        .and(warp::path("sign"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024)) // digest+key_id is tiny; cap body
        .and(warp::body::json())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || keeper_sign_server.clone()))
        .and_then(keeper_sign_handler);
    let keeper_gen_server = server.clone();
    let keeper_gen_route = warp::post()
        .and(warp::path("kms"))
        .and(warp::path("gen-keeper-eoa"))
        .and(warp::path::end())
        .and(warp::header::optional::<String>("x-signer-token"))
        .and(warp::any().map(move || keeper_gen_server.clone()))
        .and_then(keeper_gen_handler);
    let bls_health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&serde_json::json!({"status": "ok", "service": "kms-bls-signer"}))
    });
    let signer_routes = bls_sign_route
        .or(pop_route)
        .or(bls_gen_route)
        .or(bls_remove_route)
        .or(keeper_sign_route)
        .or(keeper_gen_route)
        .or(bls_health)
        .recover(handle_rejection);
    println!("🔏 Internal BLS signer (DVT) on http://127.0.0.1:3100 (localhost only, not via tunnel)");

    let main_srv = warp::serve(routes).run(([0, 0, 0, 0], 3000));
    let signer_srv = warp::serve(signer_routes).run(([127, 0, 0, 1], 3100));
    tokio::join!(main_srv, signer_srv);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    start_kms_server().await
}

#[cfg(test)]
mod request_deser_tests {
    use super::*;

    const WA: &str = r#"{"ChallengeId":"c","Credential":{"id":"i","rawId":"r","type":"public-key","response":{"clientDataJSON":"a","authenticatorData":"b","signature":"s"}}}"#;

    #[test]
    fn delete_key_request_minimal_webauthn() {
        let body = format!(r#"{{"KeyId":"abc","WebAuthn":{}}}"#, WA);
        let r: Result<DeleteKeyRequest, _> = serde_json::from_str(&body);
        assert!(r.is_ok(), "DeleteKeyRequest deser failed: {:?}", r.err());
    }

    #[test]
    fn derive_address_request_webauthn() {
        let body = format!(
            r#"{{"KeyId":"abc","DerivationPath":"m/44","WebAuthn":{}}}"#,
            WA
        );
        let r: Result<DeriveAddressRequest, _> = serde_json::from_str(&body);
        assert!(
            r.is_ok(),
            "DeriveAddressRequest deser failed: {:?}",
            r.err()
        );
    }
}
