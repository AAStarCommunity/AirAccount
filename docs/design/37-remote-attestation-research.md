# TEE 远程证明（Remote Attestation）业界方案调研

> 创建时间：2026-06-13
> 最后更新：2026-06-13（已获取并通读 NXP RM00284 Rev 4.3 + imx-secure-enclave 官方代码，§3.2 升为官方确证）
> 关联 Issue：#37 TEE 远程证明
> 文档性质：技术调研 / 方案全景 / 对抗性评估
> 配套文档：`docs/design/37-remote-attestation-design.md`（AirAccount 落地设计）

---

## 0. 阅读须知：证据可信度标注

本文每个关键技术声明尽量给出来源。**安全核心文档的要求：每个技术声明必须有官方权威来源（官方仓库代码 / 官方手册）+ URL。** 四种标注（按可信度降序）：

- **[官方源确证]**：已用**官方仓库源码 / 官方手册**逐项核对，给出文件路径 + 函数名/命令名 + URL。最高可信度。
- **[确证]**：有官方文档 / RFC / 一手仓库支撑，链接在节末（但未做逐行源码核对）。
- **[较可信]**：多个二手来源一致，但未读到一手 spec 原文。**本轮已尽量消除此档**——能升则升、不能则降为 [待验证]。
- **[待验证]**：推断或单一来源，或现有一手源未涵盖（如 R-1 的签名 key provenance，RM00284 已通读但未涉及，需 NXP 安全参考手册 / EdgeLock 2GO / 实机）。**落地前必须查指定官方源 / 真机确认。这些点是给主架构师复核、且是 Phase 0 必做项的重点。**

⚠️ **诚实声明（关于已核对 / 仍未答的官方源，2026-06-13 更新）**：
- **OP-TEE**：走公开官方仓库 `OP-TEE/optee_os`，已逐行核对（§2.1 [官方源确证]）。
- **NXP ELE**：**已获取并通读 RM00284 Rev 4.3（本地 PDF）+ imx-secure-enclave 官方代码（本地 clone）**，§3.2 升为 [官方手册确证 / 官方代码确证]。`hsm_dev_attest`/`hsm_pub_key_attest` 的输出结构、签名算法（ECDSA P-384）、「只 attest 库内 key」限制、secure-boot 度量字段均已坐实 → **R-8 架构已修正、H-2 已成立**。
- **仍未答 [待外部一手源]**：**R-1（dev_attest 签名 key 是否 NXP 工厂注入 + 连 NXP 根证书链）—— RM00284 §3.12「Detailed description」为空，API 参考手册不含 key provenance/信任根，即便通读也无法收口**，必须另查 NXP 安全参考手册 / EdgeLock 2GO / NDA / 实机。这是「不信任部署方」唯一阻塞前提，绝不伪装成已确证。

⚠️ **对既有 `docs/attestation-plan.md`（2026-06-07）的纠偏（已用官方源坐实）**：该计划声称 "OP-TEE 4.8 支持基于 DICE 的证明" 且 "MX93 预装 Attestation TA（UUID `731e279e-aafb-4575-a771-38caa6f0cca6`）"。**经官方仓库 `lib/libutee/include/pta_attestation.h` 核对，真实 PTA UUID 是 `39800861-182a-4720-9b67-2bcd622bc0b5`，上游根本不存在 `731e279e...`，该 UUID 系臆造 [官方源确证]**；DICE 也不是 OP-TEE 上游的证明路径，真实路径是 **attestation PTA**（详见 §2.1）。后续设计以本文为准。

---

## 1. 远程证明到底在解决什么（统一心智模型）

RATS（Remote ATtestation procedureS，RFC 9334）给了一套通用词汇，本文全程沿用 **[确证]**：

```
        Evidence                Attestation Result
Attester ─────────► Verifier ──────────────────► Relying Party
(被证明方=TEE)      (验证方)                      (依赖方=客户端/合约)
                      ▲
                      │ Reference Values + Endorsements
                Endorser/厂商(NXP/Intel/AMD/Arm…)
```

四个角色：
- **Attester**：产生关于自己的「证据（Evidence）」——通常是 *度量值 + 客户端给的 nonce*，用一把硬件背书的私钥签名。
- **Verifier**：核验签名链 + 把度量值比对「参考值（Reference Values）」+ 用厂商「背书（Endorsements）」确认这把签名密钥真来自合法硬件。
- **Relying Party**：拿到 Verifier 的结论后做决策（信任 / 拒绝）。
- **Endorser**：硬件厂商，提供根 CA 与参考值。

**所有 TEE 证明方案都是这个模型的实例**，差异只在四件事：
1. 证据里度量了什么（CPU 固件？VM 镜像？某个 TA 的 hash？）
2. 签名密钥的信任根是谁、证书链怎么连到厂商根。
3. 验证是否必须联网调厂商服务（中心化痛点）。
4. 证据格式（自定义 quote / COSE-EAT / X.509）。

**对 AirAccount 的根本张力**：远程证明的可信度天然来自「厂商根 CA」。而 AirAccount 定位「半去中心化、无 admin、任何人可 fork 换域名自建」。厂商根 CA = 中心化锚点。这个张力贯穿全文，§7 专门讨论。

