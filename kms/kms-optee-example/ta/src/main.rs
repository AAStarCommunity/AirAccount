#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters, Result};
use proto::{Command, CreateKeyRequest, CreateKeyResponse, SignRequest, SignResponse,
           GetPublicKeyRequest, GetPublicKeyResponse, KeySpec};

use secp256k1::{Secp256k1, SecretKey, PublicKey};
use sha3::{Digest, Sha3_256};
use rand::RngCore;

static mut KMS_STORAGE: Option<BTreeMap<Vec<u8>, SecretKey>> = None;

struct MockRng;
impl RngCore for MockRng {
    fn next_u32(&mut self) -> u32 { 0x12345678 }
    fn next_u64(&mut self) -> u64 { 0x123456789abcdef0 }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(37).wrapping_add(142);
        }
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> core::result::Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

#[ta_create]
fn create() -> Result<()> {
    trace_println!("[+] KMS TA create");
    unsafe {
        KMS_STORAGE = Some(BTreeMap::new());
    }
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> Result<()> {
    trace_println!("[+] KMS TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] KMS TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] KMS TA destroy");
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> Result<()> {
    trace_println!("[+] KMS TA invoke command: {}", cmd_id);

    match Command::from(cmd_id) {
        Command::CreateKey => handle_create_key(params),
        Command::Sign => handle_sign(params),
        Command::GetPublicKey => handle_get_public_key(params),
        Command::Unknown => {
            trace_println!("[-] Unknown command: {}", cmd_id);
            Err(Error::new(ErrorKind::BadParameters))
        }
    }
}

fn handle_create_key(params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Creating new key");

    let secp = Secp256k1::new();
    let mut rng = MockRng;

    // 生成密钥对
    let secret_key = SecretKey::new(&mut rng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // 生成密钥ID
    let mut key_id = Vec::new();
    key_id.extend_from_slice(b"kms_key_");
    let key_counter = unsafe {
        KMS_STORAGE.as_ref().map(|s| s.len()).unwrap_or(0)
    };
    key_id.extend_from_slice(&key_counter.to_be_bytes());

    // 存储密钥
    unsafe {
        if let Some(storage) = &mut KMS_STORAGE {
            storage.insert(key_id.clone(), secret_key);
        }
    }

    // 返回响应
    let response = CreateKeyResponse {
        key_id: key_id.clone(),
        public_key: public_key.serialize().to_vec(),
    };

    trace_println!("[+] Key created with ID length: {}", key_id.len());
    Ok(())
}

fn handle_sign(params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Signing message");

    // 这里应该从params中解析SignRequest，简化演示
    let message = b"Hello from TEE!";
    let mut hasher = Sha3_256::new();
    hasher.update(message);
    let hash = hasher.finalize();

    trace_println!("[+] Message signed, hash length: {}", hash.len());
    Ok(())
}

fn handle_get_public_key(params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Getting public key");
    Ok(())
}

// 包含生成的头文件
include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));