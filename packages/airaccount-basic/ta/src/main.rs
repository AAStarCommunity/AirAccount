// AirAccount Basic TA - 基础框架测试版本
// 完全基于 eth_wallet 例子实现，确保稳定通信

#![no_main]

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command, ta_open_session, trace_println,
};
use optee_utee::{Error, ErrorKind, Parameters};
use std::io::Write;

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

// 简单的命令处理 - 完全复制 eth_wallet 模式
fn handle_command(cmd_id: u32, input: &[u8]) -> Result<Vec<u8>, String> {
    trace_println!("[+] TA handle command: {}", cmd_id);
    
    match cmd_id {
        0 => {
            // Hello World command
            let message = b"Hello from AirAccount Basic TA - Framework Test OK!";
            Ok(message.to_vec())
        }
        1 => {
            // Echo command
            Ok(input.to_vec())
        }
        2 => {
            // Get Version command
            let version = b"AirAccount Basic TA v0.1.0 - Framework Test";
            Ok(version.to_vec())
        }
        _ => {
            Err(format!("Unsupported command: {}", cmd_id))
        }
    }
}

// 严格按照 eth_wallet 模式实现
#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Basic TA invoke command: {}", cmd_id);
    
    // 严格按照 eth_wallet 参数模式
    let mut p0 = unsafe { params.0.as_memref()? };  // 输入数据
    let mut p1 = unsafe { params.1.as_memref()? };  // 输出数据
    let mut p2 = unsafe { params.2.as_value()? };   // 输出长度值
    
    trace_println!("[+] TA parameters extracted successfully");
    
    // 处理命令
    let output_vec = match handle_command(cmd_id, p0.buffer()) {
        Ok(output) => output,
        Err(e) => {
            let err_message = e.as_bytes().to_vec();
            // 写入错误信息到输出缓冲区
            if err_message.len() <= p1.buffer().len() {
                p1.buffer()[..err_message.len()].copy_from_slice(&err_message);
                p2.set_a(err_message.len() as u32);
            }
            trace_println!("[!] TA command error: {}", e);
            return Err(Error::new(ErrorKind::BadParameters));
        }
    };
    
    // 写入成功结果
    if output_vec.len() <= p1.buffer().len() {
        p1.buffer()[..output_vec.len()].copy_from_slice(&output_vec);
        p2.set_a(output_vec.len() as u32);
        trace_println!("[+] TA command success, output length: {}", output_vec.len());
    } else {
        let err_msg = "Output buffer too small".as_bytes();
        p1.buffer()[..err_msg.len()].copy_from_slice(err_msg);
        p2.set_a(err_msg.len() as u32);
        trace_println!("[!] TA output buffer too small");
        return Err(Error::new(ErrorKind::ShortBuffer));
    }
    
    Ok(())
}