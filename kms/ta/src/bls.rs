// Variant B: 软件 BLS12-381 签名(blst),密钥在 TA 内生成 + 密封 secure storage，永不出 TEE。
// 与 DVT signer(blst min_pk)+ Node @noble longSignatures 同库同 DST → 签名字节一致。
// G1 公钥(48B 压缩)/ G2 签名。DST 必须与 DVT/validator 完全一致。
use anyhow::{anyhow, Result};
use blst::min_pk::{PublicKey, SecretKey, Signature};

/// 与 DVT signer/src/bls.rs + validator + Node @noble 完全一致。
pub const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// 从 32 字节 IKM(TEE TRNG)派生 BLS 私钥。返回 (32B 私钥标量, 48B 压缩 G1 公钥)。
pub fn gen_keypair(ikm: &[u8; 32]) -> Result<([u8; 32], [u8; 48])> {
    let sk = SecretKey::key_gen(ikm, &[]).map_err(|e| anyhow!("BLS key_gen: {:?}", e))?;
    let pk = sk.sk_to_pk().compress();
    Ok((sk.to_bytes(), pk))
}

/// 用密封的私钥字节签 32 字节 message(hash_to_curve + G2 sign,DST=_POP_)。
/// 返回 (EIP-2537 uncompressed G2 256B, 压缩 G2 96B)。
pub fn sign(sk_bytes: &[u8; 32], message: &[u8; 32]) -> Result<([u8; 256], [u8; 96])> {
    let sk = SecretKey::from_bytes(sk_bytes).map_err(|e| anyhow!("BLS sk from_bytes: {:?}", e))?;
    let sig: Signature = sk.sign(message, BLS_DST, &[]);
    let compact = sig.compress(); // 96B compressed G2 (= noble .toHex())
    let eip2537 = encode_g2_eip2537(&sig.serialize()); // 192B uncompressed → 256B EIP-2537
    Ok((eip2537, compact))
}

/// 从密封私钥字节恢复 48B 压缩 G1 公钥(校验/恢复用,handler 走存储的公钥)。
#[allow(dead_code)]
pub fn pubkey(sk_bytes: &[u8; 32]) -> Result<[u8; 48]> {
    let sk = SecretKey::from_bytes(sk_bytes).map_err(|e| anyhow!("BLS sk from_bytes: {:?}", e))?;
    Ok(sk.sk_to_pk().compress())
}

/// 校验:压缩 G1 公钥合法(gen 后自检)。
pub fn pubkey_valid(pk_compressed: &[u8; 48]) -> bool {
    PublicKey::from_bytes(pk_compressed).is_ok()
}

/// blst uncompressed G2(192B = 4×48 field elements)→ EIP-2537(256B = 4×64,每个 48B
/// 左填充到 64B)。blst serialize 顺序与 EIP-2537 一致(x.c0,x.c1,y.c0,y.c1)。
/// ⚠️ 需对 hash-to-g2.golden 向量验字节一致(实现后 benchmark + 验证阶段做)。
fn encode_g2_eip2537(uncompressed_192: &[u8; 192]) -> [u8; 256] {
    let mut out = [0u8; 256];
    for i in 0..4 {
        // 每个 48B field element 右对齐进 64B 槽(前 16B 零填充)。
        out[i * 64 + 16..i * 64 + 64].copy_from_slice(&uncompressed_192[i * 48..i * 48 + 48]);
    }
    out
}
