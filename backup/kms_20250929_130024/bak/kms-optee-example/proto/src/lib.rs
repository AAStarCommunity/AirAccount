#![no_std]

extern crate alloc;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    CreateKey = 0,
    Sign = 1,
    GetPublicKey = 2,
    Unknown,
}

impl From<u32> for Command {
    fn from(value: u32) -> Command {
        match value {
            0 => Command::CreateKey,
            1 => Command::Sign,
            2 => Command::GetPublicKey,
            _ => Command::Unknown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub key_spec: KeySpec,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    pub key_id: Vec<u8>,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignRequest {
    pub key_id: Vec<u8>,
    pub message: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignResponse {
    pub signature: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyRequest {
    pub key_id: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPublicKeyResponse {
    pub public_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KeySpec {
    EccSecgP256k1,
}