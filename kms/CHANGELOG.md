# KMS Changelog

> Updated: 2026-06-27

## 0.27.2 (2026-06-27) — Beta5 — 地址大小写归一化（fail-closed 隐患修复）

### 修复 (Fix) — #129/#203（coordinator review 揪出的 fail-closed 隐患）
- **地址 key 全链路归一化 lowercase**，消除大小写不匹配导致的静默 fail-closed。KMS 各处用 `hex::encode` 存小写地址，但 `lookup_address` 是 case-sensitive 精确匹配；SDK(#203) 传 EIP-55 校验和、DVT 传 userOp.sender → 不匹配 → **/contact + /verify-confirm-assertion 对合法用户永远查不到/返 false**（按地址来源时灵时不灵、极难排查）。
- 三处 DB 路径 + 一处 legacy 缓存归一化：
  - `lookup_address`：query 输入小写 → 匹配小写存储（一处根治所有 consumer 含 Sign/SignHash + contact/verify）。
  - `upsert_address`：写入小写（防御不变量）。
  - **`record_tx`**：`tx_log.addr` 写入小写（codex round-12 BLOCKER）——否则校验和 Sign 成功后写校验和 addr，dormant 检查（#42 freeze_dormant_keys 按 addr case-sensitive 比对）失配 → 活跃钱包被误判 dormant、误冻结。
  - `address_cache.rs`（legacy JSON 缓存，无活跃调用）：同步加小写（防将来被接上重现）。
- 测试：`lookup_address_is_case_insensitive`、`last_used_at_resolves_checksummed_tx_addr` + 修 `address_for_key_path_lookup`。67 host 单测全过。codex round-13 确认 DB 不变量 airtight。

### 文档 (Docs) — OpenAPI 规范追平 deployed surface
- `openapi.yaml` 版本号 0.26.1 → **0.27.2**，补齐 0.27.0 起新增、却从未写进 spec 的 6 个端点（#124/#129）：`POST /verify-confirm-assertion`、`POST /contact/{begin,claim,confirm}-binding`、`POST /contact/unbind`、`GET /contact/{account}` + 对应 request/response schema 与两个新 tag（Contact Binding / DVT Confirm）。纯文档，二进制不变。

> 双轨：CA(host) **0.27.2** · TA 0.8.0 · proto 0.7.0。CA-only，重编 CA + 重启。

## 0.27.1 (2026-06-26) — Beta5 — contact/verify 端点 account 接受 address 或 key_id（修复）

### 修复 (Fix) — #129/#124（aastar-sdk#203 review 发现）
- **`/contact/*` + `/verify-confirm-assertion` 的 `account` 现接受钱包地址或 key_id**。原实现只按 `key_id`(UUID) 查 `get_wallet`，但消费方传以太坊地址（SDK#203 用 `account: Address`；DVT 手上是 userOp.sender 地址）→ 运行时 "Key not found"，集成跑不通。
- 新增 `resolve_account_key_id`：`wallet_exists` → key_id 原样；否则 `lookup_address`（`address_index`，与现有 Sign/SignHash 端点同款）→ 其 key_id；否则回退原串（not-found 行为不变）。begin/confirm/get/unbind/verify_confirm 全程用解析后的 key_id（ceremony + DB 一致 → 无跨账户）。codex 复审 SOUND。
- host-only，TA/proto 不变。

> 双轨：CA(host) **0.27.1** · TA 0.8.0 · proto 0.7.0。CA-only，重编 CA + 重启即可。

## 0.27.0 (2026-06-26) — Beta5 — DVT 带外确认验签端点 + 通知联系方式绑定（#124 / #129）

> **host-only 新增端点，无 proto/TA 改动 → TA 不需重编/重刷**，只重编 CA + 重启 kms-api。全部新端点 **`x-api-key` 鉴权 + 限流**；POST 端点 64KB body 上限。

### 新增 (Feature) — DVT 带外确认 RP 验签（#124，Validator path-2）
- **`POST /verify-confirm-assertion`** `{account, userOpHash, passkey}` → `{verified}`。KMS 作 WebAuthn **RP** 为 DVT 节点验 owner 对 `userOpHash` 的 passkey assertion（节点自己不跑 WebAuthn）。
  - 复用 `verify_authentication_response`：`expected_challenge = userOpHash`（WYSIWYS）、`sign_count=0`（跳计数器校验+不更新 → **无状态、幂等**，quorum 多节点可各自验同一 assertion）、`delegate=false`（host 强制 challenge==userOpHash）；rpId 循环（prod 仅 aastar.io）。
  - **要求 passkey（P256）**：泄漏的 secp256k1 owner key 单独**过不了** → 真防 owner-key 被盗。账户不存在/无 passkey/frozen/验签失败统一 `{verified:false}`（无枚举 oracle）。
  - 部署不变量：生产须 `KMS_REQUIRE_API_KEY=1` + 每 DVT 节点独立可撤销 key；fail-closed（KMS 不可达=节点不放行）。

### 新增 (Feature) — 通知联系方式绑定（#129，Phase 1：Telegram）
经验证后把用户的 Telegram 绑定到 AirAccount，供 DVT 带外确认触达真实用户。**5 个端点**：
- **`POST /contact/begin-binding`** `{account, channel, WebAuthn}` → `{bindingCode, expiresAt}` —— **owner ceremony**（消费 challenge + 验 owner 再落库）；256-bit code；仅 telegram（email 待 `begin_email_binding`）。
- **`POST /contact/claim-binding`** `{bindingCode, telegramChatId, telegramUsername?}` → `{verifyToken, expiresAt}` —— bot api-key；first-claim-wins。
- **`POST /contact/confirm-binding`** `{account, bindingCode, verifyToken, WebAuthn}` → `{status:'verified'}` —— **owner ceremony 必需**（bot 知道 verifyToken，故 confirm 必须 owner 才能完成；bot 做不到）+ DB 校验 account 归属（防跨账户确认）。
- **`GET /contact/{account}`** → `{contacts:[{channel, contactRef, status, verifiedAt}]}` —— api-key（DVT 节点）；仅 verified，不返 secrets；读取走 access log。
- **`POST /contact/unbind`** `{account, channel, WebAuthn}` → `{status}` —— owner ceremony。
- 存储：host-side `contact_bindings` 表（**PII 非 TEE，与 key 存储隔离**，FK cascade，一次性 code/token）。信任模型见 `docs/design/contact-binding-kms.md` §2（被攻陷 bot 后果限于"通知误投+元数据泄漏"、非批准/盗款——批准走 path-2 passkey）。

### 加固 / 修复 (Hardening) — codex+opus 多轮 review
- **强制 owner ceremony 存在**：begin/confirm/unbind 对 `resolve_passkey_assertion` 返回 `None`（省略 `WebAuthn` 字段）显式拒绝（`ok_or_else`），堵住"省字段绕过 ceremony"。
- contact 端点接受规范 **`WebAuthn`** 字段名（与现有 API 一致）；`confirm_email_binding` 预加 account 校验（email 上线前堵跨账户洞）；malformed/超大 body → 400/413（原 500）。

### 跟踪的 Phase-1 follow-up（设计文档 §2，非阻塞）
api-key scoping/key-classes · owner ceremony 绑 `{account,channel}` commitment · bot-key-class + `bot_id` 服务端导出 · email 端点（待 `begin_email_binding`）· `contact_ref` at-rest 加密（pre-GA）。

> 双轨版本：CA(host) **0.27.0** · TA **0.8.0（不变）** · proto **0.7.0（不变）**。**CA-only（新增 host 端点）**——重编 CA + 重启即可，TA 不刷。

## 0.26.1 (2026-06-22) — Beta5 — /version 报告 challenge_mode（strict/transition 可辨）

### 可观测 (Observability)
- **`/version` 新增 `challenge_mode` 字段**（`"strict"` / `"transition"`）——一眼区分 strict 板（拒裸 nonce/无 clientDataJSON、强制 payload commitment，#63）与 transition 板。
- 机制：CA 新增 report-only `strict-challenge` feature，由**同一个 `MX93_STRICT_CHALLENGE` 构建开关**设置（与权威的 TA `strict-challenge` feature 同步）。CA **不**强制 strict——TA 才是；该 flag 纯为 `/version` 上报。`mx93-build.sh` 的 build_ca 已联动。

> 双轨版本：CA(host) **0.26.1** · TA 0.8.0（不变）· proto 0.7.0（不变）。CA-only（observability）。strict 板重刷 CA 即显示 challenge_mode=strict。
## 0.26.0 (2026-06-22) — Beta5 — mint label 绑定（#115 正确版）+ 凭证 TTL 上限 24h

### 安全 (Security) — #115（正确版，取代 v0.25.2 已回退的实现）
- **mint 把 CLIENT-AUTHORIZED 输入绑进 challenge**：`create_agent_key` / `create_p256_session_key` 的 TA 校验改为 `Some(mint_label_digest)`，`mint_label_digest = SHA-256(tag ‖ wallet_id[16] ‖ SHA-256(label))`，tag = `AA-AGENT-MINT-v2` / `AA-P256-SESSION-MINT-v2`。strict 下被攻陷的 CA **无法在用户手势里篡改 label**。
- 只绑**客户端可知/授权**的值（wallet_id + label）；服务端派生的 `index`（assertion 后才原子分配，客户端无法预知）、`subject`、`ttl` 不绑——这正是 v0.25.2 实现回退的原因（绑了 index → strict 不可满足）。proto 加 `label` 字段，host 转发 `req.label`。agent-credential **refresh** 无 label → 绑空 label（仍绑 wallet_id）。
- **transition-safe**：TA 仍接受裸 nonce（nonce 防重放始终保留）；strict 才强制 `challenge == SHA-256(nonce ‖ mint_label_digest)`。
- **#111 用户在场校验不变**（关键防线）——本次只是给那次校验**叠加** label 绑定（nonce + label，不是替换 nonce）。

### 加固 (Hardening) — 凭证 TTL 上限 24h
- `MAX_AGENT_JWT_TTL` 7 天 → **24 小时**；agent/p256 默认 ttl 3 天 → 24h。压缩委托签名窗口：泄漏/被滥用的凭证（或被攻陷 CA 想拉长的 ttl）最多有效 24h。agent/session 每天重新铸（重新 passkey）。

### 测试 (Test) — #119
- `kms/docs/test-vectors/`：mint（agent/p256/refresh 空 label）+ grant（空/非空 pad-32）锁定向量 + `compute_vectors.py` 全 ALL PASS。

### ⚠️ 部署 (Deploy)
- **proto 0.6.0 → 0.7.0（新增 `label` 字段，wire 变）→ host + TA 必须 co-deploy**。SDK 须对 mint 发 `SHA-256(nonce ‖ mint_label_digest)`（不再是 v0.25.2 的 index 版、也不是裸 nonce）。

> 双轨版本：CA(host) **0.26.0** · TA **0.8.0** · proto **0.7.0**。三者 co-deploy。

## 0.25.3 (2026-06-22) — Beta5 — grant 数组 pad-32 修复（#112）+ 回退 #115 mint 绑定 + #119 向量

### 修复 (Fix) — #112，consensus-critical
- **grant 的 callTargets/selectorAllowlist 数组打包：tight-pack → pad-32**。`keccak_packed_addresses`/`keccak_packed_selectors` 此前 tight-pack（地址 20B/选择器 4B），但 Solidity `abi.encodePacked(address[]/bytes4[])`（合约 `SessionKeyValidator._buildGrantHash` 与 SDK viem 一致）**对数组每个元素补 32 字节**（address 左补、bytes4 右补）。空数组两者相同（`keccak("")`），故此前测试没暴露；**非空 scoped grant** 的 TA final_hash ≠ 合约 → TA 签名链上验不过、strict commitment 也对不上。**独立于 strict 的真 bug，非空 scoped grant 此前就坏。** 已用 SDK 的 live 合约 oracle 向量逐字节验证（k1+p256）。

### 回退 (Revert) — #115 mint 参数绑定 → nonce-only
- v0.25.2 的 #115 把 mint challenge 绑定到 `index/subject/ttl`，但这些值**全是服务端/host 派生**（`agent_index`/`session_index` 原子分配、subject/ttl host 派生），客户端 ceremony 只发 `human_key_id+label` → **strict 下客户端算不出 commitment（不可满足），会炸 strict mint**。回退为 nonce-only：mint 的保护是 **#111 TA 内校验用户在场**（关键防线，不变）+ 一次性 nonce。真正的参数加固应绑**客户端授权输入**（human_key_id+label，需 label 接进 TA）——见 follow-up。

### 测试 (Test) — #119
- `kms/docs/test-vectors/commitment-vectors.json` + `compute_vectors.py`：grant commitment 锁定向量（空 + 非空数组）+ 可运行 cross-check（防 pad-32 规格再漂移）。

> 双轨版本：CA(host) **0.25.3** · TA **0.7.2**（pad-32 + mint 回退）· proto 0.6.0（不变）。TA 改动 → 需刷 TA。

## 0.25.2 (2026-06-22) — Beta5 — mint 参数绑进 challenge（#115）+ 凭证信任模型文档（#117）

### 安全 (Security) — #115
- **mint 类操作把 mint 参数绑进 WebAuthn challenge**：`create_agent_key` / `create_p256_session_key` 的 TA 校验从 `None`（仅 nonce）改为 `Some(mint_digest)`——绑定 `index / ttl / subject`。strict 下被攻陷的 CA **无法借一次用户手势篡改铸造参数**（如偷偷延长 ttl、换 subject）。
- mint_digest 规格（域分离、定长，SDK 须一致复算）：`SHA-256(domain_tag ‖ wallet_id[16] ‖ index[u32 BE,4] ‖ ttl_secs[i64 BE,8] ‖ SHA-256(subject)[32])`，tag = `AA-AGENT-MINT-v1` / `AA-P256-SESSION-MINT-v1`。客户端 commitment = `SHA-256(nonce ‖ mint_digest)`。
- **transition-safe**：TA 仍接受裸 nonce（SDK 现有 mint 流程不变）；strict 才强制 commitment。无 proto / host 改动（TA-only；assertion 字段 #111 已就位）。

### 文档 (Docs) — #117
- `docs/TRUST.md` §7：委托签名凭证（agent key / P256 session key）信任模型——**铸造时用户在场 + 参数绑定（强）**，**TTL 窗口内委托签名（有意折中，靠短 TTL + scope + 撤销控制）**。澄清 `sign_p256_user_op` 每笔不验在场是 by-design 委托，非 bug。

> 双轨版本：CA(host) **0.25.2** · TA **0.7.1**（mint 绑定）· proto 0.6.0（不变）。TA 改动 → 需刷 TA。

## 0.25.1 (2026-06-22) — Beta5 — grant-session 走 TA 挑战绑定（#112，strict 前置）

**主题：让 grant-session 的 challenge 由 TA 发、被 TA 绑定**——清掉 strict 翻转（#63）的最后一个 KMS 侧依赖。此前 grant 用 host 随机 challenge + host strip 掉 client_data_json → TA 拿不到 → 只有 host 绑定（弱于核心签名），且 strict 下会被硬拒。

### 修复 (Fix)
- `begin_grant_session_auth`：改用 **TA `GetChallenge`**（镜像常规 BeginAuthentication），TA nonce 进 pending 表（GetChallenge 不可用时回退 host-random）。
- `resolve_grant_passkey_assertion`：**停止 strip `client_data_json`** + host 改 `delegate_challenge_to_ta=true` —— TA 现在二次绑定 grant（`sign_grant_session`/`sign_p256_grant_session` 已传 `Some(final_hash)` → payload commitment）。host 仍验签名 + origin + rpId + 一次性 challenge_id。

### 安全 / 兼容
- **transition-safe**：SDK 当前「签 begin 返回的 challenge（裸）」照旧能用（现在那是 TA nonce，TA peek 到、transition 接受裸 nonce）。strict 时 SDK 对 grant 发 commitment（aastar-sdk#135 item3）。
- grant 现与常规 sign 共享 **per-wallet 单 nonce 槽**（TA "one live nonce per wallet"）——与「并发常规 sign」是同一既有限制（非本次新增）；ceremony 顺序执行不受影响。多槽/purpose-aware pending 表是单独 TA 改进（后续）。
- ⚠️ 这是 **#63 strict 翻转的前置**：grant 不再被 strict 硬拒。

> 双轨版本：CA(host) **0.25.1** · TA 0.7.0（不变）· proto 0.6.0（不变）。**host-only**（无需刷 TA）。

## 0.25.0 (2026-06-22) — Beta5 — TA 验证 P256 session key 创建的用户在场（#111，CRITICAL）

**主题：关闭 CreateP256SessionKey 的「无 passkey 也能铸 TEE 签名凭证」通道。** 此前 host 验 WebAuthn 但**不转发给 TA**，TA 也不验 → 被攻陷的 CA 可直接发 `CreateP256SessionKey` 给 TA，零用户在场铸出 P256 session key + TEE-HMAC JWT，再以用户身份签 UserOp。这是 #110 一致性审计 + Codex 双重确认的 CRITICAL（D1）。

### 安全 (Security)
- **TA 在铸密钥/签发 JWT 前 `verify_passkey_for_wallet`**（`ta/main.rs` create_p256_session_key）——绑定 wallet 的 passkey，None payload = nonce 绑定（mint op，比照 create_agent_key 的 C-1 防御）。host 仍验 WebAuthn → host+TA 双验。
- `CreateP256SessionKeyInput` 新增 `passkey_assertion`，host 转发已验断言给 TA。
- 被攻陷 CA 现在**无法**绕过用户在场铸 session 凭证；重放由 host(challenge_id) + TA(nonce) 双重一次性消费拦截。

### ⚠️ 部署 (Deploy)
- **proto 线格式变更**（新增字段）→ **host + TA 必须同版本一起部署**（bincode 非自描述）。dev 板 TA+CA 一并重刷。
- SDK 透明：host 本就强制 create-p256 带 WebAuthn 断言，调用方式不变。

### 审查
- CA/TA 一致性审计发现（D1）+ Codex 多轮对抗复核修复。

> 双轨版本：CA(host) **0.25.0** · TA **0.7.0**（加 passkey 校验）· proto **0.6.0**（新增字段）。三者 co-deploy。

## 0.24.2 (2026-06-22) — Beta5 — host/TA challenge-binding 对齐（#110）

**主题：修复 host(CA) 与 TA 对 WebAuthn challenge 期望不一致。** TA（#68）对签名 op 要求 challenge = `SHA256(nonce‖payload)`（payload commitment），但 host `verify_authentication_response` 硬要求 challenge == 裸 nonce → host 在 TA 之前就拒掉 commitment，commitment 路径死、strict 不可达。这是 CA/TA 不一致 bug（TA 改了 #68、host 没跟上）。

### 修复 (Fix)
- host `verify_authentication_response` 加 `delegate_challenge_to_ta` 开关：对 **TA 会二次绑定的签名 op**，host 不再以裸 nonce 硬拒非 nonce challenge，把 challenge↔payload 绑定**委托给 TA**（权威层）；仍验断言签名 + origin + rpId + flags + sign_count + 一次性消费 challenge_id。
- per-op 映射：`Sign`/`SignHash`/`SignTypedData`（含 voucher/GToken/x402 便利签名）→ delegate（TA 绑 `Some(digest)`）；`ChangePasskey`/`DeriveAddress`/`DeleteKey` → 不委托（TA 绑 `None`=nonce，host 保持 strict）；`UnfreezeKey` → **不委托**（纯 host DB、不到 TA，host 是唯一门）；agent / p256-session 创建 → 不委托。
- 一致性审计连带修复 **Finding 1**：`sign_typed_data` 的 WebAuthn 路径原本漏配（同 #110），已纳入。

### 安全 (Security)
- 信任模型：**TA 是 challenge↔nonce↔payload 绑定的唯一权威，host 为便利层**。CA 作恶（重放/payload 调包）由 TA 拦（重算 commitment + 一次性 nonce）。
- 经 CA/TA 一致性审计 + Codex 两轮对抗审查；修掉 codex 发现的 `UnfreezeKey` 委托漏洞（Q2，本次引入即修）。
- ⚠️ **已知残留风险（维护者明示接受，本 PR 不含）**：
  - **#111（CRITICAL）** `CreateP256SessionKey` TA 不验用户在场 → 被攻陷 CA 可无 passkey 铸 session key + JWT。需 proto+TA 改动+刷机，排入 #99 TA 重刷窗口。
  - **#112** grant-session strip client_data_json → 无 TA payload 绑定（#63 strict 翻转前置）。
  - 过渡模式裸 nonce 签名窗口未关 → 由 #63 strict 翻转关闭。

> 双轨版本：CA(host) **0.24.2** · TA 0.6.0（不变）· proto 0.5.0（不变）。host-only。

## 0.24.1 (2026-06-21) — Beta5 — 每请求 access log

### 新增 (Features)
- **HTTP access log**：routes 加 `warp::log("kms::access")`，每个请求一行（method / path / 状态码 / 耗时），经 `log` crate 输出、受 `RUST_LOG` 控制（`info` 即显示），写入 `/var/log/kms-api.log`。补齐了此前「只有 operation 级日志、无完整请求日志」的缺口。
- 安全：`warp::log` 只记 method/path/status/referer/user-agent/elapsed，**不记请求头** → `x-api-key` 不会进日志。

> 双轨版本：CA(host) **0.24.1** · TA 0.6.0（不变）· proto 0.5.0（不变）。CA-only 改动。
> 运维（不在仓库）：开发板 NTP 已校准（此前 RTC 偏 7 天）。TA 深度日志（trace_println）走 OP-TEE 安全串口，非本文件。

## 0.24.0 (2026-06-21) — Beta5 — 生产/测试双 profile：测试 build 支持 localhost rpId

**主题：引入编译期 profile，区分生产与本地调试。** 此前 TA 把 rpId 硬编码为 `aastar.io`（`ta/src/main.rs`），host 配 `KMS_RP_ID=localhost` 也绕不过——TA 在 TEE 内强制再校验时拒掉非 aastar.io 的 assertion（500）。本版用 `dev-rpid` feature 做成两套 build。

### 新增 (Features)
- **`dev-rpid` 编译期 feature（TA + CA）**：
  - **生产 build（默认，不带 feature）**：rpId 只认 `aastar.io`（TA 硬编码 + CA 默认）。行为与 0.23.2 一致。
  - **测试 build（`MX93_DEV_RPID=1`）**：TA 额外接受 `SHA-256("localhost")`，CA 默认 rpId/origin 含 `localhost` / `*.aastar.io` / `localhost:*`。**仅供开发板**，让本地前端（浏览器强制 rpId=localhost）能跑通真实 TA。
  - 两套 build 的**唯一差异 = rpId 接受范围**。`KMS_RP_ID`/`KMS_ORIGIN` 仍可运行时覆盖 CA 默认；但 TA 的 rpId 白名单是编译期固定的硬门（决定性）。
- **`/version` 新增 `profile` 字段**（`"dev"` / `"prod"`）——一眼区分开发板与生产板。
- 构建：`MX93_DEV_RPID=1 ./scripts/mx93-build.sh all` 产出测试 build（TA+CA 同时带 feature）。

### 安全 (Security)
- 测试 TA 接受 localhost 会**扩大**「防 rpId 替换」面（localhost 凭证可用）——**严禁刷到生产板**。`dev-rpid` 默认关闭，生产 build/CI 不得开启。`/version` profile + 启动日志 `⚠️ DEV-RPID build` 双重标记。

### 发布流程 (Release)
- RELEASE-CHECKLIST §0 新增 **profile 决策**：发版前必须确认「生产 or 测试」；测试 build 用 `MX93_DEV_RPID=1`，且 measurement 不进生产透明日志。

> 双轨版本：CA(host) **0.24.0** · TA **0.6.0**（rpId 校验逻辑变更）· proto 0.5.0（不变）。

## 0.23.2 (2026-06-20) — Beta5 — api-key CLI 输出可脚本化

### 修复 (Fix)
- `KmsDb::open` 的 `📦 SQLite DB opened: …` 诊断从 **stdout 改到 stderr**（`db.rs:306`）。`api-key generate` 把新 key 打到 stdout，之前该诊断混在 stdout 里污染了 key 捕获；现在可干净地 `KEY=$(api-key generate --label svc)`。API 服务端 stdout+stderr 同写一个日志文件，服务端行为不变。

> 双轨版本：CA(host) 0.23.2 · TA 0.5.0（不变）· proto 0.5.0（不变）。仅 host 一行 I/O 流向改动，无接口/行为变更。

## 0.23.1 (2026-06-19) — Beta5 — 运营硬化：API key 强制 + 运营商可配置 RP

**主题：把已建好但未启用的能力接通，并让 fork 运营商无需改代码即可换域名。** 无运行时行为新增，是一次运营/部署/文档补强 + API key 鉴权在生产上线。

### 安全 (Security)
- **API key 鉴权在生产启用**：kms.aastar.io 从开放模式切到强制——所有敏感路由（CreateKey / Sign / SignHash / ListKeys / DeleteKey / ChangePasskey / UnfreezeKey / WebAuthn Begin*/Complete* / agent 端点）需 `x-api-key` header；开放只读端点（/health、/version、/stats、/.well-known/* 等）不变。机制早已在 `db_api_key_filter` 实现，本次仅在 DB 注册首个 key 并重启激活（实测无 key→401 / 有 key→200，本地与公网双向验证）。

### 新增 (Features)
- **运营商可配置 WebAuthn 相对方（无需重编）**：`kms-api.service` 增加 `EnvironmentFile=-/etc/airaccount/kms.env`（可选，缺省不报错）；新增 `kms/deploy/mx93/kms.env.example`（覆盖 `KMS_RP_ID`/`KMS_RP_NAME`/`KMS_ORIGIN`/API key/限流/存储），fork 者改一行即可切到自有域名。生产仍走代码默认 `aastar.io`（板上无此文件）。
- **本地调试配置**：`kms/deploy/local/kms.env`（`rpId=localhost`，浏览器安全上下文，http 即可跑 passkey；生产勿用）。

### 测试 (Test)
- `kms/test-full-api.sh` 支持 `KMS_API_KEY` 环境变量（所有请求自动注入 `x-api-key`）+ 公网 host 自动加 `https://`，鉴权启用后仍可一键跑通。

### 文档 (Docs)
- 新增 `docs/design/backend-decomposition-kms-capacity.md`：YAA 自起后端的 5 职责拆解（各归 KMS/客户端/SuperRelay/subgraph）+ i.MX93 实测承载能力评估（2GB/8GB 足支 ~100 用户 @ 5–10% 并发；瓶颈是磁盘余量与 TEE 吞吐而非内存；扩展走横向加节点）。

### 运营 (Ops)
- 板子磁盘 83% → 54%：清理可重建的开发/测试残留（`.rustup` 1.3G + `.cargo` 170M + 旧二进制备份 + LTP/GoPoint/unit_tests 822M），不可替代数据（`kms.db`）已备份到 Mac。
- 新增 `/etc/logrotate.d/kms-api`（weekly / 50M / keep 4 / copytruncate）防 `kms-api.log` 无限增长。

## 0.23.0 (2026-06-16) — Beta5 — 可验证信任：透明日志上线

**主题：去掉「单一发布者私钥」信任点。** measurement 清单现在被公开记录在 Sigsum 透明日志里（见证人共签），客户端可验证"这份清单确实被公开登记过"——AAStar 改不了已公开承诺的 TA，任何滥用公开可查。问责制（与 Certificate Transparency 同源）。

### 新增 (Features)
- **#87 (B) 透明日志 —— 端到端上线**：
  - `@aastar/attestation-verifier` 新增 RFC 6962 Merkle inclusion 验证（`transparency.ts`）+ 完整 Sigsum proof 验证（`sigsum.ts`，leaf/checkpoint/cosignature 线格式从 sigsum-go 转写）+ `parseSigsumProof`。
  - `verifyMeasurementManifest` 新增 **Tier-2 gate**（`transparency` 选项）：验清单在公开日志、≥quorum 见证人共签，并**绑定**（proof 记录的 == SHA-256(清单 body)，防张冠李戴）。
  - host 新增 `GET /.well-known/attestation-measurements-proof.json`（Sigsum proof sidecar，静态、运行时不连日志）。
  - 发版 publish CI（`submit-manifest-to-sigsum.mjs` + workflow）+ **B-4 定时监控**（`monitor-manifest.mjs` + cron workflow，复验 live 端点 + 比对仓库源）。
  - **对公共日志 `test.sigsum.org/barreleye`（policy sigsum-test1-2025）端到端验通**；真 proof 进 fixtures、离线可复现；已部署 kms.aastar.io。
- **OpenAPI 补全**：`/attestation`、`/.well-known/attestation-measurements.json`、`/.well-known/attestation-measurements-proof.json` 三端点（v0.22.0 漏记，本版补回）+ `Attestation` tag。

### 安全 (Security)
- **信任根战略落定**：NXP NDA 对个人申请被拒（需法律实体）→ 信任根定为 **(B) 可复现+透明 ⊕ (C) 去中心化/DVT 为主、(A) 厂商根为可选**。
- publish CI 加固：secret 走 `env:`（不进命令行）、verify-before-commit fail-closed、提交开 PR 不直推。

### 文档 (Docs)
- **`docs/TRUST.md`** —— 信任模型总文档（NDA 决策 + 三类信任锚 + 透明日志解决什么 + 用户怎么验 + 运维 + 诚实边界），README 加人话「信任增强」段链接。
- `attestation-trust-root-decision.md` / `measurement-provenance-design.md` / `transparency-log-ops.md`；issue #87/#88 跟踪 B/C。
- `RELEASE-CHECKLIST.md`：补"openapi 必须补新端点、不是只 bump 版本号"。

### 版本 (Versions)
- CA(host) `0.22.0 → 0.23.0`；KMS_VERSION/OpenAPI 同步。**TA `0.5.0`、proto `0.5.0` 不变**（本版无 TA/proto 改动，纯 host + 验证器 + 文档）。

## 0.22.0 (2026-06-15) — Beta4 — 远程证明 MVP + 威胁模型 V4 闭合 + 可复现信任根

**主题：让客户端能密码学验证「签名来自真实 OP-TEE」，并彻底关闭 CA 偷换 payload（V4）。**

### 新增 (Features)
- **#37 远程证明 MVP（Phase 1）**：`GetAttestation`(cmd 26) —— TA 调 OP-TEE attestation PTA 取 TA signed-header digest，用 RSA-PSS(over `SHA-256(nonce‖ta_measurement)`，MGF1-SHA256，salt 32) 签名；新端点 `GET /attestation?nonce=<hex>`；新包 `@aastar/attestation-verifier`（RSA-PSS 验签 + nonce 防重放 + TOFU pin）。实机 FRDM-IMX93 验证 R-2/R-3 PASS。
- **#12 签名 measurement manifest**：`GET /.well-known/attestation-measurements.json`（Ed25519 签名，pin publisher key）；verifier 支持 manifest 验证（status allowlist fail-closed + sequence 防降级 + schema 校验）。
- **#37 / R-4 可复现构建**：`scripts/ta-measurement.sh` 从公开源码 + 同工具链 bit-for-bit 重算 `ta_measurement`（= stripped_ta 的 SHA-256）；信任根从「信 AAStar 登记值」升到「信源码可验」。
- **#68 payload-bound challenge**：WebAuthn challenge 改 commitment `SHA-256(nonce‖payloadDigest)`，TA 重算比对 —— passkey 不只证「在场」，还证「签的就是这笔 payload」。
- **#63 strict challenge-binding（cargo feature）**：`strict-challenge` 编译出强制镜像（拒绝无 TA-challenge 绑定的 assertion）；生产 flip 待 SDK #58 发布。
- **#70 DVT KMS 侧 binding 黄金向量**：新包 `packages/dvt-binding-vector`（可执行 KAT，证明命门 C1：用户授权的 op == DVT 共签的 userOpHash == KMS secp256k1 签的 userOpHash = 同一笔），u0/u1 逐字节对齐 airaccount-contract `HashToG2Golden.t.sol`。

### 安全 (Security)
- **威胁模型 V4 全闭（#68）**：commitment 方案在**全部签名操作**上关闭 CA payload-swap（strip + substitute 两种变体）；grant-session 也绑定。
- **#73 attestation 健壮性**：`/health` `attestation_available` 从硬编码 `true` 改**真探针**（单调 latch + ≥30s 限流，无错误字符串耦合，fail-safe）；attestation nonce 上限（≤64B）；query schema 校验（`deny_unknown_fields`，非法参数返回 400 而非 500）。
- **#70 DVT 误派更正**：协调文档曾把「KMS 闸门 + 校验 BLS 聚合」派给 #70 —— 改正：**KMS 不签 / 不验 / 不打包 BLS**，DVT 强制与验证全在链上 account 合约(#110) + 独立节点(#42)；KMS 跑在 CA 信任域内（正是 V5 要防的），自己把关形同虚设。
- ⚠️ **proto bincode 线格式变更**（新增 GetAttestation + payload commitment）：host 与 TA 必须同版本一起部署。

### 文档 (Docs)
- 威胁模型 V5（假 TEE / 伪造签名环境）章节 + MVP 半信任 / 全信任 ASCII 信任图。
- `docs/design/security-roadmap.md`：V1–V5 缺口拆成 A/B/C/D/E 任务线。
- #37 远程证明设计 + 硬件实测发现（**R-1：OP-TEE attestation key 设备自签、无 NXP 证书链 → Phase 2/ELE 锚定阻塞，需 NXP 一手资料**）。
- DVT 跨仓协调记录（hub `YetAnotherAA-Validator#42` + 双向依赖链）。

### 测试 (Testing)
- 真机 FRDM-IMX93：attestation R-2/R-3 PASS；#73 E2E（/health 真探针、超长 nonce 400、多余参数 400、正常返 evidence）板上 + 公网 kms.aastar.io 验过；binding 向量 `node --test` 绿。
- 两个 PR（#81 / #82）经 Codex 多轮对抗审查，全部 **APPROVED**（含 alignment 机器校验、探测逻辑重构、时钟回拨守卫）。

### 版本 (Versions)
- CA(host) `0.21.0 → 0.22.0`；TA `0.4.0 → 0.5.0`；proto `0.4.0 → 0.5.0`；OpenAPI `0.21.0 → 0.22.0`。

## 0.21.0 (2026-06-13) — Beta3 — 安全加固 + 生态对齐

**Issue #49 (H-2)：WebAuthn challenge binding 下沉到 TA，防 assertion 重放**

之前 TA 的 `verify_passkey_for_wallet` 只验 ECDSA 签名，不校验 clientDataJSON 里的 challenge —— 被攻陷的 CA 可重放一条捕获的 assertion 授权任意 payload。本次把 challenge 校验下沉到 TA。

### 新增 (Features)
- **`GetChallenge`(cmd 25)**：TA 用 `optee_utee::Random` 生成 32B 一次性 nonce，绑定 wallet_id 存入内存 pending 表（非 secure storage），返回给 host 当作 WebAuthn challenge
- **TA 侧 challenge 绑定**：`verify_passkey_for_wallet` 现在校验 `SHA-256(clientDataJSON) == client_data_hash` → 提取 challenge → 比对 TA 自己签发的 nonce（常量时间）→ 校验未过期(TTL 300s) → 消费(one-time) → 再验 ECDSA
- `proto::PasskeyAssertion` 新增 `client_data_json: Option<Vec<u8>>` 字段，host 透传完整 clientDataJSON 给 TA
- host `TeeHandle::get_challenge` / `webauthn::generate_authentication_options_with_challenge`；`BeginAuthentication` 现在向 TA 取 nonce 作为 challenge

### 安全 (Security)
- 关闭 H-2 重放窗口：即使 CA 被攻陷，捕获的 assertion 也无法重放（nonce 一次性 + TA 内消费 + JSON↔hash 绑定 + TTL）
- 过渡兼容：`ENFORCE_TA_CHALLENGE=false` 时，无 `client_data_json` 的旧 assertion 走 legacy ECDSA-only 路径（带告警 + 清除残留 nonce）；迁移完成后翻到 strict
- ⚠️ proto bincode 线格式变更：host 与 TA 必须同版本一起部署（bincode 非自描述，`serde(default)` 不提供跨版本兼容）

### 新增 (Features) — 其余 Beta3 内容
- **#42 密钥生命周期(freeze/unfreeze)**：久置 key 后台 sweep 自动 `frozen`；owner WebAuthn ceremony `POST /UnfreezeKey` 解冻；`last_used_at`(查 tx_log，关联主/派生地址)；9 个签名操作前 `ensure_not_frozen` gate。去中心化定位：无 admin / 无 pending_delete，owner 自主
- **#52 GToken `from` 地址绑定**：`SignGTokenAuthorization` 校验 `from` == keyId+hdPath 派生地址，防 EIP-3009 链上 `ecrecover != from` revert（白烧 gas）；X402 / Micropayment 无签名者地址字段，不受影响
- **#15 TA 侧 JWT 运行时过期检查**：`verify_jwt_wallet_claims` 用 `tee_unix_secs()`(trusted TEE time source) 拒绝 `exp <= now`
- **#21 EIP-712 domain 对齐**：MicroPaymentChannel domain version `1`→`1.0.0`（对齐合约）

### 安全 (Security) — 其余 Beta3 内容
- **#59 admin 编译期门控**：`/admin/purge-key` 移到 compile-time feature `admin-purge`，正式 release 零 admin surface（二进制无 admin symbol，物理不存在；`scripts/security-check.sh` CI 门）
- **MAX_WALLETS 100→30000**：M-4 storage-DoS 上限过保守（实测 100 wallet 仅 476K，板子 1.4G 空闲），提到生产容量(~140MB，硬 DoS 天花板)；wallet 永在 REE-FS，不受 RPMB/ELE 约束
- **DoS-on-nonce 修复**：#49 challenge nonce 改 peek → 验证 → 成功才 consume，携带错误 challenge 的请求不再烧掉受害者合法 nonce
- **#49 nonce 跨 TA 线程 flaky 修复**：pending nonce 表从 `thread_local` 改进程级 `static`（OP-TEE 跨 InvokeCommand 换线程会丢 thread_local），消除间歇性 "No pending challenge"
- **未匹配路径返回 404**：`handle_rejection` 对未知路径返回 404 而非 500（admin 编译掉后访问应读作"无此端点"）
- **#53 cla.yml SHA-pin**：GitHub Action pin 到 commit SHA（供应链加固）
- 外部 4-round PK review（DeepSeek / Sonnet / Codex / Opus）+ Codex 多轮对抗审查，全部 **APPROVED**

### 测试 (Testing) — Beta3
- 真机 FRDM-IMX93：E2E **40/40**、防重放/DoS **4/4**、freeze/unfreeze **5/5**、host 单元 **63/63**、proto 单元 42
- mainnet 前置追踪 **issue #63**（grant-session TA binding + `ENFORCE_TA_CHALLENGE` flip）

## 0.20.0 (2026-06-12) — Beta2

**Beta2 里程碑：安全加固 + RPMB 反回滚 + MX93 生产部署 + SuperPaymaster 对齐**

整合 PR #51 / #35 / #33 / #2，真机 FRDM-IMX93 全链验证。

### 新增 (Features)
- **P2 SuperPaymaster 便利签名器**：`SignMicropaymentVoucher` / `SignGTokenAuthorization`(EIP-3009 TransferWithAuthorization) / `SignX402Payment` —— host 侧构造固定 EIP-712 结构，复用 `SignTypedData` 的 WebAuthn ceremony 鉴权（含重放保护），不新增 TA 命令
- **RPMB 反回滚计数器** `ReadRollbackCounter`(cmd 24) + `GET /RollbackCounter` 端点
- **ForceRemoveWallet**(cmd 23)：gap key（无效 P-256 pubkey）的 TEE 强制清理，`DeleteKey` 自动检测
- **`GET /stats`** 机器可读监控端点（含 API key / 熔断器健康告警）
- **CAAM-bypass entropy**：CA 用 OsRng 生成钱包熵注入 TA，绕过 i.MX93 不稳定的 CAAM TRNG
- 自动备份系统（CA/TA 二进制 + metadata）

### 修复 (Fixes)
- **agent-key TA panic 根治**：`create-agent-key` / `refresh-agent-credential` 用 `std::time::SystemTime::now()` 在 OP-TEE TA 崩溃（0xffff3024），改用 `optee_utee::Time::ree_time()`(TEE_GetREETime)
- **M-4 TLS 污染**：`count_entries` 读 wallet object 污染 `tpidr_el0`，导致 CreateKey 后续 thread_local 缓存 panic —— 改为只读内存 key 列表
- `DeleteKey` 走 AWS-KMS action 名 `ScheduleKeyDeletion`
- `dirf.db` 0 字节自动修复（dirf-repair.service oneshot）
- `KMS_VERSION` 常量与 Cargo 版本统一（消除 0.19.0/0.19.1 不一致）

### 安全 (Security)
- 审计 P0/High 全部修复（命令 ID 唯一性 / TEE 调用超时+熔断 / passkey 强制 / submodule 锁定）
- TA 侧 WebAuthn rpId + User-Presence 验证（C-1 独立验签，编译进 TA）
- RPMB 钱包存储 + REE-FS fallback（防回滚）
- `DeleteKey` 正常路径用 strict passkey/WebAuthn 验证
- 测试 passkey 私钥移出 git → `.env.kms-test` keystore（git-ignored）

### 测试 (Testing)
- **真机 E2E 100% 端点覆盖：FRDM-IMX93 上 34/34 通过**（含 WebAuthn 注册/认证 ceremony 全流程、agent key、grant session、p256 session、EIP-712）
- 单元测试：proto 39 + host 56（交叉编译 aarch64 上板运行）
- 可复现的 host 单元测试 runner（`kms/test/run-host-unit-tests.sh`）

### 合规 (Compliance)
- Apache 2.0 license 合规：NOTICE / TRADEMARK / 中文 license + CLA workflow
- README license badge 修正

## 0.19.0 (2026-06-07)

**硬件里程碑：NXP FRDM-IMX93 + OP-TEE 4.8 生产部署**

- 首次在 NXP FRDM-IMX93 (aarch64 Cortex-A55, 2GB LPDDR4x) 上完整部署并验证
- TA 签名升级：OP-TEE 4.8 使用 RSA-4096 默认密钥（旧 4.5/4.6 为 RSA-2048），需用 sign_encrypt.py sign-enc 命令重签
- kms-api-server 在板子上原生编译（Rust 1.96.0），OPTEE_CLIENT_EXPORT="/" 指向 Yocto rootfs
- `libteec.so` 无版本符号链接：`ln -sf /usr/lib/libteec.so.2.0.0 /usr/lib/libteec.so`
- systemd 服务（kms-api.service）接管进程管理，依赖 tee-supplicant@teepriv0.service
- 修复：所有 AWS KMS 端点需 `x-amz-target: TrentService.<Op>` header，缺少时 Warp 返回 500（非 400）
- 修复：CreateKey 必须包含 PasskeyPublicKey（65字节 P256 uncompressed，`0x04||x||y`）
- 测试页面路径多路查找：./kms-test-page.html → /root/AirAccount/ → /root/shared/（旧 QEMU 路径）
- Cloudflare Tunnel 部署到 kms.aastar.io，cloudflared 在 MX93 板上作为 systemd 服务运行

## 0.16.8 (2026-03-26)

- 修复 TA panic 返回 500 而非 400（之前所有非 auth/circuit 错误都误报 400）

## 0.16.7 (2026-03-13)

- TX 历史统计（累计/每日 签名数、TEE 操作数、WebAuthn 次数、平均延迟、错误/Panic 计数）
- SQLite tx_log 表持久化所有 TEE 操作记录
- Wallet 列表新增 Signs 列（per-key 签名次数）

## 0.16.6 (2026-03-12)

- Stats 页面 Description 字段截断显示（隐私保护）
- TEE handler 层全面 tx 追踪日志（成功/耗时/webauthn 路径）
- TA panic 自动识别并标记（`💀 TA PANIC`）
- Journal 持久化（重启不丢日志）

## 0.15.22 (2026-03-03)

- Rate limit 默认提升至 100 req/min
- 新增 `GET /version` API
- 修复 POST 空 body 解析 (ListKeys 无 body 时 500)
- 修复 API 测试脚本 Passkey 签名格式
- TA 端 p256-m ECDSA verify 恢复 (`-O1 -fPIC -fno-common -marm`)

## 0.15.0 (2026-03-03)

- Rate limit (60 req/min per API key) + circuit breaker (3 failures → 30s block)
- CA 端输入验证 (path/hash/message/UUID)
- p256-m crash 定位并修复，CA+TA 双重 P-256 验证 (defense-in-depth)

## 0.14.0 (2026-03-02)

- SQLite 持久化 (wallets/address_index/challenges/api_keys)
- WebAuthn 仪式服务器 (BeginRegistration/CompleteRegistration/BeginAuthentication)
- DB 驱动 API Key 认证 (`api-key generate/list/revoke`)
- CA 端 P-256 ECDSA 预验证
- ChangePasskey API

## 0.13.0 (2026-03-02)

- TA 端 WebAuthn PassKey P-256 ECDSA 验证
- CreateKey 强制 PasskeyPublicKey
- 所有签名操作需 Passkey assertion

## 0.12.0 (2026-03-02)

- TEE 持久 session + LRU cache (容量 200)
- WarmupCache API
- Background address derivation

## 0.11.0 (2026-03-02)

- KeyStatus 轮询 + QueueStatus API
- Background address derivation (PBKDF2 + BIP32)

## 0.10.0 (2026-03-01)

- KMS API server (warp) 异步架构
- AWS KMS 兼容 API (CreateKey/DescribeKey/ListKeys/Sign/SignHash/DeriveAddress/GetPublicKey)
- DK2 部署 pipeline (Docker 交叉编译 + SCP + systemd)

## Features (cumulative)

- OP-TEE Trusted Application: BIP32/BIP39 HD wallet, secp256k1 签名
- AWS KMS 兼容 REST API
- P-256 PassKey 双重验证 (CA pre-verify + TA p256-m)
- WebAuthn 仪式 (注册 + 认证)
- SQLite 持久化 (WAL mode)
- DB 驱动 API Key 认证
- Rate limit + circuit breaker
- Background address derivation + KeyStatus 轮询
- TEE 持久 session + LRU cache
- EIP-155/EIP-191 签名
- Board: STM32MP157F-DK2 (Cortex-A7 650MHz)
