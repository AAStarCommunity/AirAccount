# #37 Phase 2 — ELE 硬件根锚定（B 线落地设计）

> 创建时间：2026-06-14 18:10 +07（本机时间）
> 关联：`37-remote-attestation-design.md`（Phase 2 概述）· `37-attestation-hw-findings.md`（R-1）· `RM00284.pdf` §3.4.6 · 子模块 `third_party/imx-secure-enclave`
> 目标：把 MVP 的 TOFU 信任根（OP-TEE 自签 attest key）升级为**连 NXP 根的 ELE 证书**，让验证方无需信任部署方。

## 1. 现状回顾

MVP（Phase 1，已上线）的 attestation key 是 **OP-TEE attestation PTA 设备自生成的 RSA key，无证书链**（`core/pta/attestation.c`）。验证方只能 TOFU pin。要到「全信任」必须让签名/attest key **连到 NXP 根**——这是 ELE 的活，不是 OP-TEE PTA 的活。

## 2. ELE `hsm_pub_key_attest` 真实 API（grounded，子模块核对）

`third_party/imx-secure-enclave/include/hsm/internal/hsm_pub_key_attest.h`：

```c
typedef struct {
    uint32_t key_identifier;       // 被 attest 的 key（= 我们要背书的签名/attest key）
    uint32_t key_attestation_id;   // 用来 attest（签发证书）的 key  ←★ R-1 落点
    hsm_op_pub_key_attest_algo_t attest_algo;  // ECDSA_SHA256/384/512 或 CMAC
    uint8_t *auth_challenge; uint32_t auth_challenge_size;  // 新鲜度 nonce
    uint8_t *certificate;   uint32_t certificate_size;      // 输出:signed TLV 证书
    uint32_t exp_certificate_size;  // HSM_OUT_TOO_SMALL 时返回所需大小
} op_pub_key_attest_args_t;

// "Attest the public key of an asymmetric key present in the ELE FW key storage.
//  User can call this function only after having opened a signature generation service flow."
hsm_err_t hsm_pub_key_attest(hsm_hdl_t signature_gen_hdl, op_pub_key_attest_args_t *args);
```

要点（决定设计）：
- 只能 attest **ELE 密钥库内**的 key（`key_identifier`）→ 必须先 `hsm_generate_key` 在库内生成，不能 attest OP-TEE 外部 key（与 §2 架构修正一致）。
- **`key_attestation_id` 是背书 key —— R-1 的全部要害**：它连不连 NXP 根，决定证书能否被第三方离线验到 NXP。这正是 EL2GO AN12691 / IMX93SRM 要回答的（待 EL2GO 账号 / NDA）。
- `auth_challenge` → 把验证方 nonce 揉进证书，防重放。
- 需先开 **signature generation service flow**，且 `hsm_pub_key_attest` 碰 key store → 需 **NVM-Daemon**（key store 在外部 eMMC，ELE 经 normal-world NVM-Daemon 代理读写）。

## 3. Phase 2 数据流（在 MVP 之上叠加）

```
TA/CA 初始化(一次性):
  1. hsm_open_session → hsm_open_key_store(需 NVM-Daemon) → hsm_open_sig_gen_service
  2. hsm_generate_key(ECC NIST P-256/384) → key_identifier = K_sign  (库内,私钥不出 ELE)

每次 attestation 请求(nonce 来自验证方):
  3. hsm_pub_key_attest{ key_identifier=K_sign, key_attestation_id=K_endorse,
                         attest_algo=ECDSA_SHA256, auth_challenge=nonce } → cert(TLV)
  4. evidence = { cert, K_sign 公钥, nonce, (+ Phase1 的 OP-TEE PTA TA 度量) }

验证方:
  5. 验 cert 签名链:K_sign ← K_endorse ← …(中间证书)… ← NXP 根
     —— 这一步成立的前提 = R-1(K_endorse 连 NXP 根),否则止于"ELE 自签",退 TOFU
  6. 验 nonce 新鲜 + TA 度量 == 参考值(Phase1) → 全信任
```

## 4. R-1 命门（唯一阻塞）

`key_attestation_id`（K_endorse）必须是一把**连 NXP 根**的 key。两条候选路径（待 EL2GO 文档确认，见 `37-attestation-hw-findings.md` §5.1）：
- **EL2GO provision**：用 EdgeLock 2GO 给芯片 provision 一把连 NXP/EL2GO 根的 device key 作 K_endorse（AN12691 讲证书怎么建/链到哪；待账号）。
- **设备出厂 key**：若 ELE 有出厂注入、连 NXP 根的 device attest key 可作 K_endorse（IMX93SRM 回答；待 NDA）。

**R-1 不解 → cert 只是「ELE 库内自背书」,验证方仍无法验到 NXP → 退 TOFU(= MVP 现状)。** 所以 Phase 2 的"全信任"价值**完全押在 R-1**。

## 5. 可立即做（不依赖 R-1 / EL2GO）的实测 —— Phase 0 收尾

这些**现在就能在板子上验**，确认 ELE 机制可用（与"连不连 NXP 根"无关）：

1. **启 NVM-Daemon**（当前 disabled：`hsm_open_session 0x14 HSM Feature Disabled`），开 key store + sig-gen service。
2. `hsm_generate_key`(ECC NIST) 在库内生成 K_sign。
3. `hsm_pub_key_attest`(ECDSA_SHA256, 任意库内 key 作 K_endorse) → 拿到 cert(TLV)，确认：① 调用成功 ② cert 结构/字段 ③ auth_challenge 是否进 cert。
4. 解析 cert TLV 结构，为验证方解析器打基础。

⚠️ **代价**：NVM-Daemon 会抢 ELE → 需**停 KMS 测试窗口**，测完恢复现场（用户已确认无上线/无老客户，可安排）。这是 §3 step 1-3 的实测，**不碰 R-1**。

## 6. 落地顺序

```
[现在] §5 实测(停 KMS 窗口) —— 确认 generate_key + pub_key_attest 跑通 + cert 结构
   ↓ 并行
[等批] EL2GO 账号 → AN12691 → 确认 K_endorse 能连 NXP 根(R-1) ; IMX93SRM(NDA) 查 device key
   ↓ R-1 成立
[实现] proto 扩 evidence(cert/K_sign) + TA 走 §3 流程 + verifier 补链验(到 NXP 根)
   ↓
[降级] R-1 不成立 → 诚实退 TOFU(§9),Phase 2 不强行上
```

## 7. 关联

- attest key 吊销/轮换（R-9）：K_sign / K_endorse 的吊销 + RPMB 单调计数器防回滚复活（与本设计同期）。
- 与 Phase 1 互补：Phase 1 的 OP-TEE PTA 度量「跑的是这个 TA」继续保留；Phase 2 的 ELE cert 解决「key 连 NXP 根」。两者都进 evidence。
