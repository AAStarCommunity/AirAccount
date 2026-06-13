# AirAccount KMS 远程证明设计（i.MX93 落地）

> 创建时间：2026-06-13（本地机器时间 `date '+%Y-%m-%d'`）
> 关联 Issue：#37 TEE 远程证明
> 文档性质：落地设计 / 证书链架构 / 分期路线 / 风险登记
> 前置阅读：`docs/design/37-remote-attestation-research.md`（业界调研，本文不重复论证）

---

## 0. 一句话目标与诚实边界

**目标**：让客户端能**密码学验证**「AirAccount KMS 的签名响应，确实来自一台真实 NXP i.MX93、跑着未篡改的 OP-TEE、加载的是我们发布的那个特定 KMS TA 二进制」——而不是攻击者控制的普通 host 进程在伪造。

**诚实边界（先讲清楚）**：
- 远程证明的信任根**必然落在 NXP**（你无法证明一块芯片是真 i.MX93 而不绕过 NXP 的硬件根）。半去中心化能去中心化的是 **Verifier 逻辑、参考值分发、验证发生的位置**，**不是硬件根**。
- 本设计的安全等级**强依赖一个尚未证实的前提**（见 §7 R-1）：ELE 是否提供「NXP 工厂注入的设备唯一私钥 + 可第三方验证的证书链」。**这个前提不成立时，MVP 仍有意义（证明「进了 TEE 且是这个 TA」），但「不信任部署方」这一最强目标要打折**。

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
│  → 把层 B 的 attestation key「介绍」给 NXP 信任根        │
│  签名：ELE 设备密钥（待验证是否连 NXP 根，见 §7 R-1）    │
└────────────────────────────────────────────────────────┘
```

**关键设计点**：层 B 的 attestation key（OP-TEE 自生成）必须被**层 A 签名背书一次**（"ELE 设备身份 endorse OP-TEE attestation 公钥"），这样客户端验证时才能从「NXP 根 → 设备 → OP-TEE attestation key → TA 度量证据」一路连起来。这正是社区 optee-ra 列为「future work」、而 AirAccount 必须自己补的那一环。

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
                    └───────────────┬─────────────────────┘
                                    │ 层A: ELE 对下签名背书
                    ┌───────────────▼─────────────────────┐
                    │  OP-TEE Attestation Key 证书          │   ← 一次性背书
                    │  (内容: optee attest pubkey,          │
                    │   secure_boot_state, optee_version)   │
                    └───────────────┬─────────────────────┘
                                    │ 层B: attestation key 对下签名
                    ┌───────────────▼─────────────────────┐
                    │  Attestation Evidence (每次请求)       │
                    │  {  nonce(客户端给),                   │
                    │     kms_ta_measurement = SHA256(TA),  │
                    │     业务摘要(层C),                     │
                    │     timestamp(ree_time) }             │
                    └───────────────┬─────────────────────┘
                                    │ 交付
                    ┌───────────────▼─────────────────────┐
                    │  客户端 / Verifier / (未来)链上合约    │
                    │  自顶向下验链 + 比对参考值 + 校 nonce  │
                    └─────────────────────────────────────┘
```

吊销策略：设备证书长期有效；attestation key 证书可短期（每次重启/定期重生成并由 ELE 重新背书）；evidence 一次性（nonce 防重放）。

⚠️ 若 R-1 不成立（ELE 给不出连 NXP 根的设备证书），则「NXP Root CA → 设备证书」两层塌缩，信任根退化为 **AirAccount 自己发布的「设备登记表」**（首次部署时记录设备公钥指纹，类似 TOFU / SSH known_hosts）。这是 §6 的降级路线 P0'。

---

## 4. 客户端验证流程

```
客户端                          KMS Host (CA)              TEE (KMS TA)        ELE
  │  1. 生成 nonce(32B 随机)        │                          │                │
  │  2. GET /attestation?nonce=… ─►│                          │                │
  │     (或在 Sign 响应里带证据)    │── InvokeCommand ────────►│                │
  │                                 │   GetAttestation(nonce)  │                │
  │                                 │                          │ 取 TA 度量(PTA)│
  │                                 │                          │ 读 secure_boot │
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
  │   V4. secure_boot_state == verified             (设备未被改启动链)          │
  │   V5. optee_version 在允许列表内                                            │
  │   V6. (层C) 业务摘要 == 本次请求摘要            (绑定本次操作)              │
  │   全过 → 信任后续/本次 KMS 操作; 任一失败 → 拒绝并告警                       │
```

