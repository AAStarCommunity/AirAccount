# KMS Changelog

> Updated: 2026-06-15

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
