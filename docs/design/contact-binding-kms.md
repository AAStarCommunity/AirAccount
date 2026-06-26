<!-- Created: 2026-06-26 -->
# 通知联系方式绑定 — KMS 侧设计 + 产品规划

> 跨仓库中心设计：[aastar-sdk#193](https://github.com/AAStarCommunity/aastar-sdk/issues/193) · KMS 评估：[AirAccount#129](https://github.com/AAStarCommunity/AirAccount/issues/129)
> 关联：#102（guardian P-256 去依赖）· aastar-sdk#192（绑定备份/迁移）· DVT Validator#124/#125（带外确认）
> 状态：Phase 1 实现中（2026-06-26）。本文是 KMS 侧落地设计 + 产品规划，记录 #193 收敛结论。

---

## 0. 目标
把用户的通知通道（**Telegram / email**）**经验证后**绑定到其 AirAccount，使 DVT 带外确认能触达真实用户。KMS 作**信任锚 + 存储 + 验证**。

## 1. 两道所有权证明（安全根本）
1. **账户所有权**：绑定请求由账户 **owner passkey ceremony** 授权，challenge 绑定 `{account, channel}`（commitment，strict 下 CA 改不了）。
2. **通道所有权**：用户证明控制该 Telegram/email——**verify-token 双向回环**（下方流程）。
两者都过 → `verified`。

## 2. ⚠️ 信任模型（诚实拆分 —— codex/opus 复审纠正）
两层要分开讲，别混成一句"bot 只能 DoS"（那是过度声明、且不成立）：

**(1) 绑定的 chat_id↔account 映射：共享 bot 在 TCB（可信计算基）。**
- Telegram Bot API **只把发送方 chat_id 告诉 bot**——KMS 没有独立来源。所以**共享 bot 是"谁发的 /bind"的唯一权威**，无论 verify_token 在哪生成，**被攻陷的 bot 都能谎报 chat_id、把绑定指向攻击者**。这是共享 bot 的**不可约**信任点（codex 的"token 移到 begin"也解决不了；故不做那个伪加固）。
- **后果有界**：bot 是 **AAStar 官方第一方基建**（本就运行 KMS、已被信任），且被攻陷的影响**被下面 (2) 的 path-2 兜住** → 伪造绑定 = **通知误投 + 待确认操作元数据泄漏（金额/目标）给攻击者**，**不是批准伪造 / 不是盗款**。攻击成本=攻陷官方 bot，收益=拦通知/泄元数据，无批准权。
- verify-token 回环**仍有价值**：防**非 bot 第三方**猜/抢 `binding_code`（他们没 token，过不了 confirm）。它防的是外部抢跑，**不是**防 bot 本身。
- app 展示 bot 上报的 `@username` 给用户核对：防**有 bug/半诚实**的 bot，**不防**完全恶意 bot（它连 username 也能伪造）。保留（便宜），但别当成防攻陷。

**(2) 带外确认（DVT）批准：path-2，passkey 门，bot 伪造不了。**
- 批准凭证 = **owner passkey 签 userOpHash（WYSIWYS）**，不是 bot 可见的东西。bot 降为 **ping**。被攻陷 bot **只能 DoS / 拦 ping，伪造不出 quorum 批准、动不了款**。**KMS 这块零新增——复用现有 strict sign-with-ceremony。** 这是把 (1) 的 bot 信任后果限死在"通知层"的关键。

### ⚠️ 端点层必须强制的不变量（阻塞要求，DB 层不自带）
- **begin-binding / unbind 必须先消费 owner passkey ceremony challenge** 再调 DB（`begin_contact_binding` 的 upsert **会覆盖 verified 行**——verified 绑定的全部保护就是这道门）。端点 PR **必须带测试**：无 consumed ceremony 时端点不可达。
- **claim-binding 仅限已认证 bot**；**confirm-binding 仅限 owner（app passkey 会话）提交**，bot 鉴权不得触达 confirm。
- `binding_code` / `verify_token` **必须高熵随机（≥128-bit）**（DB 按 `binding_code` 匹配，低熵=可猜）。
- **email 端点在 `begin_email_binding`（begin 时设 contact_ref+verify_token）实现前不得开放**（当前 DB 层 email 路径不完整，confirm 永远失败=安全但半成品）。
- `contact_ref` **at-rest 加密**：Phase 1 非阻塞（chat_id 低敏 + 表隔离 + getContact 严格鉴权），但 **GA / 规模化接入真实用户前必须做**（email 是可识别 PII），非无限期 TODO。

## 3. Phase 1：Telegram + Email 绑定

### 3.1 Telegram（用户发起式，因 Telegram bot 不能先私聊未 /start 用户 → 共享 bot）
```
1. app → SDK startContactBinding → passkey ceremony(challenge 绑 {account,'telegram'})
        → KMS 验 owner → 签发一次性 bindingCode(TTL，pending) → 返 app
2. app: 打开 @AAStarBot 发 /bind <bindingCode>
3. 用户 → bot: /bind <bindingCode>
4. bot → KMS POST /contact/claim-binding {bindingCode, telegram_chat_id, telegram_username}(bot 鉴权)
        → KMS 验 code → 记 claimed chat_id + 生成一次性 verify_token(短 TTL) → status=claimed → 返 verify_token 给 bot
5. bot → 把 verify_token 发到该 chat
6. 用户读 token → 回 app 填
7. app → KMS POST /contact/confirm-binding {bindingCode, verify_token} → status=verified，持久化 chat_id
8. SDK 轮询 getContact → verified
```

### 3.2 Email（无 bot，mailer 由 app/服务发——KMS 只存不代发）
```
1. app → startContactBinding({account,'email',email_address}) → passkey ceremony → KMS 验 owner
        → 签发 verify_token(绑定到 {account,'email',email_address}，TTL，pending) → 返 app
2. app/邮件服务 把 verify_token(或含 token 的链接) 发到 email_address（KMS 不发邮件）
3. 用户在邮箱读 token → 回 app 填（或点链接回 app）
4. app → KMS confirm-binding {bindingCode, verify_token} → verified，持久化 email
```
> email 无"claim"步（地址 begin 时已给）；verify-token 回环同样把"收到 token 的人"和"app passkey 会话"绑一起。

### 3.3 API（HTTP host 层；复用 ceremony/nonce/api-key，**无 proto/TA 改动**）
| 端点 | 调用方(鉴权) | 入参 | 出参 |
|---|---|---|---|
| `POST /contact/begin-binding` | app/SDK(owner ceremony) | `{account, channel, email_address?}` | `{bindingCode, expiresAt}` |
| `POST /contact/claim-binding` | bot(api-key) [仅 telegram] | `{bindingCode, telegram_chat_id, telegram_username}` | `{verifyToken, expiresAt}` |
| `POST /contact/confirm-binding` | app/SDK | `{bindingCode, verifyToken}` | `{status:'verified', channel, maskedContact}` |
| `GET /contact/{account}` | DVT node(api-key) 或 owner(ceremony) | — | `{contacts:[{channel, contactRef, status, verifiedAt}]}` |
| `POST /contact/unbind` | app/SDK(owner ceremony) | `{account, channel}` | `{status:'revoked'}` |

owner ceremony commitment：`SHA-256(nonce ‖ SHA-256("AA-CONTACT-BIND-v1" ‖ account ‖ channel))`。

### 3.4 存储 schema（host DB，**PII 非 TEE，与 key 存储隔离**）
```
contact_bindings(
  account TEXT, channel TEXT,            -- 'telegram' | 'email'
  contact_ref TEXT,                      -- verified chat_id / email（verified 后填，at-rest 加密）
  display_hint TEXT,                     -- telegram @username / 邮箱掩码，供 app 给用户核对
  status TEXT,                           -- pending | claimed | verified | revoked
  binding_code TEXT, verify_token TEXT,  -- 一次性，verified/过期清空
  bot_id TEXT,                           -- 共享 bot 固定（telegram）
  created_at, claimed_at, verified_at, expires_at,
  UNIQUE(account, channel))
```

### 3.5 PII / 隐私
- `contact_ref` **at-rest 加密**；`getContact` **严格鉴权**（仅授权 DVT 节点 api-key + owner 本人 ceremony，**绝不公开**）。
- 数据最小化（只存送达必需 + 核对用 hint）；`unbind` 物理删除；与 TEE key 存储隔离（独立表/访问控制/不进 TEE）。

## 4. Phase 2（规划）：绑定关系导出 / 社交恢复导入

**动机**：绑定关系存在 KMS-A。用户 social recovery / 迁移到 KMS-B（换实例=换 rpId，见 `project_decentralization_model`）时，绑定丢在 KMS-A。希望**可携带**。

**思路（待细化）**：
- **导出**：KMS-A 产出用户绑定的**加密 blob**。加密 key = **passkey 派生**（WebAuthn **PRF 扩展**从 passkey 派生稳定密钥；passkey 云同步、跨实例存在）。blob 由用户/app 持有（或加密备份）。
- **导入**：迁移/恢复到 KMS-B 后，用户 passkey 认证 → 导入 blob → KMS-B 用 passkey-PRF 解密 → 重建 `contact_bindings`。
- **为什么可行**：本生态恢复模型 = passkey 云同步（可靠不丢）+ social recovery 换 KMS key。**passkey 跨实例持续存在** → passkey-PRF 派生的加密 key 也跨实例可用 → 绑定 blob 可在新 KMS 解密重建。
- **边界**：blob 是 PII，加密后才可携带；导入要重新过 owner 验证；与 aastar-sdk#192（绑定备份/迁移）协同。
- **范围**：**Phase 2**，Phase 1（telegram+email 绑定）跑通后做。

## 5. 跨仓库分工（#193 收敛，最终）
| 仓库 | 活 |
|---|---|
| **KMS** | `contact_bindings` 表 + begin/claim/confirm/get/unbind；owner=passkey ceremony+commitment；getContact 严格鉴权；contact 与 key 隔离。**确认流程零新增**（复用 sign-with-ceremony）。|
| **DVT** | NotificationService 改调 KMS `getContact`（每节点各自发，fail-closed）；`/signature/confirm` 凭证 token→owner passkey 签 userOpHash；共享 bot 降为 ping。|
| **SDK** | 封装 `startContactBinding/confirmContactBinding/getContact/unbind`；确认侧 approve 助手（passkey 签 userOpHash）。|
| **YAA** | 绑定 UI + 带外确认 UI（bot ping → passkey ceremony 批准，不输 token）。|

## 6. 讨论记录（决策留痕）
- **bot 凭证：方案 2 胜出**（#193 四仓库一致）：批准凭证与 bot 解耦（passkey 签 userOpHash），bot 只 ping，被攻陷只能 DoS。否决方案 1（bot 进 TCB 的 bearer-token）作终态。
- **Telegram 约束逼出共享 bot**：bot 不能给未 /start 用户发 → per-node bot 不可行 → 共享 bot 收+投递，但**凭证不让 bot 看见**（方案 2）。
- **绑定的 chat_id 映射：共享 bot 在 TCB（不可约）**——codex/opus 复审纠正了原"bot 只能 DoS、伪造不出绑定"的过度声明：bot 是 telegram 发送方唯一来源、能谎报 chat_id，但后果被 path-2 兜在"通知误投+元数据泄漏"、非批准/盗款。verify-token 回环防的是非 bot 第三方抢 code，不是防 bot 本身。详见 §2。
- **Phase 2 导出/恢复**（本次新增产品规划）：passkey-PRF 加密的可携带绑定 blob，跨 KMS 迁移/恢复可导入。
