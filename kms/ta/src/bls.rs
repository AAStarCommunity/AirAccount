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

/// blst uncompressed G2(192B)→ EIP-2537(256B = 4×64,每个 48B 右对齐进 64B 槽,前 16B 零)。
/// ⚠️ blst serialize 的 Fp2 顺序是 [x.c1, x.c0, y.c1, y.c0],而 EIP-2537 precompile +
/// DVT validator 要 [x.c0, x.c1, y.c0, y.c1] —— **每对 c0/c1 必须交换**(src_order=[1,0,3,2])。
/// 已对 DVT `encodeG2Point` 规范编码实测字节一致(2026-07-07)。
fn encode_g2_eip2537(uncompressed_192: &[u8; 192]) -> [u8; 256] {
    let mut out = [0u8; 256];
    // EIP-2537 slot i ← blst chunk SRC[i](交换 c0/c1)。
    const SRC: [usize; 4] = [1, 0, 3, 2];
    for i in 0..4 {
        let src = SRC[i] * 48;
        out[i * 64 + 16..i * 64 + 64].copy_from_slice(&uncompressed_192[src..src + 48]);
    }
    out
}
