// SPIKE(方案 B 可行性验证 · throwaway)—— 证明 blst 能在 OP-TEE TA(带 teaclave std)里
// 交叉编译 + BLS 签名与 DVT signer(blst)/@noble 字节一致(DST=_POP_, min_pk: G1 pk / G2 sig)。
// 通过则全量做方案 B(BlsGenKey/BlsSign/BlsPubKey + secure storage 密封);不通则退回 keystore 方案。
#![allow(dead_code)]

use blst::min_pk::SecretKey;

/// 必须和 DVT signer/src/bls.rs + Node @noble 完全一致。
pub const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

/// 从 32 字节私钥材料派生并签名。返回压缩 G2(96 字节,= noble .toHex())。
pub fn spike_bls_sign(ikm: &[u8; 32], msg: &[u8]) -> Result<[u8; 96], blst::BLST_ERROR> {
    let sk = SecretKey::key_gen(ikm, &[])?;
    let sig = sk.sign(msg, BLS_DST, &[]);
    Ok(sig.compress())
}

/// 压缩 G1 公钥(48 字节)。
pub fn spike_bls_pubkey(ikm: &[u8; 32]) -> Result<[u8; 48], blst::BLST_ERROR> {
    let sk = SecretKey::key_gen(ikm, &[])?;
    Ok(sk.sk_to_pk().compress())
}
