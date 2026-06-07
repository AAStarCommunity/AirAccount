# RPMB 防回滚安全增强计划

*创建时间: 2026-06-07 | 关联 Issue: #TBD | 目标版本: v0.21.0*

---

## 背景与问题

### 当前漏洞

AirAccount KMS 当前使用 OP-TEE Secure Storage 存储钱包状态（包括 passkey 绑定关系、ChangePasskey 记录）。
Secure Storage 依赖 eMMC 普通分区（通过 AES 加密），存在**物理回滚攻击**路径：

```
攻击场景：
1. 用户执行 ChangePasskey（旧 passkey → 新 passkey）
2. 攻击者拿到设备，物理拆解 eMMC
3. 把 eMMC flash 恢复到 ChangePasskey 之前的备份镜像
4. 旧 passkey 重新有效，攻击者用旧 passkey 签名
```

这对资产托管类场景是**高危漏洞**。

### RPMB 解决原理

RPMB（Replay Protected Memory Block）是 eMMC 标准中的硬件功能：
- 内部有一个**单调递增计数器**（由 eMMC 芯片固件维护，不可回滚）
- 每次写操作都包含 MAC（基于共享密钥）和计数器值
- 读操作返回当前计数器，主机校验 MAC 和单调性
- 攻击者即使物理替换 eMMC，也无法伪造计数器

OP-TEE RPMB TA（UUID `5ce0c432-0ab0-40e5-a056-782ca0e6aba2`）已预装在 MX93，可直接调用。

---

## 目标

- **防止 ChangePasskey 回滚**：旧 passkey 撤销状态绑定到 RPMB 单调计数器
- **防止 DeleteKey 回滚**：已删除密钥不得因 flash 回滚而复活
- **关键 nonce 防重放**：Sign/SignHash 操作的 nonce 序列存 RPMB，防止签名重放

---

## 技术方案

### 方案架构

```
TEE Secure World
┌──────────────────────────────────────────┐
│  KMS TA (kms/ta)                         │
│  ┌────────────────┐  ┌─────────────────┐ │
│  │  wallet.rs     │  │  rpmb_guard.rs  │ │
│  │  ChangePasskey │→ │  write_counter  │ │
│  │  DeleteKey     │→ │  verify_counter │ │
│  │  Sign          │→ │  read_counter   │ │
│  └────────────────┘  └────────┬────────┘ │
└───────────────────────────────┼──────────┘
                                │ TEE_InvokeCommand
                                ↓
                    RPMB TA (5ce0c432…)
                    eMMC RPMB Partition
                    [单调计数器，不可回滚]
```

### 核心数据结构

```rust
// kms/ta/src/rpmb_guard.rs

/// RPMB 中存储的防回滚状态
#[derive(Serialize, Deserialize)]
pub struct AntiRollbackState {
    /// 每个 wallet_id 对应一个撤销版本号
    pub wallet_version: HashMap<String, u64>,
    /// 全局操作序列号（用于 Sign nonce）
    pub global_seq: u64,
    /// 最后更新时间（TEE 时钟）
    pub updated_at: u64,
}

pub struct RpmbGuard {
    rpmb_ta: RpmbTaClient,
    state: AntiRollbackState,
}

impl RpmbGuard {
    /// ChangePasskey 时递增版本号并写 RPMB
    pub fn increment_wallet_version(&mut self, wallet_id: &str) -> Result<u64> {
        let v = self.state.wallet_version.entry(wallet_id.to_string())
            .or_insert(0);
        *v += 1;
        self.flush_to_rpmb()?;
        Ok(*v)
    }

    /// 签名前校验 wallet 版本未被回滚
    pub fn verify_wallet_version(&self, wallet_id: &str, expected_min: u64) -> Result<()> {
        let current = self.state.wallet_version.get(wallet_id).copied().unwrap_or(0);
        if current < expected_min {
            return Err(KmsError::RollbackDetected);
        }
        Ok(())
    }

    /// 标记 wallet 已删除（写 u64::MAX 版本）
    pub fn mark_wallet_deleted(&mut self, wallet_id: &str) -> Result<()> {
        self.state.wallet_version.insert(wallet_id.to_string(), u64::MAX);
        self.flush_to_rpmb()
    }
}
```

