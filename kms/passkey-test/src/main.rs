// Passkey P-256 独立测试工具
// 模拟完整的 Passkey (FIDO2) 签名流程

use anyhow::Result;
use p256::ecdsa::{signature::Signer, signature::Verifier, SigningKey, VerifyingKey, Signature};
use p256::SecretKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
struct PasskeyTestVectors {
    // Passkey 密钥对 (模拟 FIDO2 authenticator)
    passkey_private_key: String,  // 私钥 (仅测试用,真实环境中存在硬件内)
    passkey_public_key: String,   // 公钥 (SEC1 uncompressed, 65 bytes)

    // Challenge-Response
    challenge: String,             // 挑战值 (32 bytes)
    challenge_signature: String,   // Passkey 对挑战的签名 (DER)

    // 测试消息
    test_message: String,          // 测试消息
    test_signature: String,        // 对测试消息的签名
}

fn main() -> Result<()> {
    println!("🔐 KMS Passkey 完整流程测试\n");
    println!("模拟 FIDO2/WebAuthn 认证流程\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // 1. 生成 Passkey 密钥对 (模拟 FIDO2 authenticator)
    println!("📱 1. 生成 Passkey 密钥对 (模拟硬件 authenticator)...");
    let passkey_signing_key = SigningKey::random(&mut OsRng);
    let passkey_verifying_key = VerifyingKey::from(&passkey_signing_key);

    // 导出公钥 (SEC1 uncompressed)
    let passkey_pubkey_sec1 = passkey_verifying_key.to_encoded_point(false);
    let passkey_pubkey_bytes = passkey_pubkey_sec1.as_bytes();
    println!("   Passkey 公钥 (SEC1, {} bytes):", passkey_pubkey_bytes.len());
    println!("   {}", hex::encode(passkey_pubkey_bytes));

    // 导出私钥 (仅用于测试,真实环境中私钥永不离开硬件)
    let passkey_secret = SecretKey::from(&passkey_signing_key);
    let passkey_private_bytes = passkey_secret.to_be_bytes();
    println!("\n   ⚠️  Passkey 私钥 (32 bytes, 仅测试用):");
    println!("   {}", hex::encode(&passkey_private_bytes));
    println!("   (真实环境中私钥存储在 FIDO2 硬件内,永不导出)\n");

    // 2. TEE 生成挑战 (Challenge)
    println!("🔐 2. TEE 生成挑战 (Challenge)...");
    let mut challenge = [0u8; 32];
    use rand_core::RngCore;
    OsRng.fill_bytes(&mut challenge);
    println!("   Challenge (32 bytes): {}\n", hex::encode(&challenge));

    // 3. Passkey 签名挑战 (模拟用户生物识别认证后)
    println!("✍️  3. Passkey 签名 Challenge (模拟用户指纹认证)...");
    let challenge_signature: Signature = passkey_signing_key.sign(&challenge);
    let challenge_sig_der = challenge_signature.to_der();
    println!("   签名 (DER, {} bytes): {}", challenge_sig_der.as_bytes().len(), hex::encode(challenge_sig_der.as_bytes()));

    // 4. TEE 验证 Passkey 签名
    println!("\n🔍 4. TEE 验证 Passkey 签名...");
    match passkey_verifying_key.verify(&challenge, &challenge_signature) {
        Ok(_) => println!("   ✅ Challenge 签名验证成功! 用户身份确认\n"),
        Err(e) => {
            println!("   ❌ Challenge 签名验证失败: {:?}\n", e);
            return Ok(());
        }
    }

    // 5. 测试消息签名
    println!("📝 5. 测试消息签名...");
    let message = b"Passkey Test Message";
    println!("   消息: {}", String::from_utf8_lossy(message));
    let message_signature: Signature = passkey_signing_key.sign(message);
    let message_sig_der = message_signature.to_der();
    println!("   签名 (DER, {} bytes): {}\n", message_sig_der.as_bytes().len(), hex::encode(message_sig_der.as_bytes()));

    // 6. 保存完整测试向量
    println!("💾 6. 保存完整 Passkey 测试向量...");
    let test_vectors = PasskeyTestVectors {
        passkey_private_key: hex::encode(&passkey_private_bytes),
        passkey_public_key: hex::encode(passkey_pubkey_bytes),
        challenge: hex::encode(&challenge),
        challenge_signature: hex::encode(challenge_sig_der.as_bytes()),
        test_message: hex::encode(message),
        test_signature: hex::encode(message_sig_der.as_bytes()),
    };

    let json = serde_json::to_string_pretty(&test_vectors)?;
    fs::write("/tmp/passkey_test_vectors.json", &json)?;
    println!("   保存到: /tmp/passkey_test_vectors.json\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Passkey 完整流程测试通过!");
    println!("\n📦 测试向量:\n{}", json);
    println!("\n🔑 使用说明:");
    println!("   1. Passkey 私钥: 仅用于测试,模拟 FIDO2 硬件");
    println!("   2. Passkey 公钥: 存储在 TEE Wallet 中");
    println!("   3. Challenge: TEE 生成,3分钟有效期");
    println!("   4. Challenge 签名: Passkey 用私钥签名 (需生物识别)");
    println!("   5. TEE 用公钥验证签名,确认用户身份\n");

    Ok(())
}
