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


#[derive(FromPrimitive, IntoPrimitive, Debug, Copy, Clone)]
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
    #[default]
    Unknown,
}

// If Uuid::parse_str() returns an InvalidLength error, there may be an extra
// newline in your uuid.txt file. You can remove it by running 
// `truncate -s 36 uuid.txt`.
pub const UUID: &str = &include_str!("../../uuid.txt");

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
    }

    #[test]
    fn u32_to_command() {
        assert!(matches!(Command::from(0u32), Command::CreateWallet));
        assert!(matches!(Command::from(5u32), Command::SignHash));
        assert!(matches!(Command::from(9u32), Command::WarmupCache));
    }

    #[test]
    fn unknown_u32_maps_to_unknown() {
        assert!(matches!(Command::from(99u32), Command::Unknown));
        assert!(matches!(Command::from(u32::MAX), Command::Unknown));
    }

    #[test]
    fn command_roundtrip() {
        for i in 0..=9u32 {
            let cmd = Command::from(i);
            assert_eq!(u32::from(cmd), i);
        }
    }

    // ── UUID constant ──

    #[test]
    fn uuid_constant_valid() {
        let trimmed = UUID.trim();
        assert_eq!(trimmed.len(), 36);
        Uuid::parse_str(trimmed).expect("UUID constant must be valid");
    }

    // ── bincode roundtrip helpers ──

    fn bincode_roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq>(val: &T) {
        let bytes = bincode::serialize(val).expect("serialize");
        let decoded: T = bincode::deserialize(&bytes).expect("deserialize");
        assert_eq!(val, &decoded);
    }

    // ── CreateWallet ──

    #[test]
    fn create_wallet_input_roundtrip() {
        bincode_roundtrip(&CreateWalletInput {});
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
        bincode_roundtrip(&RemoveWalletInput { wallet_id: test_uuid() });
        bincode_roundtrip(&RemoveWalletOutput {});
    }

    // ── DeriveAddress ──

    #[test]
    fn derive_address_input_roundtrip() {
        bincode_roundtrip(&DeriveAddressInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
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
        };
        bincode_roundtrip(&input);
        bincode_roundtrip(&SignTransactionOutput { signature: vec![0u8; 65] });
    }

    // ── SignMessage ──

    #[test]
    fn sign_message_roundtrip() {
        bincode_roundtrip(&SignMessageInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            message: b"hello world".to_vec(),
        });
        bincode_roundtrip(&SignMessageOutput { signature: vec![0u8; 65] });
    }

    // ── SignHash ──

    #[test]
    fn sign_hash_roundtrip() {
        bincode_roundtrip(&SignHashInput {
            wallet_id: test_uuid(),
            hd_path: "m/44'/60'/0'/0/0".into(),
            hash: [0xaa; 32],
        });
        bincode_roundtrip(&SignHashOutput { signature: vec![0u8; 65] });
    }

    // ── DeriveAddressAuto ──

    #[test]
    fn derive_address_auto_roundtrip() {
        // with existing wallet
        bincode_roundtrip(&DeriveAddressAutoInput { wallet_id: Some(test_uuid()) });
        // create new wallet
        bincode_roundtrip(&DeriveAddressAutoInput { wallet_id: None });

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
        });
        bincode_roundtrip(&ExportPrivateKeyOutput { private_key: vec![0u8; 32] });
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
        bincode_roundtrip(&WarmupCacheInput { wallet_id: test_uuid() });
        bincode_roundtrip(&WarmupCacheOutput { cached: true, cache_size: 200 });
        bincode_roundtrip(&WarmupCacheOutput { cached: false, cache_size: 0 });
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
        };
        let bytes = bincode::serialize(&input).unwrap();
        let decoded: SignHashInput = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.wallet_id, id);
        assert_eq!(decoded.hd_path, "m/44'/60'/0'/0/1");
        assert_eq!(decoded.hash, hash);
    }
}
