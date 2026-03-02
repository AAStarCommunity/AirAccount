//! WebAuthn ceremony logic — compatible with SimpleWebAuthn.
//!
//! Pure functions: parse attestation, verify assertions, generate options.
//! No IO or TA calls — those happen in api_server.rs.

use std::convert::TryInto;
use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use p256::EncodedPoint;
use uuid::Uuid;

// ========================================
// Base64URL helpers
// ========================================

pub fn b64url_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s).map_err(|e| anyhow!("base64url decode: {}", e))
}

/// Generate 32 bytes of randomness using two UUID v4s.
pub fn random_challenge() -> Vec<u8> {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(a.as_bytes());
    out.extend_from_slice(b.as_bytes());
    out
}

// ========================================
// SimpleWebAuthn-compatible types (Server → Browser)
// ========================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelyingParty {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserEntity {
    pub id: String, // base64url
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PubKeyCredParam {
    #[serde(rename = "type")]
    pub type_: String,
    pub alg: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthenticatorSelection {
    #[serde(rename = "residentKey", skip_serializing_if = "Option::is_none")]
    pub resident_key: Option<String>,
    #[serde(rename = "userVerification", skip_serializing_if = "Option::is_none")]
    pub user_verification: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CredentialDescriptor {
    pub id: String, // base64url
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<String>>,
}

/// Server → Browser: PublicKeyCredentialCreationOptionsJSON
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationOptionsResponse {
    #[serde(rename = "ChallengeId")]
    pub challenge_id: String,
    #[serde(rename = "Options")]
    pub options: RegistrationOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationOptions {
    pub rp: RelyingParty,
    pub user: UserEntity,
    pub challenge: String, // base64url
    #[serde(rename = "pubKeyCredParams")]
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    pub timeout: u64,
    pub attestation: String,
    #[serde(rename = "excludeCredentials")]
    pub exclude_credentials: Vec<CredentialDescriptor>,
    #[serde(rename = "authenticatorSelection")]
    pub authenticator_selection: AuthenticatorSelection,
}

/// Server → Browser: PublicKeyCredentialRequestOptionsJSON
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationOptionsResponse {
    #[serde(rename = "ChallengeId")]
    pub challenge_id: String,
    #[serde(rename = "Options")]
    pub options: AuthenticationOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationOptions {
    pub challenge: String, // base64url
    pub timeout: u64,
    #[serde(rename = "rpId")]
    pub rp_id: String,
    #[serde(rename = "allowCredentials")]
    pub allow_credentials: Vec<CredentialDescriptor>,
    #[serde(rename = "userVerification")]
    pub user_verification: String,
}

// ========================================
// SimpleWebAuthn-compatible types (Browser → Server)
// ========================================

/// Browser → Server: RegistrationResponseJSON
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationResponseJSON {
    pub id: String,
    #[serde(rename = "rawId")]
    pub raw_id: String,
    pub response: AttestationResponseJSON,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "authenticatorAttachment", skip_serializing_if = "Option::is_none", default)]
    pub authenticator_attachment: Option<String>,
    #[serde(rename = "clientExtensionResults", default)]
    pub client_extension_results: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttestationResponseJSON {
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String, // base64url
    #[serde(rename = "attestationObject")]
    pub attestation_object: String, // base64url
    #[serde(rename = "transports", skip_serializing_if = "Option::is_none", default)]
    pub transports: Option<Vec<String>>,
}

/// Browser → Server: AuthenticationResponseJSON
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthenticationResponseJSON {
    pub id: String,
    #[serde(rename = "rawId")]
    pub raw_id: String,
    pub response: AssertionResponseJSON,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "clientExtensionResults", default)]
    pub client_extension_results: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssertionResponseJSON {
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String, // base64url
    #[serde(rename = "authenticatorData")]
    pub authenticator_data: String, // base64url
    pub signature: String, // base64url (DER)
    #[serde(rename = "userHandle", skip_serializing_if = "Option::is_none", default)]
    pub user_handle: Option<String>,
}

// ========================================
// API request/response wrappers
// ========================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BeginRegistrationRequest {
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "UserName", default)]
    pub user_name: Option<String>,
    #[serde(rename = "UserDisplayName", default)]
    pub user_display_name: Option<String>,
    #[serde(rename = "KeyUsage", default)]
    pub key_usage: Option<String>,
    #[serde(rename = "KeySpec", default)]
    pub key_spec: Option<String>,
    #[serde(rename = "Origin", default)]
    pub origin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteRegistrationRequest {
    #[serde(rename = "ChallengeId")]
    pub challenge_id: String,
    #[serde(rename = "Credential")]
    pub credential: RegistrationResponseJSON,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "KeyUsage", default)]
    pub key_usage: Option<String>,
    #[serde(rename = "KeySpec", default)]
    pub key_spec: Option<String>,
    #[serde(rename = "Origin", default)]
    pub origin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteRegistrationResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "CredentialId")]
    pub credential_id: String, // base64url
    #[serde(rename = "Status")]
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BeginAuthenticationRequest {
    #[serde(rename = "KeyId", skip_serializing_if = "Option::is_none", default)]
    pub key_id: Option<String>,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none", default)]
    pub address: Option<String>,
}

