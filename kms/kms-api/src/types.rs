// AWS KMS compatible API types
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyUsage {
    #[serde(rename = "SIGN_VERIFY")]
    SignVerify,
    #[serde(rename = "ENCRYPT_DECRYPT")]
    EncryptDecrypt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeySpec {
    #[serde(rename = "ECC_SECG_P256K1")]
    EccSecgP256k1,
    #[serde(rename = "RSA_2048")]
    Rsa2048,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Origin {
    #[serde(rename = "AWS_KMS")]
    AwsKms,
    #[serde(rename = "EXTERNAL")]
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "RAW")]
    Raw,
    #[serde(rename = "DIGEST")]
    Digest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SigningAlgorithm {
    #[serde(rename = "ECDSA_SHA_256")]
    EcdsaSha256,
    #[serde(rename = "RSASSA_PKCS1_V1_5_SHA_256")]
    RsassaPkcs1V15Sha256,
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    #[serde(rename = "KeyUsage")]
    pub key_usage: KeyUsage,
    #[serde(rename = "KeySpec")]
    pub key_spec: KeySpec,
    #[serde(rename = "Origin")]
    pub origin: Origin,
    #[serde(rename = "Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateKeyResponse {
    #[serde(rename = "KeyMetadata")]
    pub key_metadata: KeyMetadata,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyMetadata {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Arn")]
    pub arn: String,
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "KeyUsage")]
    pub key_usage: KeyUsage,
    #[serde(rename = "KeySpec")]
    pub key_spec: KeySpec,
    #[serde(rename = "Origin")]
    pub origin: Origin,
}

#[derive(Debug, Deserialize)]
pub struct SignRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Message")]
    pub message: String, // base64 encoded
    #[serde(rename = "MessageType")]
    pub message_type: MessageType,
    #[serde(rename = "SigningAlgorithm")]
    pub signing_algorithm: SigningAlgorithm,
}

#[derive(Debug, Serialize)]
pub struct SignResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "Signature")]
    pub signature: String, // base64 encoded
    #[serde(rename = "SigningAlgorithm")]
    pub signing_algorithm: SigningAlgorithm,
}

#[derive(Debug, Deserialize)]
pub struct GetPublicKeyRequest {
    #[serde(rename = "KeyId")]
    pub key_id: String,
}

#[derive(Debug, Serialize)]
pub struct GetPublicKeyResponse {
    #[serde(rename = "KeyId")]
    pub key_id: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String, // base64 encoded DER
    #[serde(rename = "KeyUsage")]
    pub key_usage: KeyUsage,
    #[serde(rename = "KeySpec")]
    pub key_spec: KeySpec,
    #[serde(rename = "SigningAlgorithms")]
    pub signing_algorithms: Vec<SigningAlgorithm>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    #[serde(rename = "__type")]
    pub error_type: String,
    pub message: String,
}

// Internal key storage
#[derive(Debug, Clone)]
pub struct StoredKey {
    pub id: String,
    pub arn: String,
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
    pub metadata: KeyMetadata,
}