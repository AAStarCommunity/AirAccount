#![no_main]
#![feature(restricted_std)]

mod proto;
mod wallet;
mod hybrid_entropy;

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};
use proto::Command;
use wallet::{save_wallet, load_wallet, delete_wallet, Wallet};
use hybrid_entropy::{handle_create_hybrid_account, handle_sign_with_hybrid_key, handle_verify_security_state};
use anyhow::{anyhow, Result};
use std::io::Write;

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] AirAccount TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] AirAccount TA destroy");
}

#[cfg(debug_assertions)]
macro_rules! dbg_println {
    ($($arg:tt)*) => (trace_println!($($arg)*));
}

#[cfg(not(debug_assertions))]
macro_rules! dbg_println {
    ($($arg:tt)*) => {};
}

// Command handlers
fn create_wallet(_input: &proto::CreateWalletInput) -> Result<proto::CreateWalletOutput> {
    let mut wallet = Wallet::new()?;
    let wallet_id = wallet.get_id();
    let mnemonic = wallet.get_mnemonic()?;
    
    dbg_println!("[+] Wallet ID: {:?}", wallet_id);
    
    save_wallet(&wallet)?;
    dbg_println!("[+] Wallet saved in secure storage");
    
    Ok(proto::CreateWalletOutput {
        wallet_id,
        mnemonic,
    })
}

fn remove_wallet(input: &proto::RemoveWalletInput) -> Result<proto::RemoveWalletOutput> {
    dbg_println!("[+] Removing wallet: {:?}", input.wallet_id);
    
    delete_wallet(&input.wallet_id)?;
    dbg_println!("[+] Wallet removed");
    
    Ok(proto::RemoveWalletOutput { success: true })
}

fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput> {
    let mut wallet = load_wallet(&input.wallet_id)
        .map_err(|e| anyhow!("Wallet not found: {}", e))?;
    dbg_println!("[+] Deriving address: wallet loaded");
    
    let (address, public_key) = wallet.derive_address(&input.hd_path)?;
    dbg_println!("[+] Deriving address: address: {}", hex::encode(&address));
    dbg_println!("[+] Deriving address: public key: {}", hex::encode(&public_key));
    
    // Save wallet with updated derivation count
    save_wallet(&wallet)?;
    
    Ok(proto::DeriveAddressOutput {
        address,
        public_key,
    })
}

fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput> {
    let mut wallet = load_wallet(&input.wallet_id)
        .map_err(|e| anyhow!("Wallet not found: {}", e))?;
    dbg_println!("[+] Sign transaction: wallet loaded");
    
    let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
    dbg_println!("[+] Sign transaction: signature: {}", hex::encode(&signature));
    
    Ok(proto::SignTransactionOutput { signature })
}

fn get_wallet_info(input: &proto::GetWalletInfoInput) -> Result<proto::GetWalletInfoOutput> {
    let wallet = load_wallet(&input.wallet_id)
        .map_err(|e| anyhow!("Wallet not found: {}", e))?;
    
    Ok(proto::GetWalletInfoOutput {
        wallet_id: wallet.id,
        created_at: wallet.created_at,
        derivations_count: wallet.derivations_count,
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
        Command::HelloWorld => {
            let output = proto::HelloWorldOutput {
                message: "Hello from AirAccount TA!".to_string(),
                version: "0.1.0".to_string(),
            };
            let serialized_output = bincode::serialize(&output)?;
            Ok(serialized_output)
        }
        Command::Echo => {
            let input: proto::EchoInput = bincode::deserialize(serialized_input)?;
            let output = proto::EchoOutput {
                echoed_message: input.message,
            };
            let serialized_output = bincode::serialize(&output)?;
            Ok(serialized_output)
        }
        Command::GetVersion => {
            let output = proto::GetVersionOutput {
                version: "0.1.0".to_string(),
                build_info: "AirAccount TA with secure storage and eth_wallet compatibility".to_string(),
            };
            let serialized_output = bincode::serialize(&output)?;
            Ok(serialized_output)
        }
        Command::CreateWallet => process(serialized_input, create_wallet),
        Command::RemoveWallet => process(serialized_input, remove_wallet),
        Command::DeriveAddress => process(serialized_input, derive_address),
        Command::SignTransaction => process(serialized_input, sign_transaction),
        Command::GetWalletInfo => process(serialized_input, get_wallet_info),
        
        // P0安全修复：混合熵源命令处理
        Command::CreateHybridAccount => process(serialized_input, handle_create_hybrid_account),
        Command::SignWithHybridKey => process(serialized_input, handle_sign_with_hybrid_key),
        Command::VerifySecurityState => process(serialized_input, handle_verify_security_state),
    }
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    dbg_println!("[+] AirAccount TA invoke command: {}", cmd_id);
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    let output_vec = match handle_invoke(Command::from(cmd_id), p0.buffer()) {
        Ok(output) => output,
        Err(e) => {
            let err_message = format!("{:?}", e);
            let err_bytes = err_message.as_bytes();
            p1.buffer()
                .write(err_bytes)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            p2.set_a(err_bytes.len() as u32);
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