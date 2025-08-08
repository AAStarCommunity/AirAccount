#![no_std]
#![no_main]
#![feature(restricted_std)]

mod proto;
mod wallet;

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};
use proto::Command;
use wallet::{save_wallet, load_wallet, delete_wallet, Wallet};

// No_std 环境的 Result 类型
type Result<T> = core::result::Result<T, &'static str>;

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
    
    Ok(proto::RemoveWalletOutput {})
}

fn derive_address(input: &proto::DeriveAddressInput) -> Result<proto::DeriveAddressOutput> {
    let mut wallet = load_wallet(&input.wallet_id)
        .map_err(|_| "Wallet not found")?;
    dbg_println!("[+] Deriving address: wallet loaded");
    
    let (address, public_key) = wallet.derive_address(&input.hd_path)?;
    dbg_println!("[+] Deriving address: address: {:02x?}", &address[..4]);
    dbg_println!("[+] Deriving address: public key: {:02x?}", &public_key[..4]);
    
    // Save wallet with updated derivation count
    save_wallet(&wallet)?;
    
    Ok(proto::DeriveAddressOutput {
        address,
        public_key,
    })
}

fn sign_transaction(input: &proto::SignTransactionInput) -> Result<proto::SignTransactionOutput> {
    let mut wallet = load_wallet(&input.wallet_id)
        .map_err(|_| "Wallet not found")?;
    dbg_println!("[+] Sign transaction: wallet loaded");
    
    let signature = wallet.sign_transaction(&input.hd_path, &input.transaction)?;
    dbg_println!("[+] Sign transaction: signature: {:02x?}", &signature[..8]);
    
    Ok(proto::SignTransactionOutput { signature })
}

fn get_wallet_info(input: &proto::GetWalletInfoInput) -> Result<proto::GetWalletInfoOutput> {
    let wallet = load_wallet(&input.wallet_id)
        .map_err(|_| "Wallet not found")?;
    
    Ok(proto::GetWalletInfoOutput {
        wallet_id: wallet.id,
        created_at: wallet.created_at,
        derivations_count: wallet.derivations_count,
    })
}

fn handle_invoke(command: Command, input: &[u8]) -> Result<Vec<u8>> {
    match command {
        Command::HelloWorld => {
            let message = "Hello from AirAccount TA! Version 0.1.0";
            Ok(message.as_bytes().to_vec())
        }
        Command::Echo => {
            // Echo back the input
            Ok(input.to_vec())
        }
        Command::GetVersion => {
            let version = "AirAccount TA 0.1.0 with eth_wallet compatibility";
            Ok(version.as_bytes().to_vec())
        }
        Command::CreateWallet => {
            // 简化实现：返回模拟的钱包ID和助记词
            let wallet = Wallet::new()?;
            let wallet_id = wallet.get_id();
            let mnemonic = wallet.get_mnemonic()?;
            save_wallet(&wallet)?;
            
            // 简化：返回固定格式的响应
            let response = b"wallet_created";
            Ok(response.to_vec())
        }
        Command::RemoveWallet => {
            // TODO: 解析wallet_id并删除钱包
            Ok(b"wallet_removed".to_vec())
        }
        Command::DeriveAddress => {
            // TODO: 解析wallet_id和hd_path，派生地址
            Ok(b"address:0x0000000000000000000000000000000000000000".to_vec())
        }
        Command::SignTransaction => {
            // TODO: 解析交易数据并签名
            Ok(b"signature:0x00".to_vec())
        }
        Command::GetWalletInfo => {
            // TODO: 解析wallet_id并返回钱包信息
            Ok(b"wallet_info:mock".to_vec())
        }
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
            let err_message = b"TA_ERROR";
            // 将错误消息复制到输出缓冲区
            for (i, &byte) in err_message.iter().enumerate() {
                if i < p1.buffer().len() {
                    p1.buffer()[i] = byte;
                }
            }
            p2.set_a(err_message.len() as u32);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };
    
    // 将输出复制到缓冲区
    for (i, &byte) in output_vec.iter().enumerate() {
        if i < p1.buffer().len() {
            p1.buffer()[i] = byte;
        }
    }
    p2.set_a(output_vec.len() as u32);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));