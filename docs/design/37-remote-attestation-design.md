# AirAccount KMS 远程证明设计（i.MX93 落地）

> 创建时间：2026-06-13
> 最后更新：2026-06-13（据 NXP RM00284 Rev 4.3 手册 + imx-secure-enclave 官方代码 + 实机结果回写 R-1/R-8/H-2）
> 关联 Issue：#37 TEE 远程证明
> 文档性质：落地设计 / 证书链架构 / 分期路线 / 风险登记
> 前置阅读：`docs/design/37-remote-attestation-research.md`（业界调研，本文不重复论证）

---

## 0. 一句话目标与诚实边界

**目标**：让客户端能**密码学验证**「AirAccount KMS 的签名响应，确实来自一台真实 NXP i.MX93、跑着未篡改的 OP-TEE、加载的是我们发布的那个特定 KMS TA 二进制」——而不是攻击者控制的普通 host 进程在伪造。

**诚实边界（先讲清楚）**：
- 远程证明的信任根**必然落在 NXP**（你无法证明一块芯片是真 i.MX93 而不绕过 NXP 的硬件根）。半去中心化能去中心化的是 **Verifier 逻辑、参考值分发、验证发生的位置**，**不是硬件根**。
- **2026-06-13 一手核对后的现状**：
  - **R-8 已基本解决（架构修正）[官方确证]**：ELE 不能背书 OP-TEE 自生成的外部 key（`hsm_pub_key_attest` 只 attest ELE 库内 key），所以改为「attest/签名 key 在 ELE 密钥库内生成 + ELE 出证书」；secure-boot 度量 ELE 确实提供并签名（H-2 成立）。详见 §2。
  - **R-1 仍是唯一阻塞「不信任部署方」的前提 [RM00284 已查仍未答]**：ELE `hsm_dev_attest` 的签名 key 是否 NXP 工厂注入、有无连 NXP 根的可离线验证书链——**RM00284 是 API 参考手册，对此完全沉默**，需 NXP 安全参考手册 / EdgeLock 2GO / NDA 才能收口。另有残留：pub_key_attest 的背书 key 本身如何连 NXP 根（§2 / R-8 残留）。
- **R-1 不成立时**：MVP 仍有意义（证明「进了 TEE 且是这个 TA」），但「不信任部署方」这一最强目标要打折，退到 §6 P0' 的 TOFU 安全降级（§9 已澄清：TOFU 是安全妥协，非去中心化优势）。

---

## 1. 现有信任链 vs 目标信任链

当前（#37 之前）：

```
用户 ──盲信──► kms.aastar.io ──盲信──► 我们部署的硬件 ──► TEE
                  ▲ 攻击者控制服务器即可返回伪造签名，客户端无从察觉
```

目标：

```
用户 ──验证 attestation 证书链──► OP-TEE TA（NXP 硬件根锚定）
            │
            └─ 不再需要盲信部署方；信任下沉到「NXP 芯片是真的 + TA 是公开发布的那个」
```

---

## 2. 总体方案：三段式证据，逐层锚定

AirAccount 的证据由内到外三层，每层补上前一层的信任缺口（对应调研 §2.1 的核心坑：OP-TEE attestation key 默认自签、无根）：

```
┌─ 层 C：业务绑定 ────────────────────────────────────────┐
│  把「本次 KMS 签名/WebAuthn ceremony」绑进证据           │
│  → 防「证据是真的，但和这次签名无关」的拼接攻击          │
│  载体：nonce + 业务摘要（passkey pubkey / signHash 请求）│
├─ 层 B：TA 度量 ────────────────────────────────────────┤
│  OP-TEE attestation PTA: GET_TA_SHDR_DIGEST             │
│  → 证明「加载进 TEE 的是我们发布的 KMS TA 二进制」       │
│  签名：OP-TEE attestation key（EC/RSA）                  │
├─ 层 A：硬件根锚定 ─────────────────────────────────────┤
│  ELE 设备 attestation（uid + ROM hash + FW hash + 签名）│
│  → 期望：把层 B 的 attest key 锚回 NXP 信任根            │
│  签名：ELE 设备密钥（连 NXP 根=R-1；能否绑外部 key=R-8） │
└────────────────────────────────────────────────────────┘
```

**关键设计点（已用官方代码 + RM00284 手册逐项核对，2026-06-13 更新）**：原先设想的「OP-TEE 自生成 attest key + ELE 背书它」**经一手核对已确认走不通**，必须改架构。下面分两半讲清楚。

⚠️ **OP-TEE 侧（H-1）已被官方代码坐实为硬约束 [官方源确证]**：
- OP-TEE attestation key **设备自生成、零证书链、零硬件锚定**（`core/pta/attestation.c`：`generate_key()`→`crypto_acipher_gen_rsa_key()`，无任何 anchoring）。
- OP-TEE attestation PTA **只有 4 个命令，没有任何一个对 caller 传入的外部公钥/数据签名**（`pta_attestation.h`：`GET_PUBKEY`/`GET_TA_SHDR_DIGEST`/`HASH_TA_MEMORY`/`HASH_TEE_MEMORY`，全部签 `nonce|自身度量`）。