来源：[RFC 9334 RATS Architecture](https://www.rfc-editor.org/rfc/rfc9334.html)（经 §1 各方案交叉印证为 [确证]）

---

## 2. OP-TEE / TrustZone 上的证明现状（本项目最相关）

### 2.1 OP-TEE 原生 attestation PTA **[官方源确证]**

> 本节事实经 OP-TEE 官方仓库源码逐行核对（非二手摘要）。来源 URL 见节末。

OP-TEE 上游 `optee_os` 自带一个 **attestation PTA**。**官方核对结论**：

- **真实 PTA UUID = `39800861-182a-4720-9b67-2bcd622bc0b5`**（来自 `lib/libutee/include/pta_attestation.h`）。**[官方源确证]**
  ⚠️ **据此坐实：旧 `docs/attestation-plan.md` 里的 `731e279e-aafb-4575-a771-38caa6f0cca6` 是臆造的，OP-TEE 上游不存在此 UUID。**
- **PTA 只有 4 个命令**（`pta_attestation.h`）**[官方源确证]**：
  | 命令 | 值 | 作用 |
  |---|---|---|
  | `PTA_ATTESTATION_GET_PUBKEY` | `0x0` | 取 PTA 内部 RSA **公钥** |
  | `PTA_ATTESTATION_GET_TA_SHDR_DIGEST` | `0x1` | 取**调用该 PTA 的 TA 自身**的 signed-header digest（签名形式）。⚠️ **L-1**：度量对象是 caller 自己，**不能**用来度量「任意另一个用户态 TA」——KMS TA 自度量的用法正确，勿误读为可枚举度量他人 TA |
  | `PTA_ATTESTATION_HASH_TA_MEMORY` | `0x2` | 度量某 TA 的内存 |
  | `PTA_ATTESTATION_HASH_TEE_MEMORY` | `0x3` | 度量 TEE 自身内存 |
- **签名内容 = `(nonce | 自己算出的 digest / memory hash)`，全部由 PTA 内部那把 key 签。没有任何命令对 caller 传入的外部公钥/外部数据签名。** **[官方源确证 — H-1 的官方依据]**
- **attestation RSA key 是设备首次使用时自生成的**（`core/pta/attestation.c`：`load_key()` 失败 → `generate_key()` → `crypto_acipher_gen_rsa_key()`），**非工厂注入**。**[官方源确证]**
- **该 key 无证书链、无 vendor CA、无任何硬件根锚定**——是一把 bare self-generated key，官方代码里没有任何 anchoring 逻辑。私钥存 secure storage（`sec_storage_obj_write(..., TEE_STORAGE_PRIVATE, ...)`，i.MX93 上通常落 RPMB）。**[官方源确证]**

⚠️ **这是最关键的坑，现已被官方代码坐实（给主架构师）**：开箱即用的 OP-TEE attestation 私钥是设备**自生成、零锚定**的，只能证明「同一个 TEE 实例前后一致」，**无法向一个从没见过这台设备的客户端证明「这是真 NXP i.MX93 + 未篡改 OP-TEE」**——客户端凭什么信那把自签公钥？要补上这一环，**只能靠 ELE 去背书/锚定这把自生成 key**；而「ELE 能不能背书一把外部 key」恰恰是 OP-TEE attestation PTA **做不到**的（它不签外部公钥，见上），所以这个动作必须发生在 ELE 侧——这就是设计文档 **R-8 的死结**（§3、§6 与设计文档）。

来源（官方一手）：
- [OP-TEE `lib/libutee/include/pta_attestation.h`（master，UUID + 4 命令）](https://raw.githubusercontent.com/OP-TEE/optee_os/master/lib/libutee/include/pta_attestation.h)
- [OP-TEE `core/pta/attestation.c`（master，key 自生成 + 无锚定）](https://raw.githubusercontent.com/OP-TEE/optee_os/master/core/pta/attestation.c)
- 背景讨论（非一手，仅佐证设计意图）：[OP-TEE Issue #3189](https://github.com/OP-TEE/optee_os/issues/3189/)、[Issue #5942](https://github.com/OP-TEE/optee_os/issues/5942)

### 2.2 社区端到端方案：optee-ra + Veraison **[确证]**

岩崎研究室（iisec-suzaki）的 **optee-ra** 项目把 OP-TEE attestation PTA 接到了 **Veraison verifier**，做成端到端远程证明，**代码于 2024-11-22 合并**，FOSDEM 2025 Attestation Devroom 有专门演讲。**[确证]**

要点 **[较可信]**（来自演讲摘要，slides PDF 本次抓取被网络策略拦截，**待验证细节**）：
- 机制：度量 TA 的 hash，用存在 OP-TEE 里的 attestation key 生成证据，PTA 形态、跨 OP-TEE 版本可移植。
- 验证：用 Veraison 框架。
- 测试平台：**QEMU 和 Raspberry Pi 3 B+**（⚠️ **没有 i.MX93 的验证记录**，迁移到 NXP 平台是未知数）。
- 明确列出的未来工作（= 现在的缺口）：① 把 attestation key 存进 HSM；② 启动时确认 OP-TEE 被安全加载（secure boot 绑定）；③ 给 attestation key 引入证书（解决 §2.1 的「凭什么信这把 key」）。

来源：
- [FOSDEM 2025: Remote Attestation on Arm TrustZone OP-TEE with VERAISON Verifier](https://archive.fosdem.org/2025/schedule/event/fosdem-2025-4952-remote-attestation-on-arm-trustzone-op-tee-with-veraison-verifier-current-status-and-future-plan-/)
- [GitHub iisec-suzaki/optee-ra](https://github.com/iisec-suzaki/optee-ra)
- [Project Veraison](https://github.com/veraison) / [veraison/services](https://github.com/veraison/services)

### 2.3 GlobalPlatform / measured boot / fTPM **[较可信]**

- **GlobalPlatform** 在推 TEE 证明的标准化（Attestation Workshop），但目前更多是规范层面，没有「拿来即用、所有 OP-TEE 都实现了」的 GP attestation API。**[较可信]**
- **measured boot / event log**：OP-TEE 可与 measured boot 链结合（把固件/TA 度量进某种 PCR / event log），但 i.MX93 上具体如何串（是否走 fTPM TA、是否有 event log）**[待验证]**。
- **DICE**：是 TCG / Android 体系的「逐层派生密钥+度量」架构，Android Keystore/AVF 用得多；**OP-TEE 上游证明走的是 attestation PTA，不是 DICE**。把两者混为一谈是既有 plan 的错误。**[确证 - 反驳既有 plan]**

来源：
- [GlobalPlatform Attestation Workshop takeaways](https://globalplatform.org/scaling-remote-attestation-key-takeaways-from-the-globalplatform-attestation-workshop/)
- [TCG DICE Attestation Architecture](https://trustedcomputinggroup.org/wp-content/uploads/DICE-Attestation-Architecture-r23-final.pdf)
- [Android: Applications of DICE](https://source.android.com/docs/security/features/dice/applications-of-dice)

### 2.4 OP-TEE / TrustZone 证明的坑（对抗性）

1. **信任根缺失**：attestation key 默认自签，不连厂商根（§2.1）。不补这一环，远程证明只是「自说自话签个名」，挡不住「攻击者自建一台真 TEE 给你签」。
2. **未在 i.MX93 验证**：社区方案只在 QEMU/RPi3 跑通；NXP BSP 的 OP-TEE 是否带 attestation PTA、ELE 集成如何，**全是未知**。
3. **TA 度量 ≠ 系统完整性**：GET_TA_SHDR_DIGEST 只证 TA 二进制，不证 normal world（REE）有没有被改、也不证 RPMB key、不证 secure boot 真开了——需要额外把 secure boot state、ELE 设备身份一起塞进证据。
4. **版本绑定脆弱**：客户端要硬编码「期望的 TA hash」，每次发版 TA 都变，客户端验证逻辑得同步更新参考值，运维负担实打实。

---

## 3. NXP i.MX93 ELE（EdgeLock Enclave）证明能力

### 3.1 ELE 是什么 **[确证]**

- ELE = EdgeLock secure Enclave，NXP 在 i.MX 8ULP/91/93/95/943 上的独立安全子系统（HSM）。i.MX93 上的 Sentinel IP 代号 **S401**（8ULP 是 S400）。**[确证]**
- 提供：硬件信任根、secure boot、生命周期管理、tamper detection、密钥存储、加解密。**[确证]**
- **i.MX93 EdgeLock Secure Enclave 已通过 PSA Certified 认证**。⚠️ **务必区分:PSA Certified（安全鲁棒性 Level 1/2/3 认证）≠ PSA Functional API Certified（后者才认证「暴露可用的 Initial Attestation Token API」），更不等于「BSP 已把可用的 remote attestation token 接口暴露出来」**——它说明架构上具备 PSA-RoT 能力、理论上可产出 PSA attestation token（EAT），但**BSP/固件是否真把这条链路暴露给 OP-TEE/应用层调用,是另一回事 [待验证]**。**[确证(认证事实) + 待验证(实际可用性)]**

### 3.2 ELE 的 attestation 原语 **[官方手册确证 + 官方代码确证]**

> ✅ **本节已用一手材料逐项核对（2026-06-13）**：NXP **RM00284 Rev 4.3**（EdgeLock Enclave HSM API，本地 PDF）+ **imx-secure-enclave** 官方参考实现（本地 clone）。ELE **有两个**独立的 attestation 原语，必须分清：

**(1) Device attestation — `hsm_dev_attest`（RM00284 §3.12, p65-66）[官方手册确证]**
签设备自身度量 + 客户端 nonce。输出结构 `op_dev_attest_args_t`（`hsm_dev_attest.h`）：
- `uid`(16B 芯片唯一 ID)、`soc_id`/`soc_rev`、`lmda_val`(lifecycle: OPEN=0x10/CLOSED=0x40/LOCKED=0x200)、`ssm_state`(安全子系统状态机)。
- `sha_rom_patch`(32B, Sentinel ROM patch SHA-256)、`sha_fw`(32B, 已装 FW SHA 前 256bit)。
- **`oem_srkh`(64B, OEM Super Root Key Hash = OEM 安全启动根公钥 hash)**、`oem_pqc_srkh`(v3, 后量子 SRKH)、`imem_state`/`csal_state`/`trng_state`。
- `nounce`+`rsp_nounce`(防重放)、`info_buf`(验签用的被签内容 = dev_info+nounce)、`signature`。
- **签名 = ECDSA P-384**（`sab_dev_attest.h`: `DEV_ATTEST_SIGN_SIZE=96`）；走 **`ROM_DEV_ATTEST_REQ`(0xDB) ROM 级请求**。**[官方代码确证]**

**(2) Public key attestation — `hsm_pub_key_attest`（RM00284 §3.4.6, p39）[官方手册确证]**
- 手册原文：「Attest the public key of an asymmetric key **present in the ELE FW key storage**」。
- 参数：`key_identifier`(被 attest 的 key) + `key_attestation_id`(做背书的 key) + `attest_algo`(CMAC / ECDSA_SHA224-512) → 输出 `certificate`(signed TLV)。
- **关键限制：被 attest 的 key 和做背书的 key 都必须是「ELE 密钥库内」的 key**（`hsm_generate_key` 生成）。**无法 attest 一把存在 OP-TEE secure storage 里、ELE 库外的 key。** `#ifdef PSA_COMPLIANT` 才编入。**[官方手册 + 代码确证]**

⚠️ **三条已被一手材料坐实的硬结论（直接喂给设计文档）**：
1. **H-2 成立**：ELE **确实**提供经硬件签名的 secure-boot/lifecycle 度量（dev_attest 的 `oem_srkh`/`sha_fw`/`lmda_val`），**不需要也不应由 OP-TEE 自填**。
2. **R-8 架构修正**：「ELE 背书 OP-TEE 自生成 attest pubkey」**做不到**——pub_key_attest 只收库内 key。正解是 attest/签名 key 直接在 ELE 库内生成、用 pub_key_attest 出证书。
3. **R-8/R-1 残留缺口**：pub_key_attest 的背书 key（`key_attestation_id`）本身是**用户用 `hsm_generate_key` 生成的普通库 key**——**RM00284 没有提供任何「内置的、NXP 工厂注入并连 NXP 根」的 attestation key**；dev_attest 的设备 key 也**不会去签库内 key**（它只签设备度量）。所以「库内 attestation key 如何连到 NXP 根」在 RM00284 里**没有答案**。

⚠️ **R-1 — RM00284 已查仍未答（最关键，诚实标注）[待 NXP 安全参考手册 / EdgeLock 2GO / NDA]**：
`hsm_dev_attest` 的 `signature` 到底用哪把 key 签、那把 key 是否 NXP 工厂注入的设备唯一私钥、有没有可被第三方离线验证到 NXP 根 CA 的证书链——**RM00284 §3.12 的「Detailed description」一节为空，整章只给输出结构 + 函数签名，对签名 key 的 provenance 和证书链完全沉默**。这是 API 参考手册的固有边界（不含 key provisioning / 信任根架构）。**结论：即便拿到并通读了 RM00284，R-1 仍无解**，必须另查 NXP 安全参考手册 / EdgeLock 2GO provisioning 文档 / NDA 材料，或实机抓 dev_attest 输出逆向证书链。**这是「不信任部署方」能否成立的唯一阻塞前提。**

⚠️ **R-3 [待实机]**：imx-secure-enclave 的 dev_attest / pub_key_attest demo 均为 **normal-world 用户态库**调用；OP-TEE TA 内能否经 SCMI/mailbox 直接调 ELE attestation 未确认。

⚠️ **实机已验证（架构师提供）**：`hsm_dev_attest exchange Passed`（真机跑通）；lifecycle = OPEN（开发态）；完整 HSM session（含 pub_key_attest）需 **NVM-Daemon**，当前 disabled（`hsm_open_session 0x14 HSM Feature Disabled`），pub_key_attest 未实测。

### 3.3 对照：NXP 离散安全芯片 SE05x **[确证，作反差]**

NXP 的**离散** Secure Element **SE05x** 有非常成熟、文档清晰的 attestation：芯片出厂带 NXP 签发的证书，attestation 对象用「attestation key + NXP 信任根」签，第三方可离线验证（AN13254）。**[确证]**

⚠️ **重要反差**：SE05x（离散 SE）的成熟 attestation **不等于** i.MX93 集成 ELE 也有同样开箱即用的、连 NXP 根的 attestation。**不要把 SE05x 的能力想当然套到 ELE 上**。i.MX93 没有板载 SE05x（FRDM-i.MX93 默认不带），所以走的是集成 ELE 这条**文档更模糊**的路。

来源：
- [NXP RM00284 EdgeLock Enclave HSM API（Rev 4.3, 2026-03）](https://www.nxp.com/docs/en/reference-manual/RM00284.pdf) ← **一手手册，落地前必读**
- [i.MX93 EdgeLock Secure Enclave on PSA Certified](https://products.psacertified.org/products/imx93-edgelock-secure-enclave)
- [NXP imx-secure-enclave 用户态库](https://github.com/nxp-imx/imx-secure-enclave/)
- [NXP imx-ele-demo](https://github.com/nxp-imx-support/imx-ele-demo)
- [SPSDK nxpele 工具文档](https://spsdk.readthedocs.io/en/latest/examples/ele/nxpele.html)
- [EdgeLock 2GO（云端 provisioning）](https://www.nxp.com/products/security-and-authentication/authentication/edgelock-2go:EDGELOCK-2GO)
- [AN13254 Secure attestation with EdgeLock SE05x](https://www.nxp.com/docs/en/application-note/AN13254.pdf)（离散 SE，作对照）

---

## 4. 业界 TEE 证明方案全景对比

各方案我都按「证据里度量啥 / 信任根 / 证书链 / 验证是否需厂商在线服务 / 去中心化友好度」拆解。**[确证，逐条来源见节末]**

### 4.1 横向对比表

| 方案 | 度量对象 | 签名密钥 / 信任根 | 证书链 | 证据格式 | 验证是否需厂商在线服务 | 去中心化友好度 |
|---|---|---|---|---|---|---|
| **Intel SGX (EPID)** | enclave MRENCLAVE/MRSIGNER | EPID group key / Intel | → Intel | 自定义 quote | **是**，必须问 Intel IAS | ✗ 差（强依赖 IAS） |
| **Intel SGX/TDX (DCAP/ECDSA)** | enclave/TD 度量 | PCK→QE ECDSA key / Intel | Intel Root→PCK→QE→quote | quote (ECDSA) | **半**：证书可缓存进 PCCS，离线验，但根仍 Intel；**可上链验** | △ 中（可自建 PCCS、可上链） |
| **AMD SEV-SNP** | VM launch 度量 + TCB | VCEK / AMD | AMD Root(ARK)→ASK→VCEK→report | 自定义 report | **半**：证书从 AMD KDS 取，可缓存离线验 | △ 中 |
| **Arm CCA (RME/RMM)** | Realm RIM/REM + 平台度量 | RAK(realm)+CPAK(平台) / Arm-soc厂 | 平台 token + realm token（双 EAT） | **COSE/EAT** | 取决于厂商；用 Veraison 可自建 verifier | △ 中（标准化好） |
| **Arm PSA (PSA-RoT)** | 启动度量 + 软件组件 | IAK，PSA-RoT 直接签 / 芯片厂 | 厂商背书 + 参考值 | **COSE/EAT (RFC 9783)** | 用 Veraison 等可自建 | ○ 较好（开放标准） |
| **AWS Nitro Enclaves** | enclave 镜像 PCR | Nitro Hypervisor / AWS | AWS Nitro PKI Root→...→doc | **COSE_Sign1 (CBOR), ES384** | 验签离线，但根是 AWS；**可上链验** | △ 中（可上链，根仍 AWS） |
| **TPM 2.0** | PCR（启动度量） | AK，经 EK 背书 / TPM 厂 | TPM厂→EK cert→AK | TPMS_ATTEST quote | 离线（拿到 EK/AK cert 即可） | ○ 较好（生态成熟、多厂商） |
| **OP-TEE attestation PTA** | **TA signed header digest** | **自生成 EC/RSA（默认无根！）** | **默认无 → 需自建/绑 ELE** | 自定义（hash+RSA sig） | 默认无（自建 Veraison） | ○ 本质友好，但缺信任根 |

### 4.2 各方案要点与坑（对抗性）

**Intel SGX — EPID vs DCAP** **[确证]**
- EPID（老）：匿名 group 签名，验证**必须在线问 Intel IAS**，单点 + 强中心化，已基本被弃。
- DCAP/ECDSA（新）：QE 生成 ECDSA attestation key，PCE 签发证书，链到 Intel Root。验证用 DCAP Quote Verification Library + 本地 **PCCS** 缓存证书，可离线。**但 PCS（Provisioning Certification Service）本身无法去中心化——Intel 是硬件根**。坑：TCB recovery（微码漏洞后 TCB 版本翻新）会让旧参考值失效，运维持续负担。
- 来源：[Gramine SGX intro](https://gramine.readthedocs.io/en/stable/sgx-intro.html)、[sgx-ra-tls ECDSA](https://github.com/cloud-security-research/sgx-ra-tls/blob/master/README-ECDSA.md)

**AMD SEV-SNP** **[确证]**：链 `ARK → ASK → VCEK → report`，VCEK 绑定具体 CPU + TCB 版本，证书从 **AMD KDS** 取。Verifier 比对 report 里的 TCB 与 VCEK 扩展字段。坑：VCEK 随 TCB 版本变，缓存管理复杂。
- 来源：[AMD 58217 platform attestation](https://www.amd.com/content/dam/amd/en/documents/developer/58217-epyc-9004-ug-platform-attestation-using-virtee-snp.pdf)、[Contrast SNP docs](https://docs.edgeless.systems/contrast/1.9/architecture/snp)、[AMD KDS @ IETF wiki](https://wiki.ietf.org/group/rats/referencevalues/amd-key-distribution-service)

**Arm CCA** **[确证]**：面向**机密虚拟机（Realm）**，不是给嵌入式 TrustZone TA 用的（i.MX93 没有 RME/CCA）。证据是**双 EAT**（平台 token by CPAK + realm token by RAK），COSE 封装。标准化最干净，Veraison 有 rust-ccatoken 参考实现。对 AirAccount 仅作「未来/对比」意义，**当前硬件用不上**。
- 来源：[draft-ffm-rats-cca-token](https://datatracker.ietf.org/doc/draft-ffm-rats-cca-token/)、[Arm CCA + Veraison learning path](https://learn.arm.com/learning-paths/servers-and-cloud-computing/cca-veraison/cca-attestation/)、[veraison/rust-ccatoken](https://github.com/veraison/rust-ccatoken)

**Arm PSA Attestation（EAT / RFC 9783）** **[确证]**：**与 AirAccount 最对味**。PSA token 是 EAT 的 profile，COSE 签名，PSA-RoT 直接签，支持 nonce freshness。**i.MX93 ELE 已 PSA Certified**，所以理论上可走 PSA token 路线——若 BSP 暴露了 IAK/attestation 接口（§3.2 待验证）。这是「标准化 + 可用 Veraison 自建 verifier」的最佳结合点。
- 来源：[RFC 9783 PSA Attestation Token](https://www.rfc-editor.org/rfc/rfc9783.html)、[PSA Certified: what is an EAT](https://www.psacertified.org/blog/what-is-an-entity-attestation-token/)

**AWS Nitro** **[确证]**：COSE_Sign1/CBOR/ES384，PCR0-N 度量 enclave 镜像，链到 AWS Nitro PKI。**Marlin/NitroProver 已实现链上验证**——对 Web3 场景是重要先例（证明 TEE 证据可在 EVM 上验）。坑：根是 AWS，且只在 AWS 云内可用。
- 来源：[AWS Nitro attestation docs](https://docs.aws.amazon.com/enclaves/latest/user/set-up-attestation.html)、[Trail of Bits notes](https://blog.trailofbits.com/2024/02/16/a-few-notes-on-aws-nitro-enclaves-images-and-attestation/)、[on-chain verification (Marlin)](https://blog.marlin.org/on-chain-verification-of-aws-nitro-enclave-attestations)

**TPM 2.0** **[确证]**：`EK(厂商背书) → AK → TPM2_Quote(PCR+nonce)`。生态最成熟、多厂商（不绑单一硬件商），离线可验。坑：PCR 反映的是 normal-world 启动度量，不直接证 TEE 内某段逻辑；fTPM（TrustZone 里跑 TPM）可在 i.MX 上提供，但又回到「fTPM 本身可信吗」。
- 来源：[tpm2-software Remote Attestation](https://tpm2-software.github.io/tpm2-tss/getting-started/2019/12/18/Remote-Attestation.html)、[Keylime TPM attestation](https://deepwiki.com/keylime/keylime/3.1-tpm-attestation)

### 4.3 通用 verifier：Veraison **[确证]**

- Veraison（Linaro/Arm 主导）= 按 RFC 9334 实现的**通用证明验证服务框架**，scheme 用插件（已支持 PSA、CCA 等），用 **CoRIM** 格式喂参考值与背书。
- 对 AirAccount 的价值：**不必自己从零写 verifier**，可复用 Veraison 的 PSA/自定义 scheme；且 Veraison 可**自部署**（不强制用某中心化云），契合半去中心化。
- 来源：[veraison-project.org](https://www.veraison-project.org/book/services/overview.html)、[Standard-Based Remote Attestation: Veraison（论文）](https://ceur-ws.org/Vol-3731/paper28.pdf)、[draft-ietf-rats-corim](https://datatracker.ietf.org/doc/draft-ietf-rats-corim/)

---

## 5. 证书链设计的通用范式（供 §6 设计借鉴）

业界所有「能不信任部署方」的方案，证书链都是这个结构 **[确证，综合 §4]**：

```
厂商 Root CA（离线、极少轮换，公开发布指纹）
   └─► 中间/产品族 CA
          └─► 设备唯一证书（绑芯片唯一 ID，工厂注入或云 provisioning）
                 └─► 运行时 attestation key 证书（设备签发给本次/本实例）
                        └─► Evidence（度量 + nonce，被 attestation key 签）
```

- **谁签发**：每层由上一层私钥签。根由厂商持有。
- **谁验证**：客户端/Verifier 自顶向下验链，最后比对度量值=参考值、nonce=自己发的。
- **吊销**：靠 CRL/OCSP 或「短有效期 + 频繁重签」。嵌入式现实里多用后者（设备证书长期，attestation key 证书短期）。
- **半去中心化下的难点**：根 CA 必然是 NXP（你造不出 i.MX93）。**你能去中心化的是 Verifier 和参考值分发，不是硬件根**。

---

## 6. 对 AirAccount 的启示（本文结论）

1. **既有 plan 的技术前提已被官方源推翻**：没有 "OP-TEE 4.8 DICE 证明"、没有预装 `731e279e` Attestation TA（真实 UUID = `39800861-...`，`pta_attestation.h` 核对 [官方源确证]）。真实可用的是 **OP-TEE attestation PTA**（4 命令、度量 TA/内存、RSA 签名）+ **ELE 两个 attestation 原语**（`hsm_dev_attest` 签设备度量 ECDSA P-384、`hsm_pub_key_attest` 只 attest 库内 key，[官方手册确证]）。设计文档据此重做。

2. **最大缺口 = 信任根锚定（官方坐实）**：`core/pta/attestation.c` 确认 OP-TEE attest key 设备自生成、零锚定 [官方源确证]；PTA **不签外部公钥**、ELE `hsm_pub_key_attest` **只 attest 库内 key**（RM00284 §3.4.6）[官方确证]。→ 「OP-TEE 自生成 key + ELE 背书它」两头堵死,**架构改为「attest/签名 key 直接在 ELE 库内生成 + pub_key_attest 出证书」**。

3. **R-1 是唯一阻塞「不信任部署方」的前提，且 RM00284 已查仍未答 [待 NXP 安全参考手册 / EdgeLock 2GO]**：`hsm_dev_attest` 的签名 key 是否 NXP 工厂注入、有无连 NXP 根的可离线验证书链——**RM00284 §3.12「Detailed description」为空，API 参考手册不含 key provenance/信任根**。残留:pub_key_attest 的背书 key 本身也是普通库 key,无内置 NXP 根 key,无 device-key→keystore-key 桥。Phase 0 必须另找一手源收口。

4. **H-2 已确证为可用**：ELE `hsm_dev_attest` 提供经硬件 ECDSA P-384 签名的 secure-boot/lifecycle 度量（`oem_srkh`/`sha_fw`/`lmda_val`），evidence 的 secure_boot 字段应取自此处，非 OP-TEE 自填。**最对味的标准仍是 PSA Attestation Token（EAT/RFC 9783）**：i.MX93 已过 PSA Certified（合规等级 ≠ 已暴露可用 EAT 接口）；RM00284 未见标准 EAT/COSE 输出（dev_attest 是 NXP 私有结构 + 裸 ECDSA 签名）,走 EAT 需自己在 verifier 侧封装，大概率 fallback 到自定义证据格式 + Veraison 自定义 scheme。

5. **Web3 先例可借**：Automata 的链上 DCAP、Marlin 的链上 Nitro 证明说明「TEE 证据可在 EVM 上验」。AirAccount 管的是以太坊私钥，未来可把 attestation 验证做成链上合约，让 SuperPaymaster/SuperRelay 等依赖方在链上确认「KMS 跑在真 TEE」，这比纯客户端 SDK 验证更去中心化。

6. **去中心化的本质权衡**：硬件根永远是 NXP（中心化锚点，不可消除）。AirAccount 能做的是：① Verifier 逻辑开源、可自部署/上链；② 参考值（TA hash）公开发布、可复现构建核对；③ 把 attestation 绑进 WebAuthn ceremony 或 KMS 签名响应，让验证「默认发生」而非可选。承认「信 NXP 造的芯片是真的」这一条无法去中心化，诚实写进文档。

7. **务实分期**：MVP 先用 OP-TEE attestation PTA 把「TA 度量 + nonce」签出来交付端到端可验（即便信任根暂时只是设备自签，先解决「证明确实进了 TEE 且是这个 TA」）；再补 ELE 锚定信任根；最后考虑 PSA/EAT 标准化与链上验证。详见设计文档。

---

## 权威来源 / Authoritative Sources（本文引用来源汇总）

> **可信度分层**：打 ✅【已核对】的，是本设计期间从官方源码（OP-TEE 仓库 WebFetch）/ 官方一手材料（RM00284 本地 PDF、imx-secure-enclave 本地 clone）逐行核对过的（[官方源/手册/代码确证]）；其余为二手参考来源。⚠️ R-1（dev_attest 签名 key provenance + NXP 根证书链）即便通读 RM00284 仍未涵盖，单列「仍待外部一手源」。

### ✅ 已用官方源核对（authoritative — 本设计已逐行核对）
- **OP-TEE attestation PTA 头文件** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/lib/libutee/include/pta_attestation.h
  - 核对结论：真实 PTA UUID = `39800861-182a-4720-9b67-2bcd622bc0b5`（确认旧 `attestation-plan.md` 的 `731e279e-...` 是臆造）；仅 4 个命令（`GET_PUBKEY`=0x0 / `GET_TA_SHDR_DIGEST`=0x1 / `HASH_TA_MEMORY`=0x2 / `HASH_TEE_MEMORY`=0x3）；**无任何命令签 caller 提供的外部公钥**，全部签 `(nonce | 自身度量/摘要)`。→ H-1 的官方依据。
- **OP-TEE attestation PTA 实现** — https://raw.githubusercontent.com/OP-TEE/optee_os/master/core/pta/attestation.c
  - 核对结论：attest RSA key **首次使用时设备自生成**（`load_key` 失败 → `generate_key()` → `crypto_acipher_gen_rsa_key()`），**无证书链 / 无 vendor CA / 无硬件根锚定**（bare self-generated）；存 secure storage（`sec_storage_obj_write(..., TEE_STORAGE_PRIVATE, ...)` → 通常 RPMB）。→ H-1 的官方依据。
- **NXP RM00284 EdgeLock Enclave HSM API, Rev 4.3（本地 PDF `RM00284.pdf`，139 页，2026-06-13 通读）[官方手册确证]**
  - §3.12 Dev attest (p65-66)：`hsm_dev_attest` 输出 uid/ssm_state/lmda_val/sha_rom_patch/sha_fw/`oem_srkh`/oem_pqc_srkh/nounce/info_buf/signature。⚠️ **§3.12.1 Detailed description 为空 → 不含签名 key provenance / 证书链 → R-1 未答**。
  - §3.4.6 (p39)：`hsm_pub_key_attest` 只 attest「present in the ELE FW key storage」的 key；§3.4.5.1 attest algo = CMAC/ECDSA_SHA224-512。
  - §3.2 (p9) `hsm_key_type_t` 无 secp256k1；§3.13 (p67) lifecycle 枚举。
- **NXP imx-secure-enclave（本地 clone `third_party/imx-secure-enclave/`）[官方代码确证]**
  - `hsm_dev_attest.h` / `sab_dev_attest.c`：`DEV_ATTEST_SIGN_SIZE=96`(ECDSA P-384)、`ROM_DEV_ATTEST_REQ=0xDB`、oem_srkh 64B。
  - `hsm_pub_key_attest.h` / `test_pub_key_attest.c`：key_identifier + key_attestation_id 均为库内 key,demo 用 AES-CMAC,输出 signed-TLV certificate,`#ifdef PSA_COMPLIANT`。
- **本地代码核对**：`third_party/teaclave-trustzone-sdk/examples/` **不含 attestation 示例**（已 `find` 确认）；本地无 `optee_os` 源码（构建用 Docker 内预编译 OP-TEE）。故 attestation PTA 权威代码只在 `OP-TEE/optee_os` 官方仓库。

### 仍待外部一手源（落地前必须收口）
- **R-1**：dev_attest 签名 key 是否 NXP 工厂注入 + 连 NXP 根证书链 → **RM00284 未涵盖**,待 NXP 安全参考手册 / EdgeLock 2GO / NDA / 实机逆向。
- **R-3**：OP-TEE TA 内能否调 ELE attestation（demo 均 normal-world）。

### 参考来源（二手 / 公开）
OP-TEE / TrustZone：
- https://www.rfc-editor.org/rfc/rfc9334.html （RATS 架构）
- https://github.com/OP-TEE/optee_os/blob/master/lib/libutee/include/pta_attestation.h
- https://github.com/OP-TEE/optee_os/issues/3189/
- https://github.com/OP-TEE/optee_os/issues/5942
- https://github.com/iisec-suzaki/optee-ra
- https://archive.fosdem.org/2025/schedule/event/fosdem-2025-4952-remote-attestation-on-arm-trustzone-op-tee-with-veraison-verifier-current-status-and-future-plan-/
- https://globalplatform.org/scaling-remote-attestation-key-takeaways-from-the-globalplatform-attestation-workshop/

NXP i.MX93/95 ELE：
- https://www.nxp.com/docs/en/reference-manual/RM00284.pdf （RM00284 ELE HSM API，一手手册）
- https://products.psacertified.org/products/imx93-edgelock-secure-enclave
- https://github.com/nxp-imx/imx-secure-enclave/
- https://github.com/nxp-imx-support/imx-ele-demo
- https://spsdk.readthedocs.io/en/latest/examples/ele/nxpele.html
- https://www.nxp.com/products/security-and-authentication/authentication/edgelock-2go:EDGELOCK-2GO
- https://www.nxp.com/docs/en/application-note/AN13254.pdf （SE05x，作对照）
- https://www.nxp.com/products/i.MX95

业界方案：
- https://gramine.readthedocs.io/en/stable/sgx-intro.html
- https://github.com/cloud-security-research/sgx-ra-tls/blob/master/README-ECDSA.md
- https://www.amd.com/content/dam/amd/en/documents/developer/58217-epyc-9004-ug-platform-attestation-using-virtee-snp.pdf
- https://docs.edgeless.systems/contrast/1.9/architecture/snp
- https://datatracker.ietf.org/doc/draft-ffm-rats-cca-token/
- https://learn.arm.com/learning-paths/servers-and-cloud-computing/cca-veraison/cca-attestation/
- https://www.rfc-editor.org/rfc/rfc9783.html （PSA Attestation Token）
- https://www.psacertified.org/blog/what-is-an-entity-attestation-token/
- https://docs.aws.amazon.com/enclaves/latest/user/set-up-attestation.html
- https://blog.trailofbits.com/2024/02/16/a-few-notes-on-aws-nitro-enclaves-images-and-attestation/
- https://blog.marlin.org/on-chain-verification-of-aws-nitro-enclave-attestations
- https://tpm2-software.github.io/tpm2-tss/getting-started/2019/12/18/Remote-Attestation.html
- https://deepwiki.com/keylime/keylime/3.1-tpm-attestation

通用 verifier / 标准 / Web3：
- https://www.veraison-project.org/book/services/overview.html
- https://ceur-ws.org/Vol-3731/paper28.pdf
- https://datatracker.ietf.org/doc/draft-ietf-rats-corim/
- https://github.com/dineshpinto/awesome-tee-blockchain
- https://arxiv.org/html/2511.22317 （Decentrally Attested TEEs for rollup sequencers）
