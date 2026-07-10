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

/// G1 uncompressed(96B = x[48]‖y[48] big-endian)→ EIP-2537 128B(2×64 槽,每 48B 右对齐、
/// 前 16B 零)。G1 是 Fp(非 Fp2),无 c0/c1 交换。与 SDK core encodeG1Point + 合约 publicKey
/// 布局逐字节一致。
fn encode_g1_eip2537(uncompressed_96: &[u8; 96]) -> [u8; 128] {
    let mut out = [0u8; 128];
    out[16..64].copy_from_slice(&uncompressed_96[0..48]); // x @ slot0
    out[80..128].copy_from_slice(&uncompressed_96[48..96]); // y @ slot1
    out
}

/// hash_to_curve(message, BLS_DST) → EIP-2537 256B G2(c0-first,复用 encode_g2_eip2537）。
/// blst_hash_to_g2 与 noble G2.hashToCurve 同为 RFC 9380 SSWU、同 DST → 同点 → SDK popPoint
/// 逐字节一致。
fn hash_to_g2_eip2537(message: &[u8]) -> [u8; 256] {
    let mut p = blst::blst_p2::default();
    unsafe {
        blst::blst_hash_to_g2(
            &mut p,
            message.as_ptr(),
            message.len(),
            BLS_DST.as_ptr(),
            BLS_DST.len(),
            core::ptr::null(),
            0,
        );
    }
    let mut aff = blst::blst_p2_affine::default();
    unsafe { blst::blst_p2_to_affine(&mut aff, &p) };
    let mut ser = [0u8; 192];
    unsafe { blst::blst_p2_affine_serialize(ser.as_mut_ptr(), &aff) };
    encode_g2_eip2537(&ser)
}

/// CC-37 staked registration: BLS proof-of-possession. RFC-standard **self-PoP** over the node's
/// OWN 128B EIP-2537 public key with DST=BLS_DST (…_POP_) — byte-identical to SDK core
/// buildDvtPop + on-chain registerWithProof (golden-vector aligned). Returns
/// (publicKey 128B, popPoint 256B, popSig 256B) = the DvtPop tuple.
///
/// Security: the message is the node's OWN pubkey — KMS-derived, the caller supplies NO message —
/// so /pop is not a general signing oracle. And a 128B pubkey can never equal a 32B co-sign
/// userOpHash, so a PoP can't be replayed as a co-sign signature even though the DST is shared.
pub fn sign_pop(sk_bytes: &[u8; 32]) -> Result<([u8; 128], [u8; 256], [u8; 256])> {
    let sk = SecretKey::from_bytes(sk_bytes).map_err(|e| anyhow!("BLS sk from_bytes: {:?}", e))?;
    let pk_uncompressed = sk.sk_to_pk().serialize(); // 96B uncompressed G1 (x‖y)
    let public_key = encode_g1_eip2537(&pk_uncompressed);
    let pop_point = hash_to_g2_eip2537(&public_key);
    let sig: Signature = sk.sign(&public_key, BLS_DST, &[]); // popSig = sk·hashToG2(pubkey128, BLS_DST)
    let pop_sig = encode_g2_eip2537(&sig.serialize());
    Ok((public_key, pop_point, pop_sig))
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

#[cfg(test)]
mod pop_golden {
    use super::sign_pop;

    // Golden vector from SDK core buildDvtPop (packages/core/src/crypto/dvtPop.ts) for the
    // BLS secret key 0x…c0ffee. sign_pop MUST be byte-identical: same publicKey (EIP-2537 G1),
    // same popPoint = hashToCurve(publicKey, BLS_DST), same popSig = sk·popPoint. This proves the
    // TA's blst PoP matches @noble + the on-chain registerWithProof convention (CC-37).
    #[test]
    fn sign_pop_matches_sdk_builddvtpop() {
        let mut sk = [0u8; 32];
        sk[29] = 0xc0;
        sk[30] = 0xff;
        sk[31] = 0xee;
        let (pk, pop_point, pop_sig) = sign_pop(&sk).expect("sign_pop");
        assert_eq!(
            hex::encode(pk),
            "0000000000000000000000000000000004ab31668afb74bfbb84fbc4602c783fd13fc95b20daa51cd45c0b9b82296c60217516d0e959cf91462b0068ff13e37e000000000000000000000000000000000fa6ffcfdfec5259fb7b7c46ea447b793035e023f6fe0dd5c5f9ff2204e84fbc58501257f4ea9827373a0764770438a8",
            "publicKey mismatch vs SDK buildDvtPop"
        );
        assert_eq!(
            hex::encode(pop_point),
            "000000000000000000000000000000000fd9cfdad02fa76f28f830742c9f13818cd7b2a73d7851ed3a57e679e98342be611a5bd0db54f1d9964706d16619cb090000000000000000000000000000000017af8fb2319cdf43f51d53c11f7532eddb07de1c8ed99c4514e3bb7775fc27062db7b7f4e19ff04cde3b49ba60dfb682000000000000000000000000000000000cefd3d7b3e70a87cca1e334eae75058a9b8fb1ebfa6b386ed6186f6f1924eab626f1434dc961054c97676ad77844f6b00000000000000000000000000000000073323e0018b6743e1963db45fee1d0455dbca4f929a9c968290fc16cdd0e848427ff4d26f212a28f459dadc93a86fb0",
            "popPoint mismatch vs SDK buildDvtPop"
        );
        assert_eq!(
            hex::encode(pop_sig),
            "0000000000000000000000000000000005b25aff113df19e14c5c4b6c4205863870a507ed8ca1daebb2959daa37da38ee493492245baa33650384e3dd9c3ca4d000000000000000000000000000000000cd5e829ccdf4f493281f3793f0ca111baa0b09404831b4e02f58fab87e5cf7e1c23ced4abffc03d9a5c2e99b63361830000000000000000000000000000000011ba087642fe31336872ed6be3fac5c8f89a06424e6d7fb01ee548596903370080a83c88933560aa4d6b5b81f681cc910000000000000000000000000000000012217f660bd95795146327ca4893044d5959400858710caf8f532d9b32e66e552b8ef5f005799b6f8b38a3371da569b7",
            "popSig mismatch vs SDK buildDvtPop"
        );
    }
}