// ========================================
// Verification results
// ========================================

pub struct VerifiedRegistration {
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>, // 65 bytes uncompressed P-256
    pub sign_count: u32,
    pub transports: Option<Vec<String>>,
}

pub struct VerifiedAuthentication {
    pub new_counter: u32,
    pub credential_id: Vec<u8>,
    pub proto_assertion: proto::PasskeyAssertion,
}

// ========================================
// Registration options generation
// ========================================

pub fn generate_registration_options(
    rp_name: &str,
    rp_id: &str,
    user_name: &str,
    user_display_name: &str,
    exclude_credentials: Vec<CredentialDescriptor>,
) -> (String, Vec<u8>, RegistrationOptionsResponse) {
    let challenge_id = Uuid::new_v4().to_string();
    let challenge_bytes = random_challenge();
    let challenge_b64 = b64url_encode(&challenge_bytes);
    let user_id = b64url_encode(Uuid::new_v4().as_bytes());

    let resp = RegistrationOptionsResponse {
        challenge_id: challenge_id.clone(),
        options: RegistrationOptions {
            rp: RelyingParty {
                name: rp_name.to_string(),
                id: rp_id.to_string(),
            },
            user: UserEntity {
                id: user_id,
                name: user_name.to_string(),
                display_name: user_display_name.to_string(),
            },
            challenge: challenge_b64,
            pub_key_cred_params: vec![
                PubKeyCredParam { type_: "public-key".to_string(), alg: -7 }, // ES256
            ],
            timeout: 300_000,
            attestation: "none".to_string(),
            exclude_credentials,
            authenticator_selection: AuthenticatorSelection {
                resident_key: Some("preferred".to_string()),
                user_verification: Some("required".to_string()),
            },
        },
    };
    (challenge_id, challenge_bytes, resp)
}

// ========================================
// Registration verification
// ========================================