### Secure Storage 与 RPMB 的配合

不把全量数据存 RPMB（RPMB 空间有限，通常 128KB-4MB），只存**版本向量**：

```
Secure Storage（加密，可被物理回滚）：
  wallet_id → { secp256k1_seed, passkey_pubkey, bip32_state, ... }

RPMB（防回滚，不可被物理回滚）：
  { wallet_id → version, global_seq }

校验逻辑：
  每次 Sign/ChangePasskey/DeleteKey 前，先读 RPMB 版本，
  与 Secure Storage 中存的版本比对，
  若 Secure Storage 版本 < RPMB 版本 → 拒绝操作，上报 RollbackDetected
```

---

## 实施计划

### Phase 1：RPMB TA 通信层（1 周）

- [ ] 在 `kms/ta/src/` 新增 `rpmb_guard.rs`
- [ ] 实现通过 `TEE_InvokeCommand` 调用 RPMB TA（UUID `5ce0c432`）的读/写/计数器接口
- [ ] 单元测试：在 MX93 真实硬件上验证 RPMB 读写正常
- [ ] 确认 MX93 上 RPMB 可用空间（`cat /sys/class/mmc_host/mmc0/mmc0:0001/rpmb_size_mult`）

### Phase 2：ChangePasskey 防回滚（1 周）

- [ ] `wallet.rs::change_passkey()` 完成后调用 `rpmb_guard.increment_wallet_version()`
- [ ] `wallet.rs::sign()` 和 `sign_hash()` 入口处调用 `rpmb_guard.verify_wallet_version()`
- [ ] 添加 `KmsError::RollbackDetected` 错误码，host 侧映射为 HTTP 409
- [ ] E2E 测试：模拟 Secure Storage 回滚，验证 Sign 被正确拒绝

### Phase 3：DeleteKey 防回滚（0.5 周）

- [ ] `wallet.rs::remove_wallet()` 完成后调用 `rpmb_guard.mark_wallet_deleted()`
- [ ] CreateKey 时校验 wallet_id 未被标记为已删除
- [ ] E2E 测试：删除 key 后回滚 Secure Storage，验证 key 无法复活

### Phase 4：文档与发版（0.5 周）

- [ ] 更新 `docs/ta-security-enhancement-plan.md`
- [ ] 更新 `CLAUDE.md` 安全约束章节
- [ ] 版本号 bump 到 v0.21.0
- [ ] PR review + 合并到 main

**总工期：3 周**

---

## 测试验证方法

```bash
# 1. 功能测试（正常路径）
./kms/test-full-api.sh localhost:3000

# 2. 防回滚测试（需要物理操作或 TEE 模拟）
# 步骤：
#   a. CreateKey → ChangePasskey（旧 → 新）
#   b. 备份当前 Secure Storage（cp /data/tee/* /backup/）
#   c. 用新 passkey Sign → 成功
#   d. 恢复旧 Secure Storage（cp /backup/* /data/tee/）
#   e. 用旧 passkey Sign → 应返回 409 RollbackDetected ✅
#   f. 用新 passkey Sign → 应返回 409 RollbackDetected ✅（版本不一致）

# 3. 自动化 E2E（在 e2e-test.py 增加 SEC-RPMB 测试组）
```

---

## 风险与限制

| 风险 | 说明 | 缓解 |
|------|------|------|
| RPMB 空间不足 | 默认 128KB，存大量 wallet_id 可能超限 | 只存 wallet_id 的 hash（32B），可存 4096 个钱包 |
| RPMB TA 不可用 | MX93 上 OP-TEE RPMB TA 需验证是否可用 | Phase 1 先做可用性验证，降级方案：soft counter + 告警 |
| RPMB 密钥丢失 | eMMC 更换后 RPMB 认证密钥失效，所有 wallet 需重新 provision | 文档化出厂流程，设备更换 eMMC 时需管理员重置 |
| 性能开销 | 每次 Sign 多一次 RPMB 读（~10ms） | 可缓存到 TA session，session 内只读一次 |

---

## 关联

- 父文档：[ta-security-enhancement-plan.md](ta-security-enhancement-plan.md)
- 并行计划：[attestation-plan.md](attestation-plan.md)
- GitHub Issue：#36
- 目标版本：v0.21.0
