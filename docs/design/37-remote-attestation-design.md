# AirAccount KMS 远程证明设计（i.MX93 落地）

> 创建时间：2026-06-13
> 关联 Issue：#37 TEE 远程证明
> 文档性质：落地设计 / 证书链架构 / 分期路线 / 风险登记
> 前置阅读：`docs/design/37-remote-attestation-research.md`（业界调研，本文不重复论证）

---

## 0. 一句话目标与诚实边界

**目标**：让客户端能**密码学验证**「AirAccount KMS 的签名响应，确实来自一台真实 NXP i.MX93、跑着未篡改的 OP-TEE、加载的是我们发布的那个特定 KMS TA 二进制」——而不是攻击者控制的普通 host 进程在伪造。

**诚实边界（先讲清楚）**：
- 远程证明的信任根**必然落在 NXP**（你无法证明一块芯片是真 i.MX93 而不绕过 NXP 的硬件根）。半去中心化能去中心化的是 **Verifier 逻辑、参考值分发、验证发生的位置**，**不是硬件根**。
- 本设计的安全等级**强依赖两个尚未证实的前提**：① 见 §7 **R-1** —— ELE 是否提供「NXP 工厂注入的设备唯一私钥 + 可第三方验证的证书链」；② 见 §7 **R-8** —— ELE 能否「背书/锚定 OP-TEE 自生成的 attest key」并「提供可信的 secure-boot 度量值」。**这两个前提不成立时，MVP 仍有意义（证明「进了 TEE 且是这个 TA」），但「不信任部署方」这一最强目标要打折，并退到 §6 P0' 的 TOFU 安全降级**。

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

**关键设计点（强不确定，勿当既定事实）**：理想链路是层 B 的 attestation key（OP-TEE 自生成）被**层 A 背书一次**，从而连成「NXP 根 → 设备 → OP-TEE attest key → TA 度量证据」。

⚠️ **H-1 已被官方代码坐实为硬约束 [官方源确证 + 待验证]**：这条链路的「OP-TEE 侧」部分已无悬念——
- OP-TEE attestation key **设备自生成、零证书链、零硬件锚定**（`core/pta/attestation.c`：`generate_key()`→`crypto_acipher_gen_rsa_key()`，无任何 anchoring）。**[官方源确证]**
- OP-TEE attestation PTA **只有 4 个命令，没有任何一个对 caller 传入的外部公钥/数据签名**（`pta_attestation.h`：`GET_PUBKEY`/`GET_TA_SHDR_DIGEST`/`HASH_TA_MEMORY`/`HASH_TEE_MEMORY`，全部签 `nonce|自身度量`）。**[官方源确证]**

→ **结论（官方坐实）：把自生成 attest key 锚回硬件根，OP-TEE 侧自己做不到，动作只能发生在 ELE 侧。** 而「ELE 能不能背书一把外部公钥」属于 ELE 能力，**官方手册 RM00284 本环境拿不到，无法确证**：ELE 的 device attestation **通常只签它自己的内部度量**（uid + ROM hash + FW measurement），**未必接受对外部公钥签名**。若 ELE 不提供 key attestation / 外部数据签名能力，则「ELE 背书 OP-TEE attest pubkey」**做不到**。这就是 **R-8 的死结**——一边（OP-TEE 不签外部 key）已被官方代码确证，另一边（ELE 能否补位）[待 RM00284 + 实机验证]。单列复核点 **R-8**（§7）。

R-8 未定前，层 A→B 有三条候选,按优先级:
1. **(R-8=是,最佳) ELE 直接背书外部公钥**：ELE 提供 sign-external-data / key-attestation 原语，直接对 OP-TEE attest pubkey 签名，链路如上图。
2. **(R-8=否,折中) attest key 从 ELE 背书的 HUK/device key 派生**：不让 ELE 签外部 key，而是让 OP-TEE 的 attest key **确定性派生自 ELE 持有的 device key / HUK**（OP-TEE secure storage 本就用 HUK 加密）。客户端信任链变成「设备 device key 决定了唯一的 attest key，攻击者无法在别处复现同一把 key」。⚠️ 这要求 HUK 派生关系本身可被外部验证或锚定,可行性 [待验证],并入 R-8。
3. **(都不成立,无奈降级) TOFU 登记 attest pubkey**：见 §3 降级说明与 §6 P0'。这是**安全降级**,不是更优解。

