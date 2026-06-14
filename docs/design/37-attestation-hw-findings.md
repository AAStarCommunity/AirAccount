# #37 远程证明 — 硬件能力摸底 + MVP 实机结果（Phase 0 收口 + Phase 1 落地）

> 创建时间：2026-06-14 11:50 +07（本机时间）
> 板子：NXP FRDM-IMX93（aarch64, Cortex-A55），OP-TEE **4.8.0.imx**（NXP LF 6.18.2-1.0.0）
> 关联：[`37-remote-attestation-design.md`](./37-remote-attestation-design.md) · [`37-remote-attestation-research.md`](./37-remote-attestation-research.md)
> 代码：分支 `feat/37-attestation-mvp`（proto/TA/host + `packages/attestation-verifier`）

本文档记录设计文档 §7 登记的 R-1..R-9 中**可实机收口项**的结论，以及 Phase 1 MVP 的端到端验证结果。**实机验证使用 attestation PTA 路径，不依赖 NVM-Daemon，不抢占 ELE，因此未中断 KMS 生产服务**（验证后 `ListKeys` + `kms.aastar.io/health` 均正常）。

---

## 1. R-2 — attestation PTA 是否编入 NXP BSP 的 OP-TEE？ → ✅ **PASS（实机坐实）**

设计文档遗留的最大未知项之一（"社区只在 QEMU/RPi3 验过，无 i.MX93 记录"）。实机证据：

| 证据 | 命令 | 结果 |
|---|---|---|
| OP-TEE 版本 | `dmesg \| grep optee` | `optee: revision 4.8 (e7ed997213779e3d)` |
| **PTA 编入 core** | `strings /usr/lib/firmware/tee-pager_v2.bin \| grep -i attestation` | `/usr/src/debug/optee-os/4.8.0.imx/core/pta/attestation.c` + `attestation.pta` |
| xtest 含 attestation 套件 | `strings /usr/bin/xtest \| grep -i attest` | `TEE attestation` / `TA attestation (shdr)` / `TA attestation (memory)` / `Remote attestation` |

`attestation.pta` 名串 + `core/pta/attestation.c` 源路径出现在 **OP-TEE core 二进制**里，等于 NXP BSP 构建时 `CFG_ATTESTATION_PTA=y`。**结论：i.MX93 的 OP-TEE 4.8 原生带 attestation PTA，MVP 的 TA 度量路线成立，无需自行移植 PTA。**

> 注：`/lib/optee_armtz/731e279e-aafb-4575-a771-38caa6f0cca6.ta` 等 2011 时间戳的 `.ta` 是标准 OP-TEE 示例/xtest 测试 TA（可复现构建 epoch 时间戳），与旧 plan 臆造的"Attestation TA"无关——attestation 是 **PTA**（在 core 内），不是 `.ta` 文件。

---

## 2. R-3 — TA 内能否调到 attestation PTA？ → ✅ **PASS（实机坐实）**

设计文档存疑："imx-secure-enclave 的 demo 均 normal-world 用户态，TA 内路径未确认"。这针对的是 **ELE dev_attest**；**attestation PTA 是 OP-TEE 内的 PTA，TA 可经 `TEE_OpenTASession` 直接调**，与 ELE/NVM-Daemon 无关。实机确认：

- TA（`kms/ta/src/attestation.rs`）用 `optee_utee::TaSessionBuilder` 打开 PTA `39800861-182a-4720-9b67-2bcd622bc0b5` session **成功**；
- 在 TEE 内调 `GET_TA_SHDR_DIGEST`（0x1）拿到 32B 度量 + RSA-PSS 签名、`GET_PUBKEY`（0x0）拿到公钥 —— 全在 secure world 完成。

**结论：层 B（TA 度量）可完全在 TEE 内取证，无需 normal-world 协助。**这是相对 ELE dev_attest（demo 在 normal-world）的关键优势。

### ⚠️ 实现陷阱（已修复，务必记录）：PTA 的 UUID 参数是 **native `TEE_UUID` 内存布局**，不是 canonical big-endian octets

`core/pta/attestation.c` 对 `params[0]` 是**直接 cast**：`TEE_UUID *uuid = params[0].memref.buffer;`，**不调** `tee_uuid_from_octets()`。所以 16 字节必须是 `TEE_UUID` 结构体的本机字节序布局（aarch64 小端：`timeLow`(u32)/`timeMid`(u16)/`timeHiAndVersion`(u16) 按 CPU 端序，`clockSeqAndNode[8]` 保持字节序）。

