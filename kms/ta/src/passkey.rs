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

//! Passkey (FIDO2/WebAuthn) P-256 signature verification module

use anyhow::{anyhow, Result};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use sha2::{Digest, Sha256};

/// Verify a P-256 ECDSA signature (Passkey/FIDO2)
///
/// # Arguments
/// * `pubkey_sec1` - SEC1 encoded public key (65 bytes uncompressed: 0x04 + x + y)
/// * `message` - Message that was signed
/// * `signature_der` - DER encoded signature
///
/// # Returns
/// * `Ok(())` if signature is valid
/// * `Err(_)` if signature is invalid or verification fails
pub fn verify_passkey_signature(
    pubkey_sec1: &[u8],
    message: &[u8],
    signature_der: &[u8],
) -> Result<()> {
    // Parse public key from SEC1 format
    let verifying_key = VerifyingKey::from_sec1_bytes(pubkey_sec1)
        .map_err(|e| anyhow!("Invalid P-256 public key: {:?}", e))?;

    // Parse signature from DER format
    let signature = Signature::from_der(signature_der)
        .map_err(|e| anyhow!("Invalid DER signature: {:?}", e))?;

    // Verify signature
    verifying_key
        .verify(message, &signature)
        .map_err(|e| anyhow!("Signature verification failed: {:?}", e))?;

    Ok(())
}

/// Hash message with SHA-256 (used for Passkey challenge-response)
pub fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hash() {
        let data = b"hello world";
        let hash = sha256_hash(data);

        // Expected SHA-256 of "hello world"
        let expected =
            hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
                .unwrap();

        assert_eq!(&hash[..], &expected[..]);
    }
}
