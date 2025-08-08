#![feature(restricted_std)]
#![no_main]

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};

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

// Simple command constants
const CMD_HELLO_WORLD: u32 = 0;
const CMD_ECHO: u32 = 1;

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount TA invoke command: {}", cmd_id);
    
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    
    match cmd_id {
        CMD_HELLO_WORLD => {
            let response = "Hello from AirAccount TA!";
            p1.buffer()
                .write(response.as_bytes())
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            trace_println!("[+] Sent hello world response");
            Ok(())
        }
        CMD_ECHO => {
            let input_data = p0.buffer();
            let input_str = std::str::from_utf8(input_data)
                .unwrap_or("Invalid UTF-8");
            trace_println!("[+] Echo received: {}", input_str);
            
            p1.buffer()
                .write(input_data)
                .map_err(|_| Error::new(ErrorKind::BadState))?;
            Ok(())
        }
        _ => {
            trace_println!("[-] Unknown command: {}", cmd_id);
            Err(Error::new(ErrorKind::BadParameters))
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));