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
use kms::db::{KmsDb, WalletRow};
use kms::webauthn;
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

/// Parse DER-encoded ECDSA signature into (r, s) 32-byte arrays
fn parse_der_signature(der: &[u8]) -> Result<([u8; 32], [u8; 32])> {
    if der.len() < 8 || der[0] != 0x30 {
        return Err(anyhow!("Invalid DER signature"));
    }
    let mut pos = 2; // skip 0x30 + total_len
    if der[pos] != 0x02 {
        return Err(anyhow!("Expected INTEGER tag for r"));
    }
    pos += 1;
    let r_len = der[pos] as usize;
    pos += 1;
    let r_raw = &der[pos..pos + r_len];
    pos += r_len;
    if der[pos] != 0x02 {
        return Err(anyhow!("Expected INTEGER tag for s"));
    }
    pos += 1;
    let s_len = der[pos] as usize;
    pos += 1;
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
    rp_name: String,
    rp_id: String,
    expected_origin: String,
}

impl KmsApiServer {
    pub fn new(db: KmsDb) -> Self {
        let rp_id = std::env::var("KMS_RP_ID").unwrap_or_else(|_| "aastar.io".to_string());
        let rp_name = std::env::var("KMS_RP_NAME").unwrap_or_else(|_| "AirAccount KMS".to_string());
        let expected_origin = std::env::var("KMS_ORIGIN")
            .unwrap_or_else(|_| format!("https://{}", rp_id));
        Self {
            db,
            tee: TeeHandle::new(),
            rp_name,
            rp_id,
            expected_origin,
        }
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
        QueueStatusResponse {
            queue_depth: depth,
            estimated_wait_seconds: depth as u64 * TEE_OP_ESTIMATE_SECS,
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
                &self.expected_origin,
                &self.rp_id,
                &pk_bytes,
                w.sign_count,
            )?;

            // Update sign_count in DB
            let _ = self.db.update_wallet_sign_count(key_id, verified.new_counter);

            Ok(Some(verified.proto_assertion))
        } else if raw.is_some() {
            // Legacy hex path
            let assertion = Self::parse_passkey_assertion(raw)?;
            self.pre_verify_passkey(key_id, &assertion).await?;
            Ok(assertion)
        } else {
            Ok(None)
        }
    }

    pub async fn derive_address(&self, req: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        println!("📝 KMS DeriveAddress API called for key: {}", req.key_id);

        if !self.db.wallet_exists(&req.key_id)? {
            return Err(anyhow!("Key not found: {}", req.key_id));
        }

        let wallet_uuid = Uuid::parse_str(&req.key_id)?;
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
            let mut to_array = [0u8; 20];
            to_array.copy_from_slice(&to_bytes[..20]);

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
        // 支持三种方式:
        // 1. Address (优先级最高,从 DB 查找)
        // 2. KeyId + DerivationPath (手动指定路径)
        // 3. KeyId only (自动使用默认路径)
        let (wallet_uuid, derivation_path) = if let Some(address) = &req.address {
            println!("📝 KMS SignHash API called with Address: {}", address);

            let row = self.db.lookup_address(address)?
                .ok_or_else(|| anyhow!("Address not found: {}", address))?;

            (Uuid::parse_str(&row.key_id)?, row.derivation_path)
        } else if let Some(key_id) = &req.key_id {
            println!("📝 KMS SignHash API called with KeyId: {}", key_id);

            let w = self.db.get_wallet(key_id)?
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;

            let derivation_path = req.derivation_path
                .or(w.derivation_path)
                .ok_or_else(|| anyhow!("No derivation path available for this key"))?;

            (Uuid::parse_str(key_id)?, derivation_path)
        } else {
            return Err(anyhow!("Either KeyId or Address must be provided"));
        };

        // Resolve passkey assertion (WebAuthn ceremony or legacy hex)
        let key_id_str = wallet_uuid.to_string();
        let passkey_assertion = self.resolve_passkey_assertion(
            &key_id_str, req.passkey.as_ref(), req.webauthn.as_ref(),
        ).await?;

        let hash_bytes = if req.hash.starts_with("0x") {
            hex::decode(&req.hash[2..])?
        } else {
            hex::decode(&req.hash)?
        };

        if hash_bytes.len() != 32 {
            return Err(anyhow!("Hash must be exactly 32 bytes, got {} bytes", hash_bytes.len()));
        }

        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(&hash_bytes);

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

    pub async fn begin_registration(&self, req: webauthn::BeginRegistrationRequest) -> Result<webauthn::RegistrationOptionsResponse> {
        let user_name = req.user_name.as_deref().unwrap_or("wallet-user");
        let user_display = req.user_display_name.as_deref().unwrap_or("AirAccount Wallet");

        let (challenge_id, challenge_bytes, resp) = webauthn::generate_registration_options(
            &self.rp_name, &self.rp_id, user_name, user_display, vec![],
        );

        self.db.store_challenge(&challenge_id, &challenge_bytes, None, "registration", &self.rp_id, 300)?;

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
            meta_json.as_bytes(), None, "registration_meta", &self.rp_id, 300,
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

        // 3. Verify attestation
        let verified = webauthn::verify_registration_response(
            &req.credential, &challenge_row.challenge, &self.expected_origin, &self.rp_id,
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

    pub async fn begin_authentication(&self, req: webauthn::BeginAuthenticationRequest) -> Result<webauthn::AuthenticationOptionsResponse> {
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

        let (challenge_id, challenge_bytes, resp) = webauthn::generate_authentication_options(
            &self.rp_id, allow_credentials,
        );

        self.db.store_challenge(&challenge_id, &challenge_bytes, Some(&key_id), "authentication", &self.rp_id, 300)?;

        println!("📝 WebAuthn BeginAuthentication: challenge_id={}, key_id={}", challenge_id, key_id);
        Ok(resp)
    }
}

// ========================================
// HTTP Server Routes
// ========================================

async fn health_check() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&serde_json::json!({
        "status": "healthy",
        "service": "kms-api",
        "version": "0.1.0",
        "ta_mode": "real",
        "endpoints": {
            "POST": ["/CreateKey", "/DeleteKey", "/DescribeKey", "/ListKeys", "/DeriveAddress", "/Sign", "/SignHash", "/ChangePasskey", "/BeginRegistration", "/CompleteRegistration", "/BeginAuthentication"],
            "GET": ["/health", "/KeyStatus?KeyId=xxx", "/QueueStatus"]
        }
    })))
}

