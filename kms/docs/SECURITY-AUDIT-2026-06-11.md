# AirAccount KMS 全面安全审计报告

**日期**: 2026-06-11
**审计分支**: `fix/review-bugfix`（commit `9aeb685`，含全部 challenger-review 修复）
**审计范围**: TA（Secure World）、Host CA（Normal World）、架构、测试、8 个 Open PR、13 个 Open Issue
**方法**: 6 路并行深度审计 + 主审人对全部 Critical/High 发现逐条源码复核（标 ✓ 为主审人亲自验证）

---

## 执行摘要

| 维度 | 评分 | 一句话结论 |
|------|:----:|-----------|
| TA 安全 | 7.5/10 | 私钥生命周期与反回滚扎实，无密钥外泄路径；迁移期 TLS 顺序有一处 High |
| Host 安全 | 6.5/10 | 新路径（agent/p256/grant）防护严密，**老路径（Sign/SignHash）鉴权明显偏弱**；无 TEE 调用超时是全局 DoS 单点 |
| 架构合理性 | 7/10 | proto 边界干净、会话池化正确；状态一致性（TA↔SQLite 无对账）与 submodule 补丁未上游化是两大债 |
| 测试成熟度 | 4/10 | 108 个单测但**覆盖与风险完全倒挂**——安全核心（RPMB/WebAuthn/迁移）覆盖率为零，CI 只查 proto |
| **综合** | **6.5/10** | 上 beta 前必须修复 3 个 P0（见下） |

---

## 一、P0 — Beta 阻断项（全部经主审人源码复核确认）

### P0-1 ✓ 无 TEE 调用超时 → 单点全局 DoS
**位置**: `kms/host/src/ta_client.rs:444-480`（`TeeHandle::call` 的 `reply_rx.await` 无超时）

经亲自 grep 确认：全 crate 唯一的 30s 是断路器恢复窗口 `CB_RECOVERY_SECS`，**不存在单次调用超时**（PR #35 描述里的"30s TEE call timeout"在 #35 分支上，未进本分支）。所有 TEE 调用经唯一 worker 线程串行；一条让 TA 挂起的请求会令**所有用户的所有请求**永久排队。断路器只对"返回错误"生效，对"挂起不返回"无效。

**修复**: `tokio::time::timeout(30s, reply_rx)` + 超时计入断路器失败 + 文档化"TEE 调用不可取消"语义。注意超时后 TA 可能已完成副作用（尤其 ChangePasskey：TA 改成功但 host DB 没更新 → wallet 被锁死），有副作用的操作需对账。

### P0-2 ✓ 传统签名主路径 passkey 可整体省略 + legacy 断言可无限重放
**位置**: `kms/host/src/api_server.rs:1306-1372`（`resolve_passkey_assertion`）

经亲自读码确认两点：
1. 既无 `webauthn` 又无 `passkey` 字段时返回 `Ok(None)` 直接透传 TA——host 侧对 Sign/SignHash/DeriveAddress/DeleteKey **不强制断言存在**（agent/p256/grant 端点全都强制，唯独最核心的老路径不强制）。
2. legacy raw 断言路径仍被接受，代码注释自认 "Vulnerable to replay"——只打了一行 deprecation 警告。这是 Issue #49 之外的**另一条**重放通道。

**修复**: 对绑定了 passkey 的 wallet 强制 `assertion.is_some()`；签名/删除端点彻底移除 legacy raw 路径。

### P0-3 ✓ Command ID 23 冲突 — 合并即静默错路由
**位置**: `kms/proto/src/lib.rs:47`

亲自 diff 确认：本分支 `ReadRollbackCounter = 23`，`feat/mx93-deployment`（PR #35）`ForceRemoveWallet = 23`。两个 open 分支对同一命令 ID 的分配冲突。若合并时处理不当，host 发 ForceRemoveWallet 会被 TA 当 ReadRollbackCounter 执行（或反之）——**静默错路由比编译失败更危险**。

**修复**: 合并前把其中一个重编号为 24，并在 proto 测试里加 ID 唯一性断言。

### P0-4 Submodule RPMB 补丁未上游化 — fresh clone 必挂
**位置**: `.gitmodules` + `third_party/teaclave-trustzone-sdk`（本地 commits `b21d6be`/`a75d631`）

RPMB 支持和 C-1 crash-safe 修复只存在于本地 submodule 工作树；`.gitmodules` 指向 apache 上游（永远不会包含这些 commit）。**任何人 fresh clone + `git submodule update` 拿到的代码编译不过**。CI 也无法构建 TA（这正是 CI 只查 proto 的根因之一）。

**修复**: fork 到 AAStarCommunity 名下推送这两个 commit 并改 `.gitmodules` URL；或把 patched `secure_db` vendor 进主仓库。

