#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    trace_println!("[+] AirAccount Simple TA invoke command: {}", cmd_id);
    
    // 基础验证 - 只检查命令ID范围
    if cmd_id > 50 {
        trace_println!("[!] Invalid command ID: {}", cmd_id);
        return Err(Error::new(ErrorKind::BadParameters));
    }
    
    // 根据需要获取参数
    let result = match cmd_id {
        0 => {
            // Hello World command - 只需要输出缓冲区和长度参数
            let mut p1 = unsafe { params.1.as_memref()? };
            let mut p2 = unsafe { params.2.as_value()? };
            
            let message = b"Hello from AirAccount Simple TA with Wallet Support!";
            let len = message.len().min(p1.buffer().len());
            p1.buffer()[..len].copy_from_slice(&message[..len]);
            p2.set_a(len as u32);  // 设置输出长度
            Ok(())
        }
        1 => {
            // Echo command - 需要输入缓冲区、输出缓冲区和长度参数
            let p0 = unsafe { params.0.as_memref()? };
            let mut p1 = unsafe { params.1.as_memref()? };
            let mut p2 = unsafe { params.2.as_value()? };
            
            let input_size = p0.buffer().len().min(p1.buffer().len());
            p1.buffer()[..input_size].copy_from_slice(&p0.buffer()[..input_size]);
            p2.set_a(input_size as u32);  // 设置输出长度
            Ok(())
        }
        2 => {
            // Get Version command - 只需要输出缓冲区和长度参数
            let mut p1 = unsafe { params.1.as_memref()? };
            let mut p2 = unsafe { params.2.as_value()? };
            
            let version = b"AirAccount Simple TA v0.1.0 - Basic Wallet Support";
            let len = version.len().min(p1.buffer().len());
            p1.buffer()[..len].copy_from_slice(&version[..len]);
            p2.set_a(len as u32);  // 设置输出长度
            Ok(())
        }
        
        // 钱包管理命令 (10-19)
        10 => {
            // Create Wallet - 只需要输出缓冲区和长度参数
            let mut p1 = unsafe { params.1.as_memref()? };
            let mut p2 = unsafe { params.2.as_value()? };
            
            match handle_create_wallet(p1.buffer()) {
                Ok(len) => {
                    p2.set_a(len as u32);
                    Ok(())
                }
                Err(_e) => Err(optee_utee::ErrorKind::Generic.into())
            }
        }
        
        // 暂时禁用其他命令，专注于修复基础命令 (0,1,2,10)
        _ => {
            trace_println!("[!] Command {} temporarily disabled during parameter fix", cmd_id);
            Err(Error::new(ErrorKind::NotImplemented))
        }
    };

    result
}