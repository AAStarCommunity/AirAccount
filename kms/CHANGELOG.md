# KMS Changelog

> Updated: 2026-06-13

## Unreleased — Beta3 (security)

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