⚠️ **ELE 侧（R-8）已被 RM00284 + imx-secure-enclave 源码坐实 [官方手册确证 + 官方代码确证]**：
- ELE **有** public key attestation 原语 `hsm_pub_key_attest` / `hsm_do_pub_key_attest`，但 RM00284 §3.4.6（p39）明写它只能 **「attest the public key of an asymmetric key present in the ELE FW key storage」**——**只能 attest「已经在 ELE 密钥库里」的 key，无法 attest OP-TEE 自己生成、存在 OP-TEE secure storage 里的外部 key**。`hsm_pub_key_attest.h` 注释与 `test_pub_key_attest.c` demo 一致确认。
- 即:**「ELE 背书 OP-TEE 自生成 attest pubkey」这条路，OP-TEE 侧不签外部 key、ELE 侧不收外部 key，两头都堵死。原三段式设计作废。**

→ **架构修正（新主候选，已是官方能力的最现实组合）**：**不要在 OP-TEE 里生成 attest/签名 key，改为在 ELE 密钥库里生成**（`hsm_generate_key` → ECC NIST P-256/P-384，key 永不出 ELE），用 ELE 做实际签名 + 用 `hsm_pub_key_attest`(ECDSA attest algo) 产出 **certificate（signed TLV）**。这样 key 由硬件 HSM 持有、证书由 ELE 出，比 OP-TEE 自签强一个量级。⚠️ **代价**：以太坊私钥本就因 ELE 不支持 secp256k1 而软件管理（见 §8），attest/签名 key 走 ELE 是新增的「ELE 内 P-256 身份 key」，与业务 secp256k1 签名是两套 key、两套信任链，设计上要分清。

⚠️ **但 R-8 暴露了一个更深、RM00284 也没补上的缺口（必须诚实写明）**：`hsm_pub_key_attest` 是「**用密钥库里的 key B（`key_attestation_id`）去 attest 密钥库里的 key A（`key_identifier`）**」。**key B 本身也是用户用 `hsm_generate_key` 生成的普通密钥库 key——RM00284 没有提供任何「内置的、NXP 工厂注入并连 NXP 根的 attestation key」让你拿来当 key B。** 也就是说 pub_key_attest 把 A 链到 B，但 **B 凭什么连到 NXP 根，手册没给答案**。要补这一环只有两条路，都未在 RM00284 闭合：
- 通过 **EdgeLock 2GO**（NXP 云）provision 一把 NXP 背书的 key 当 key B —— 强中心化依赖 NXP 云，[待 EdgeLock 2GO 文档/实测]。
- 用 **device attestation（`hsm_dev_attest`，§3.12）的设备 key 去背书 key B** —— **但 RM00284 §3.12 确认 dev_attest 只签设备度量（uid/ROM/FW/SRKH）+ nonce，不签密钥库里的 key**，**没有任何 API 让「设备身份 key」给「密钥库 attestation key」背书**。这座桥在 RM00284 里不存在。[待 NXP 安全参考手册/NDA 确认是否另有机制]

层 A→B 候选（按现实性重排，2026-06-13）:
1. **(新主候选) attest/签名 key 在 ELE 密钥库生成 + pub_key_attest(ECDSA) 出证书**。可做到「key 在 HSM 内、ELE 出证书」，但**根仍卡在 key B 如何连 NXP**（上面的缺口）。
2. **(补根，二选一) EdgeLock 2GO provision NXP 背书 key 当 key B**（中心化）/ 或确认是否有 device-key→keystore-key 的桥（手册未见）。[待验证]
3. **(都不成立,无奈降级) TOFU 登记 attest pubkey**：见 §3 降级说明与 §6 P0'。这是**安全降级**,不是更优解。

社区 optee-ra 把「给 attest key 引入证书/HSM 锚定」列为 future work——AirAccount 现在已确认：OP-TEE 侧补不了，必须靠 ELE 密钥库 + pub_key_attest，而「根如何连 NXP」是 R-1 与 R-8 共同的、RM00284 未闭合的真问题。

---

## 3. 证书链架构（ASCII）

```
                    ┌─────────────────────────────────────┐
                    │  NXP Root CA  (离线, 公开指纹)         │   ← 中心化锚点(不可消除)
                    └───────────────┬─────────────────────┘
                                    │ 签发
                    ┌───────────────▼─────────────────────┐
                    │  i.MX93 设备身份 (ELE dev_attest)      │   ← hsm_dev_attest §3.12
                    │  ECDSA P-384 签名(96B)over:           │
                    │   uid + sha_rom_patch + sha_fw +      │
                    │   oem_srkh(OEM 安全启动根 hash) +     │   ← secure_boot 度量, H-2 确证
                    │   lmda_val(lifecycle)+ssm_state+nonce │
                    │  ⚠️ 签名 key 来源/连 NXP 根 = R-1 未解 │   ← RM00284 §3.12 对此沉默!
                    └───────────────┬─────────────────────┘
                                    │ 层A→B: 缺桥! dev_attest 不签 keystore key
                                    │ ⚠️ key B 如何连 NXP 根 = R-1/R-8 真缺口
                    ┌───────────────▼─────────────────────┐
                    │  ELE 密钥库 attestation key (key B)    │   ← hsm_generate_key 生成
                    │  hsm_pub_key_attest(ECDSA) 出 cert:    │   ← §3.4.6, 只 attest 库内 key
                    │   attest( key A = 签名/会话 pubkey )   │
                    └───────────────┬─────────────────────┘
                                    │ 层B: ELE 库内 key 对下签名 / 出证书
                    ┌───────────────▼─────────────────────┐
                    │  Attestation Evidence (每次请求)       │
                    │  {  nonce(客户端给),                   │
                    │     kms_ta_measurement = SHA256(TA),  │
                    │     secure_boot_state(透传自 ELE 层A), │   ← 值源自 ELE, 非 OP-TEE 自填
                    │     业务摘要(层C),                     │
                    │     timestamp(ree_time) }             │
                    └───────────────┬─────────────────────┘
                                    │ 交付
                    ┌───────────────▼─────────────────────┐
                    │  客户端 / Verifier / (未来)链上合约    │
                    │  自顶向下验链 + 比对参考值 + 校 nonce  │
                    └─────────────────────────────────────┘
```