- 初版传 `uuid::Uuid::as_bytes()`（大端 canonical）→ ts_store 用错误 UUID 重建文件名 → `TEE_ERROR_ITEM_NOT_FOUND (0xffff0008, origin 0x4)`。
- 修正：`as_fields()` 取逻辑整数后用 `to_ne_bytes()` 拼装（TA 与 PTA 同核同端序，native-endian 正确）。

---

## 3. Phase 1 MVP — 端到端实机结果 → ✅ **全链路打通**

`GET https://kms.aastar.io/attestation?nonce=<hex>`（公网隧道已上线，`/health` 的 `attestation_available=true`）返回 evidence，本机 `@aastar/attestation-verifier` 验签：

| 项 | 实测值 |
|---|---|
| `ta_measurement`（TA 签名头摘要 SHA-256） | `3b9435d635ce2d3730fd203b22c9e30659cf8414feb08a5a149258465706bd6b` |
| attestation key | **RSA-3072**（签名 384B，模数 384B；`CFG_ATTESTATION_PTA_KEY_SIZE=3072`） |
| 公钥指数 `e` | `0x010001`（65537） |
| `sig_alg` | `0x70414930` = `TEE_ALG_RSASSA_PKCS1_PSS_MGF1_SHA256`（与官方头一致） |
| attest key 指纹（SHA-256(modulus)） | `151f5a63e31308de878cf3bababfc5733179c166a1b1875e4e12bb0e61c33494` |
| **验签** | `signatureValid: true` —— RSA-PSS(SHA-256, salt 32) over `SHA256(nonce‖measurement)` 通过 |
| 防重放负向对照 | 错误 nonce → 拒绝（`echoed nonce does not match`） |

证据格式 `airaccount.attestation.v1`，签名约定与官方 `pta_attestation.h` + `core/pta/attestation.c` 逐行核对一致。验证器 5 项单测（模拟 PTA 签名）+ 1 次真机 evidence 验证全过。

---

## 4. 信任根定位（诚实声明，对应 R-1 / §9）

MVP 的 attestation key 是 **OP-TEE 首次使用时设备自生成的 RSA key，无连 NXP 根的证书链**（`core/pta/attestation.c`：`load_key` 失败→`generate_key()`，私钥存 secure storage，无 vendor CA）。因此：

- MVP **证明**：「evidence 由真实 OP-TEE 产生 + 跑的正是这个 TA 构建（度量可比对）+ 绑定了本次 nonce」；
- MVP **不证明**：「这是一块验证方此前从未信任过的真 NXP 芯片」。

验证方必须用 **TOFU（首次见到即 pin）或发布的签名参考值**信任 attest key —— 这是**安全妥协，不是去中心化优势**（§9）。连 NXP 根是 Phase 2（ELE 锚定），仍卡 **R-1**（RM00284 对 dev_attest 签名 key 来源沉默，需 NXP 安全参考手册 / EdgeLock 2GO）。

---

## 5. 仍未收口 / 后续

| ID | 状态 | 说明 |
|---|---|---|
| **R-1** | ⚠️ 找到可行路径（EL2GO），细节待 EL2GO 账号收口 | 见下 §5.1。NXP 根证书链：EdgeLock 2GO 可给 i.MX93 provision「设备唯一证书 + 出厂注入的 root of trust」，是把 MVP 的 TOFU 升级到 NXP 连根的可行路径。剩余：证书链结构 + 根证书离线获取方式，待 EL2GO 服务文档 / i.MX93 Security Reference Manual（NDA）。 |
| **R-4** | 待做（已确认是构建工程问题） | 见下 §5.2。同源两次编译产出不同二进制（实测 `0320215b…` ≠ 部署版 `62f2fdc1…`）→ measurement 每次变。非硬件/架构/逻辑问题，是构建未配置确定性。修法见 §5.2。当前 MVP 用 TOFU/pin 实际部署的二进制兜底。 |
| **R-9** | 待设计 | attest key 吊销/轮换；设备攻破后旧 key 失效机制。 |
| 板子 RTC | 观察 | 板子时钟比真实落后 ~4 天（`date` 显示 Jun 10，实际 Jun 14）——无 RTC 电池/未对时。`ree_time_secs` 仅信息字段（新鲜度靠 nonce，不受影响），但需注意别把它当可信时间源。 |
| 部署竞态 | ✅ 已修（PR #72） | `mx93-deploy.sh` 重启 tee-supplicant 后立刻起 kms-api 偶发 TEE worker 启动期 `open_session` 失败（`0xffff0000` origin 3）。修法：等 tee-supplicant `active`+3s settle，再用 `/RollbackCounter` 碰 TA 冒烟测，失败则重启 kms-api 重试（最多 3 次，隔 3s）。真机验证：第 1 次仍 race、重试后恢复——**重试是真正起效的机制**。 |

