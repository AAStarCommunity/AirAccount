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

#![no_main]

mod challenge;
mod hash;
mod passkey;
mod wallet;

// Register custom getrandom implementation for OP-TEE
// Reference: https://docs.rs/getrandom/0.2.16/getrandom/macro.register_custom_getrandom.html
fn optee_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    use optee_utee::Random;
    Random::generate(buf);
    Ok(())
}
getrandom::register_custom_getrandom!(optee_getrandom);

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};
use proto::Command;
use secure_db::SecureStorageClient;

use anyhow::{anyhow, bail, Result};
use challenge::ChallengeManager;
use std::io::Write;
use std::sync::Mutex;
use wallet::Wallet;

const DB_NAME: &str = "eth_wallet_db";

// Global challenge manager (initialized on first use)
static CHALLENGE_MANAGER: Mutex<Option<ChallengeManager>> = Mutex::new(None);

fn get_challenge_manager() -> &'static Mutex<Option<ChallengeManager>> {
    &CHALLENGE_MANAGER
}

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] TA destroy");
}

#[cfg(debug_assertions)]
macro_rules! dbg_println {
    ($($arg:tt)*) => (trace_println!($($arg)*));
}

#[cfg(not(debug_assertions))]
macro_rules! dbg_println {
    ($($arg:tt)*) => {};
}

fn create_wallet(_input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    let wallet = Wallet::new()?;
    let wallet_id = wallet.get_id();
    let mnemonic = wallet.get_mnemonic()?;
    dbg_println!("[+] Wallet ID: {:?}", wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    db_client.put(&wallet)?;
    dbg_println!("[+] Wallet saved in secure storage");

    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,
    })
}

fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput> {
    dbg_println!("[+] Removing wallet: {:?}", input.wallet_id);

    let db_client = SecureStorageClient::open(DB_NAME)?;
    db_client.delete_entry::<Wallet>(&input.wallet_id)?;
    dbg_println!("[+] Wallet removed");

    Ok(proto::RemoveWalletOutput {})
}

fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput> {
    let db_client = SecureStorageClient::open(DB_NAME)?;
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("[+] Deriving address: error: wallet not found: {:?}", e))?;
    dbg_println!("[+] Deriving address: wallet loaded");

    let (address, public_key) = wallet.derive_address(&input.hd_path)?;
    dbg_println!("[+] Deriving address: address: {:?}", address);
    dbg_println!("[+] Deriving address: public key: {:?}", public_key);

    Ok(proto::DeriveAddressOutput {
        address,
        public_key,
    })
}

fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput> {
    let db_client = SecureStorageClient::open(DB_NAME)?;
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("[+] Sign transaction: error: wallet not found: {:?}", e))?;
    dbg_println!("[+] Sign transaction: wallet loaded");

    let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
    dbg_println!("[+] Sign transaction: signature: {:?}", signature);

    Ok(proto::SignTransactionOutput { signature })
}

fn test_p256_verify(input: &proto::TestP256VerifyInput) -> Result<proto::TestP256VerifyOutput> {
    dbg_println!("[+] Testing P-256 signature verification");
    dbg_println!("    Pubkey length: {}", input.pubkey_sec1.len());
    dbg_println!("    Message length: {}", input.message.len());
    dbg_println!("    Signature length: {}", input.signature_der.len());

    match passkey::verify_passkey_signature(
        &input.pubkey_sec1,
        &input.message,
        &input.signature_der,
    ) {
        Ok(_) => {
            dbg_println!("[+] ✅ P-256 signature verification: SUCCESS");
            Ok(proto::TestP256VerifyOutput {
                success: true,
                error_msg: String::new(),
            })
        }
        Err(e) => {
            let err_msg = format!("{:?}", e);
            dbg_println!("[+] ❌ P-256 signature verification: FAILED - {}", err_msg);
            Ok(proto::TestP256VerifyOutput {
                success: false,
                error_msg: err_msg,
            })
        }
    }
}

fn export_private_key(
    input: &proto::ExportPrivateKeyInput,
) -> Result<proto::ExportPrivateKeyOutput> {
    dbg_println!(
        "[+] Exporting private key for wallet: {:?}",
        input.wallet_id
    );

    let db_client = SecureStorageClient::open(DB_NAME)?;
    let wallet = db_client
        .get::<Wallet>(&input.wallet_id)
        .map_err(|e| anyhow!("[+] Export private key: wallet not found: {:?}", e))?;
    dbg_println!("[+] Export private key: wallet loaded");

    let private_key = wallet.derive_prv_key(&input.hd_path)?;
    dbg_println!("[+] Export private key: derived for path {}", input.hd_path);

    Ok(proto::ExportPrivateKeyOutput { private_key })
}

fn get_challenge(_input: &proto::GetChallengeInput) -> Result<proto::GetChallengeOutput> {
    dbg_println!("[+] Generating new challenge for Passkey authentication");

    // Initialize challenge manager if not already done
    let manager_lock = get_challenge_manager();
    let mut manager_opt = manager_lock
        .lock()
        .map_err(|e| anyhow!("Failed to lock challenge manager: {:?}", e))?;

    if manager_opt.is_none() {
        dbg_println!("[+] Initializing challenge manager");
        *manager_opt = Some(ChallengeManager::new());
    }

    let manager = manager_opt.as_mut().unwrap();
    let challenge = manager.generate_challenge()?;

    dbg_println!(
        "[+] Challenge generated: {} bytes",
        challenge.challenge.len()
    );

    Ok(proto::GetChallengeOutput {
        challenge: challenge.challenge.to_vec(),
        expires_in: 180, // 3 minutes
    })
}

fn handle_invoke(command: Command, serialized_input: &[u8]) -> Result<Vec<u8>> {
    fn process<T: serde::de::DeserializeOwned, U: serde::Serialize, F: Fn(&T) -> Result<U>>(
        serialized_input: &[u8],
        handler: F,
    ) -> Result<Vec<u8>> {
        let input: T = bincode::deserialize(serialized_input)?;
        let output = handler(&input)?;
        let serialized_output = bincode::serialize(&output)?;
        Ok(serialized_output)
    }

    match command {
        Command::CreateWallet => process(serialized_input, create_wallet),
        Command::RemoveWallet => process(serialized_input, remove_wallet),
        Command::DeriveAddress => process(serialized_input, derive_address),
        Command::SignTransaction => process(serialized_input, sign_transaction),
        Command::TestP256Verify => process(serialized_input, test_p256_verify),
        Command::ExportPrivateKey => process(serialized_input, export_private_key),
        Command::GetChallenge => process(serialized_input, get_challenge),
        _ => bail!("Unsupported command"),
    }
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    dbg_println!("[+] TA invoke command");
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    let output_vec = match handle_invoke(Command::from(cmd_id), p0.buffer()) {
        Ok(output) => output,
        Err(e) => {
            let err_message = format!("{:?}", e).as_bytes().to_vec();
            p1.buffer()
                .write(&err_message)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            p2.set_a(err_message.len() as u32);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };
    p1.buffer()
        .write(&output_vec)
        .map_err(|_| Error::new(ErrorKind::BadState))?;
    p2.set_a(output_vec.len() as u32);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));