---

## 二、High 发现

### H-A ✓ 迁移启动当次的 TLS 顺序违规（TA）
**位置**: `kms/ta/src/main.rs:495→510`（`load_wallet_cached`）、`706→736`（`create_wallet`）、`832→835`（`derive_address_auto`）

经亲自读码确认：`open_rpmb_migrating` 在**迁移启动当次**会执行 RPMB put/marker 写和 REE clear（全是 TEE 写，污染 `tpidr_el0`），随后的 `cache_put`/`cache_get`（thread_local）违反 TLS 铁律。防护注释覆盖了 `db.put`/`delete_entry`/`rpmb_write_counter`，唯独漏判了 `open_rpmb_migrating` 自身是写操作。

**缓解**: marker 先落盘，panic 后重试走只读快路径，**可自愈、不砖化**——固件升级后首批命令各失败一次。
**修复**: 升级后首次启动在 `ta_open_session`（或 invoke 入口、任何 thread_local 之前）单独跑一次迁移，与业务命令解耦。

### H-B 超时/取消语义缺失 → TA 与 host 状态分歧（host）
`ChangePasskey` TA 成功而 host DB 更新失败 → DB 存旧公钥 → 该 wallet 的 WebAuthn 永远验证失败（用户锁死）。需要"TA 成功后立即原子更新 DB + 对账任务"。

### H-C 状态一致性无对账机制（架构）
- `create_key`: 先 TA 后 DB，中间失败 → TA 孤儿钱包占 RPMB 槽位，host 无从感知
- `delete_key`: 先 TA 后 DB，中间失败 → DB 幽灵记录
- db.rs 注释承诺"DB 可从 TA 恢复"，**该恢复工具不存在**
- P256 session key 子系统有完整两阶段状态机（pending + TTL + GC + tee_deleted 标志），是正确范本——wallet 路径应照抄

### H-D 测试与风险倒挂（测试）
零覆盖清单（全是安全核心）: `epoch_check` 边界、`open_rpmb_migrating` 幂等性、WebAuthn 负向（错误 rpId / UP=0）、`WalletLegacy` bincode 回退、backup 往返。其中 `epoch_check` 和 `WalletLegacy` 是**纯函数，今天就能补单测**。CI 实质只跑 `cargo check -p proto`，host 现成的 94 个单测都没纳入门禁。

---

## 三、Medium 发现（摘要）

| # | 位置 | 问题 |
|---|------|------|
| M-a | `agent_jwt.rs` | JWT 无 `aud`/`iss`；agent JWT 与 p256 session JWT 格式相同，仅靠 DB hash 比对隔离（防住了，但脆弱） |
| M-b | `api_server.rs:3937` | 限速 per-API-key，key 关闭时全员共享 anonymous 桶；Begin* 写库端点无限速 |
| M-c | `db.rs:1006` + 无调用者 | `cleanup_expired_challenges` 从未被调用，challenges 表可被灌爆（存储 DoS） |
| M-d | `db.rs:1072` | API key 比对非恒定时间（SQLite 字符串比较 + Rust `==`） |
| M-e | 跨层 | 错误处理靠字符串子串匹配反推 HTTP 状态码（`contains("0xffff")`），文案一改 401/503 静默变 400 |
| M-f | `db.rs:228` | 单 Mutex SQLite 连接抵消了 WAL 并发读优势；纯读请求被无谓串行化 |
| M-g | proto | bincode 下 `#[serde(default)]` 不提供真正的向后兼容（按字节顺序读），host/TA 必须同版本部署——无版本协商机制 |
| M-h | PR #46 | 备份分支误含 #43 的一半（有 ff41bbf 无 8bf56eb 兼容修复），**单独合并会砖存量钱包** |
| M-i | PR #44 | TA 硬编码 rpId="aastar.io" 与 CA 的 KMS_RP_ID 多域配置矛盾，无灰度开关；更优方案是注册时 per-wallet 绑定 rpIdHash |

Low/Info 项（错误信息泄露、emoji 日志、死代码 address_cache.rs、中英注释混用、Cargo 元数据等）详见各分项报告，不再展开。

---

## 四、PR 处置建议

**核心结论：#43/#44/#45/#46/#47 全部关闭，由 `fix/review-bugfix` 开一个统一 PR 取代。**

理由（已验证）：
1. `fix/review-bugfix` 是五者严格超集，且独有 `9aeb685` 修复（含 C-2 eMMC 重刷砖机修复——重刷是你们的常规运维操作，PR #43/#47 原分支版本会在重刷后把所有旧钱包判为篡改）
2. #46 单独合并会砖存量钱包（M-h）
3. 五个 PR 中三个描述已过期（#44 方案变了、#45 方案变了、#46 混入了 TA 代码）

