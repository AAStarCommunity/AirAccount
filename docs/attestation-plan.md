# TEE 远程证明（Attestation）开发计划

*创建时间: 2026-06-07 | 关联 Issue: #TBD | 目标版本: v0.22.0*

---

## 背景与问题

### 当前信任模型的缺口

AirAccount KMS 现在的信任链：

```
用户 → 信任 kms.aastar.io → 信任我们部署的硬件 → 信任 TEE
```

**问题**：用户无法独立验证"签名真的来自合法 OP-TEE 环境"，必须盲目信任部署方（我们）。
攻击者若能控制服务器，可以返回伪造的签名，而用户没有任何手段检测。

### 远程证明解决什么

远程证明（Remote Attestation）让客户端可以**密码学验证**：
1. 响应来自真实 TEE（不是普通进程模拟）
2. TEE 内运行的是特定版本的 KMS TA（TA hash 可验证）
3. 设备未被篡改（Secure Boot 状态可验证）

有了 Attestation，信任链变成：

```
用户 → 验证 Attestation 证书链 → 信任 OP-TEE（硬件根） → 信任 KMS TA
                                          ↑
                            不再需要信任部署方
```

---

## 技术方案

### OP-TEE Attestation 机制

OP-TEE 4.8 支持基于 **DICE（Device Identifier Composition Engine）** 的证明。
MX93 的 Attestation TA（UUID `731e279e-aafb-4575-a771-38caa6f0cca6`）已预装。

证明报告包含：
- `ta_measurement`：KMS TA 二进制的 SHA-256
- `optee_version`：OP-TEE 版本（4.8.x）
- `device_id`：基于 EdgeLock ELE 派生的设备唯一标识
- `secure_boot_state`：安全启动验证状态
- `nonce`：客户端提供的挑战值（防重放）
- `signature`：由 OP-TEE 设备私钥（存于 fuse/ELE）签名

### 系统架构

```
Client                    Host (CA)                  TEE
  │                          │                         │
  │─── GET /attestation ────→│                         │
  │      (nonce=xxx)         │                         │
  │                          │─── invoke Attestation ─→│
  │                          │         TA              │
  │                          │                         │ 读 device fuse key
  │                          │                         │ 计算 TA measurement
  │                          │                         │ 签名 {nonce, meas, ver}
  │                          │←── AttestationReport ───│
  │                          │                         │
  │←── AttestationReport ────│
  │                          │
  │ 本地验证：
  │  1. 校验证书链（OP-TEE CA → Device Cert → Report）
  │  2. 校验 nonce（防重放）
  │  3. 校验 ta_measurement == 期望值
  │  4. 校验 secure_boot_state == verified
  │  通过 → 信任后续 KMS 操作
```

### API 设计

新增端点 `GET /attestation`：

```http
GET /attestation?nonce=<32字节hex>
```

响应：

```json
{
  "report": {
    "nonce": "aabbcc...",
    "ta_measurement": "sha256:...",
    "optee_version": "4.8.0",
    "device_id": "hex...",
    "secure_boot": "verified",
    "timestamp": 1749305000
  },
  "signature": "hex...",
  "cert_chain": ["base64_der...", "base64_der..."]
}
```

### TA 侧实现

```rust
// kms/ta/src/attestation.rs

pub struct AttestationReport {
    pub nonce: [u8; 32],
    pub ta_measurement: [u8; 32],      // SHA-256 of this TA binary
    pub optee_version: [u8; 16],       // "4.8.0\0..."
    pub device_id: [u8; 32],           // ELE-derived device identifier
    pub secure_boot_state: u32,        // 0=unverified, 1=verified
    pub timestamp: u64,
}

pub fn generate_attestation(nonce: &[u8; 32]) -> Result<(AttestationReport, Vec<u8>)> {
    // 1. 读取 TA 自身度量（OP-TEE 在加载时计算并存储）
    let ta_meas = get_ta_measurement()?;

    // 2. 从 EdgeLock ELE 获取设备 ID
    let device_id = get_device_id_from_ele()?;

    // 3. 获取安全启动状态
    let sb_state = get_secure_boot_state()?;

    let report = AttestationReport {
        nonce: *nonce,
        ta_measurement: ta_meas,
        optee_version: *b"4.8.0\0\0\0\0\0\0\0\0\0\0\0",
        device_id,
        secure_boot_state: sb_state,
        timestamp: get_tee_time()?,
    };

    // 4. 用 OP-TEE 设备私钥签名报告
    let report_bytes = report.to_bytes();
    let sig = sign_with_device_key(&report_bytes)?;

    Ok((report, sig))
}
```