⚠️ **候选 1 的「背书调用形态」必须 Phase 0 落实（H-1）**：「ELE 背书外部公钥」在 RM00284 里可能对应**三种语义完全不同**的原语，安全含义差很大，Phase 0 须确认 ELE 到底提供哪一种（并入 R-8）：
- **(i) 签发证书 / 处理 CSR**：ELE 用设备唯一私钥对「OP-TEE attest pubkey + 属性」签发一张 X.509 式证书。**最强**——产出可独立验证、可链到 NXP 根的证书，正是上图理想链路。
- **(ii) 对任意 blob 签名（sign-arbitrary-data）**：ELE 把设备私钥当通用签名 oracle，对喂入的 pubkey 字节签一下。**够用但要小心**——等于设备私钥变成通用签名能力，须确认 ELE 是否允许（多数 HSM 的 attestation key 被限制为只签固定 attestation 结构，**不做通用 oracle**）。
- **(iii) 把 OP-TEE pubkey 塞进 ELE evidence 由 device attestation 间接覆盖**：不直接签外部 key，而是让 OP-TEE pubkey 作为 ELE device attestation 的一个输入字段被一并度量+签。**最弱/最易误解**——只证明「出具 evidence 时这把 pubkey 在场」，绑定语义弱，须评估是否足以支撑 V1 链验。
若 ELE 三者都不提供，候选 1 不成立，退候选 2（派生）或候选 3（TOFU）。

社区 optee-ra 把「给 attest key 引入证书/HSM 锚定」列为 future work——AirAccount 必须自己补,且补法取决于 R-8 的实测结论。

---

## 3. 证书链架构（ASCII）

```
                    ┌─────────────────────────────────────┐
                    │  NXP Root CA  (离线, 公开指纹)         │   ← 中心化锚点(不可消除)
                    └───────────────┬─────────────────────┘
                                    │ 签发
                    ┌───────────────▼─────────────────────┐
                    │  i.MX93 设备唯一证书 (ELE 持有)        │   ← 绑芯片 UID
                    │  公钥 = ELE 设备公钥                   │   ⚠️ 是否存在/可公开验 = R-1
                    │  ELE device attestation 同时产出:     │
                    │   secure_boot_state(ELE 度量 boot 链) │   ← 唯一可信来源, 见 H-2
                    └───────────────┬─────────────────────┘
                                    │ 层A: 把 attest key 锚回 device key
                                    │ ⚠️ 实现方式取决于 R-8(背书/派生/TOFU)
                    ┌───────────────▼─────────────────────┐
                    │  OP-TEE Attestation Key (+证书/背书)   │   ← 锚定方式见 §2 三候选
                    │  (内容: optee attest pubkey,          │
                    │   optee_version)                      │   ← 不含 secure_boot_state
                    └───────────────┬─────────────────────┘
                                    │ 层B: attestation key 对下签名
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

**`secure_boot_state` 的唯一可信来源（H-2 统一口径）**：该字段**必须来自 ELE 对 boot chain 的可信度量**（随 ELE device attestation 一并签出），**严禁由 OP-TEE/TA 自行填写**——否则一条被攻破、篡改了启动链的设备完全可以自报 `verified`，§4 的 V4 校验就形同虚设。evidence 里的 `secure_boot_state` 只是把「ELE 在层 A 签过的那个值」原样透传,其可信度由层 A 的 ELE 签名保证,不引入新的信任。⚠️ **ELE 是否真能提供可信的 secure-boot 度量值,并入 R-8 复核**;若不能,V4 不可信,必须降级处理(见 §4 V4 标注)。

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
  │   V4. secure_boot_state == verified  ⚠️仅当值源自 ELE(R-8)才可信;          │
  │       ELE 无可信 boot 度量时此项不可信, 须降级(见下)                        │
  │   V5. optee_version 在允许列表内                                            │
  │   V6. (层C) 业务摘要 == 本次请求摘要            (绑定本次操作)              │
  │   全过 → 信任后续/本次 KMS 操作; 任一失败 → 拒绝并告警                       │
```