**合并顺序**:
1. #2（CLA+CONTRIBUTING）→ #33（README，与 #2 有一处冲突，可 squash 进 #2 后关闭）
2. #35（生产基线，必须进 main）——**合并前先解决 Command 23 冲突（P0-3）**
3. 解决 submodule 上游化（P0-4）
4. `fix/review-bugfix` rebase 到新 main（与 #35 有 7 处冲突），开统一 PR，合并
5. 关闭 #43-#47 并留言 "superseded by #N"

---

## 五、Issue 路线图

**建议关闭**（条件达成后）：
- #36（RPMB 防回滚）→ 统一 PR 合并 + 硬件 E2E 后关闭，实现已超出原方案
- #39（FIDO/TA 内验证）→ 同上，关闭语注明重放 gap 转 #49
- #41（ForceRemoveWallet）→ #35 合并后关闭，**前提是先解决 cmd 23 冲突**
- #40（CAAM 加速）→ 关闭或并入 #48：**i.MX93 没有 CAAM，是 ELE**（与 memory 中 TRNG 教训一致），性能估算基础错误
- #11（grant 收紧通知）→ 跑一次与合约的字节级向量测试后关闭

**需更新正文**: #21（引用的代码行不在 main——p2-sp-signers 滞留分支未落 main）、#37（真实前置是 Secure Boot/AHAB + ELE 证书，不是 #36）、#42（"#41 已合并"表述不实）、#6（勾选进度）

**执行顺序**:
- **Beta 前 (P0)**: 统一 PR 合并 + 硬件 E2E；#49 challenge 绑定（注意：TA 需收完整 clientDataJSON 自己算哈希，仅收 32 字节哈希无法解析 challenge）
- **Beta 前后 (P1)**: #15 方案 B（TEE_GetTAPersistentTime 相对 TTL，可与 #49 同批）；#6 P3 Sepolia E2E
- **Post-beta (P2)**: #48 ELE 调研（上游，决定 #40/#37/#42/#38 走向）→ #37 attestation → #42 生命周期 → #38 PKCS#11（**最后做，且先出"哪类 key 允许绕过 WebAuthn"的设计决议**，否则违反项目安全红线）

---

## 六、做得好的地方（经核实）

- 私钥生命周期：Drop 全量清零、export-secrets feature gate 生产硬拒绝、release 日志不含密钥、输出缓冲 C-4 边界
- RPMB 反回滚：单调护栏、C-2 自愈、恢复幂等、TLS 顺序（除 H-A 一处）全部正确
- 命令枚举演进：删除 ID 留空不复用 + 回归测试（工业级做法）
- WebAuthn ceremony 完整实现（type/challenge/origin/rpIdHash/UP+UV/signCount）
- P256 session key 两阶段状态机 + `BEGIN IMMEDIATE` 防 TOCTOU
- TEE 会话池化 + 断路器 + 自动重连；TA 内 LRU 用 Vec 规避 getrandom panic（对 TEE 约束理解正确）
- 所有 SQL 参数化，无注入；请求体 256KB 上限
- JWT alg 锁死 HS256，HMAC 在 TEE 内验证，host 不持密钥

---

## 七、行动清单（按优先级）

| # | 行动 | 工作量 | 阻断 beta? |
|---|------|:------:|:----------:|
| 1 | Command 23 重编号 + proto ID 唯一性测试 | 0.5h | ✅ |
| 2 | TEE 调用 30s 超时 + 断路器联动 | 2h | ✅ |
| 3 | Sign/SignHash/DeriveAddress/DeleteKey 强制 passkey 断言 + 移除 legacy 路径 | 3h | ✅ |
| 4 | Submodule fork 上游化 / vendor | 2h | ✅（CI 依赖） |
| 5 | 迁移与 thread_local 解耦（H-A） | 2h | 建议 |
| 6 | `epoch_check` + `WalletLegacy` 纯函数单测 | 2h | 建议 |
| 7 | CI 纳入 host 单测 + TA 交叉编译 job | 3h | 建议 |
| 8 | challenges 表定时清理 + Begin* 限速 | 2h | 否 |
| 9 | wallet create/delete 两阶段状态机（照抄 P256 范式） | 1-2d | 否 |
| 10 | 类型化错误枚举替换字符串匹配 | 1-2d | 否 |

---

*审计方法说明: 本报告由 6 路并行深度审计（TA 安全、Host 安全、架构一致性、测试覆盖、PR 元审查、Issue 评审）汇总而成，全部 P0/High 发现经主审人逐条源码复核（文中标 ✓）。已排除已知跟踪项（Issue #48/#49、M-8 等）的重复报告。*
