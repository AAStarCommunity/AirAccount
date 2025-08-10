// Licensed to AirAccount under the Apache License, Version 2.0
// AirAccount Trusted Application - Simplified Entry Point

#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
    Error, ErrorKind, Parameters, Result,
};

// TA UUID: 11223344-5566-7788-99AA-BBCCDDEEFF01
const TA_UUID: &str = "11223344-5566-7788-99AA-BBCCDDEEFF01";

#[ta_create]
fn create() -> Result<()> {
    trace_println!("[+] AirAccount TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> Result<()> {
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

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> Result<()> {
    trace_println!("[+] AirAccount TA invoke command: {}", cmd_id);
    
    match cmd_id {
        0x1000 => handle_wallet_command(params),
        0x2000 => handle_core_operation(params),
        0x3000 => handle_security_operation(params),
        _ => {
            trace_println!("[-] Unknown command: {}", cmd_id);
            Err(Error::new(ErrorKind::BadParameters))
        }
    }
}

fn handle_wallet_command(params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Handling wallet command");
    
    // For now, return a simple success response
    let response = b"Wallet operation completed";
    
    // Extract buffer from parameters
    if let Ok(mut buf) = params.0.memref_mut() {
        let buffer = buf.buffer_mut();
        if response.len() <= buffer.len() {
            buffer[..response.len()].copy_from_slice(response);
            buf.set_updated_size(response.len());
        } else {
            return Err(Error::new(ErrorKind::ShortBuffer));
        }
    } else {
        return Err(Error::new(ErrorKind::BadParameters));
    }
    
    trace_println!("[+] Wallet command completed");
    Ok(())
}

fn handle_core_operation(_params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Handling core operation");
    Ok(())
}

fn handle_security_operation(_params: &mut Parameters) -> Result<()> {
    trace_println!("[+] Handling security operation");
    Ok(())
}