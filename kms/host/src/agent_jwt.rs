//! JWT HS256 helpers for agent credentials.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
#[cfg(feature = "tee")]
use chrono::Utc;
use proto;
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

/// Assemble a JWT string from the material returned by `create_agent_key` TA call.
/// The payload was constructed inside TEE — wallet_id and agent_index are TEE-validated.
pub fn assemble_jwt(tee_out: &proto::CreateAgentKeyOutput) -> Result<(String, i64)> {
    let signature_b64 = URL_SAFE_NO_PAD.encode(tee_out.jwt_hmac);
    let jwt = format!(
        "{}.{}.{}",
        tee_out.jwt_header_b64, tee_out.jwt_payload_b64, signature_b64
    );
    let payload: JwtPayload = decode_json(&tee_out.jwt_payload_b64)?;
    Ok((jwt, payload.exp))
}

/// Assemble a JWT string from the material returned by `create_p256_session_key` TA call.
/// Reuses the same JWT format and verification path as agent keys.
pub fn assemble_p256_session_jwt(
    tee_out: &proto::CreateP256SessionKeyOutput,
) -> Result<(String, i64)> {
    let signature_b64 = URL_SAFE_NO_PAD.encode(tee_out.jwt_hmac);
    let jwt = format!(
        "{}.{}.{}",
        tee_out.jwt_header_b64, tee_out.jwt_payload_b64, signature_b64
    );
    let payload: JwtPayload = decode_json(&tee_out.jwt_payload_b64)?;
    Ok((jwt, payload.exp))
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
        return Err(anyhow::anyhow!(
            "JWT HMAC must be 32 bytes, got {}",
            hmac_bytes.len()
        ));
    }
    Ok((header.kid, signing_input, hmac_bytes))
}

fn decode_json<T: serde::de::DeserializeOwned>(segment: &str) -> Result<T> {
    let bytes = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|e| anyhow!("JWT base64url decode: {}", e))?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}