**`secure_boot_state` 的可信来源（H-2 已用 RM00284 + 官方代码确证）**：该字段**来自 ELE `hsm_dev_attest`（RM00284 §3.12 / `hsm_dev_attest.h`）的输出，由 ELE 度量、ELE 用设备 key ECDSA P-384 签名**，**严禁由 OP-TEE/TA 自行填写**。dev_attest 输出里可作 secure-boot 判据的字段 **[官方代码确证]**：
- `oem_srkh`（64B，OEM Super Root Key Hash——OEM 安全启动根公钥的 hash，即 secure boot 信任锚）；version 3 还有 `oem_pqc_srkh`（后量子 SRKH）。
- `sha_rom_patch`（32B，Sentinel ROM patch SHA-256）、`sha_fw`（32B，已装 FW SHA 前 256 bit）。
- `lmda_val`（lifecycle：`HSM_LMDA_OEM_OPEN=0x10` / `CLOSED=0x40` / `LOCKED=0x200`）、`ssm_state`（Security Subsystem State Machine 状态）。

→ **H-2 结论**：ELE **确实**提供可信、经硬件签名的 secure-boot / lifecycle 度量，V4 可以建立在它之上（而不是 OP-TEE 自报）。⚠️ **但有两个未解约束**：① 验证这把 ELE 签名要 R-1 的「设备 key 连 NXP 根」成立（否则只是另一把自签 key，见下）；② lifecycle 实测为 OPEN（开发态），生产前应推进到 CLOSED/LOCKED，否则 secure-boot 语义不完整。V4 的可信度因此仍取决于 R-1。

**吊销/轮换策略（含 R-9）**：设备证书长期有效；attestation key 短期（每次重启/定期重生成并按 R-8 选定方式重新锚定）；evidence 一次性（nonce 防重放）。attest key 的吊销/过期机制本身单列为复核点 **R-9**（§7）。

⚠️ **降级 fallback（非更优解，是安全妥协）**：若 R-1/R-8 均不成立（ELE 既给不出连 NXP 根的设备证书,也无法背书/锚定 attest key），则「NXP Root CA → 设备证书」两层塌缩，只能退化为 **AirAccount 发布的「设备登记表」(TOFU / SSH known_hosts 式)**：首次部署登记设备 attest 公钥指纹。**这是牺牲「不信任部署方」换可用性的安全降级,不是去中心化优势**(见 §9 澄清)。这是 §6 的降级路线 P0'。

---

## 4. 客户端验证流程

```
客户端                          KMS Host (CA)              TEE (KMS TA)        ELE
  │  1. 生成 nonce(32B 随机)        │                          │                │
  │  2. GET /attestation?nonce=… ─►│                          │                │
  │     (或在 Sign 响应里带证据)    │── InvokeCommand ────────►│                │
  │                                 │   GetAttestation(nonce)  │                │
  │                                 │                          │ 取 TA 度量(PTA)│
  │                                 │                          │ (见 ELE 背书↓) │
  │                                 │                          │── 请求 ELE 背书►│
  │                                 │                          │◄ 设备签名/证书 ┤
  │                                 │                          │ 组装三层证据    │
  │                                 │◄── evidence + cert_chain ┤                │
  │◄── evidence + cert_chain ───────┤                          │                │
  │                                                                            │
  │  本地验证（@aastar/attestation-verifier）:                                  │
  │   V1. 验证书链: NXP Root → 设备证 → attest key 证 → evidence 签名 都过      │
  │   V2. evidence.nonce == 我发的 nonce            (防重放)                    │
  │   V3. evidence.kms_ta_measurement == 已知发布值 (TA 真实性, 发版时公布)     │
  │   V4. ELE dev_attest: lmda_val(lifecycle)/oem_srkh/sha_fw 比对参考值        │
  │       ⚠️值由 ELE 签(H-2确证可信), 但验签需 R-1(设备key连NXP根)成立          │
  │   V5. optee_version 在允许列表内                                            │
  │   V6. (层C) 业务摘要 == 本次请求摘要            (绑定本次操作)              │
  │   全过 → 信任后续/本次 KMS 操作; 任一失败 → 拒绝并告警                       │
```

**V4 标注 [官方确证 H-2 + 待 R-1]**：`secure_boot`/lifecycle 度量**确实由 ELE `hsm_dev_attest` 度量并签名**（RM00284 §3.12，字段见 §3），**不是 OP-TEE 自报**——H-2 成立。客户端 V4 应比对 `lmda_val`(lifecycle)、`oem_srkh`(安全启动根 hash)、`sha_fw`/`sha_rom_patch` 等参考值。⚠️ 但**这把 ELE 签名能不能验、值能不能信，取决于 R-1**（ELE 设备 key 是否连 NXP 根）：R-1 不成立时 ELE 签名只是另一把自签 key，V4 退化为「自说自话」；且实测 lifecycle = OPEN（开发态），生产前需推进 CLOSED/LOCKED。

**流程图中 `secure_boot_state` 的来源（H-2 消歧）**：上图 TEE 泳道里的 `(见 ELE 背书↓)` 表示 **TEE 不自行读取 secure_boot**——该值是 ELE device attestation 响应（`◄ 设备签名/证书`，层 A）的一部分，由设备密钥签出，TEE 只做转组装。任何「OP-TEE/TA 自读 secure_boot」的实现都不可信（见 §3 H-2 统一口径）。