### 客户端验证库

提供 `@aastar/attestation-verifier`（TypeScript/Rust 双版本）：

```typescript
import { verifyKmsAttestation } from '@aastar/attestation-verifier';

// 在调用任何 KMS 操作前执行
const nonce = crypto.getRandomValues(new Uint8Array(32));
const report = await fetch(`https://kms.aastar.io/attestation?nonce=${hex(nonce)}`);

const result = await verifyKmsAttestation(report, {
  nonce,
  expectedTaMeasurement: KNOWN_GOOD_TA_HASH,   // 发版时公布
  expectedOpteeVersion: '4.8.x',
  requireSecureBoot: true,
  rootCert: NXP_MX93_ROOT_CERT,                 // NXP 公开的根证书
});

if (!result.valid) {
  throw new Error(`TEE attestation failed: ${result.reason}`);
}
// 现在可以信任后续 KMS 操作
```

---

## 实施计划

### Phase 1：探索 Attestation TA 接口（1 周）

- [ ] 在 MX93 板子上调用 `731e279e` TA，探索其接口（通过串口实验）
- [ ] 确认 OP-TEE 4.8 DICE/证明接口的具体 `TEE_InvokeCommand` 参数
- [ ] 确认 NXP EdgeLock ELE 是否提供设备证书链（或需要 provisioning）
- [ ] 输出：接口文档 `docs/attestation-ta-interface.md`

### Phase 2：TA 侧实现（1.5 周）

- [ ] `kms/ta/src/attestation.rs`：AttestationReport 结构体 + 生成函数
- [ ] 集成 OP-TEE Attestation TA 调用（inter-TA invoke）
- [ ] 在 `kms/proto/src/lib.rs` 增加 `GetAttestation = 20` 命令
- [ ] 在 `kms/ta/src/main.rs` 增加命令处理分支
- [ ] TA 单元测试（Mock 模式 + 真实硬件）

### Phase 3：Host API 实现（1 周）

- [ ] `kms/host/src/api_server.rs` 新增 `GET /attestation` 路由（无 x-amz-target 要求，GET 端点）
- [ ] `handle_get_attestation()` 处理函数
- [ ] 响应格式：JSON with report + signature + cert_chain
- [ ] 在 `/health` 响应中增加 `attestation_available: true` 字段

### Phase 4：客户端验证库（1 周）

- [ ] `packages/attestation-verifier/`（TypeScript）
  - 证书链校验
  - 报告结构解析
  - 签名验证（ECDSA over SHA-256）
  - nonce 防重放校验
  - TA measurement 比对
- [ ] 发布已知良好 TA hash（CI/CD 在每次 TA 构建时输出并存档）

### Phase 5：集成测试与文档（0.5 周）

- [ ] E2E 测试增加 `Phase 13 — Attestation` 测试组：
  - GET /attestation 正常返回
  - nonce 不同时签名不同（防重放）
  - 篡改报告后签名验证失败
- [ ] 更新 `CLAUDE.md` 和 `docs/KMS-API-DOCUMENTATION.md`
- [ ] 版本号 bump 到 v0.22.0

**总工期：5 周**

---

## 发版时公布内容

每次 TA 构建后，在 GitHub Release Notes 中公布：

```
KMS TA v0.22.0
  SHA-256: abcdef1234567890...
  OP-TEE version: 4.8.0
  Build timestamp: 2026-07-01T00:00:00Z
  Secure Boot: required
```

用户可以通过 Attestation API 独立验证自己使用的服务是否运行此版本。

---

## 风险与限制

| 风险 | 说明 | 缓解 |
|------|------|------|
| NXP ELE 设备证书未 provision | 部分开发板出厂未预置设备证书 | Phase 1 验证；若缺失需走 NXP provisioning 流程 |
| OP-TEE DICE 接口未稳定 | 4.8 版本的 DICE 接口可能与预期不同 | Phase 1 充分探索，记录实际接口 |
| 证书链更新 | NXP 根证书有效期/轮换 | 在客户端库中支持证书更新机制 |
| Mock 模式无法做真实证明 | 开发环境无法生成有效 Attestation | Mock 模式返回标记为 `mock` 的假报告，客户端需检查 `ta_mode` 字段 |

---

## 关联

- 父文档：[ta-security-enhancement-plan.md](ta-security-enhancement-plan.md)
- 并行计划：[rpmb-anti-rollback-plan.md](rpmb-anti-rollback-plan.md)
- GitHub Issue：#37
- 目标版本：v0.22.0
- 依赖：v0.21.0（RPMB 防回滚）建议先完成