### 5.1 R-1 阶段性结论（2026-06-14，据 AN14544 + SPSDK i.MX93 EL2GO 文档核对）

**「不信任部署方」不是死路——EdgeLock 2GO 是把 MVP 的 TOFU 升级到「连 NXP 根」的可行路径。** 两份官方文档坐实：

- AN14544 §2/§3：EL2GO 的安全**「依赖出厂时注入设备的 root of trust」**；Table 1 三种 provisioning flow 均支持 **「Device unique certificate generation」+「Counterfeit chip detection」**，并可 provision「证书 + 密钥对（含中间证书）」到 ELE 安全飞地。
- SPSDK i.MX93 indirect flow：`el2go-host get-secure-objects`→`provision-objects` 实测 chip=MX93（板子序列号 `DC193F680C2142FF`），把 EL2GO 下发的 secure objects（含 certificates/keys）注入 ELE。
- NXP 还是 **CSA 认证的 PAA**，本就在签发可追溯的设备证明证书（DAC）。

**落地路径（对应 B 线 / 任务 #14）**：用 EL2GO 给板子 provision 一把 attestation key + 一张连 NXP/EL2GO 根的设备证书，再用 ELE `hsm_pub_key_attest`（attest 库内 key）出证书 → attestation key 有了「出生证明」，半信任→全信任。

**R-1 真正还差的最后两点（这两份文档未覆盖，需 EL2GO 服务文档 / SRM）**：
1. 设备证明**证书链的确切结构**，以及验证方如何**离线获取 NXP/EL2GO 根证书**来验；
2. ELE `hsm_dev_attest` 那把 key **本身**连不连根，还是必须另外 provision 一把库内 key + 证书。

**收口动作**：① 已申请 EdgeLock 2GO 服务访问（待批准）→ 解锁 EL2GO 开发者文档（含证书链结构 + 根获取）；② NXP i.MX 93 Security Reference Manual（NDA，经 NXP 产品页 Secure Files / field rep 获取）回答上面第 2 点。

### 5.2 R-4 可复现构建——根因与修法

`ta_measurement` = SHA-256(TA 签名前载荷)。同源两次编译产出字节不同的二进制 → measurement 变。**这是构建工具链未配置确定性的工程问题，不是硬件/架构/逻辑问题。** 常见非确定性来源：嵌入的时间戳、绝对构建路径、链接器随机 build-id、`codegen-units>1` 的并行代码生成顺序。修法（任务 C1）：

```bash
RUSTFLAGS="--remap-path-prefix=<src>=. --remap-path-prefix=<cargo>=/cargo \
           -C codegen-units=1 -C debuginfo=0 -C link-arg=-Wl,--build-id=none"
export SOURCE_DATE_EPOCH=<固定值>      # 固定时间戳；在固定路径容器内编(已具备)
# 连编两次 diff 两个 .ta,反复调到字节一致
```

PSS 签名的随机 salt 只进签名字段、不进 `shdr::hash`，故不影响 measurement。做到可复现后，信任根从「信 AAStar 公布的数」升级为「任何人 clone 源码自行重算核对」（§7.1 第 3 档）。

---

## 6. 复现命令（实机）

```bash
# R-2:确认 PTA 编入 core
ssh root@<board> "strings /usr/lib/firmware/tee-pager_v2.bin | grep -i attestation"

# MVP E2E
NONCE=$(python3 -c 'import secrets;print(secrets.token_hex(32))')
curl -s "https://kms.aastar.io/attestation?nonce=$NONCE"   # 公网
# 或板上本地: ssh root@<board> "curl -s http://127.0.0.1:3000/attestation?nonce=$NONCE"

# 验签
cd packages/attestation-verifier && pnpm install && pnpm build && pnpm test
```