两种交付模式（建议都支持）：
- **独立端点 `GET /attestation?nonce=`**：客户端在「会话开始」验一次（轻量，握手语义）。
- **签名内联**：`Sign`/`SignHash` 响应里直接带 evidence（nonce = 请求里带的 challenge），把证明绑进每一次关键签名（强，零信任）。

---

## 5. 与现有机制的结合

| 现有机制 | 如何结合 |
|---|---|
| **RSA-4096 TA 签名**（OP-TEE 4.8 NXP key 签 TA 镜像） | 这是「TA 加载时被 OP-TEE 校验」的基础；attestation PTA 的 `GET_TA_SHDR_DIGEST` 度量的正是 signed header。两者互补：加载校验防跑非法 TA，attestation 把「跑的是哪个 TA」告诉远端。 |
| **WebAuthn challenge binding（nonce 已下沉 TA）** | 复用同一套 TA 内一次性 nonce 设施做 evidence freshness；甚至可把 attestation evidence 绑进 WebAuthn ceremony（注册/认证时一并产出证据，层 C）。⚠️ 注意现有 nonce 用了 thread_local 跨 TA 线程有 flaky 记录（见 MEMORY），attestation nonce 不要复用同一坑，需用持久化/会话级存储。 |
| **RPMB 防回滚** | secure_boot_state、optee_version、设备身份不易回滚；attestation key 与其证书可存 RPMB 防回滚，防「降级到旧的、已知漏洞 TA 还能出具看似合法的证据」。建议把「已撤销的旧 TA measurement 黑名单」也绑 RPMB 单调计数器。 |
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
- [ ] 发版流程：CI 输出并公布 `kms_ta_measurement`（可复现构建，供客户端硬编码参考值）。
- **降级路线 P0'（若无 NXP 链）**：用 TOFU——首次部署登记设备 attestation 公钥指纹，发布到 kms.aastar.io/.well-known 和 git，客户端比对登记值。诚实标注「信任根是 AirAccount 登记表，非 NXP」。

### Phase 2 — 硬件根锚定（视 Phase 0 结果，2-3 周）
- [ ] TA 内调 ELE device attestation，让 ELE 设备密钥**背书 OP-TEE attestation 公钥**（层 A）。
- [ ] evidence 增 `secure_boot_state`、`device_uid`；cert_chain 补设备证书。
- [ ] 客户端库补「NXP Root → 设备证 → attest key」链验证 + secure_boot 校验。
- [ ] 把 attestation key 证书 + 旧 TA measurement 黑名单存 RPMB（防降级）。

### Phase 3 — 标准化 + 业务绑定 + 链上验证（按需）
- [ ] 评估输出 **PSA Attestation Token（EAT/RFC 9783，COSE）** 格式（i.MX93 已 PSA Certified），用 **Veraison** 做可自部署 verifier。
- [ ] 层 C：把 attestation 绑进 WebAuthn ceremony 与 `SignHash` 响应（默认发生，非可选）。
- [ ] **链上 verifier 合约**（参考 Automata 链上 DCAP / Marlin 链上 Nitro）：让 SuperPaymaster/SuperRelay 在链上确认「KMS 跑在真 TEE」，比纯 SDK 验证更去中心化。

---

## 7. 风险与未决问题登记（给主架构师复核）

| ID | 风险/未决 | 影响 | 状态 |
|---|---|---|---|
| **R-1** | **ELE attestation 的签名密钥是否为 NXP 工厂注入的设备唯一私钥，且有可第三方离线验证的证书链（NXP Root → 设备证）？还是必须走 EdgeLock 2GO 云 provisioning？** | **决定能否实现「不信任部署方」这一最强目标**。不成立则退到 TOFU/登记表（P0'），去中心化更好但信任根变成 AirAccount。 | **待验证（RM00284 + 实机）** |
| **R-2** | NXP BSP 的 OP-TEE 是否真的编入了 attestation PTA？社区方案只在 QEMU/RPi3 验证过，**没有 i.MX93 记录**。 | 不成立则 MVP 都做不了，需要自己往 OP-TEE 移植 PTA（重活）。 | **待验证（实机）** |
| **R-3** | TA 内能否直接调 ELE attestation（经 mailbox/SCMI/imx-secure-enclave）？还是只能从 normal world 调（失去 TEE 内取证意义）？ | 决定层 A 是否能在 TEE 内安全完成。 | **待验证（实机 + 文档）** |
| **R-4** | TA measurement 每次发版都变，客户端参考值需同步更新。可复现构建能否做到 bit-for-bit 一致（Rust + OP-TEE 工具链）？ | 不一致则客户端无法独立核对 TA hash，参考值只能盲信我们公布的值。 | 待验证（构建实验） |
| **R-5** | 半去中心化张力：硬件根永远是 NXP。是否接受「信 NXP 芯片为真」作为不可消除的中心化前提？是否值得为 i.MX95（PQC、更强 ELE）迁移以增强证明？ | 定位/路线决策。 | **需主架构师拍板** |
| **R-6** | attestation 是否应「默认强制」（每次 Sign 内联）还是「可选握手」？强制增延迟（ELE 调用 + 签名），可选则多数客户端可能不验=形同虚设。 | 安全 vs 性能/体验。 | 需主架构师拍板 |
| **R-7** | nonce freshness 复用现有 TA nonce 设施，但其 thread_local 实现有跨线程 flaky 历史（MEMORY 记录）。attestation 须用会话级/持久存储，勿踩同坑。 | 实现正确性。 | 设计已规避，实现需注意 |