async fn handle_create_key(
    body: CreateKeyRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.create_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("CreateKey error: {}", e);
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
    match server.derive_address(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("DeriveAddress error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign(
    body: SignRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.sign(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("Sign error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_sign_hash(
    body: SignHashRequest,
    server: Arc<KmsApiServer>
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.sign_hash(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("SignHash error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
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
    match server.delete_key(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ScheduleKeyDeletion error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_change_passkey(
    body: ChangePasskeyRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.change_passkey(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("ChangePasskey error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_begin_registration(
    body: webauthn::BeginRegistrationRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.begin_registration(body).await {
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
    match server.complete_registration(body).await {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(e) => {
            eprintln!("CompleteRegistration error: {}", e);
            Err(warp::reject::custom(ApiError(e.to_string())))
        }
    }
}

async fn handle_begin_authentication(
    body: webauthn::BeginAuthenticationRequest,
    server: Arc<KmsApiServer>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match server.begin_authentication(body).await {
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

#[derive(Debug)]
struct ApiError(String);

impl warp::reject::Reject for ApiError {}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    if let Some(api_error) = err.find::<ApiError>() {
        let status = if api_error.0.contains("API key") {
            warp::http::StatusCode::UNAUTHORIZED
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

fn aws_kms_body<T: serde::de::DeserializeOwned + Send>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::bytes().and_then(|bytes: bytes::Bytes| async move {
        serde_json::from_slice(&bytes)
            .map_err(|e| {
                eprintln!("JSON parse error: {}", e);
                warp::reject::custom(ApiError(format!("Invalid JSON: {}", e)))
            })
    })
}

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

    // API Key guard: enabled if DB has keys or KMS_API_KEY env var is set
    let legacy_key = std::env::var("KMS_API_KEY").ok();
    let has_db_keys = db.has_api_keys().unwrap_or(false);
    let api_key_enabled = has_db_keys || legacy_key.is_some();
    if api_key_enabled {
        let source = match (has_db_keys, legacy_key.is_some()) {
            (true, true) => "DB + env",
            (true, false) => "DB",
            (false, true) => "env (KMS_API_KEY)",
            _ => unreachable!(),
        };
        println!("🔑 API Key authentication: ENABLED (source: {})", source);
    } else {
        println!("⚠️  API Key authentication: DISABLED (run `api-key generate` to enable)");
    }
    let api_key_filter = db_api_key_filter(db, legacy_key, api_key_enabled);

    // Root path - serve simple welcome message
    let index = warp::path::end()
        .and(warp::get())
        .map(|| {
            warp::reply::html(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>KMS API</title></head>
<body style="font-family: system-ui; max-width: 800px; margin: 50px auto; padding: 20px;">
<h1>🔐 AirAccount KMS API</h1>
<p>Welcome to the KMS API Server. This server provides AWS KMS-compatible APIs powered by OP-TEE.</p>
<h2>Endpoints:</h2>
<ul>
<li>POST /CreateKey - Create new wallet</li>
<li>POST /DescribeKey - Query wallet metadata</li>
<li>POST /ListKeys - List all wallets</li>
<li>POST /DeriveAddress - Derive Ethereum address</li>
<li>POST /Sign - Sign message</li>
<li>POST /GetPublicKey - Get public key</li>
<li>POST /DeleteKey - Delete wallet (requires PassKey)</li>
<li>POST /BeginRegistration - WebAuthn registration ceremony (step 1)</li>
<li>POST /CompleteRegistration - WebAuthn registration ceremony (step 2)</li>
<li>POST /BeginAuthentication - WebAuthn authentication ceremony</li>
<li>GET /health - Health check</li>
</ul>
<p>For interactive testing, visit: <a href="/test">Test UI</a></p>
<p>API is running on OP-TEE Secure World with TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd</p>
</body>
</html>"#)
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

    // ChangePasskey API
    let server_cp = server.clone();
    let change_passkey = warp::path("ChangePasskey")
        .and(warp::post())
        .and(api_key_filter.clone())
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

    // CreateKey API
    let create_key = warp::path("CreateKey")
        .and(warp::post())
        .and(api_key_filter.clone())
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

    // DeriveAddress API
    let derive_address = warp::path("DeriveAddress")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.DeriveAddress"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server4.clone()))
        .and_then(handle_derive_address);

    // Sign API
    let sign = warp::path("Sign")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.Sign"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server5.clone()))
        .and_then(handle_sign);

    // SignHash API
    let server6_clone = Arc::clone(&server);
    let sign_hash = warp::path("SignHash")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.SignHash"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6_clone.clone()))
        .and_then(handle_sign_hash);

    // GetPublicKey API
    let get_public_key = warp::path("GetPublicKey")
        .and(warp::post())
        .and(api_key_filter.clone())
        .and(warp::header::exact("x-amz-target", "TrentService.GetPublicKey"))
        .and(aws_kms_body())
        .and(warp::any().map(move || server6.clone()))
        .and_then(handle_get_public_key);

    // DeleteKey API
    let server7 = Arc::clone(&server);
    let delete_key = warp::path("DeleteKey")
        .and(warp::post())
        .and(api_key_filter.clone())
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
        .and_then(handle_begin_authentication);

    let routes = index
        .or(test_ui)
        .or(health)
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
        .recover(handle_rejection);

    println!("🚀 KMS API Server starting on http://0.0.0.0:3000");
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