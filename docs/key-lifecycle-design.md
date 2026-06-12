# AirAccount 密钥生命周期管理设计

> Issue 关联: #42（待创建）  
> 状态: RFC / 设计阶段  
> 作者: @jhfnetboy  
> 依赖: Email 注册 + Passkey Binding（通知功能前置依赖）

---

## 背景

AirAccount 的 TEE 安全存储空间有限，且长期不活跃的密钥会成为潜在的安全隐患和运维负担。同时，用户需要对"自己的密钥何时会被删除"有清晰的预期和充足的窗口期。

本设计参考了以下业界实践：
- **AWS KMS**: 7-30 天计划删除窗口，删除前可随时取消
- **Vault (HashiCorp)**: TTL + 续租机制，过期自动吊销
- **Keybase**: 设备撤销 + 社交恢复双轨
- **1Password**: 不活跃账户提醒，但不自动删除
- **Gnosis Safe**: 多签触发链上操作（用于高危操作授权）

**关键设计原则**：
1. 用户是主权所有者——主动删除必须经过 passkey 验证
2. 超时删除必须公示且给足窗口期
3. 即便 TEE 私钥删除，链上账户资产通过社交恢复仍可重建
4. 管理员强制删除需多签授权且全程透明可审计

---

## 状态机设计

```
创建
  │
  ▼
┌─────────┐  任意 Sign/Derive 操作    ┌─────────┐
│  active │ ◄──────────────────────── │  active │
└─────────┘                           └─────────┘
     │
     │ 连续 365 天无 Sign/Derive 操作
     ▼
┌─────────┐   用户通过 passkey 重新激活  ┌─────────┐
│ frozen  │ ─────────────────────────► │  active │
└─────────┘                           └─────────┘
     │
     │ frozen 后仍 90 天无活动（delete_after 到期）
     ▼
┌─────────────────┐  用户 passkey 确认 OR Admin 多签触发
│ pending_delete  │ ──────────────────────────────────►  deleted（TEE + SQLite 双清）
└─────────────────┘
```

**状态说明**：

| 状态 | 可以做什么 | 不可以做什么 |
|------|-----------|-------------|
| `active` | Sign / Derive / DescribeKey / RemoveWallet | — |
| `frozen` | DescribeKey / ActivateKey（passkey） | Sign / Derive |
| `pending_delete` | DescribeKey（只读查询） | Sign / Derive / Activate |
| `deleted` | — | 全部 |

---

## 时间线参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `INACTIVITY_DAYS` | 365 | active → frozen 触发阈值 |
| `FROZEN_GRACE_DAYS` | 90 | frozen → pending_delete 触发阈值 |
| `NOTIFY_BEFORE_FREEZE_DAYS` | 30, 7, 1 | 进入 frozen 前的提醒节点 |
| `NOTIFY_BEFORE_DELETE_DAYS` | 30, 7, 1 | 进入 pending_delete 前的提醒节点 |

总计：最少 **455 天**（365 + 90）从最后一次使用到可以被删除。

---

## 数据模型变更

### `wallets` 表新增字段

```sql
ALTER TABLE wallets ADD COLUMN last_used_at      TEXT;   -- ISO8601 UTC，每次 Sign/Derive 更新
ALTER TABLE wallets ADD COLUMN lifecycle_stage   TEXT NOT NULL DEFAULT 'active';
-- 取值: 'active' | 'frozen' | 'pending_delete' | 'deleted'
ALTER TABLE wallets ADD COLUMN frozen_at         TEXT;   -- 进入 frozen 的时间
ALTER TABLE wallets ADD COLUMN delete_after      TEXT;   -- frozen_at + 90d，到期后可触发删除
ALTER TABLE wallets ADD COLUMN notified_at       TEXT;   -- 上次发送通知的时间
ALTER TABLE wallets ADD COLUMN deletion_tx       TEXT;   -- 最终删除的 tx hash 或 admin 签名记录
```

### `KeyStatus` 响应新增字段

```json
{
  "KeyId": "...",
  "KeyState": "Enabled",
  "lifecycle": {
    "stage": "active",
    "last_used_at": "2025-10-01T12:00:00Z",
    "frozen_at": null,
    "delete_after": null,
    "inactivity_days_remaining": 273
  }
}
```

---

## API 变更

### 现有接口增强

| 接口 | 变更 |
|------|------|
| `Sign` / `SignHash` / `DeriveAddress` | 成功操作后 `UPDATE wallets SET last_used_at = NOW(), lifecycle_stage = 'active'` |
| `DescribeKey` | 响应体新增 `lifecycle` 字段 |
| `DeleteKey` | frozen/pending_delete 状态也可触发（需 passkey 验证） |

### 新接口

#### `POST /ActivateKey` — 重新激活 frozen 密钥

```json
// Request
{
  "KeyId": "uuid",
  "WebAuthnAssertion": "..."   // passkey 验证，必填
}

// Response
{
  "KeyId": "uuid",
  "lifecycle": { "stage": "active", "last_used_at": "2026-06-07T..." }
}
```

#### `POST /AdminForceDelete` — 管理员多签触发删除（pending_delete 阶段）

```json
// Request
{
  "KeyId": "uuid",
  "AdminSignatures": ["sig1_hex", "sig2_hex"],   // 需要 2/3 admin passkey 多签
  "Reason": "用户申请 | 长期不活跃超时 | 安全事件"
}

// Response
{
  "KeyId": "uuid",
  "DeletionTimestamp": "2026-06-07T...",
  "AuditRecord": "tx_hash_or_record_id"
}
```

---

## 后台任务

