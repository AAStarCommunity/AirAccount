#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use core::panic::PanicInfo;

use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};
use sha3::{Digest, Sha3_256};

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// 模拟TEE环境的RNG
struct TeeRng {
    state: u64,
}

impl TeeRng {
    fn new() -> Self {
        TeeRng { state: 0x123456789abcdef0 }
    }

    fn next_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
            *byte = (self.state >> 24) as u8;
        }
    }
}

fn validate_kms_in_tee() -> bool {
    let secp = Secp256k1::new();
    let mut rng = TeeRng::new();

    // 1. 模拟TEE中的密钥生成
    let mut secret_bytes = [0u8; 32];
    rng.next_bytes(&mut secret_bytes);

    let secret_key = match SecretKey::from_slice(&secret_bytes) {
        Ok(key) => key,
        Err(_) => return false,
    };

    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // 2. 模拟TEE中的消息哈希
    let message = b"KMS message from TEE environment";
    let mut hasher = Sha3_256::new();
    hasher.update(message);
    let hash = hasher.finalize();

    // 3. 模拟TEE中的签名
    let message_hash = match Message::from_digest_slice(&hash) {
        Ok(msg) => msg,
        Err(_) => return false,
    };

    let signature = secp.sign_ecdsa(&message_hash, &secret_key);

    // 4. 模拟TEE中的验证
    let verification_result = secp.verify_ecdsa(&message_hash, &signature, &public_key);

    // 返回验证结果
    verification_result.is_ok()
}

// 为了在std环境中测试，提供一个std版本
fn main() {
    #[cfg(feature = "std")]
    {
        println!("🔐 KMS TEE Validation Test");
        let result = validate_kms_in_tee();
        if result {
            println!("✅ KMS functions work correctly in TEE-like environment!");
        } else {
            println!("❌ KMS validation failed");
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // TEE环境中的KMS功能验证
        let _result = validate_kms_in_tee();
    }
}