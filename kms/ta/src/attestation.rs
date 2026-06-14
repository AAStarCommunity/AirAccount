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

//! Issue #37 — remote-attestation MVP (Phase 1).
//!
//! Produces an attestation evidence blob proving that *this TA*, with its
//! measured signed-header digest, is executing inside a real OP-TEE on this
//! device, bound to a caller-supplied nonce.
//!
//! It does so by invoking the **OP-TEE attestation PTA**
//! (`39800861-182a-4720-9b67-2bcd622bc0b5`), verified present in the NXP i.MX93
//! BSP (`tee-pager_v2.bin` contains `attestation.pta` + `core/pta/attestation.c`
//! — R-2 confirmed 2026-06-14). Two PTA commands are used:
//!
//!  * `GET_TA_SHDR_DIGEST` (0x1): params = (MEMREF_IN uuid[16], MEMREF_IN nonce,
//!    MEMREF_OUT). Output = `[32-byte SHA-256 TA digest | RSA-PSS signature]`,
//!    where the signature is over `SHA256(nonce | digest)` with
//!    `TEE_ALG_RSASSA_PKCS1_PSS_MGF1_SHA256` (salt length 32).
//!  * `GET_PUBKEY` (0x0): params = (MEMREF_OUT exp, MEMREF_OUT mod, VALUE_OUT
//!    alg). Returns the attestation RSA public key (big-endian e and n).
//!
//! All param layouts verified verbatim against the official sources:
//!   - lib/libutee/include/pta_attestation.h
//!   - core/pta/attestation.c
//!
//! ⚠️ Trust-root caveat (MVP): the PTA's attestation RSA key is self-generated
//! by the device's OP-TEE on first use and has NO certificate chain to an NXP
//! root (confirmed in `core/pta/attestation.c`). Verifiers therefore trust this
//! key via TOFU / a published reference value — see
//! `docs/design/37-remote-attestation-design.md` §9 (R-1).

use anyhow::{anyhow, bail, Result};
use optee_utee::{ParamIndex, TaSession, TaSessionBuilder, TeeParams, Time, Uuid};

/// OP-TEE attestation PTA UUID (lib/libutee/include/pta_attestation.h).
const PTA_ATTESTATION_UUID: &str = "39800861-182a-4720-9b67-2bcd622bc0b5";

/// PTA command IDs (pta_attestation.h).
const PTA_ATTESTATION_GET_PUBKEY: u32 = 0x0;
const PTA_ATTESTATION_GET_TA_SHDR_DIGEST: u32 = 0x1;

/// TA signed-header digest is SHA-256 → always 32 bytes.
const TA_MEASUREMENT_LEN: usize = 32;

/// Output buffer for `GET_TA_SHDR_DIGEST`: 32-byte digest + RSA signature.
/// Sized for an RSA-4096 attestation key (512-byte sig) with margin; the PTA
/// writes back the real length so over-allocation is harmless.
const SHDR_OUT_BUF: usize = TA_MEASUREMENT_LEN + 640;
/// Output buffer for the modulus (RSA-4096 → 512 bytes) with margin.
const PUBKEY_MOD_BUF: usize = 640;
/// Output buffer for the public exponent (typically 3 bytes: 0x010001).
const PUBKEY_EXP_BUF: usize = 64;

