# CC-103 KMS 侧验收标准(Acceptance)

> 定稿前提:KMS = 签名代码零改动(见 SPEC.md)。验收 = 用**证据**证明"零改动是安全的",而不是改代码。
> 每条给机器可验证命令;跑不了的(需 testnet)明确标注并给人工判据。

## 验收总纲

KMS 侧 CC-98 committee 交付 = 4 项,**全绿才算完**:
1. **B2 审计**:证明 KMS 所有签名路径不构造/注入 accountId。
2. **DST golden-vector 回归**:把 KMS↔dvt↔validator 三方 DST 一致钉死(防未来任一改 DST 无声破网)。
3. **legacy 集成盘点**:对新 Sepolia 地址,legacy 路径签名可被 #237 validator 验过。
4. **边界设计说明**:KMS 角色边界落文档(不可破边界写死)。

---

## A. B2 审计 — 无 accountId

**判据**:KMS 无任何签名路径把 accountId 塞进签名输出。

```bash
# A1 全签名路径静态审计:签名相关源码零处构造/注入 accountId 字段
cd <repo>
grep -rniE 'account_?id' kms/ta/src/ kms/proto/src/in_out.rs kms/host/src/ta_client.rs \
  | grep -viE 'test|//|comment|doc' 
# 期望:空(或仅出现在与签名无关的路径,需逐条人工确认非签名注入)

# A2 BLS/owner/session/keeper 输入输出结构不含 accountId
grep -rniE 'struct.*(Sign|Bls|Keeper|Session).*(Input|Output)' -A8 kms/proto/src/in_out.rs \
  | grep -iE 'account'
# 期望:空(签名 I/O 结构无 account 字段)
```

**通过条件**:A1/A2 均空;审计结论写入 `kms/docs/` 边界说明(见 D)。若 A1 有命中,逐条证明是非签名路径(如日志/审计元数据),否则 **FAIL**。

---

## B. DST golden-vector 回归测试(核心新增)

**判据**:一个 KAT(known-answer test)钉死 `bls_sign` 的 DST + 字节输出,任何人改 KMS 的 `BLS_DST` 或 hash-to-curve 即红。

```bash
# B1 DST 常量逐字节锁定
grep -q 'BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_' kms/ta/src/bls.rs && echo OK || echo FAIL
# 期望:OK(与 dvt #237 validator + SDK noble 三方一致的 DST)

# B2 known-answer 测试(新增):固定 sk + 固定 userOpHash(32B)→ 固定 BLS 签名字节
#    在 kms/ta 或 host 交叉编译测试里加:
#    #[test] fn bls_sign_kat_dst_locked() {
#        let sk = <fixed 32B>;  let msg = <fixed userOpHash 32B>;
#        let sig = bls_sign_with(sk, msg);   // DST=BLS_DST
#        assert_eq!(hex(sig), "<golden 从 dvt/noble 交叉产出>");  // 三方对齐的 golden
#    }
cargo test -p <kms-ta-or-host-crate> bls_sign_kat_dst_locked
# 期望:通过。golden 字节需与 dvt #237 / SDK noble 对同一 (sk,msg) 的输出**逐字节相同**。
```

**通过条件**:B1=OK 且 B2 KAT 通过。**golden 字节的三方一致性**:至少注释交叉引用 dvt 的 golden vector 来源(#237 / CC-104);理想是同一 (sk, msg) 在 blst/noble/validator 三处产出相同 96/256B。

> 若拿不到 dvt 的 golden fixture:B2 先锁 KMS 自产 KAT(防 KMS 自身回归),并在 tasks 里挂一条"向 dvt 要 golden fixture 做三方对齐"的跟进项(非阻塞本交付,但要显式记录)。

---

## C. legacy 集成盘点(对新 Sepolia 地址)

**判据**:KMS 的 BLS partial 在 legacy(whole-set)模式下能被新 committee validator `0x1A8Db639…` 验过。

- **静态**(可跑):确认 KMS 用的 validator/router 地址配置可指向新地址(Router `0xA15127…`,validator `0x1A8Db639…`),且 legacy 路径不因 committee 字段缺失而崩。
- **E2E**(需 testnet,可能阻塞):对 enrolled 测试账户 `0xf249d5708…` 发一笔 tier-2/3 UserOp,KMS-TEE 出 BLS partial → aggregator 聚合 → 账户 validate 通过。**现 committeeActive=false → 走 legacy,验证签名原像 = userOpHash 路径通。**

**通过条件**:静态配置盘点通过 + 边界文档记录 E2E 步骤;E2E 若受 testnet/keeper 阻塞,标 `BLOCKED` 记入 progress,不算 KMS 代码失败。

---

## D. 边界设计说明

**判据**:`kms/docs/committee-signing-boundary.md`(新增)落地 SPEC.md 第 4/8 节:KMS=不透明签名机、BLS 原像=userOpHash 不变、DST 三方一致、绝不碰 accountId、绝不在 TEE 加 accountId 绑定。

```bash
test -f kms/docs/committee-signing-boundary.md && \
  grep -qE 'userOpHash|不透明|accountId|BLS_SIG_BLS12381G2' kms/docs/committee-signing-boundary.md \
  && echo OK || echo FAIL
```

---

## 全局硬约束(违反即不通过)

- **不动 TA 逻辑**(committee 对 KMS 零改动;若发现必须改 TA 的情形 → 停下,说明,不擅自改)。
- 现有测试全绿:`bash kms/test-full-api.sh localhost:3000`(本地) + 相关 cargo test 无回归。
- 板 CA 构建若涉及,须带 `MX93_DEV_RPID=1 MX93_STRICT_CHALLENGE=1`(见 [[build_mx93_board_features]])。
- 绝不打印/存储私钥出 TEE;绝不 `git add -A`;绝不直推 main;一 task 一分支一 PR。

## 完成定义(Definition of Done)

A✅ + B✅ + C(静态✅ / E2E 记录) + D✅ + 现有测试无回归 → **KMS 侧 CC-103 committee 交付完成**。回帖 CC-103 更新交付状态,标 committee 正向路径 KMS 侧就绪(等 SDK per-signer + dvt 翻转)。