**`timestamp(ree_time)` 非安全承载（L-2）**：evidence 里的时间戳取自 REE（`optee_utee::Time::ree_time()`），**REE 可篡改**，**严禁**用于有效期/新鲜性窗口判断；证据新鲜性**仅由客户端 nonce 保证**，时间戳只作日志/排障参考。实现者勿据它做 TTL 校验。

### 4.1 两种交付模式与握手绑定（M-1）

- **签名内联（强，推荐默认）**：`Sign`/`SignHash` 响应直接带 evidence，nonce = 请求里携带的 challenge，业务摘要(层C) = 本次签名请求摘要。每次关键签名自带证明，天然零信任，无绑定缺口。
- **独立握手端点 `GET /attestation?nonce=`（轻量,但需补绑定）**：客户端会话开始验一次。

⚠️ **握手模式的绑定缺口与修复（M-1）**：只验一次握手、后续 Sign 不绑定，等于「握手时是真 TEE,之后的签名谁签的不知道」——中间人可在握手后接管,形同虚设。握手模式**必须**补一条「把后续签名绑回那个已验证 TEE 实例」的机制,三选一:
1. **会话绑定密钥（推荐）**：握手 evidence 里额外包含一把 TEE 内生成的**会话公钥** `session_pubkey`（与 attest key 一同被层 B 签,证明这把会话 key 确实在被证明的 TEE 内）。客户端记下它；后续每个 Sign 响应都用对应的会话私钥**额外签一遍**(在 TEE 内),客户端用 `session_pubkey` 验。这样「已验证的 TEE 实例」与「后续每次签名」密码学绑定。
2. **attest key 直接复用为签名响应签名 key**：让 KMS 签名响应本身由那把已被锚定的 attest key（或其派生子 key）副署,省掉单独会话 key,但增加 attest key 使用频率(与 R-9 轮换策略需协调)。
3. **短 TTL 握手 + 频繁重握手**：给握手结论设很短有效期,过期即强制重新出具 evidence。最弱,仅作 1/2 不可行时的兜底。

无论哪种,**握手结论不可无限期复用**;默认建议直接用「签名内联」避免该类缺口。

---

## 5. 与现有机制的结合

| 现有机制 | 如何结合 |
|---|---|
| **RSA-4096 TA 签名**（OP-TEE 4.8 NXP key 签 TA 镜像） | 这是「TA 加载时被 OP-TEE 校验」的基础；attestation PTA 的 `GET_TA_SHDR_DIGEST` 度量的正是 signed header。两者互补：加载校验防跑非法 TA，attestation 把「跑的是哪个 TA」告诉远端。 |
| **WebAuthn challenge binding（nonce 已下沉 TA）** | 复用同一套 TA 内一次性 nonce 设施做 evidence freshness；甚至可把 attestation evidence 绑进 WebAuthn ceremony（注册/认证时一并产出证据，层 C）。⚠️ 注意现有 nonce 用了 thread_local 跨 TA 线程有 flaky 记录（见 MEMORY），attestation nonce 不要复用同一坑，需用持久化/会话级存储。 |
| **RPMB 防回滚** | ⚠️ **RPMB 是防回滚/防降级机制,不是 attestation 的证据源**——证据(度量值、设备身份)来自 PTA 和 ELE,RPMB 不产出任何被签进 evidence 的内容。RPMB 的作用是:① 把「已撤销的旧 attest key / 旧 TA measurement 黑名单」绑定 RPMB 单调计数器,防「物理回滚到旧的、已知漏洞 TA,再出具看似合法的证据」(降级攻击);② 配合 R-9 的 key 轮换,使被吊销的 attest key 无法靠回滚复活。⚠️ **M-3**：(a) RPMB 防回滚本身依赖 **RPMB 认证密钥**（通常来自 CAAM/ELE/fuse），该密钥 compromise 即可绕过回滚保护——属**单点**，纳入 R-9 的 key 治理范畴登记；(b) RPMB 是**本地**防回滚（让本机 TA **自我拒绝**跑被吊销的旧 TA/key），**远端客户端看不到 RPMB 状态**——**远端防降级的真正机制是 V3 参考值比对 + attest key 证书时效**，两者职责不同，勿混。 |
| **AWS KMS 兼容 API** | 新增 `GetAttestation`（非 AWS 标准操作，自定义端点，**不强制 `x-amz-target` header**，走独立 GET 路由）。不污染现有 AWS 兼容面。 |

---

## 6. 分期实现路线

### Phase 0 — 硬件能力摸底（部分已完成，2026-06-13）
**已完成 [官方手册/代码 + 实机]**：
- [x] RM00284 + imx-secure-enclave 源码核对：ELE `hsm_dev_attest`（§3.12，签 uid/ROM/FW/oem_srkh/lifecycle，ECDSA P-384 96B）+ `hsm_pub_key_attest`（§3.4.6，只 attest ELE 库内 key，CMAC/ECDSA）能力已确证。
- [x] 实机：`hsm_dev_attest exchange Passed`（dev_attest 真机跑通）；lifecycle = OPEN（0x1 / lmda OEM_OPEN）。
- [x] 实机：完整 HSM session（含 `hsm_pub_key_attest`）需 **NVM-Daemon**，当前 disabled（`hsm_open_session 0x14 HSM Feature Disabled`），pub_key_attest 未实测。

