<!-- Created: 2026-07-07 -->
# DVT BLS 私钥托管:KMS TEE 托管 + 自启(安全分析与实现方案)

> 两轮讨论(2026-07-07)的分析背景、决策、结论与实现方案。
> 问题:DVT 的 BLS 签名私钥现在是盘上 EIP-2335 keystore + 手动密码(断电需重输)。
> 能否像 KMS 一样把密钥交给板子 TEE 托管、做到**自启不要密码**,且安全?

---

## 0. 两种模式(不强绑定 KMS)

同一个 DVT 二进制,`RUST_SIGNER_URL` env 决定 BLS 密钥托管模式:
- **独立 DVT(keystore 模式)**:`RUST_SIGNER_URL` unset → BLS 私钥在 EIP-2335 keystore(盘上加密 + 手动密码)。零 KMS 依赖。**只跑 DVT 的社区用这个。**
- **合并(KMS-TEE 模式,本文档)**:`RUST_SIGNER_URL=http://127.0.0.1:3100` → 委托 KMS TA 签名,私钥进 TEE 永不出、自启。**跑 KMS+DVT 的社区用这个(最强安全)。**

不是两套代码/两个方案,是一个方案两个 profile。Community Node Kit 提供 `dvt-only` / `kms+dvt` 两个 profile。keystore 模式永远是独立 fallback,Variant B 是可选增强。

## 1. 背景

- **KMS**:用户私钥在 TEE,永不可导出;每次 Sign 要**当场 passkey/WebAuthn**,且 **TA 在安全世界内部验 passkey**(纯密码学,不联网)。→ 自启安全:密钥出不来 + 不给 passkey 签不了 + 被攻破的 host 也没法让 TA 乱签。
- **DVT**:职责是给 UserOp 做 BLS 门限共签(**第二因子**,防 owner key 被盗)。私钥现在盘上 keystore。
- **诉求**:让 DVT 也能自启(断电后自动恢复,不用人工输密码),同时不牺牲安全。

## 2. DVT 签名不是"无门槛"—— 三道门

担心"DVT 起来就执行、无门槛"是误解。DVT 签名被三道门管着,**门在每次签名/链上/门限,不在开机**:

1. **owner-auth 门(每操作)**:DVT 只在 `isValidOwnerAuth(userOpHash, ownerAuth)→0xa0cf00cf` 通过(账户 owner 授权了这个具体 userOp)才签。= DVT 版的 passkey 门。
2. **链上验证门**:DVT 签名是第二因子,**单独授权不了任何事**;链上账户自己的 owner-auth 不过 → 交易废。
3. **门限门(≥3 独立节点)**:伪造需 **≥门限个独立节点同时被攻破**,不是一个。

## 3. 三种密钥托管方案对比(抗提取 vs 抗滥用)

| 方案 | BLS 私钥在哪 | 抗提取(偷密钥) | 抗滥用(逼它乱签) | 自启 |
|---|---|---|---|---|
| **现:EIP-2335 + tmpfs 密码** | 解密后在 DVT 进程内存 | 盘窃安全;攻破运行中 DVT 可从 RAM 挖出 | owner-auth(host)+链上+门限 | ❌ 断电需重输 |
| **A:TEE 存密码,开机解封给 DVT** | 仍进 DVT 内存 | 同上(RAM 可挖) | 同上 | ✅ |
| **B:密钥封 TEE,TA 内软件 BLS 签名,只回签名** | **永不出 TEE** | **强(= KMS 级,攻破 DVT 也挖不到)** | owner-auth(**仍 host**)+链上+门限 | ✅ |

**方案 B = 用户描述的"DVT 验证通过 → 调内部 API 让 TA 签 → 只收签名"。**

## 4. KMS 与 DVT 的本质差异(为什么 B 补不了"抗滥用")

- **KMS 的 TA 能自己验 passkey**(纯密码学,不联网)→ 被攻破的 host 也没法让 TA 乱签。**抗提取 + 抗滥用都强。**
- **DVT 的 owner-auth 在链上**(要 `eth_call isValidOwnerAuth`)→ TA 在安全世界联不了网、**验不了** → owner-auth 门只能在 host(DVT)做 → **被攻破的 host 能绕过它、逼 TA 签任意 userOp**。B 把"抗提取"提到 KMS 级,但"抗滥用"补不上。

## 5. 但"抗滥用"漏洞被后两道门兜住 —— 方案整体安全

被攻破的 DVT 逼 TA 签了恶意 userOp:
- **链上账户自己的 owner-auth 不过 → 废**(DVT 只是第二因子);
- 要真得手,攻击者需 **owner key(用户 UA passkey)+ ≥门限个独立节点** —— **两个独立、都难的攻破**;
- **攻破板子只拿到 DVT 这层,拿不到用户 owner-auth 这层**(passkey 在用户设备/TEE,板上没有)。

