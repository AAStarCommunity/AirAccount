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

use num_enum::{FromPrimitive, IntoPrimitive};

mod in_out;
pub use in_out::*;

#[derive(FromPrimitive, IntoPrimitive, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    CreateWallet,
    RemoveWallet,
    DeriveAddress,
    SignTransaction,
    SignMessage,
    SignHash,
    DeriveAddressAuto,
    ExportPrivateKey,
    VerifyPasskey,
    WarmupCache,
    RegisterPasskeyTa,
    CreateAgentKey = 11,
    SignAgentUserOp = 12,
    JwtHmacVerify = 14,
    JwtRotateSecret = 15,
    SignTypedData = 17,
    CreateP256SessionKey = 18,
    SignP256UserOp = 19,
    DeleteP256SessionKey = 20,
    SignGrantSession = 21,
    SignP256GrantSession = 22,
    #[default]
    Unknown,
}

// If Uuid::parse_str() returns an InvalidLength error, there may be an extra
// newline in your uuid.txt file. You can remove it by running
// `truncate -s 36 uuid.txt`.
pub const UUID: &str = include_str!("../../uuid.txt");

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_uuid() -> Uuid {
        Uuid::parse_str("4319f351-0b24-4097-b659-80ee4f824cdd").unwrap()
    }

    fn test_uuid2() -> Uuid {
        Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap()
    }

    // ── Command enum ──

    #[test]
    fn command_to_u32() {
        assert_eq!(u32::from(Command::CreateWallet), 0);
        assert_eq!(u32::from(Command::RemoveWallet), 1);
        assert_eq!(u32::from(Command::DeriveAddress), 2);
        assert_eq!(u32::from(Command::SignTransaction), 3);
        assert_eq!(u32::from(Command::SignMessage), 4);
        assert_eq!(u32::from(Command::SignHash), 5);
        assert_eq!(u32::from(Command::DeriveAddressAuto), 6);
        assert_eq!(u32::from(Command::ExportPrivateKey), 7);
        assert_eq!(u32::from(Command::VerifyPasskey), 8);
        assert_eq!(u32::from(Command::WarmupCache), 9);
        assert_eq!(u32::from(Command::RegisterPasskeyTa), 10);
        assert_eq!(u32::from(Command::CreateAgentKey), 11);
        assert_eq!(u32::from(Command::SignAgentUserOp), 12);
        assert_eq!(u32::from(Command::JwtHmacVerify), 14);
        assert_eq!(u32::from(Command::JwtRotateSecret), 15);
        assert_eq!(u32::from(Command::SignTypedData), 17);
        assert_eq!(u32::from(Command::CreateP256SessionKey), 18);
        assert_eq!(u32::from(Command::SignP256UserOp), 19);
        assert_eq!(u32::from(Command::DeleteP256SessionKey), 20);
        assert_eq!(u32::from(Command::SignGrantSession), 21);
        assert_eq!(u32::from(Command::SignP256GrantSession), 22);
    }

    #[test]
    fn u32_to_command() {
        assert!(matches!(Command::from(0u32), Command::CreateWallet));
        assert!(matches!(Command::from(5u32), Command::SignHash));
        assert!(matches!(Command::from(10u32), Command::RegisterPasskeyTa));
        assert!(matches!(Command::from(17u32), Command::SignTypedData));
        assert!(matches!(
            Command::from(18u32),
            Command::CreateP256SessionKey
        ));
        assert!(matches!(Command::from(19u32), Command::SignP256UserOp));
        assert!(matches!(
            Command::from(20u32),
            Command::DeleteP256SessionKey
        ));
        assert!(matches!(Command::from(21u32), Command::SignGrantSession));
        assert!(matches!(
            Command::from(22u32),
            Command::SignP256GrantSession
        ));
    }

    #[test]
    fn unknown_u32_maps_to_unknown() {
        assert!(matches!(Command::from(99u32), Command::Unknown));
        assert!(matches!(Command::from(u32::MAX), Command::Unknown));
    }

    #[test]
    fn command_roundtrip() {
        // 13 (JwtHmacSign) and 16 (JwtSignPayload) removed — JWT signing oracle closed (Issue #16)
        let valid_ids: &[u32] = &[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 17, 18, 19, 20, 21, 22,
        ];
        for &i in valid_ids {
            let cmd = Command::from(i);
            assert_eq!(u32::from(cmd), i);
        }
        // Removed command IDs must map to Unknown (prevent silent ID reuse regression)
        assert_eq!(Command::from(13), Command::Unknown);
        assert_eq!(Command::from(16), Command::Unknown);
    }

    // ── UUID constant ──

    #[test]
    fn uuid_constant_valid() {
        let trimmed = UUID.trim();
        assert_eq!(trimmed.len(), 36);
        Uuid::parse_str(trimmed).expect("UUID constant must be valid");
    }

    // ── bincode roundtrip helpers ──

    fn bincode_roundtrip<
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
    >(
        val: &T,
    ) {
        let bytes = bincode::serialize(val).expect("serialize");
        let decoded: T = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(val, &decoded);
    }

    // ── CreateWallet ──

    #[test]
    fn create_wallet_input_roundtrip() {
        bincode_roundtrip(&CreateWalletInput {
            passkey_pubkey: vec![0x04; 65],
        });
    }

    #[test]
    fn create_wallet_output_roundtrip() {
        let out = CreateWalletOutput {
            wallet_id: test_uuid(),
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".into(),
        };
        bincode_roundtrip(&out);
    }

    // ── RemoveWallet ──

    #[test]
    fn remove_wallet_roundtrip() {
        bincode_roundtrip(&RemoveWalletInput {
            wallet_id: test_uuid(),
            passkey_assertion: None,
        });
        bincode_roundtrip(&RemoveWalletOutput {});
    }

    // ── DeriveAddress ──

    #[test]
    fn derive_address_input_roundtrip() {
        bincode_roundtrip(&DeriveAddressInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            passkey_assertion: None,
        });
    }

    #[test]
    fn derive_address_output_roundtrip() {
        bincode_roundtrip(&DeriveAddressOutput {
            address: [0xab; 20],
            public_key: vec![0x04; 65],
        });
    }

    // ── EthTransaction ──

    #[test]
    fn eth_transaction_basic_roundtrip() {
        let tx = EthTransaction {
            chain_id: 1,
            nonce: 42,
            to: Some([0x11; 20]),
            value: 1_000_000_000_000_000_000, // 1 ETH
            gas_price: 20_000_000_000,
            gas: 21_000,
            data: vec![],
        };
        bincode_roundtrip(&tx);
    }

    #[test]
    fn eth_transaction_none_to() {
        let tx = EthTransaction {
            chain_id: 5,
            nonce: 0,
            to: None, // contract creation
            value: 0,
            gas_price: 1,
            gas: 100_000,
            data: vec![0x60, 0x80, 0x60, 0x40],
        };
        bincode_roundtrip(&tx);
    }

    #[test]
    fn eth_transaction_u128_max() {
        let tx = EthTransaction {
            chain_id: u64::MAX,
            nonce: u128::MAX,
            to: Some([0xff; 20]),
            value: u128::MAX,
            gas_price: u128::MAX,
            gas: u128::MAX,
            data: vec![0xff; 1024],
        };
        bincode_roundtrip(&tx);
    }

    // ── SignTransaction ──

    #[test]
    fn sign_transaction_roundtrip() {
        let input = SignTransactionInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            transaction: EthTransaction {
                chain_id: 1,
                nonce: 0,
                to: Some([0x22; 20]),
                value: 100,
                gas_price: 1,
                gas: 21_000,
                data: vec![],
            },
            passkey_assertion: None,
        };
        bincode_roundtrip(&input);
        bincode_roundtrip(&SignTransactionOutput {
            signature: vec![0u8; 65],
        });
    }

    // ── SignMessage ──

    #[test]
    fn sign_message_roundtrip() {
        bincode_roundtrip(&SignMessageInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            message: b"hello world".to_vec(),
            passkey_assertion: None,
        });
        bincode_roundtrip(&SignMessageOutput {
            signature: vec![0u8; 65],
        });
    }

    // ── SignHash ──

    #[test]
    fn sign_hash_roundtrip() {
        bincode_roundtrip(&SignHashInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            hash: [0xaa; 32],
            passkey_assertion: None,
        });
        bincode_roundtrip(&SignHashOutput {
            signature: vec![0u8; 65],
        });
    }

    // ── DeriveAddressAuto ──

    #[test]
    fn derive_address_auto_roundtrip() {
        bincode_roundtrip(&DeriveAddressAutoInput {
            wallet_id: test_uuid(),
        });

        bincode_roundtrip(&DeriveAddressAutoOutput {
            wallet_id: test_uuid(),
            address: [0x33; 20],
            public_key: vec![0x04; 65],
            derivation_path: "m/44'/60'/0'/0/0".into(),
        });
    }

    // ── ExportPrivateKey ──

    #[test]
    fn export_private_key_roundtrip() {
        bincode_roundtrip(&ExportPrivateKeyInput {
            wallet_id: test_uuid(),
            derivation_path: "m/44'/60'/0'/0/0".into(),
            passkey_assertion: None,
        });
        bincode_roundtrip(&ExportPrivateKeyOutput {
            private_key: vec![0u8; 32],
        });
    }

    // ── VerifyPasskey ──

    #[test]
    fn verify_passkey_roundtrip() {
        bincode_roundtrip(&VerifyPasskeyInput {
            wallet_id: test_uuid(),
            public_key: vec![0x04; 65],
            authenticator_data: vec![0u8; 37],
            client_data_hash: [0xbb; 32],
            signature_r: [0x11; 32],
            signature_s: [0x22; 32],
        });
        bincode_roundtrip(&VerifyPasskeyOutput { valid: true });
        bincode_roundtrip(&VerifyPasskeyOutput { valid: false });
    }

    // ── WarmupCache ──

    #[test]
    fn warmup_cache_roundtrip() {
        bincode_roundtrip(&WarmupCacheInput {
            wallet_id: test_uuid(),
        });
        bincode_roundtrip(&WarmupCacheOutput {
            cached: true,
            cache_size: 200,
        });
        bincode_roundtrip(&WarmupCacheOutput {
            cached: false,
            cache_size: 0,
        });
    }

    #[test]
    fn create_agent_key_roundtrip() {
        bincode_roundtrip(&CreateAgentKeyInput {
            wallet_id: test_uuid(),
            agent_index: 0,
            subject: "4319f351-0b24-4097-b659-80ee4f824cdd".to_string(),
            ttl_secs: 259200i64,
            passkey_assertion: None,
        });
        bincode_roundtrip(&CreateAgentKeyInput {
            wallet_id: test_uuid(),
            agent_index: 1,
            subject: "test-agent".to_string(),
            ttl_secs: 86400i64,
            passkey_assertion: Some(PasskeyAssertion {
                authenticator_data: vec![0xad; 37],
                client_data_hash: [0xcd; 32],
                signature_r: [0x11; 32],
                signature_s: [0x22; 32],
                rp_id_hash: None,
            }),
        });
        bincode_roundtrip(&CreateAgentKeyOutput {
            agent_address: [0xab; 20],
            public_key_compressed: vec![0x02; 33],
            jwt_kid: "v1234".to_string(),
            jwt_header_b64: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InYxMjM0In0".to_string(),
            jwt_payload_b64: "eyJzdWIiOiJ0ZXN0In0".to_string(),
            jwt_hmac: [0xbb; 32],
        });
    }

    #[test]
    fn sign_agent_user_op_roundtrip() {
        bincode_roundtrip(&SignAgentUserOpInput {
            wallet_id: test_uuid(),
            agent_index: 3,
            user_op_hash: [0xcc; 32],
            jwt_kid: "v1234".to_string(),
            jwt_signing_input: b"header.payload".to_vec(),
            jwt_hmac: vec![0xaa; 32],
            account_address: [0xab; 20],
        });
        bincode_roundtrip(&SignAgentUserOpOutput {
            signature: vec![0u8; 106], // v0.17.2: [0x08][account(20)][key(20)][ECDSA(65)]
        });
    }

    #[test]
    fn jwt_hmac_roundtrip() {
        bincode_roundtrip(&JwtHmacVerifyInput {
            kid: "v1".to_string(),
            message: b"header.payload".to_vec(),
            expected_hmac: vec![0xaa; 32],
        });
        bincode_roundtrip(&JwtHmacVerifyOutput { valid: true });
        bincode_roundtrip(&JwtRotateSecretInput { force: false });
        bincode_roundtrip(&JwtRotateSecretOutput {
            new_kid: "v2".to_string(),
            retired_kid: Some("v1".to_string()),
        });
        bincode_roundtrip(&JwtRotateSecretOutput {
            new_kid: "v1".to_string(),
            retired_kid: None,
        });
    }

    // ── JSON compatibility ──

    #[test]
    fn json_roundtrip_create_wallet_output() {
        let out = CreateWalletOutput {
            wallet_id: Uuid::parse_str("4319f351-0b24-4097-b659-80ee4f824cdd").unwrap(),
            mnemonic: "test mnemonic".into(),
        };
        let json = serde_json::to_string(&out).unwrap();
        let decoded: CreateWalletOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(out.wallet_id, decoded.wallet_id);
        assert_eq!(out.mnemonic, decoded.mnemonic);
    }

    #[test]
    fn json_eth_transaction() {
        let tx = EthTransaction {
            chain_id: 1,
            nonce: 0,
            to: Some([0xde; 20]),
            value: 0,
            gas_price: 20_000_000_000,
            gas: 21_000,
            data: vec![],
        };
        let json = serde_json::to_string(&tx).unwrap();
        assert!(json.contains("\"chain_id\":1"));
        let decoded: EthTransaction = serde_json::from_str(&json).unwrap();
        assert_eq!(tx.chain_id, decoded.chain_id);
        assert_eq!(tx.to, decoded.to);
    }

    // ── PartialEq requirement for bincode_roundtrip ──
    // (not derived on original structs, so we test field-by-field for JSON)

    #[test]
    fn sign_hash_input_fields_preserved() {
        let id = test_uuid();
        let hash = [0x42; 32];
        let input = SignHashInput {
            wallet_id: id,
            hd_path: "m/44'/60'/0'/0/1".into(),
            hash,
            passkey_assertion: None,
        };
        let bytes = bincode::serialize(&input).unwrap();
        let decoded: SignHashInput = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.wallet_id, id);
        assert_eq!(decoded.hd_path, "m/44'/60'/0'/0/1");
        assert_eq!(decoded.hash, hash);
    }

    // ── PasskeyAssertion ──

    #[test]
    fn passkey_assertion_roundtrip() {
        // Without rp_id_hash (backwards-compatible None default)
        bincode_roundtrip(&PasskeyAssertion {
            authenticator_data: vec![0u8; 37],
            client_data_hash: [0xbb; 32],
            signature_r: [0x11; 32],
            signature_s: [0x22; 32],
            rp_id_hash: None,
        });
        // With rp_id_hash present (TA will verify authenticatorData[9..41] == this)
        bincode_roundtrip(&PasskeyAssertion {
            authenticator_data: vec![0u8; 37],
            client_data_hash: [0xbb; 32],
            signature_r: [0x11; 32],
            signature_s: [0x22; 32],
            rp_id_hash: Some([0xcc; 32]),
        });
    }

    // ── RegisterPasskeyTa ──

    #[test]
    fn register_passkey_ta_roundtrip() {
        bincode_roundtrip(&RegisterPasskeyTaInput {
            wallet_id: test_uuid(),
            passkey_pubkey: vec![0x04; 65],
            passkey_assertion: None,
        });
        bincode_roundtrip(&RegisterPasskeyTaOutput { registered: true });
    }

    // ── Sign with passkey assertion ──

    #[test]
    fn sign_hash_with_passkey_roundtrip() {
        let assertion = PasskeyAssertion {
            authenticator_data: vec![0u8; 37],
            client_data_hash: [0xcc; 32],
            signature_r: [0xaa; 32],
            signature_s: [0xbb; 32],
            rp_id_hash: None,
        };
        bincode_roundtrip(&SignHashInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            hash: [0xff; 32],
            passkey_assertion: Some(assertion),
        });
    }

    // ── EIP-712 SignTypedData ──

    #[test]
    fn sign_typed_data_roundtrip() {
        let input = SignTypedDataInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            domain: Eip712Domain {
                name: Some("MyDApp".into()),
                version: Some("1".into()),
                chain_id: Some(1),
                verifying_contract: Some([0xde; 20]),
            },
            primary_type: "Transfer".into(),
            types: vec![Eip712TypeDef {
                name: "Transfer".into(),
                fields: vec![
                    Eip712TypeField {
                        name: "to".into(),
                        field_type: "address".into(),
                    },
                    Eip712TypeField {
                        name: "amount".into(),
                        field_type: "uint256".into(),
                    },
                    Eip712TypeField {
                        name: "memo".into(),
                        field_type: "string".into(),
                    },
                ],
            }],
            message: vec![
                Eip712FieldValue {
                    name: "to".into(),
                    value: Eip712Value::Address([0xab; 20]),
                },
                Eip712FieldValue {
                    name: "amount".into(),
                    value: Eip712Value::Uint(vec![0x00, 0x0f, 0x42, 0x40]), // 1000000
                },
                Eip712FieldValue {
                    name: "memo".into(),
                    value: Eip712Value::Str("hello".into()),
                },
            ],
            passkey_assertion: None,
            jwt_kid: None,
            jwt_signing_input: None,
            jwt_hmac: None,
        };
        bincode_roundtrip(&input);

        // JWT-path variant: jwt_kid/signing_input/hmac present, passkey absent
        let input_jwt = SignTypedDataInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/1/0".into(),
            domain: Eip712Domain {
                name: None,
                version: None,
                chain_id: Some(1),
                verifying_contract: None,
            },
            primary_type: "Transfer".into(),
            types: vec![],
            message: vec![],
            passkey_assertion: None,
            jwt_kid: Some("kid-abc".into()),
            jwt_signing_input: Some(b"header.payload".to_vec()),
            jwt_hmac: Some(vec![0xde; 32]),
        };
        bincode_roundtrip(&input_jwt);

        bincode_roundtrip(&SignTypedDataOutput {
            signature: vec![0u8; 65],
        });
    }

    // ── Grant Session ──

    #[test]
    fn sign_grant_session_roundtrip() {
        bincode_roundtrip(&SignGrantSessionInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            chain_id: 1,
            verifying_contract: [0x11; 20],
            account: [0x22; 20],
            session_key: [0x33; 20],
            expiry: 1_000_000,
            contract_scope: [0x00; 20],
            selector_scope: [0x00; 4],
            velocity_limit: 0,
            velocity_window: 0,
            call_targets: vec![[0x44; 20]],
            selector_allowlist: vec![[0xaa, 0xbb, 0xcc, 0xdd]],
            nonce: [0x00; 32],
            passkey_assertion: None,
        });
        bincode_roundtrip(&SignGrantSessionOutput {
            signature: vec![0u8; 65],
        });
    }

    #[test]
    fn sign_p256_grant_session_roundtrip() {
        bincode_roundtrip(&SignP256GrantSessionInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            chain_id: 11155111,
            verifying_contract: [0x11; 20],
            account: [0x22; 20],
            key_x: [0xaa; 32],
            key_y: [0xbb; 32],
            expiry: 2_000_000,
            contract_scope: [0x00; 20],
            selector_scope: [0x00; 4],
            velocity_limit: 5,
            velocity_window: 3600,
            call_targets: vec![],
            selector_allowlist: vec![],
            nonce: [0x00; 32],
            passkey_assertion: None,
        });
        bincode_roundtrip(&SignP256GrantSessionOutput {
            signature: vec![0u8; 65],
        });
    }

    #[test]
    fn eip712_domain_minimal_roundtrip() {
        bincode_roundtrip(&Eip712Domain {
            name: None,
            version: None,
            chain_id: Some(137),
            verifying_contract: None,
        });
    }

    #[test]
    fn eip712_value_variants_roundtrip() {
        bincode_roundtrip(&Eip712FieldValue {
            name: "flag".into(),
            value: Eip712Value::Bool(true),
        });
        bincode_roundtrip(&Eip712FieldValue {
            name: "data".into(),
            value: Eip712Value::Bytes(vec![0xca, 0xfe, 0xba, 0xbe]),
        });
        bincode_roundtrip(&Eip712FieldValue {
            name: "hash".into(),
            value: Eip712Value::Bytes32([0x11; 32]),
        });
    }

    // ── P256 Session Key ──

    #[test]
    fn create_p256_session_key_roundtrip() {
        bincode_roundtrip(&CreateP256SessionKeyInput {
            wallet_id: test_uuid(),
            session_index: 0,
            subject: "test-wallet-id".to_string(),
            ttl_secs: 259200,
        });
        bincode_roundtrip(&CreateP256SessionKeyOutput {
            pub_key_x: [0xaa; 32],
            pub_key_y: [0xbb; 32],
            jwt_kid: "v1".to_string(),
            jwt_header_b64: "eyJhbGciOiJIUzI1NiJ9".to_string(),
            jwt_payload_b64: "eyJzdWIiOiJ0ZXN0In0".to_string(),
            jwt_hmac: [0xcc; 32],
        });
    }

    #[test]
    fn sign_p256_user_op_roundtrip() {
        bincode_roundtrip(&SignP256UserOpInput {
            wallet_id: test_uuid(),
            session_index: 2,
            user_op_hash: [0xcc; 32],
            jwt_kid: "v1234".to_string(),
            jwt_signing_input: b"header.payload".to_vec(),
            jwt_hmac: vec![0xaa; 32],
            account_address: [0xab; 20],
        });
        bincode_roundtrip(&SignP256UserOpOutput {
            signature: vec![0u8; 149],
        });
    }

    #[test]
    fn delete_p256_session_key_roundtrip() {
        bincode_roundtrip(&DeleteP256SessionKeyInput {
            wallet_id: test_uuid(),
            session_index: 1,
        });
        bincode_roundtrip(&DeleteP256SessionKeyOutput { deleted: true });
        bincode_roundtrip(&DeleteP256SessionKeyOutput { deleted: false });
    }
}