/// Build the attestation evidence for issue #37 (MVP / Phase 1).
pub fn get_attestation(
    input: &proto::GetAttestationInput,
) -> Result<proto::GetAttestationOutput> {
    // The attestation PTA rejects an empty nonce (it is the replay defence).
    if input.nonce.is_empty() {
        bail!("attestation nonce must be non-empty");
    }

    // This TA's own UUID. The attestation PTA reads params[0] by *directly
    // casting* the buffer to `TEE_UUID` (core/pta/attestation.c:
    // `TEE_UUID *uuid = params[0].memref.buffer;` — NO tee_uuid_from_octets()).
    // So the 16 bytes must be the native in-memory `TEE_UUID` layout, NOT the
    // canonical big-endian RFC-4122 octets: `timeLow` (u32), `timeMid` (u16) and
    // `timeHiAndVersion` (u16) are CPU-endian, only `clockSeqAndNode` is a raw
    // byte array. TA and PTA share the core, so native-endian is exactly right.
    // Passing big-endian octets instead makes ts_store reconstruct a byte-
    // swapped filename and fail with TEE_ERROR_ITEM_NOT_FOUND.
    let self_uuid = uuid::Uuid::parse_str(proto::UUID.trim())
        .map_err(|e| anyhow!("invalid TA UUID constant: {e}"))?;
    let (time_low, time_mid, time_hi_ver, clock_seq) = self_uuid.as_fields();
    let mut pta_uuid_bytes = [0u8; 16];
    pta_uuid_bytes[0..4].copy_from_slice(&time_low.to_ne_bytes());
    pta_uuid_bytes[4..6].copy_from_slice(&time_mid.to_ne_bytes());
    pta_uuid_bytes[6..8].copy_from_slice(&time_hi_ver.to_ne_bytes());
    pta_uuid_bytes[8..16].copy_from_slice(clock_seq);
    // Canonical big-endian octets for the human-readable evidence field.
    let canonical_uuid_bytes: [u8; 16] = *self_uuid.as_bytes();

    let pta_uuid = Uuid::parse_str(PTA_ATTESTATION_UUID)
        .map_err(|e| anyhow!("invalid attestation PTA UUID: {:?}", e))?;
    let mut session = TaSessionBuilder::new(pta_uuid)
        .build()
        .map_err(|e| anyhow!("open attestation PTA session failed: {:?} (is CFG_ATTESTATION_PTA enabled?)", e))?;

    let (ta_measurement, signature) =
        get_ta_shdr_digest(&mut session, &pta_uuid_bytes, &input.nonce)?;
    let (attest_pubkey_exp, attest_pubkey_mod, sig_alg) = get_pubkey(&mut session)?;

    let mut t = Time::new();
    t.ree_time();
    let ree_time_secs = t.seconds as u64;

    Ok(proto::GetAttestationOutput {
        nonce: input.nonce.clone(),
        ta_uuid: canonical_uuid_bytes.to_vec(),
        ta_measurement,
        signature,
        attest_pubkey_exp,
        attest_pubkey_mod,
        sig_alg,
        ree_time_secs,
    })
}

/// Invoke `GET_TA_SHDR_DIGEST`. Returns `(ta_measurement[32], signature)`.
fn get_ta_shdr_digest(
    session: &mut TaSession,
    uuid_bytes: &[u8; 16],
    nonce: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut out_buf = [0u8; SHDR_OUT_BUF];
    let mut params = TeeParams::new()
        .with_memref_in(ParamIndex::Arg0, uuid_bytes)
        .with_memref_in(ParamIndex::Arg1, nonce)
        .with_memref_out(ParamIndex::Arg2, &mut out_buf);

    session
        .invoke_command(PTA_ATTESTATION_GET_TA_SHDR_DIGEST, &mut params)
        .map_err(|e| anyhow!("GET_TA_SHDR_DIGEST failed: {:?}", e))?;

    let written = params[ParamIndex::Arg2]
        .written_slice()
        .ok_or_else(|| anyhow!("GET_TA_SHDR_DIGEST returned no output"))?;
    if written.len() <= TA_MEASUREMENT_LEN {
        bail!(
            "GET_TA_SHDR_DIGEST output too short: {} bytes (need digest + signature)",
            written.len()
        );
    }
    let ta_measurement = written[..TA_MEASUREMENT_LEN].to_vec();
    let signature = written[TA_MEASUREMENT_LEN..].to_vec();
    Ok((ta_measurement, signature))
}

/// Invoke `GET_PUBKEY`. Returns `(exponent_be, modulus_be, sig_alg)`.
fn get_pubkey(session: &mut TaSession) -> Result<(Vec<u8>, Vec<u8>, u32)> {
    let mut exp_buf = [0u8; PUBKEY_EXP_BUF];
    let mut mod_buf = [0u8; PUBKEY_MOD_BUF];
    let mut params = TeeParams::new()
        .with_memref_out(ParamIndex::Arg0, &mut exp_buf)
        .with_memref_out(ParamIndex::Arg1, &mut mod_buf)
        .with_value_out(ParamIndex::Arg2, 0, 0);

    session
        .invoke_command(PTA_ATTESTATION_GET_PUBKEY, &mut params)
        .map_err(|e| anyhow!("GET_PUBKEY failed: {:?}", e))?;

    let exp = params[ParamIndex::Arg0]
        .written_slice()
        .ok_or_else(|| anyhow!("GET_PUBKEY returned no exponent"))?
        .to_vec();
    let modulus = params[ParamIndex::Arg1]
        .written_slice()
        .ok_or_else(|| anyhow!("GET_PUBKEY returned no modulus"))?
        .to_vec();
    let sig_alg = params[ParamIndex::Arg2]
        .output_value()
        .ok_or_else(|| anyhow!("GET_PUBKEY returned no algorithm value"))?
        .0;

    if exp.is_empty() || modulus.is_empty() {
        bail!("GET_PUBKEY returned empty key material");
    }
    Ok((exp, modulus, sig_alg))
}