**剩余待做**：
- [ ] 在 MX93 串口实机确认：NXP BSP 的 OP-TEE 是否带 **attestation PTA**（`pta_attestation.h` UUID `39800861-...`）（R-2）。
- [ ] **R-1（最关键，RM00284 未答）**：dev_attest 的 `signature` 用哪把 key 签、有无连 NXP 根、可否第三方离线验？RM00284 §3.12 对此沉默 → 查 **NXP 安全参考手册 / EdgeLock 2GO / NDA 渠道**。
- [ ] **R-8 补根**：确认 EdgeLock 2GO 能否 provision「NXP 背书的 key」当 pub_key_attest 的 `key_attestation_id`（key B），或是否有 device-key→keystore-key 的桥（手册未见）。
- [ ] 启 NVM-Daemon 后实测 `hsm_pub_key_attest`（ECDSA attest algo）出证书。
- [ ] 确认 TA 内能否经 imx-secure-enclave/SCMI 调到 ELE attestation（dev_attest/pub_key_attest demo 均 normal-world 用户态）（R-3）。
- [ ] 产出：`docs/design/37-attestation-hw-findings.md`，据 R-1 结论决定走主路线还是降级路线 P0'。

### Phase 1 — MVP：TA 度量 + nonce 端到端可验（2 周）
> 即使 Phase 0 发现 ELE 给不出 NXP 证书链，**这一阶段照样有价值**：先证明「确实进了 TEE 且跑的是这个 TA」。
- [ ] proto 增 `GetAttestation = <next id>` 命令（`kms/proto`）。
- [ ] TA 侧 `kms/ta/src/attestation.rs`：调 attestation PTA 取 `GET_TA_SHDR_DIGEST`，组装 evidence（nonce + ta_measurement + optee_version + ree_time），用 OP-TEE attestation key 签。**注意：用 `optee_utee::Time::ree_time()` 取时间，禁用 `SystemTime::now()`（会 panic，见 MEMORY）。**
- [ ] Host 侧 `GET /attestation?nonce=` 路由 + handler；`/health` 增 `attestation_available` 字段。
- [ ] 客户端库 `packages/attestation-verifier`（TS）：解析 + 验签 + nonce + ta_measurement 比对。
- [ ] 发版流程：按 §7.1 设计的参考值分发机制公布 `kms_ta_measurement`（可复现构建）。
- **降级路线 P0'（安全降级,非更优解,仅当 R-1/R-8 均不成立）**：用 TOFU——首次部署登记设备 attest 公钥指纹,发布到 kms.aastar.io/.well-known 和 git,客户端比对登记值。⚠️ 必须诚实标注「信任根是 AirAccount 登记表,非 NXP;这是牺牲『不信任部署方』换可用性的妥协,不是去中心化优势」。

### Phase 2 — 硬件根锚定（视 Phase 0 / R-1 结果，2-3 周）
- [ ] 在 ELE 密钥库生成 attestation/签名 key（`hsm_generate_key`，ECC NIST），用 `hsm_pub_key_attest`(ECDSA attest algo) 出证书；**不再用 OP-TEE 自生成 key**（§2 架构修正）。
- [ ] evidence 携带 ELE `hsm_dev_attest` 输出（`oem_srkh`/`sha_fw`/`lmda_val`/`uid` + ECDSA P-384 签名，值源自 ELE 非 OP-TEE 自填）。
- [ ] **补根（R-1/R-8 缺口）**：确定 pub_key_attest 的 `key_attestation_id`(key B) 如何连 NXP 根——EdgeLock 2GO provision，或确认 dev_attest 设备 key 与 keystore key 之间是否有桥；**此环未闭合则 V1 链验止于「ELE 自签」，须如实降级（见 §9 P0' TOFU）**。
- [ ] 客户端库补链验证 + V4（dev_attest lifecycle/oem_srkh 比对，仅当 R-1 成立才算可信，否则按 §4 V4 标注处理）。
- [ ] 把「已吊销 attest key + 旧 TA measurement 黑名单」绑 RPMB 单调计数器(防降级,见 §5 / R-9);RPMB 不作证据源。

### Phase 3 — 标准化 + 业务绑定 + 链上验证（按需）
- [ ] 评估输出 **PSA Attestation Token（EAT/RFC 9783，COSE）** 格式,用 **Veraison** 做可自部署 verifier。⚠️ **注意:i.MX93 通过 PSA Certified 是合规认证等级,不自动等于「BSP 已暴露可用的 PSA Attestation Token 接口」**——能否真正产出 EAT 需 Phase 0 实测(并入 R-8 周边调研),大概率仍需 fallback 到自定义证据格式。
- [ ] 层 C：把 attestation 绑进 WebAuthn ceremony 与 `SignHash` 响应（默认发生，非可选）。
- [ ] **链上 verifier 合约**（参考 Automata 链上 DCAP / Marlin 链上 Nitro）：让 SuperPaymaster/SuperRelay 在链上确认「KMS 跑在真 TEE」，比纯 SDK 验证更去中心化。

---

## 7. 风险与未决问题登记（给主架构师复核）

