//! JWT HS256 helpers for agent credentials.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "tee")]
use crate::ta_client::TeeHandle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JwtHeader {
    pub alg: String,
    pub typ: String,
    pub kid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JwtPayload {
    pub sub: String,
    pub wallet_id: String,
    pub agent_index: u32,
    pub agent_address: String,
    pub iat: i64,
    pub exp: i64,
}

#[cfg(feature = "tee")]
pub async fn issue_credential(
    tee: &TeeHandle,
    subject: &str,
    wallet_id: uuid::Uuid,
    agent_index: u32,
    agent_address: &str,
    ttl_secs: i64,
) -> Result<(String, i64)> {
    let now = Utc::now().timestamp();
    let exp = now + ttl_secs;

    let payload = JwtPayload {
        sub: subject.to_string(),
        wallet_id: wallet_id.to_string(),
        agent_index,
        agent_address: agent_address.to_string(),
        iat: now,
        exp,
    };
    let payload_b64 = b64_json(&payload)?;

    // Single atomic TA call: TA picks current kid, builds header, signs — no kid race possible
    let sig = tee.jwt_sign_payload(&payload_b64).await?;
    let signing_input = format!("{}.{}", sig.header_b64, payload_b64);
    let signature_b64 = URL_SAFE_NO_PAD.encode(sig.hmac);

    Ok((format!("{}.{}", signing_input, signature_b64), exp))
}

#[cfg(feature = "tee")]
pub async fn verify_credential(tee: &TeeHandle, jwt: &str) -> Result<JwtPayload> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid JWT format"));
    }

    let header: JwtHeader = decode_json(parts[0])?;
    if header.alg != "HS256" {
        return Err(anyhow!("Unsupported JWT alg: {}", header.alg));
    }

    let payload: JwtPayload = decode_json(parts[1])?;
    if payload.exp <= Utc::now().timestamp() {
        return Err(anyhow!("Agent credential expired"));
    }

    let signature = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| anyhow!("JWT signature base64url decode: {}", e))?;
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let valid = tee
        .jwt_hmac_verify(&header.kid, signing_input.as_bytes(), &signature)
        .await?;
    if !valid {
        return Err(anyhow!("Invalid agent credential signature"));
    }

    Ok(payload)
}

pub fn credential_hash(jwt: &str) -> String {
    hex::encode(Sha256::digest(jwt.as_bytes()))
}

/// Extract (kid, signing_input_bytes, hmac_bytes) from a JWT for TA-side verification.
/// `signing_input` = the bytes of `header_b64.payload_b64` (what was HMAC'd).
/// `hmac_bytes` = the raw signature bytes (32 bytes after base64url decode).
pub fn extract_signing_proof(jwt: &str) -> anyhow::Result<(String, Vec<u8>, Vec<u8>)> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid JWT format"));
    }
    let header: JwtHeader = decode_json(parts[0])?;
    let signing_input = format!("{}.{}", parts[0], parts[1]).into_bytes();
    let hmac_bytes = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| anyhow::anyhow!("JWT signature base64url decode: {}", e))?;
    if hmac_bytes.len() != 32 {
        return Err(anyhow::anyhow!("JWT HMAC must be 32 bytes, got {}", hmac_bytes.len()));
    }
    Ok((header.kid, signing_input, hmac_bytes))
}

fn b64_json<T: Serialize>(value: &T) -> Result<String> {
    let json = serde_json::to_vec(value)?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn decode_json<T: serde::de::DeserializeOwned>(segment: &str) -> Result<T> {
    let bytes = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|e| anyhow!("JWT base64url decode: {}", e))?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}