```rust
// 每小时运行一次 lifecycle_check_task
async fn lifecycle_check_task(db: &KmsDb, notifier: &EmailNotifier) {
    let now = Utc::now();

    // 1. active → frozen: 超过 365 天未使用
    db.mark_frozen_if_inactive(now - Duration::days(INACTIVITY_DAYS));

    // 2. frozen → pending_delete: frozen 后超过 90 天
    db.mark_pending_delete_if_frozen_expired(now - Duration::days(FROZEN_GRACE_DAYS));

    // 3. 发送预警通知（需要 email binding 功能上线后才生效）
    for wallet in db.get_wallets_needing_notification(now) {
        notifier.send_lifecycle_warning(&wallet).await;
        db.update_notified_at(&wallet.key_id, now);
    }
}
```

**注意**：通知功能依赖 Email 注册 + Passkey Binding 完成，该功能上线前仅记录 warning 日志。

---

## 公示透明页面

### `GET /lifecycle/public`

无需认证，返回所有非活跃密钥的摘要信息（不含私钥、不含完整 key_id）：

```json
{
  "policy": {
    "inactivity_to_frozen_days": 365,
    "frozen_to_delete_days": 90,
    "total_min_days_before_delete": 455,
    "last_updated": "2026-06-07"
  },
  "summary": {
    "total_active": 1024,
    "total_frozen": 12,
    "total_pending_delete": 2
  },
  "frozen_keys": [
    {
      "key_id_prefix": "d48af6**",   // 仅显示前6位
      "frozen_at": "2026-01-01T00:00:00Z",
      "delete_after": "2026-04-01T00:00:00Z",
      "days_until_delete": 23
    }
  ],
  "pending_delete_keys": [
    {
      "key_id_prefix": "1d93a1**",
      "frozen_at": "2025-09-01T00:00:00Z",
      "delete_after": "2025-12-01T00:00:00Z",
      "status": "awaiting_trigger"
    }
  ]
}
```

此页面将嵌入到 kms.aastar.io 的 Web UI 中，提供公众可审计的透明度。

---

## 最终删除：双轨触发机制

### 轨道 1：用户自主删除
- 用户登录后，通过 passkey 验证，调用 `DeleteKey`
- 适用阶段：active / frozen / pending_delete 均可

### 轨道 2：管理员多签触发（用于 pending_delete 阶段）
- 需要 **2/3 Admin Passkey** 联合签名
- 触发后立即执行 `ForceRemoveWallet`（TEE 删除）+ SQLite 清理
- 所有操作写入不可变审计日志（`deletion_tx` 字段）
- 公示页面实时更新

**多签 Admin 地址**（初始设置，后续可通过链上投票修改）：
- 由 3 个独立的 admin passkey 组成
- 任意 2 个签名即可触发

---

## 社交恢复说明（公示页面展示文字）

> **私钥删除 ≠ 链上资产丢失**
>
> 即便 AirAccount TEE 中的私钥被删除，您的链上账户（ERC-4337 Smart Account）的资产依然安全。
>
> 您可以通过 **AirAccount 社交恢复**重建密钥：
> - 至少需要 **1 位 Guardian**（您信任的人）配合验证
> - 或使用 **社区公共 Guardian 服务**（需支付少量服务费）
>
> 恢复完成后，新密钥将与您的链上账户重新绑定，资产完全不受影响。

---

## 实现阶段划分

### Phase 1（本 PR，不依赖 Email）
- [ ] 数据库迁移：新增 lifecycle 字段
- [ ] Sign/Derive 更新 `last_used_at`
- [ ] `DescribeKey` 响应新增 `lifecycle` 字段
- [ ] 后台任务框架（暂时只记录日志，不发邮件）
- [ ] `GET /lifecycle/public` 端点

### Phase 2（依赖 Email 注册上线后）
- [ ] 通知邮件集成
- [ ] `ActivateKey` 接口
- [ ] `AdminForceDelete` 接口（需配置 admin 多签地址）
- [ ] Web UI 公示页面嵌入

### Phase 3（后续优化）
- [ ] 链上审计记录（可选，需 gas）
- [ ] 社区投票修改生命周期参数

---

## Gap Key 状态说明（已解决问题）

### 什么是 Gap Key？
Security 测试阶段（2024 年底-2025 年初），通过直接操作 SQLite 插入了若干 `passkey_pubkey` 字节不在 P-256 曲线上的记录（`04 + 随机64字节`），这类密钥因为无法通过 `verify_passkey_for_wallet()` 验证而永远无法通过正常接口删除。

### 已修复（v0.19.x）
1. **新 Gap Key 不再产生**：`CreateKey` 接口已要求 `PasskeyPublicKey` 必须是合法的 65 字节 P-256 uncompressed 点（`0x04 + x + y`），API 层验证曲线合法性。
2. **历史 Gap Key 已清理**：通过 `ForceRemoveWallet`（TEE cmd 23，Issue #41）机制，所有历史 gap key 的 SQLite 记录已删除。由于旧 TA 二进制不支持 cmd 23，TEE 内的孤儿存储条目（~1-2KB 每条）目前仍存在，待 TA 重新编译后完全清除。
3. **验证**：`SELECT COUNT(*) FROM wallets WHERE key_id='...'` 均返回 0。

---

## 参考资料
- [AWS KMS Key Deletion](https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html)
- [HashiCorp Vault TTL/Lease](https://developer.hashicorp.com/vault/docs/concepts/lease)
- [ERC-4337 Account Recovery](https://eips.ethereum.org/EIPS/eip-4337)
- Issue #41: ForceRemoveWallet for gap keys
- Issue #29: ExportPrivateKey security gate