**V4 降级标注 [待验证, H-2]**：`secure_boot_state` 只有在「值由 ELE 可信度量产出并经层 A 签名」时才有意义。若 Phase 0 实测发现 ELE 不提供可信 secure-boot 状态（R-8 的一部分），则 V4 **不可信**：要么从证据中移除该字段（避免给客户端虚假安全感），要么明确标注「secure_boot 未经硬件背书,仅供参考」,绝不能让客户端把它当通过条件。

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

### Phase 0 — 硬件能力摸底（必须先做，1 周，无代码）
- [ ] 在 MX93 串口实机确认：NXP BSP 的 OP-TEE 是否带 **attestation PTA**（`pta_attestation.h` 对应 PTA UUID）。**[这是整个方案的地基，先证实再投入]**
- [ ] 实机跑 ELE：`nxpele get-info` 看 uid/ROM hash/FW hash；查 RM00284 确认是否有 **device attestation 命令 + 签名密钥来源 + 证书链**（对应调研 §3.2 三问 / 本文 R-1）。
- [ ] 确认 TA 内能否经 imx-secure-enclave/SCMI 调到 ELE attestation（还是只能从 REE 调）。
- [ ] 产出：`docs/design/37-attestation-hw-findings.md`，明确 R-1/R-2/R-3 的真实答案，决定走主路线还是降级路线 P0'。

### Phase 1 — MVP：TA 度量 + nonce 端到端可验（2 周）
> 即使 Phase 0 发现 ELE 给不出 NXP 证书链，**这一阶段照样有价值**：先证明「确实进了 TEE 且跑的是这个 TA」。
- [ ] proto 增 `GetAttestation = <next id>` 命令（`kms/proto`）。
- [ ] TA 侧 `kms/ta/src/attestation.rs`：调 attestation PTA 取 `GET_TA_SHDR_DIGEST`，组装 evidence（nonce + ta_measurement + optee_version + ree_time），用 OP-TEE attestation key 签。**注意：用 `optee_utee::Time::ree_time()` 取时间，禁用 `SystemTime::now()`（会 panic，见 MEMORY）。**
- [ ] Host 侧 `GET /attestation?nonce=` 路由 + handler；`/health` 增 `attestation_available` 字段。
- [ ] 客户端库 `packages/attestation-verifier`（TS）：解析 + 验签 + nonce + ta_measurement 比对。
- [ ] 发版流程：按 §7.1 设计的参考值分发机制公布 `kms_ta_measurement`（可复现构建）。
- **降级路线 P0'（安全降级,非更优解,仅当 R-1/R-8 均不成立）**：用 TOFU——首次部署登记设备 attest 公钥指纹,发布到 kms.aastar.io/.well-known 和 git,客户端比对登记值。⚠️ 必须诚实标注「信任根是 AirAccount 登记表,非 NXP;这是牺牲『不信任部署方』换可用性的妥协,不是去中心化优势」。

### Phase 2 — 硬件根锚定（视 Phase 0 / R-8 结果，2-3 周）
- [ ] 按 R-8 实测结论选定层 A→B 锚定方式(§2 三候选:ELE 背书外部 key / 从 ELE device key 派生 / TOFU 降级)并实现。
- [ ] evidence 增 `secure_boot_state`(**值源自 ELE device attestation,非 OP-TEE 自填**)、`device_uid`；cert_chain 补设备证书(若 R-1 成立)。
- [ ] 客户端库补链验证 + V4 secure_boot 校验(**仅当 ELE 提供可信 boot 度量时启用,否则按 §4 V4 降级**)。
- [ ] 把「已吊销 attest key + 旧 TA measurement 黑名单」绑 RPMB 单调计数器(防降级,见 §5 / R-9);RPMB 不作证据源。