**两层互补**:owner-auth 防 DVT 被攻破;DVT 防 owner key 被盗。单独攻破任一层都不够。

**承重安全 = 门限 + 运营方独立性 > 单节点密钥托管。** 单节点用密码还是 TEE 是次要防线(defense-in-depth);真正的安全在 ≥3 个独立社区节点。

## 6. 结论(决策)

1. **"自启就执行"不危险** —— 签名被 owner-auth + 链上 + 门限管着,开机不解门。前提不成立。
2. **现方案(EIP-2335 + 手动密码)已够用** —— 手动密码是盘窃/物理的额外防线,非主门。
3. **方案 B(BLS 密钥封 TEE + TA 软件签名)= 单节点抗提取到 KMS 级 + 自启** —— 补掉"密钥可从 DVT 内存挖出"这唯一比 KMS 弱的点。**采纳为目标方案。**
4. **B 的残留抗滥用面**(被攻破 host 逼 TA 签)由链上第二因子 + 门限 + 用户 owner-auth 兜底,**系统整体安全**。
5. **投资优先级**:门限 + 独立性(Community Node Kit,≥3 独立节点)> 单节点托管。

## 7. 实现方案(方案 B)

复用 DVT 已有的 **`RUST_SIGNER_URL` 抽象**(DVT 本就把 BLS 签名可外包给一个 signer 服务)——让 **KMS TEE 充当这个 signer**:

```
DVT(host, 做 owner-auth 验证)
   │ 验证通过 → 内部调用(localhost only)
   ▼
KMS 内部 BLS-sign 端点(不对公网)
   │ CA → TA 命令
   ▼
KMS TA(安全世界):软件 BLS12-381 签名,密钥密封在 secure storage,永不出 TEE
   ▲ 只回签名(EIP-2537 G2)
```

**改动分工**:
1. **TA(kms/ta)**:引入 **no_std 软件 BLS12-381**(如 zkcrypto `bls12_381` crate + hash-to-curve),新增命令 `BlsGenKey`(生成+密封)/`BlsSign(keyId, msg)`/`BlsPubKey`;私钥密封进 secure storage(复用 wallet key 模式)。**必须**与 DVT/validator 的 DST=`BLS_SIG_..._POP_` + EIP-2537 编码**字节一致**(对 `hash-to-g2.golden` 向量)。
2. **Host(kms/host)**:internal BLS-sign 端点(仅 127.0.0.1,不进公网 KMS API),实现 DVT `RUST_SIGNER_URL` 期望的 signer 契约。
3. **DVT**:`RUST_SIGNER_URL=http://127.0.0.1:<port>` + `RUST_SIGNER_REQUIRED=true`;`node_state.json` 不再持有私钥(密钥在 TEE)。
4. **自启**:BLS 密钥密封 TEE(device-bound,TA 开 session 时解封,无密码)→ KMS + DVT 都自启;手动密码保留为可选"高安全模式"。
5. **验证**:签名与现有 @noble 路径**字节一致**(golden 向量)+ 链上 `validate=0` 通过;fail-closed 与门限不变。

## 8. 可行性 spike —— ✅ 已通过(2026-07-07)

**最大风险(TA 里跑软件 BLS12-381)已实测打通:**
- **板子原生无 BLS**:OP-TEE/GP TEE API 只支持 NIST P-192..521(r1)、SM2、Ed25519、RSA(`tee_api_defines.rs`)—— **无 BLS12-381、无 pairing**。→ 软件是唯一路(无硬件捷径)。
- **blst 编进 OP-TEE TA 成功**:`blst 0.3.15`(features `portable`+`no-threads`)+ 最小 sign(`min_pk`,DST=`BLS_SIG_..._POP_`)**交叉编译成 `aarch64-unknown-optee` 通过(25s)**。TA 本就带 teaclave OP-TEE std + 已用 cc 交叉编 C 密码库(secp256k1-sys / p256-m.c),blst 走同一路。
- **签名字节一致**:与 DVT `signer/`(也用 blst `min_pk`)+ Node `@noble longSignatures` 同库同 DST → 天然一致(已跨端互通)。PoC 见 `kms/ta/src/bls_spike.rs`。

**离线 vendoring 方法(容器无网,新增 crate 必做)**:TA build 用 git 协议 crates.io index(`6f17...`),新 crate 的元数据只在 sparse index(`1949...`)→ 需把 `.cache/xx/xx/name` 从 sparse 拷到 git index,并确保 `.crate` 在 git registry cache。blst 全链补齐:`blst → zeroize_derive, threadpool→num_cpus→hermit-abi`(hermit-abi 从 Mac `docker cp` 进容器)。