| ID | 风险/未决 | 影响 | 状态 |
|---|---|---|---|
| **R-1** | **ELE `hsm_dev_attest` 的 `signature` 用哪把 key 签？有无连 NXP 根、可第三方离线验的证书链？** ⚠️ **已查 RM00284 §3.12：手册只给输出结构 + 函数签名，「Detailed description」一节为空，对签名 key 来源、证书链、如何离线验 NXP 根完全沉默**——API 参考手册不含 key provisioning/信任根。需另查 NXP 安全参考手册 / EdgeLock 2GO / NDA。 | **决定「不信任部署方」能否成立**。不成立则退 TOFU 登记表（P0',§9：安全降级,非去中心化优势）。 | **待验证（RM00284 已查仍未答 → 待 NXP 安全参考手册 / EdgeLock 2GO / 实机）** |
| **R-2** | NXP BSP 的 OP-TEE 是否真的编入了 attestation PTA？社区方案只在 QEMU/RPi3 验证过，**没有 i.MX93 记录**。 | 不成立则 MVP 都做不了，需要自己往 OP-TEE 移植 PTA（重活）。 | **待验证（实机）** |
| **R-3** | TA 内能否直接调 ELE attestation（经 mailbox/SCMI/imx-secure-enclave）？还是只能从 normal world 调（失去 TEE 内取证意义）？ ⚠️ imx-secure-enclave 的 dev_attest/pub_key_attest demo 均为 **normal-world 用户态库**调用,TA 内路径未确认。 | 决定层 A 是否能在 TEE 内安全完成。 | **待验证（实机 + 文档）** |
| **R-4** | TA measurement 每次发版都变，客户端参考值需同步更新。可复现构建能否做到 bit-for-bit 一致（Rust + OP-TEE 工具链）？ | 不一致则客户端无法独立核对 TA hash，参考值只能盲信我们公布的值。 | 待验证（构建实验） |
| **R-5** | 半去中心化张力：硬件根永远是 NXP。是否接受「信 NXP 芯片为真」作为不可消除的中心化前提？是否值得为 i.MX95（PQC、更强 ELE）迁移以增强证明？ | 定位/路线决策。 | **需主架构师拍板** |
| **R-6** | attestation 是否应「默认强制」（每次 Sign 内联）还是「可选握手」？强制增延迟（ELE 调用 + 签名），可选则多数客户端可能不验=形同虚设。 | 安全 vs 性能/体验。 | 需主架构师拍板 |
| **R-7** | nonce freshness 复用现有 TA nonce 设施，但其 thread_local 实现有跨线程 flaky 历史（MEMORY 记录）。attestation 须用会话级/持久存储，勿踩同坑。 | 实现正确性。 | 设计已规避，实现需注意 |
| **R-8** | **ELE 能否背书 OP-TEE 自生成的外部 key？以及能否提供可信 secure-boot 度量？** ⚠️ **已查 RM00284 + 源码,部分坐实**：① `hsm_pub_key_attest`(§3.4.6) **只能 attest「ELE 密钥库内」的 key,无法收 OP-TEE 外部 key** → 原三段式作废,改「key 在 ELE 库内生成 + pub_key_attest 出证书」(§2 架构修正) [官方确证]。② secure-boot 度量 **ELE 确实提供并签名**(`hsm_dev_attest` 的 oem_srkh/sha_fw/lifecycle) → H-2 成立 [官方确证]。③ **残留缺口**：pub_key_attest 的背书 key(key B) 本身是普通库 key,**RM00284 没有内置 NXP 根 attestation key,也没有 device-key→keystore-key 的桥** → 「根如何连 NXP」未解。 | 架构已据此修正;残留缺口与 R-1 合流。 | **部分确证(RM00284)+残留待 EdgeLock 2GO/实机** |
| **R-9** | **attestation key 的吊销/轮换/过期机制?** 设备被攻破或 attest key 泄露后,如何让旧 attest key 失效、不被客户端继续接受?如何防止靠 flash 回滚复活旧 key? | 长期运维安全。无吊销机制=一次泄露永久可伪造。 | **需设计 + 实机(RPMB 计数器锚定,见 §5)** |

### 7.1 参考值（`kms_ta_measurement`）分发机制（M-6）

V3 校验要求 verifier 持有「期望的 TA hash」作为 reference value。这个值怎么可信地分发给客户端/verifier,是独立于硬件能力的设计项,分三档(建议全做,逐级增强可信度):

1. **随 SDK 内置(基线)**：`@aastar/attestation-verifier` 发版时内嵌当前已知良好的 `kms_ta_measurement` 列表(支持多版本并存,滚动升级期)。最简单,但客户端信的是「我们 npm 包里写的值」。
2. **签名清单 + .well-known(增强)**：发布一份 **signed measurement manifest**(JSON,列出 `version → ta_measurement → 构建元数据`,由 AirAccount 发布 key 签名),挂在 `https://kms.aastar.io/.well-known/attestation-measurements.json` 并同步进 git tag。客户端可独立拉取核对,且签名防篡改。⚠️ 这把发布 key 又是一个需管理/可轮换的信任锚,纳入 R-9 的 key 治理范畴。
3. **可复现构建 + 公开核对(最强,对应 R-4)**：CI 用可复现构建产出 TA,任何人 clone 源码 + 同工具链能 bit-for-bit 重算出同一 `ta_measurement`,核对 manifest。这样参考值**不需要信任 AirAccount**,只需信任「源码 = 公开的那份」。R-4 是这一档能否成立的前提。
4. **(未来,Phase 3)链上发布**：把 signed manifest 的 hash 上链,让链上 verifier 合约与依赖方(SuperPaymaster/SuperRelay)读同一可信源,去除对 kms.aastar.io 的可用性依赖。

**与 §9 去中心化的关系**：第 3 档是关键——它让「TA 真实性」的判断不依赖信任部署方,只依赖「源码公开 + 构建可复现」,这是参考值分发能进「可去中心化」栏的前提。

---

## 8. i.MX95 迁移评估（针对 attestation）

