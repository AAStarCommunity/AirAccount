// AirAccount Basic TA - following eth_wallet pattern
#![no_main]

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};

use anyhow::{Result};
use std::io::Write;

// Simple commands - start with basic hello world functionality
#[derive(Debug)]
#[repr(u32)]
pub enum Command {
    HelloWorld = 0,
    Echo = 1,
    GetVersion = 2,
    Unknown = 0xFFFFFFFF,
}

impl From<u32> for Command {
    fn from(value: u32) -> Self {
        match value {
            0 => Command::HelloWorld,
            1 => Command::Echo,
            2 => Command::GetVersion,
            _ => Command::Unknown,
        }
    }
}

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Basic TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Basic TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] AirAccount Basic TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] AirAccount Basic TA destroy");
}

fn handle_hello_world() -> Result<Vec<u8>> {
    let response = "Hello from AirAccount TA!";
    trace_println!("[+] HelloWorld: {}", response);
    Ok(response.as_bytes().to_vec())
}

fn handle_echo(input: &[u8]) -> Result<Vec<u8>> {
    let input_str = std::str::from_utf8(input)
        .unwrap_or("Invalid UTF-8");
    trace_println!("[+] Echo: {}", input_str);
    
    let response = format!("Echo: {}", input_str);
    Ok(response.as_bytes().to_vec())
}

fn handle_get_version() -> Result<Vec<u8>> {
    let version = "AirAccount TA v0.1.0";
    trace_println!("[+] GetVersion: {}", version);
    Ok(version.as_bytes().to_vec())
}

fn handle_invoke(command: Command, input: &[u8]) -> Result<Vec<u8>> {
    trace_println!("[+] Handle invoke command: {:?}", command);
    
    match command {
        Command::HelloWorld => handle_hello_world(),
        Command::Echo => handle_echo(input),
        Command::GetVersion => handle_get_version(),
        Command::Unknown => {
            trace_println!("[-] Unknown command");
            Err(anyhow::anyhow!("Unknown command"))
        }
    }
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount TA invoke command: {}", cmd_id);
    
    let mut p0 = unsafe { params.0.as_memref()? };  // input buffer
    let mut p1 = unsafe { params.1.as_memref()? };  // output buffer
    let mut p2 = unsafe { params.2.as_value()? };   // output length
    
    let input_data = p0.buffer();
    let command = Command::from(cmd_id);
    
    let output_vec = match handle_invoke(command, input_data) {
        Ok(output) => output,
        Err(e) => {
            let err_message = format!("Error: {:?}", e);
            trace_println!("[-] Command failed: {}", err_message);
            
            let err_bytes = err_message.as_bytes();
            p1.buffer()
                .write(err_bytes)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            p2.set_a(err_bytes.len() as u32);
            
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };
    
    // Write successful output
    p1.buffer()
        .write(&output_vec)
        .map_err(|_| Error::new(ErrorKind::BadState))?;
    p2.set_a(output_vec.len() as u32);
    
    trace_println!("[+] Command completed successfully");
    Ok(())
}

// Include the generated TA header
include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));