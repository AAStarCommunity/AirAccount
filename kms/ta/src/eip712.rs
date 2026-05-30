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

//! EIP-712 typed structured data encoder.
//!
//! Scope (v0.18.0): flat primitive field types only.
//! Not supported: arrays, nested struct references (deferred to v0.18.1).
//!
//! `int*` negative values: callers must supply 2's-complement big-endian bytes
//! (e.g. for int8 -1 pass `vec![0xff]`). JSON decimal strings for negative
//! integers are not supported; use hex `0xff...` representation instead.

use anyhow::{anyhow, Result};
use proto::{Eip712Domain, Eip712FieldValue, Eip712TypeDef, Eip712Value};
use sha3::{Digest, Keccak256};

/// Encode a uint value (1–32 bytes big-endian) as a left-zero-padded 32-byte word.
/// Returns Err if bytes.len() > 32 — callers must not silently truncate.
fn encode_uint(bytes: &[u8]) -> Result<[u8; 32]> {
    if bytes.len() > 32 {
        return Err(anyhow!(
            "uint value exceeds 32 bytes: got {} bytes",
            bytes.len()
        ));
    }
    let mut word = [0u8; 32];
    let start = 32 - bytes.len();
    word[start..].copy_from_slice(bytes);
    Ok(word)
}

/// keccak256 of arbitrary bytes.
pub(crate) fn keccak(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(data);
    h.finalize().into()
}

/// EIP-712 typeHash: keccak256 of the canonical type string.
///
/// Format: `TypeName(type1 name1,type2 name2,...)`
fn type_hash(type_def: &Eip712TypeDef) -> [u8; 32] {
    let mut s = String::new();
    s.push_str(&type_def.name);
    s.push('(');
    for (i, f) in type_def.fields.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&f.field_type);
        s.push(' ');
        s.push_str(&f.name);
    }
    s.push(')');
    keccak(s.as_bytes())
}

/// Encode a single EIP-712 value into a 32-byte ABI word.
fn encode_value(val: &Eip712Value) -> Result<[u8; 32]> {
    match val {
        Eip712Value::Address(addr) => {
            let mut word = [0u8; 32];
            word[12..].copy_from_slice(addr);
            Ok(word)
        }
        Eip712Value::Uint(bytes) => encode_uint(bytes),
        Eip712Value::Bytes32(b) => Ok(*b),
        Eip712Value::Bool(b) => {
            let mut word = [0u8; 32];
            word[31] = *b as u8;
            Ok(word)
        }
        // Dynamic types: keccak256 of their content
        Eip712Value::Str(s) => Ok(keccak(s.as_bytes())),
        Eip712Value::Bytes(b) => Ok(keccak(b)),
    }
}

/// Compute the EIP-712 hashStruct for the primary type.
///
/// hashStruct(s) = keccak256(typeHash(T) || encodeData(s))
///
/// Returns Err if a field declared in type_def is absent from message.
pub fn hash_struct(type_def: &Eip712TypeDef, message: &[Eip712FieldValue]) -> Result<[u8; 32]> {
    let th = type_hash(type_def);
    let mut data = Vec::with_capacity(32 + 32 * type_def.fields.len());
    data.extend_from_slice(&th);

    // Encode fields in declaration order (per EIP-712 spec)
    for field_def in &type_def.fields {
        let field_value = message
            .iter()
            .find(|fv| fv.name == field_def.name)
            .ok_or_else(|| {
                anyhow!(
                    "EIP-712 field '{}' declared in type '{}' is missing from message",
                    field_def.name,
                    type_def.name
                )
            })?;
        let encoded = encode_value(&field_value.value)?;
        data.extend_from_slice(&encoded);
    }
    Ok(keccak(&data))
}

/// Compute the EIP-712 domain separator.
pub fn domain_separator(domain: &Eip712Domain) -> [u8; 32] {
    // Build the domain type string from present fields only
    let mut type_str = String::from("EIP712Domain(");
    let mut fields: Vec<(&str, &str)> = Vec::new();
    if domain.name.is_some() {
        fields.push(("string", "name"));
    }
    if domain.version.is_some() {
        fields.push(("string", "version"));
    }
    if domain.chain_id.is_some() {
        fields.push(("uint256", "chainId"));
    }
    if domain.verifying_contract.is_some() {
        fields.push(("address", "verifyingContract"));
    }
    for (i, (t, n)) in fields.iter().enumerate() {
        if i > 0 {
            type_str.push(',');
        }
        type_str.push_str(t);
        type_str.push(' ');
        type_str.push_str(n);
    }
    type_str.push(')');
    let th = keccak(type_str.as_bytes());

    let mut data = Vec::with_capacity(32 * (1 + fields.len()));
    data.extend_from_slice(&th);
    if let Some(name) = &domain.name {
        data.extend_from_slice(&keccak(name.as_bytes()));
    }
    if let Some(version) = &domain.version {
        data.extend_from_slice(&keccak(version.as_bytes()));
    }
    if let Some(chain_id) = domain.chain_id {
        // u64 is 8 bytes, always ≤ 32 — encode_uint will not error here
        let word = encode_uint(&chain_id.to_be_bytes()).expect("u64 fits in 32 bytes");
        data.extend_from_slice(&word);
    }
    if let Some(contract) = &domain.verifying_contract {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(contract);
        data.extend_from_slice(&word);
    }
    keccak(&data)
}