| 维度 | i.MX93（当前） | i.MX95 | 对 attestation 的意义 |
|---|---|---|---|
| ELE | 标准 ELE（S401），已过 **PSA Certified 认证**(合规等级,≠ 已暴露可用 attestation token 接口) | EdgeLock Secure Enclave **Advanced Profile** | MX95 实时消息签名、更强 RoT，理论上 attestation 链路更完整 [较可信] |
| PQC | 无明确 PQC | **后量子 RoT，hybrid ML-DSA + ECDSA** 签固件 | 长期资产托管面对「先存后破」量子威胁时，MX95 的 PQC 签名更抗未来攻击 [确证(NXP宣传)] |
| OP-TEE / BSP | LF 6.18，已跑通 | 同体系，BSP 更新 | 迁移成本主要是 BSP/OP-TEE 重编 + ELE API 版本差异 |
| secp256k1 | **不支持**（RM00284 §3.2 `hsm_key_type_t` 仅列 ECC_NIST/BP_R1/MONTGOMERY/TWISTED_EDWARDS/BP_T1/RSA/AES/SM4/HMAC/CHACHA20/DERIVE，**无 secp256k1** [官方手册确证]） | **大概率仍不支持**（RM00284 覆盖 MX95，同一 key type 集合）[较可信] | **attestation 不改变这个约束**——证明的是「TA 在真 TEE 跑」，私钥仍 k256 软件签 + RPMB 防回滚；ELE 内 attest/身份 key 只能用 P-256/384 等 NIST 曲线 |
| 迁移性价比 | — | — | **仅为 attestation 不值得现在迁**：MX93 已 PSA Certified、ELE 有设备信息/attestation 原语，足够做 Phase 1-2。MX95 的 PQC + Advanced Profile 是「主网放大资金量 + 长期抗量子」时的升级项，非 #37 阻塞项。 |

**结论**：#37 在 i.MX93 上做到 Phase 1-2（TA 度量可验 + ELE 锚定）即满足「主网前强烈建议」的需求。i.MX93 已过 PSA Certified 认证、ELE 有设备信息/attestation 原语(具体能力以 R-8 实测为准),足够支撑 Phase 1-2。i.MX95 迁移留给「后量子 + 生产级认证」的未来里程碑，与既有 `docs/migration-to-MX95.md` 的「先 MX93、量变才升 MX95」结论一致。secp256k1 不支持的硬约束两代都在，attestation 不解决也不受其影响。

---

## 9. 与半去中心化定位的权衡（明确写给生态）

| 维度 | 中心化部分（诚实承认） | 可去中心化部分（我们做） |
|---|---|---|
| 硬件根 | NXP Root CA（不可消除） | — |
| 设备证书 | 理想:NXP / EdgeLock 2GO 提供连根证书链（R-1） | — (这一项无法去中心化;NXP 是唯一能背书芯片真伪的实体) |
| Verifier | — | 验证逻辑开源、客户端本地验 / 自部署 Veraison / 链上合约 |
| 参考值（TA hash） | — | 可复现构建 + 公开发布(§7.1 第 3 档),任何人可核对 |
| 验证发生位置 | — | 客户端 SDK / 链上，不依赖我们的服务器自证 |

