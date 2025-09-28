use optee_teec::{Context, Operation, OperationBuilder, Session, Uuid};
use proto::Command;
use std::error::Error;

const TA_UUID: Uuid = Uuid {
    time_low: 0x8aaaf200,
    time_mid: 0x2450,
    time_hi_and_version: 0x11e4,
    clock_seq_and_node: [0xab, 0xe2, 0x00, 0x02, 0xa5, 0xd5, 0xc5, 0x1b],
};

fn main() -> Result<(), Box<dyn Error>> {
    println!("🔐 KMS Host Application Starting...");

    // 连接到TEE
    let mut context = Context::new()?;
    println!("✅ Connected to TEE context");

    // 打开KMS TA会话
    let mut session = context.open_session(TA_UUID)?;
    println!("✅ Opened session with KMS TA");

    // 测试创建密钥
    test_create_key(&mut session)?;

    // 测试签名
    test_sign(&mut session)?;

    // 测试获取公钥
    test_get_public_key(&mut session)?;

    println!("🎉 All KMS operations completed successfully!");
    Ok(())
}

fn test_create_key(session: &mut Session) -> Result<(), Box<dyn Error>> {
    println!("\n📝 Testing CreateKey operation...");

    let op = OperationBuilder::new()
        .param_1(0x1234, 0x5678)
        .build();

    session.invoke_command(Command::CreateKey as u32, op)?;
    println!("✅ CreateKey operation completed");

    Ok(())
}

fn test_sign(session: &mut Session) -> Result<(), Box<dyn Error>> {
    println!("\n✍️  Testing Sign operation...");

    let op = OperationBuilder::new()
        .param_1(0xabcd, 0xef01)
        .build();

    session.invoke_command(Command::Sign as u32, op)?;
    println!("✅ Sign operation completed");

    Ok(())
}

fn test_get_public_key(session: &mut Session) -> Result<(), Box<dyn Error>> {
    println!("\n🔑 Testing GetPublicKey operation...");

    let op = OperationBuilder::new()
        .param_1(0x9999, 0x8888)
        .build();

    session.invoke_command(Command::GetPublicKey as u32, op)?;
    println!("✅ GetPublicKey operation completed");

    Ok(())
}