# KMS Changelog

> Updated: 2026-03-12

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