⚠️ **关于 TOFU 的定位澄清(M-5)**：当 R-1/R-8 不成立、拿不到 NXP 连根证书链时,会退到 TOFU 登记表(P0')。**这不是「更去中心化所以更好」——恰恰相反,它是安全降级**:放弃了「客户端无需信任部署方即可验真」这一最强属性,改为「首次见到即信任 AirAccount 登记的那把 key」。它牺牲安全换可用,只在硬件给不出更强锚定时作为无奈 fallback,绝不是相对 NXP 连根方案的优势项。

**一句话**：我们无法、也不假装能去中心化「NXP 造的芯片是真的」这一前提；我们能做到的是**验证逻辑与参考值完全开放、可自验、可上链**。这与 AirAccount「代码开源可 fork、passkey 锚 rpId 域名、无 admin」的半去中心化模型一致——但要诚实区分:可去中心化的是验证侧,不是硬件信任根。

---

## 10. 立即行动项（Phase 0 清单，可直接执行）

1. 串口登 MX93，`ls` OP-TEE TA 目录 / 查 BSP 构建配置，确认 attestation PTA(`39800861-...`)是否编入（R-2）。
2. **R-1 收口（RM00284 已查未答）**：找 NXP 安全参考手册 / EdgeLock 2GO 文档 / NDA 渠道,确认 `hsm_dev_attest` 的签名 key 是否 NXP 工厂注入、有无连 NXP 根的可离线验证书链。这是「不信任部署方」成立与否的唯一阻塞项。
3. 启 NVM-Daemon,实测 `hsm_pub_key_attest`(ECDSA);评估 EdgeLock 2GO 能否 provision NXP 背书 key 当 `key_attestation_id`(补 R-8 残留缺口)。
4. 输出 `docs/design/37-attestation-hw-findings.md`，据 R-1 结论选「主路线(NXP 连根)」或「降级 P0'(TOFU,安全妥协)」。
5. 设计 attest key 吊销/轮换方案(R-9)。
6. 主架构师就 R-5（是否为 attestation 迁 MX95）、R-6（强制内联 vs 握手+绑定）拍板。

---

## 11. 权威来源 / Authoritative Sources

> 安全核心设计:每个技术声明须有官方权威来源。本节区分「已用官方源逐行核对 [官方源确证]」与「需指定官方源/真机核对 [待验证]」,后者是 Phase 0 阻塞项。

### A. 已用官方源逐行核对 [官方源确证]（本设计直接 WebFetch / 实测）
- **OP-TEE attestation PTA 头文件** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/lib/libutee/include/pta_attestation.h
  - 坐实:真实 PTA UUID = `39800861-182a-4720-9b67-2bcd622bc0b5`(旧 plan 的 `731e279e-...` 系臆造);仅 4 命令(`GET_PUBKEY`=0x0 / `GET_TA_SHDR_DIGEST`=0x1 / `HASH_TA_MEMORY`=0x2 / `HASH_TEE_MEMORY`=0x3);**无任何命令签外部公钥/数据**。→ §2 H-1 硬约束依据。
- **OP-TEE attestation PTA 实现** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/core/pta/attestation.c
  - 坐实:attest RSA key 首次使用设备自生成(`load_key` 失败→`generate_key()`→`crypto_acipher_gen_rsa_key()`),**无证书链/无 vendor CA/无硬件根锚定**;私钥存 secure storage(`sec_storage_obj_write(..., TEE_STORAGE_PRIVATE, ...)`,通常 RPMB)。→ §2 层 B 自签、必须靠 ELE 锚定的依据。
- **本地 `third_party/teaclave-trustzone-sdk`(已 `find` 实测)** — **不含 attestation 示例**(`examples/` 无 `*attest*`),repo 内无 `pta_attestation.h`/`attestation.c`,**无 optee_os 源码**。→ attestation PTA 权威代码只在上方 `OP-TEE/optee_os` 官方仓库,**不要在本地 SDK 找**。

### B. NXP ELE：已用本地一手材料逐项核对 [官方手册确证 + 官方代码确证]（2026-06-13）
- **NXP RM00284 EdgeLock Enclave HSM API, Rev 4.3（本地 PDF `RM00284.pdf`，139 页）**
  - §3.12 Dev attest（p65-66）：`hsm_dev_attest` 输出 uid/soc_id/`ssm_state`/`lmda_val`(lifecycle)/`sha_rom_patch`/`sha_fw`/`oem_srkh`(OEM 安全启动根 hash)/`oem_pqc_srkh`/nounce+rsp_nounce/`info_buf`/`signature`。→ H-2 secure-boot 度量来源确证。
  - §3.4.6（p39）：`hsm_pub_key_attest` / `hsm_do_pub_key_attest`「attest the public key of an asymmetric key **present in the ELE FW key storage**」；attest algo = CMAC / ECDSA_SHA224-512（§3.4.5.1, p38）。→ R-8 架构修正确证（只 attest 库内 key）。
  - §3.2（p9）`hsm_key_type_t`：无 secp256k1。§3.13 Dev Info（p67）`hsm_lmda_val_t` lifecycle 枚举。
  - ⚠️ **§3.12.1「Detailed description」为空**：RM00284 是 API 参考，**不含 dev_attest 签名 key 的 provenance、证书链、如何离线验 NXP 根** → R-1 在本手册无解，需 NXP 安全参考手册 / EdgeLock 2GO / NDA。
- **NXP imx-secure-enclave（本地 clone `third_party/imx-secure-enclave/`）[官方代码确证]**
  - `include/hsm/internal/hsm_dev_attest.h` / `src/common/sab_msg/sab_dev_attest.c`：dev_attest 走 `ROM_DEV_ATTEST_REQ`(0xDB, ROM 级)；`DEV_ATTEST_SIGN_SIZE=96` → **ECDSA P-384** 签名；oem_srkh 64B、rom/fw sha 各 32B。
  - `include/hsm/internal/hsm_pub_key_attest.h` / `test/common/test_pub_key_attest.c`：`key_identifier`(被 attest 的库内 key) + `key_attestation_id`(做背书的库内 key,demo 用 AES-CMAC) → 输出 `certificate`(signed TLV)；`#ifdef PSA_COMPLIANT` 才编入。→ R-8「背书 key 本身也是普通库 key、无内置 NXP 根 key」的依据。
- 在线镜像（同物，便于引用）— https://www.nxp.com/docs/en/reference-manual/RM00284.pdf ・ https://github.com/nxp-imx/imx-secure-enclave/

### B'. 仍待外部一手源 [待 NXP 安全参考手册 / EdgeLock 2GO / 实机]
- **R-1**：dev_attest 签名 key 是否 NXP 工厂注入 + 连 NXP 根证书链（RM00284 未涵盖）。
- **R-8 残留**：pub_key_attest 的 `key_attestation_id` 如何连 NXP 根（EdgeLock 2GO provision？device-key→keystore-key 桥？手册均未见）。
- **R-3**：OP-TEE TA 内能否调 ELE attestation（demo 均 normal-world 用户态）。
- EdgeLock 2GO provisioning — https://www.nxp.com/products/security-and-authentication/authentication/edgelock-2go:EDGELOCK-2GO

### C. 标准 / 参考(公开)
- RFC 9334 RATS 架构 — https://www.rfc-editor.org/rfc/rfc9334.html
- RFC 9783 Arm PSA Attestation Token(EAT) — https://www.rfc-editor.org/rfc/rfc9783.html
- Veraison 验证服务 — https://www.veraison-project.org/book/services/overview.html
- 社区 optee-ra(端到端参考,非一手) — https://github.com/iisec-suzaki/optee-ra
- Web3 链上 TEE 证明先例 — https://blog.marlin.org/on-chain-verification-of-aws-nitro-enclave-attestations
- 更完整的方案级来源见配套调研文档 `37-remote-attestation-research.md` 的「权威来源」节。