### Phase 3 — 标准化 + 业务绑定 + 链上验证（按需）
- [ ] 评估输出 **PSA Attestation Token（EAT/RFC 9783，COSE）** 格式,用 **Veraison** 做可自部署 verifier。⚠️ **注意:i.MX93 通过 PSA Certified 是合规认证等级,不自动等于「BSP 已暴露可用的 PSA Attestation Token 接口」**——能否真正产出 EAT 需 Phase 0 实测(并入 R-8 周边调研),大概率仍需 fallback 到自定义证据格式。
- [ ] 层 C：把 attestation 绑进 WebAuthn ceremony 与 `SignHash` 响应（默认发生，非可选）。
- [ ] **链上 verifier 合约**（参考 Automata 链上 DCAP / Marlin 链上 Nitro）：让 SuperPaymaster/SuperRelay 在链上确认「KMS 跑在真 TEE」，比纯 SDK 验证更去中心化。

---

## 7. 风险与未决问题登记（给主架构师复核）

| ID | 风险/未决 | 影响 | 状态 |
|---|---|---|---|
| **R-1** | **ELE attestation 的签名密钥是否为 NXP 工厂注入的设备唯一私钥，且有可第三方离线验证的证书链（NXP Root → 设备证）？还是必须走 EdgeLock 2GO 云 provisioning？** | **决定能否实现「不信任部署方」这一最强目标**。不成立则退到 TOFU 登记表（P0',§9 澄清:这是安全降级,**非**去中心化优势,信任根变成 AirAccount）。 | **待验证（RM00284 + 实机）** |
| **R-2** | NXP BSP 的 OP-TEE 是否真的编入了 attestation PTA？社区方案只在 QEMU/RPi3 验证过，**没有 i.MX93 记录**。 | 不成立则 MVP 都做不了，需要自己往 OP-TEE 移植 PTA（重活）。 | **待验证（实机）** |
| **R-3** | TA 内能否直接调 ELE attestation（经 mailbox/SCMI/imx-secure-enclave）？还是只能从 normal world 调（失去 TEE 内取证意义）？ | 决定层 A 是否能在 TEE 内安全完成。 | **待验证（实机 + 文档）** |
| **R-4** | TA measurement 每次发版都变，客户端参考值需同步更新。可复现构建能否做到 bit-for-bit 一致（Rust + OP-TEE 工具链）？ | 不一致则客户端无法独立核对 TA hash，参考值只能盲信我们公布的值。 | 待验证（构建实验） |
| **R-5** | 半去中心化张力：硬件根永远是 NXP。是否接受「信 NXP 芯片为真」作为不可消除的中心化前提？是否值得为 i.MX95（PQC、更强 ELE）迁移以增强证明？ | 定位/路线决策。 | **需主架构师拍板** |
| **R-6** | attestation 是否应「默认强制」（每次 Sign 内联）还是「可选握手」？强制增延迟（ELE 调用 + 签名），可选则多数客户端可能不验=形同虚设。 | 安全 vs 性能/体验。 | 需主架构师拍板 |
| **R-7** | nonce freshness 复用现有 TA nonce 设施，但其 thread_local 实现有跨线程 flaky 历史（MEMORY 记录）。attestation 须用会话级/持久存储，勿踩同坑。 | 实现正确性。 | 设计已规避，实现需注意 |
| **R-8** | **ELE 是否提供「对外部传入的公钥/数据签名」或「key attestation / key-import-with-attestation」原语?以及是否提供可信的 secure-boot 度量值?** ELE 的 device attestation 常常只签它自己的内部度量(uid/ROM/FW),未必能背书外部的 OP-TEE attest pubkey。 | **决定 §2 层 A→B 锚定走哪条候选**:能背书外部 key→候选1(最佳);不能但能派生→候选2;都不行→TOFU 降级。同时决定 H-2 的 `secure_boot_state` 是否可信(影响 V4)。 | **待验证（RM00284 + 实机,阻塞 Phase 2）** |
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
| secp256k1 | **不支持**（实测确认，以太坊私钥仍软件管理） | **大概率仍不支持**（ELE crypto 集合相近）[待验证] | **attestation 不改变这个约束**——证明的是「TA 在真 TEE 跑」，私钥仍 k256 软件签 + RPMB 防回滚 |
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

