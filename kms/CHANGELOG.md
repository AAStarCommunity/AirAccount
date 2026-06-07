# KMS Changelog

> Updated: 2026-06-07

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