**BLS 库选型决策(2026-07-07,已定 blst)**:
- **用 blst**(Supranational,C 核心 + 手写 aarch64/x86 asm + Rust crate 封装)。不换纯 Rust。
- 理由:① 性能 —— C+asm 域运算比纯 Rust 快 2-4x;② 安全/稳定 —— BLS12-381 事实标准,以太坊共识关键基建(Lighthouse/Teku/Prysm),审计充分、constant-time;③ 零互通风险 —— DVT signer + Node @noble 已用 blst,签名天然字节一致。
- **TA 里 asm vs portable(实现时 benchmark 定)**:spike 已证 `portable`(纯 C 无 asm)能编进 OP-TEE TA(保底);TA handler 阶段试 blst **带 aarch64 asm**(去 portable feature)→ 能 assemble 就用(更快)+ benchmark 签名延迟;assemble 不了(PIC/汇编器 flag)回退 portable C(DVT 偶发共签,portable 延迟大概率可接受)。

## 8b. 真机端到端验证 —— ✅ 全部通过(2026-07-07,FRDM-IMX93)

KMS 侧 Variant B **实现完成 + 部署板子 + 真机验证全绿**:
- **TA 生成+密封**:`POST 127.0.0.1:3100/gen-key` → TA 内 TEE-TRNG 生成 BLS 密钥、密封 secure storage、返回 48B 压缩 G1 公钥(`0x9039ffa8…`)。私钥从未离开 TEE。
- **TA 签名**:`POST /sign {node_id,user_op_hash}` → TA 内 blst 签名 → 返回 EIP-2537 G2(256B)+ compact(96B)+ pubkey。
- **Golden 验证 PASS**:TA 签名用 DVT 的 `@noble/curves` bls12_381 `longSignatures.verify(sig, G2.hashToCurve(msg,{DST}), pubkey)` 验 → **✅ 字节一致**。证明 TA 的 blst(DST=`_POP_`/hash-to-curve/EIP-2537 编码)与 DVT/validator/@noble 完全兼容。
- 实现:proto ABI(cmd 27/28/29)+ TA `bls.rs`+`BlsKey` Storable 密封 + 3 handler + host TeeHandle wrappers + internal signer(:3100 localhost,tokio::join 与主 :3000 并发)+ `/gen-key` provision。全部交叉编译通过,签名 TA 858K。

**结论:最大技术风险(TA 内软件 BLS 且字节兼容)彻底retired。** DVT 侧接入 = 指 `RUST_SIGNER_URL=http://127.0.0.1:3100`;迁移注意:TEE 新生成的 pubkey 与 DVT 原盘上 key 不同 → 需在链上 validator **重新注册新 TEE pubkey**(一次性迁移 tx),之后 slot 稳定。

**剩余(DVT 接入 + 迁移)**:
- 给 TA 加命令 `BlsGenKey`/`BlsSign`/`BlsPubKey` + secure storage 密封;proto ABI 扩展。
- Host internal signer 端点(仅 127.0.0.1)对齐 DVT `RUST_SIGNER_URL` 契约。
- 重编刷 TA + **重算 measurement**(改了 TA 二进制,attestation 清单要更新)+ 迁移(公钥不变 → 链上 slot 平滑)。

## 9. 落地顺序(待 review 后执行)

1. **可行性 spike**:TA 里软件 BLS sign,对 `hash-to-g2.golden` + 一个 @noble 签名互验字节一致。**gate:过了才继续。**
2. TA 命令(gen/seal/sign/pubkey)+ secure storage 密封。
3. Host internal signer 端点(localhost)。
4. DVT 接 `RUST_SIGNER_URL` + 去掉盘上私钥。
5. 端到端:自启(无密码)+ 签名字节一致 + 链上 validate=0 + fail-closed/门限不变。
6. 生产板:重编刷 TA + measurement + 迁移。

## 10. X-Signer-Token 部署 wiring(co-location)

signer 端点(`:3100`)默认 localhost-only。要收紧到「只有 DVT 进程能调」:
1. 部署时生成一个随机共享密钥:`TOKEN=$(openssl rand -hex 32)`。
2. **KMS 侧**:`KMS_BLS_SIGNER_TOKEN=$TOKEN` 写进 kms-api.service 的 EnvironmentFile(只 root 可读)。
3. **DVT 侧**:`RUST_SIGNER_TOKEN=$TOKEN` 写进 DVT 的 env(同一密钥)。
4. DVT 调 `/sign` 自动带 `X-Signer-Token: $TOKEN`(v1.10.0+);KMS 比对放行,读不到该 env 的其他本地进程签不了。
- **不设 = 不强制**(向后兼容;门限 + owner-auth 仍兜底)。KMS 已支持(#153),DVT 侧 v1.10.0(PR AAStarCommunity/YetAnotherAA-Validator#182)。