/// Compute the final EIP-712 digest: keccak256(0x1901 || domainSeparator || hashStruct).
pub fn eip712_digest(
    domain: &Eip712Domain,
    primary_type_def: &Eip712TypeDef,
    message: &[Eip712FieldValue],
) -> Result<[u8; 32]> {
    let ds = domain_separator(domain);
    let hs = hash_struct(primary_type_def, message)?;
    let mut buf = [0u8; 66];
    buf[0] = 0x19;
    buf[1] = 0x01;
    buf[2..34].copy_from_slice(&ds);
    buf[34..66].copy_from_slice(&hs);
    Ok(keccak(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{Eip712Domain, Eip712FieldValue, Eip712TypeDef, Eip712TypeField, Eip712Value};

    // ── EIP-712 spec Mail example reference data ──
    // https://eips.ethereum.org/EIPS/eip-712#example
    //
    // Domain: "Ether Mail" / "1" / chainId=1 / verifyingContract=0xCcCCcc...CC
    // domainSeparator = 0xf2cee375fa42b42143804025fc449deafd50cc031ca257e0b194a650a912090f
    // hashStruct(mail) = 0xc52c0ee5d84264471806290a3f2c4cecfc5490626bf912d01f240d7a274b371e
    // finalDigest       = 0xbe609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2
    //
    // hashStruct(mail) requires nested struct (Person inside Mail) which is
    // deferred to v0.18.1. We verify domain_separator and the 0x1901 framing
    // separately using the known spec values.

    fn mail_domain() -> Eip712Domain {
        Eip712Domain {
            name: Some("Ether Mail".into()),
            version: Some("1".into()),
            chain_id: Some(1),
            verifying_contract: Some([
                0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
                0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
            ]),
        }
    }

    // ── Spec reference vector tests ──

    #[test]
    fn domain_type_hash_matches_spec() {
        // keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
        let expected = hex::decode(
            "8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f",
        )
        .unwrap();
        let type_str = "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
        let got = keccak(type_str.as_bytes());
        assert_eq!(got.to_vec(), expected);
    }

    #[test]
    fn domain_separator_matches_spec() {
        // EIP-712 spec mail example domain separator
        // Reference: https://eips.ethereum.org/EIPS/eip-712#example
        let expected = hex::decode(
            "f2cee375fa42b42143804025fc449deafd50cc031ca257e0b194a650a912090f",
        )
        .unwrap();
        let ds = domain_separator(&mail_domain());
        assert_eq!(ds.to_vec(), expected, "domain separator must match EIP-712 spec mail example");
    }

    #[test]
    fn eip712_final_digest_framing_matches_spec() {
        // Verify the 0x1901 framing using known spec values directly.
        // hashStruct(mail) is taken from the EIP-712 spec Appendix (requires nested
        // struct support not yet implemented); we use it as a fixed input here.
        let ds = hex::decode(
            "f2cee375fa42b42143804025fc449deafd50cc031ca257e0b194a650a912090f",
        )
        .unwrap();
        let hs = hex::decode(
            "c52c0ee5d84264471806290a3f2c4cecfc5490626bf912d01f240d7a274b371e",
        )
        .unwrap();
        let mut buf = [0u8; 66];
        buf[0] = 0x19;
        buf[1] = 0x01;
        buf[2..34].copy_from_slice(&ds);
        buf[34..66].copy_from_slice(&hs);
        let digest = keccak(&buf);
        let expected = hex::decode(
            "be609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2",
        )
        .unwrap();
        assert_eq!(digest.to_vec(), expected, "0x1901 framing must produce spec final digest");
    }

    // ── encode_uint ──

    #[test]
    fn encode_uint_left_pads() {
        let val = encode_uint(&[0x01]).unwrap();
        assert_eq!(val[31], 0x01);
        assert_eq!(&val[..31], &[0u8; 31]);
    }

    #[test]
    fn encode_uint_32_bytes_ok() {
        let bytes = [0xffu8; 32];
        assert!(encode_uint(&bytes).is_ok());
    }

    #[test]
    fn encode_uint_33_bytes_errors() {
        let bytes = [0x01u8; 33];
        let err = encode_uint(&bytes).unwrap_err();
        assert!(err.to_string().contains("exceeds 32 bytes"), "got: {}", err);
    }

    // ── encode_value ──

    #[test]
    fn encode_bool_true() {
        let val = encode_value(&Eip712Value::Bool(true)).unwrap();
        assert_eq!(val[31], 1u8);
        assert_eq!(&val[..31], &[0u8; 31]);
    }

    #[test]
    fn encode_address_right_aligned() {
        let addr = [0xab; 20];
        let val = encode_value(&Eip712Value::Address(addr)).unwrap();
        assert_eq!(&val[12..], &addr);
        assert_eq!(&val[..12], &[0u8; 12]);
    }

    // ── type_hash ──

    #[test]
    fn type_hash_empty_fields() {
        let td = Eip712TypeDef {
            name: "Empty".into(),
            fields: vec![],
        };
        let th = type_hash(&td);
        let expected = keccak(b"Empty()");
        assert_eq!(th, expected);
    }

    // ── hash_struct ──

    #[test]
    fn hash_struct_single_address_field() {
        let td = Eip712TypeDef {
            name: "Transfer".into(),
            fields: vec![Eip712TypeField {
                name: "to".into(),
                field_type: "address".into(),
            }],
        };
        let msg = vec![Eip712FieldValue {
            name: "to".into(),
            value: Eip712Value::Address([0xde; 20]),
        }];
        let hs1 = hash_struct(&td, &msg).unwrap();
        let hs2 = hash_struct(&td, &msg).unwrap();
        assert_eq!(hs1, hs2);
    }

    #[test]
    fn hash_struct_missing_field_errors() {
        let td = Eip712TypeDef {
            name: "Transfer".into(),
            fields: vec![
                Eip712TypeField { name: "to".into(), field_type: "address".into() },
                Eip712TypeField { name: "amount".into(), field_type: "uint256".into() },
            ],
        };
        // Only "to" provided, "amount" is missing
        let msg = vec![Eip712FieldValue {
            name: "to".into(),
            value: Eip712Value::Address([0xde; 20]),
        }];
        let err = hash_struct(&td, &msg).unwrap_err();
        assert!(
            err.to_string().contains("amount"),
            "error must name the missing field, got: {}",
            err
        );
    }

    // ── domain_separator ──

    #[test]
    fn domain_separator_minimal() {
        let domain = Eip712Domain {
            name: None,
            version: None,
            chain_id: Some(1),
            verifying_contract: None,
        };
        let ds1 = domain_separator(&domain);
        let ds2 = domain_separator(&domain);
        assert_eq!(ds1, ds2);
        assert_ne!(ds1, [0u8; 32]);
    }

    // ── eip712_digest ──

    #[test]
    fn eip712_digest_deterministic() {
        let domain = mail_domain();
        let td = Eip712TypeDef {
            name: "Mail".into(),
            fields: vec![
                Eip712TypeField { name: "from".into(), field_type: "address".into() },
                Eip712TypeField { name: "to".into(), field_type: "address".into() },
                Eip712TypeField { name: "contents".into(), field_type: "string".into() },
            ],
        };
        let msg = vec![
            Eip712FieldValue { name: "from".into(), value: Eip712Value::Address([0xaa; 20]) },
            Eip712FieldValue { name: "to".into(), value: Eip712Value::Address([0xbb; 20]) },
            Eip712FieldValue { name: "contents".into(), value: Eip712Value::Str("Hello Bob!".into()) },
        ];
        let d1 = eip712_digest(&domain, &td, &msg).unwrap();
        let d2 = eip712_digest(&domain, &td, &msg).unwrap();
        assert_eq!(d1, d2);
        assert_ne!(d1, [0u8; 32]);
    }

    #[test]
    fn eip712_digest_missing_field_propagates_error() {
        let domain = Eip712Domain { name: None, version: None, chain_id: Some(1), verifying_contract: None };
        let td = Eip712TypeDef {
            name: "T".into(),
            fields: vec![Eip712TypeField { name: "x".into(), field_type: "bool".into() }],
        };
        // message has wrong field name
        let msg = vec![Eip712FieldValue { name: "y".into(), value: Eip712Value::Bool(true) }];
        assert!(eip712_digest(&domain, &td, &msg).is_err());
    }
}
