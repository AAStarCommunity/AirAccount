#![no_std]
#![no_main]
#![feature(restricted_std)]

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};

#[ta_create]
fn create() -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA create");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA open session");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("[+] AirAccount Simple TA close session");
}

#[ta_destroy]
fn destroy() {
    trace_println!("[+] AirAccount Simple TA destroy");
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA invoke command: {}", cmd_id);
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };

    match cmd_id {
        0 => {
            // Hello World command
            let message = b"Hello from AirAccount Simple TA!";
            for (i, &byte) in message.iter().enumerate() {
                if i < p1.buffer().len() {
                    p1.buffer()[i] = byte;
                }
            }
            p2.set_a(message.len() as u32);
        }
        1 => {
            // Echo command
            let input_size = p0.buffer().len();
            for i in 0..input_size {
                if i < p1.buffer().len() {
                    p1.buffer()[i] = p0.buffer()[i];
                }
            }
            p2.set_a(input_size as u32);
        }
        _ => {
            return Err(Error::new(ErrorKind::BadParameters));
        }
    }

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/user_ta_header.rs"));