---

## 8. i.MX95 迁移评估（针对 attestation）

| 维度 | i.MX93（当前） | i.MX95 | 对 attestation 的意义 |
|---|---|---|---|
| ELE | 标准 ELE（S401），**PSA Certified** | EdgeLock Secure Enclave **Advanced Profile** | MX95 实时消息签名、更强 RoT，理论上 attestation 链路更完整 [较可信] |
| PQC | 无明确 PQC | **后量子 RoT，hybrid ML-DSA + ECDSA** 签固件 | 长期资产托管面对「先存后破」量子威胁时，MX95 的 PQC 签名更抗未来攻击 [确证(NXP宣传)] |
| OP-TEE / BSP | LF 6.18，已跑通 | 同体系，BSP 更新 | 迁移成本主要是 BSP/OP-TEE 重编 + ELE API 版本差异 |
| secp256k1 | **不支持**（实测确认，以太坊私钥仍软件管理） | **大概率仍不支持**（ELE crypto 集合相近）[待验证] | **attestation 不改变这个约束**——证明的是「TA 在真 TEE 跑」，私钥仍 k256 软件签 + RPMB 防回滚 |
| 迁移性价比 | — | — | **仅为 attestation 不值得现在迁**：MX93 已 PSA Certified、ELE 有设备信息/attestation 原语，足够做 Phase 1-2。MX95 的 PQC + Advanced Profile 是「主网放大资金量 + 长期抗量子」时的升级项，非 #37 阻塞项。 |

**结论**：#37 在 i.MX93 上做到 Phase 1-2（TA 度量可验 + ELE 锚定）即满足「主网前强烈建议」的需求。i.MX95 迁移留给「后量子 + 生产级认证」的未来里程碑，与既有 `docs/migration-to-MX95.md` 的「先 MX93、量变才升 MX95」结论一致。secp256k1 不支持的硬约束两代都在，attestation 不解决也不受其影响。

---

## 9. 与半去中心化定位的权衡（明确写给生态）

| 维度 | 中心化部分（诚实承认） | 可去中心化部分（我们做） |
|---|---|---|
| 硬件根 | NXP Root CA（不可消除） | — |
| 设备证书 | 可能需 NXP / EdgeLock 2GO（R-1） | 若无则用公开登记表（TOFU），任何 fork 实例自己登记 |
| Verifier | — | 验证逻辑开源、客户端本地验 / 自部署 Veraison / 链上合约 |
| 参考值（TA hash） | — | 可复现构建 + 公开发布，任何人可核对 |
| 验证发生位置 | — | 客户端 SDK / 链上，不依赖我们的服务器自证 |

**一句话**：我们无法、也不假装能去中心化「NXP 造的芯片是真的」这一前提；我们能做到的是**验证逻辑与参考值完全开放、可自验、可上链**，且 fork 实例（换域名 = 换 rpId）能用自己的设备登记表独立运转。这与 AirAccount「代码开源可 fork、passkey 锚 rpId 域名、无 admin」的半去中心化模型一致。

---

## 10. 立即行动项（Phase 0 清单，可直接执行）

1. 串口登 MX93，`ls` OP-TEE TA 目录 / 查 BSP 构建配置，确认 attestation PTA 是否编入（R-2）。
2. 跑 `nxpele get-info`，记录 uid/ROM hash/FW hash；下载并精读 **RM00284** 的 device attestation 章节（R-1/R-3）。
3. 输出 `docs/design/37-attestation-hw-findings.md`，据此选「主路线（有 NXP 链）」或「降级 P0'（TOFU 登记表）」。
4. 主架构师就 R-5（是否为 attestation 迁 MX95）、R-6（强制 vs 可选）拍板。
</content>