pub fn verify_registration_response(
    response: &RegistrationResponseJSON,
    expected_challenge: &[u8],
    expected_origin: &str,
    expected_rp_id: &str,
) -> Result<VerifiedRegistration> {
    // 1. Decode and verify clientDataJSON
    let client_data_bytes = b64url_decode(&response.response.client_data_json)?;
    let client_data: serde_json::Value = serde_json::from_slice(&client_data_bytes)
        .map_err(|e| anyhow!("Invalid clientDataJSON: {}", e))?;

    let cd_type = client_data["type"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'type'"))?;
    if cd_type != "webauthn.create" {
        return Err(anyhow!("clientDataJSON type must be 'webauthn.create', got '{}'", cd_type));
    }

    let cd_challenge = client_data["challenge"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'challenge'"))?;
    let decoded_challenge = b64url_decode(cd_challenge)?;
    if decoded_challenge != expected_challenge {
        return Err(anyhow!("Challenge mismatch"));
    }

    let cd_origin = client_data["origin"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'origin'"))?;
    if cd_origin != expected_origin {
        return Err(anyhow!("Origin mismatch: expected '{}', got '{}'", expected_origin, cd_origin));
    }

    // 2. Decode attestationObject (CBOR)
    let att_obj_bytes = b64url_decode(&response.response.attestation_object)?;
    let cbor: ciborium::Value = ciborium::from_reader(&att_obj_bytes[..])
        .map_err(|e| anyhow!("CBOR decode attestationObject: {}", e))?;

    let map = match &cbor {
        ciborium::Value::Map(m) => m,
        _ => return Err(anyhow!("attestationObject is not a CBOR map")),
    };

    // 3. Extract authData
    let auth_data = map.iter()
        .find_map(|(k, v)| {
            if matches!(k, ciborium::Value::Text(s) if s == "authData") {
                match v {
                    ciborium::Value::Bytes(b) => Some(b.clone()),
                    _ => None,
                }
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("missing authData in attestationObject"))?;

    // 4. Verify rpIdHash
    let expected_rp_hash = Sha256::digest(expected_rp_id.as_bytes());
    if auth_data.len() < 37 {
        return Err(anyhow!("authData too short: {} bytes", auth_data.len()));
    }
    if auth_data[0..32] != expected_rp_hash[..] {
        return Err(anyhow!("rpIdHash mismatch"));
    }

    // 5. Parse flags
    let flags = auth_data[32];
    let up = flags & 0x01 != 0;
    let at = flags & 0x40 != 0;
    if !up {
        return Err(anyhow!("User Presence flag not set"));
    }
    if !at {
        return Err(anyhow!("AT flag not set — no attested credential data"));
    }

    let sign_count = u32::from_be_bytes(auth_data[33..37].try_into()
        .map_err(|_| anyhow!("bad signCount bytes"))?);

    // 6. Parse attested credential data
    if auth_data.len() < 55 {
        return Err(anyhow!("authData too short for attested credential data"));
    }
    // aaguid = auth_data[37..53] (skip)
    let cred_id_len = u16::from_be_bytes(auth_data[53..55].try_into()
        .map_err(|_| anyhow!("bad credIdLength bytes"))?) as usize;

    if auth_data.len() < 55 + cred_id_len + 1 {
        return Err(anyhow!("authData too short for credentialId + COSE key"));
    }
    let credential_id = auth_data[55..55 + cred_id_len].to_vec();

    // 7. Parse COSE key
    let cose_key_bytes = &auth_data[55 + cred_id_len..];
    let cose_key: ciborium::Value = ciborium::from_reader(cose_key_bytes)
        .map_err(|e| anyhow!("CBOR decode COSE key: {}", e))?;

    let cose_map = match &cose_key {
        ciborium::Value::Map(m) => m,
        _ => return Err(anyhow!("COSE key is not a CBOR map")),
    };

    let x = find_cose_bytes(cose_map, -2)
        .ok_or_else(|| anyhow!("COSE key missing x (-2)"))?;
    let y = find_cose_bytes(cose_map, -3)
        .ok_or_else(|| anyhow!("COSE key missing y (-3)"))?;

    if x.len() != 32 || y.len() != 32 {
        return Err(anyhow!("COSE key x/y not 32 bytes: x={}, y={}", x.len(), y.len()));
    }

    // 8. Construct uncompressed P-256 point
    let mut pubkey = Vec::with_capacity(65);
    pubkey.push(0x04);
    pubkey.extend_from_slice(&x);
    pubkey.extend_from_slice(&y);

    // Validate it's a valid P-256 point
    EncodedPoint::from_bytes(&pubkey)
        .map_err(|e| anyhow!("Invalid P-256 point: {:?}", e))?;

    Ok(VerifiedRegistration {
        credential_id,
        public_key: pubkey,
        sign_count,
        transports: response.response.transports.clone(),
    })
}

fn find_cose_bytes(map: &[(ciborium::Value, ciborium::Value)], label: i64) -> Option<Vec<u8>> {
    map.iter().find_map(|(k, v)| {
        let matches = match k {
            ciborium::Value::Integer(i) => {
                let n: i128 = (*i).into();
                n == label as i128
            }
            _ => false,
        };
        if matches {
            match v {
                ciborium::Value::Bytes(b) => Some(b.clone()),
                _ => None,
            }
        } else {
            None
        }
    })
}

// ========================================
// Authentication options generation
// ========================================

pub fn generate_authentication_options(
    rp_id: &str,
    allow_credentials: Vec<CredentialDescriptor>,
) -> (String, Vec<u8>, AuthenticationOptionsResponse) {
    let challenge_id = Uuid::new_v4().to_string();
    let challenge_bytes = random_challenge();
    let challenge_b64 = b64url_encode(&challenge_bytes);

    let resp = AuthenticationOptionsResponse {
        challenge_id: challenge_id.clone(),
        options: AuthenticationOptions {
            challenge: challenge_b64,
            timeout: 300_000,
            rp_id: rp_id.to_string(),
            allow_credentials,
            user_verification: "required".to_string(),
        },
    };
    (challenge_id, challenge_bytes, resp)
}

// ========================================
// Authentication verification
// ========================================

/// Verify an authentication assertion (browser response from navigator.credentials.get()).
///
/// Returns a proto::PasskeyAssertion that can be forwarded to TA, plus the new sign counter.
pub fn verify_authentication_response(
    response: &AuthenticationResponseJSON,
    expected_challenge: &[u8],
    expected_origin: &str,
    expected_rp_id: &str,
    stored_pubkey_uncompressed: &[u8], // 65 bytes
    stored_counter: u32,
) -> Result<VerifiedAuthentication> {
    // 1. Decode and verify clientDataJSON
    let client_data_bytes = b64url_decode(&response.response.client_data_json)?;
    let client_data: serde_json::Value = serde_json::from_slice(&client_data_bytes)
        .map_err(|e| anyhow!("Invalid clientDataJSON: {}", e))?;

    let cd_type = client_data["type"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'type'"))?;
    if cd_type != "webauthn.get" {
        return Err(anyhow!("clientDataJSON type must be 'webauthn.get', got '{}'", cd_type));
    }

    let cd_challenge = client_data["challenge"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'challenge'"))?;
    let decoded_challenge = b64url_decode(cd_challenge)?;
    if decoded_challenge != expected_challenge {
        return Err(anyhow!("Challenge mismatch"));
    }

    let cd_origin = client_data["origin"].as_str()
        .ok_or_else(|| anyhow!("clientDataJSON missing 'origin'"))?;
    if cd_origin != expected_origin {
        return Err(anyhow!("Origin mismatch: expected '{}', got '{}'", expected_origin, cd_origin));
    }

    // 2. Decode authenticatorData
    let auth_data = b64url_decode(&response.response.authenticator_data)?;
    if auth_data.len() < 37 {
        return Err(anyhow!("authenticatorData too short: {} bytes", auth_data.len()));
    }

    // 3. Verify rpIdHash
    let expected_rp_hash = Sha256::digest(expected_rp_id.as_bytes());
    if auth_data[0..32] != expected_rp_hash[..] {
        return Err(anyhow!("rpIdHash mismatch"));
    }

    // 4. Check flags
    let flags = auth_data[32];
    if flags & 0x01 == 0 {
        return Err(anyhow!("User Presence flag not set"));
    }

    // 5. Check signCount
    let sign_count = u32::from_be_bytes(auth_data[33..37].try_into()
        .map_err(|_| anyhow!("bad signCount bytes"))?);
    if stored_counter > 0 && sign_count > 0 && sign_count <= stored_counter {
        return Err(anyhow!("signCount not incremented ({} <= {}), possible cloned authenticator",
            sign_count, stored_counter));
    }

    // 6. Compute client_data_hash
    let client_data_hash: [u8; 32] = Sha256::digest(&client_data_bytes).into();

    // 7. Construct signatureBase = authenticatorData || client_data_hash
    let mut signature_base = Vec::with_capacity(auth_data.len() + 32);
    signature_base.extend_from_slice(&auth_data);
    signature_base.extend_from_slice(&client_data_hash);

    // 8. Decode DER signature and verify
    let sig_bytes = b64url_decode(&response.response.signature)?;

    let encoded_point = EncodedPoint::from_bytes(stored_pubkey_uncompressed)
        .map_err(|e| anyhow!("Invalid stored pubkey: {:?}", e))?;
    let verifying_key = VerifyingKey::from_encoded_point(&encoded_point)
        .map_err(|e| anyhow!("Failed to parse verifying key: {:?}", e))?;

    let der_sig = p256::ecdsa::DerSignature::from_bytes(&sig_bytes)
        .map_err(|e| anyhow!("Invalid DER signature: {:?}", e))?;
    let signature: Signature = der_sig.try_into()
        .map_err(|e| anyhow!("DER to Signature: {:?}", e))?;

    verifying_key.verify(&signature_base, &signature)
        .map_err(|_| anyhow!("WebAuthn signature verification failed"))?;

    // 9. Extract r, s for proto::PasskeyAssertion
    let (r_bytes, s_bytes) = signature.split_bytes();
    let mut signature_r = [0u8; 32];
    let mut signature_s = [0u8; 32];
    signature_r.copy_from_slice(&r_bytes);
    signature_s.copy_from_slice(&s_bytes);

    let credential_id = b64url_decode(&response.id)?;

    Ok(VerifiedAuthentication {
        new_counter: sign_count,
        credential_id,
        proto_assertion: proto::PasskeyAssertion {
            authenticator_data: auth_data,
            client_data_hash,
            signature_r,
            signature_s,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64url_roundtrip() {
        let data = vec![0u8, 1, 2, 255, 254, 253];
        let encoded = b64url_encode(&data);
        let decoded = b64url_decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn random_challenge_is_32_bytes() {
        let c = random_challenge();
        assert_eq!(c.len(), 32);
    }

    #[test]
    fn registration_options_structure() {
        let (cid, challenge, resp) = generate_registration_options(
            "AirAccount", "aastar.io", "alice", "Alice", vec![],
        );
        assert!(!cid.is_empty());
        assert_eq!(challenge.len(), 32);
        assert_eq!(resp.options.rp.id, "aastar.io");
        assert_eq!(resp.options.pub_key_cred_params[0].alg, -7);
        assert_eq!(resp.options.attestation, "none");
    }

    #[test]
    fn authentication_options_structure() {
        let creds = vec![CredentialDescriptor {
            id: b64url_encode(b"cred-123"),
            type_: "public-key".to_string(),
            transports: Some(vec!["internal".to_string()]),
        }];
        let (cid, challenge, resp) = generate_authentication_options("aastar.io", creds);
        assert!(!cid.is_empty());
        assert_eq!(challenge.len(), 32);
        assert_eq!(resp.options.rp_id, "aastar.io");
        assert_eq!(resp.options.allow_credentials.len(), 1);
    }

    #[test]
    fn verify_registration_bad_challenge() {
        // Minimal test: bad clientDataJSON should fail
        let fake_response = RegistrationResponseJSON {
            id: "test".to_string(),
            raw_id: "test".to_string(),
            response: AttestationResponseJSON {
                client_data_json: b64url_encode(br#"{"type":"webauthn.create","challenge":"AAAA","origin":"https://example.com"}"#),
                attestation_object: b64url_encode(b"fake"),
                transports: None,
            },
            type_: "public-key".to_string(),
            authenticator_attachment: None,
            client_extension_results: serde_json::Value::Object(Default::default()),
        };
        let result = verify_registration_response(
            &fake_response, b"wrong-challenge", "https://example.com", "example.com",
        );
        assert!(result.is_err());
    }
}