1. 串口登 MX93，`ls` OP-TEE TA 目录 / 查 BSP 构建配置，确认 attestation PTA 是否编入（R-2）。
2. 跑 `nxpele get-info`，记录 uid/ROM hash/FW hash；下载并精读 **RM00284**,重点确认四件事:device attestation 命令 + 签名密钥来源 + 证书链(R-1/R-3),以及**是否提供「对外部公钥/数据签名 / key attestation」原语 + 可信 secure-boot 度量值(R-8)**。
3. 输出 `docs/design/37-attestation-hw-findings.md`，据此:① 选层 A→B 锚定候选(R-8);② 选「主路线(NXP 连根)」或「降级 P0'(TOFU,安全妥协)」。
4. 设计 attest key 吊销/轮换方案(R-9)。
5. 主架构师就 R-5（是否为 attestation 迁 MX95）、R-6（强制内联 vs 握手+绑定）拍板。

---

## 11. 权威来源 / Authoritative Sources

> 安全核心设计:每个技术声明须有官方权威来源。本节区分「已用官方源逐行核对 [官方源确证]」与「需指定官方源/真机核对 [待验证]」,后者是 Phase 0 阻塞项。

### A. 已用官方源逐行核对 [官方源确证]（本设计直接 WebFetch / 实测）
- **OP-TEE attestation PTA 头文件** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/lib/libutee/include/pta_attestation.h
  - 坐实:真实 PTA UUID = `39800861-182a-4720-9b67-2bcd622bc0b5`(旧 plan 的 `731e279e-...` 系臆造);仅 4 命令(`GET_PUBKEY`=0x0 / `GET_TA_SHDR_DIGEST`=0x1 / `HASH_TA_MEMORY`=0x2 / `HASH_TEE_MEMORY`=0x3);**无任何命令签外部公钥/数据**。→ §2 H-1 硬约束依据。
- **OP-TEE attestation PTA 实现** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/core/pta/attestation.c
  - 坐实:attest RSA key 首次使用设备自生成(`load_key` 失败→`generate_key()`→`crypto_acipher_gen_rsa_key()`),**无证书链/无 vendor CA/无硬件根锚定**;私钥存 secure storage(`sec_storage_obj_write(..., TEE_STORAGE_PRIVATE, ...)`,通常 RPMB)。→ §2 层 B 自签、必须靠 ELE 锚定的依据。
- **本地 `third_party/teaclave-trustzone-sdk`(已 `find` 实测)** — **不含 attestation 示例**(`examples/` 无 `*attest*`),repo 内无 `pta_attestation.h`/`attestation.c`,**无 optee_os 源码**。→ attestation PTA 权威代码只在上方 `OP-TEE/optee_os` 官方仓库,**不要在本地 SDK 找**。

### B. 需官方源核对,本环境拿不到 → Phase 0 阻塞项 [待 RM00284 + 实机验证]
- **NXP RM00284 EdgeLock Enclave HSM API**(ELE attestation 全部能力的权威来源,**需 NXP 账号注册,本环境无法访问**) — https://www.nxp.com/docs/en/reference-manual/RM00284.pdf
  - 待核对:R-1(签名 key 是否工厂注入 + 有无连 NXP 根证书链)、R-8(能否对外部公钥/数据签名 + 能否提供可信 secure_boot_state)、R-3(TA 内能否调 ELE attestation)。
- ELE 用户态参考实现(代码可读,语义仍以 RM00284 为准) — https://github.com/nxp-imx/imx-secure-enclave/ ・ https://github.com/nxp-imx-support/imx-ele-demo

### C. 标准 / 参考(公开)
- RFC 9334 RATS 架构 — https://www.rfc-editor.org/rfc/rfc9334.html
- RFC 9783 Arm PSA Attestation Token(EAT) — https://www.rfc-editor.org/rfc/rfc9783.html
- Veraison 验证服务 — https://www.veraison-project.org/book/services/overview.html
- 社区 optee-ra(端到端参考,非一手) — https://github.com/iisec-suzaki/optee-ra
- Web3 链上 TEE 证明先例 — https://blog.marlin.org/on-chain-verification-of-aws-nitro-enclave-attestations
- 更完整的方案级来源见配套调研文档 `37-remote-attestation-research.md` 的「权威来源」节。
</content>
