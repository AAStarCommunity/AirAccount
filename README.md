# KMS (Key Management System) on TEE

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Service Status](https://img.shields.io/badge/Status-Online-brightgreen.svg)](https://kms.aastar.io/health)
[![API Compatibility](https://img.shields.io/badge/AWS%20KMS-Compatible-orange.svg)](#aws-kms-compatibility)
[![API Docs](https://img.shields.io/badge/API%20Docs-Swagger%20UI-85ea2d.svg)](https://kms.aastar.io/docs)
[![Version](https://img.shields.io/badge/version-v0.28.0%20Beta6-blue.svg)](kms/CHANGELOG.md)

A production-ready private key management system built on Trusted Execution Environment (TEE) using the eth_wallet sample from Teaclave TrustZone SDK. Provides enterprise-grade security with AWS KMS API compatibility.

## 部署方案 / Deployment Modes

AirAccount(KMS)与 DVT([aNode](https://github.com/AAStarCommunity/YetAnotherAA-Validator) BLS 门限共签)可**独立**或**合并**部署 —— **一个方案、配置切换,改动很小**。

| 场景 | 部署什么 | BLS/私钥托管 | 关键配置 |
|---|---|---|---|
| **① 独立 KMS** | 只 KMS | 用户私钥在 TEE,每次 passkey/WebAuthn 签、永不导出 | — |
| **② 独立 DVT** | 只 DVT | DVT 的 BLS 私钥在 EIP-2335 keystore(盘上加密 + 手动密码) | `RUST_SIGNER_URL` 不设 |
| **③ KMS + DVT 合并** | 单板 co-located | DVT 的 BLS 私钥托管进 **KMS TEE**(永不出、自启无需密码) | `RUST_SIGNER_URL=http://127.0.0.1:3100` |

- **只想跑密钥/签名服务**(SuperPaymaster/SuperRelay 等消费方直接调 AWS-KMS 兼容 API)→ **①**。
- **只想跑 DVT 门限节点、不跑 KMS** → **②**(标准 DVT 部署,密钥自管)。
- **跑完整节点、要最强安全** → **③**(BLS 密钥抗提取提到 KMS 级,断电自启不用重输密码)。

③ 是 ② 的**可选增强,不强绑定**:同一个 DVT 二进制,`RUST_SIGNER_URL` 一个 env 切换模式。设计与安全分析见 [`kms/docs/dvt-tee-bls-custody-design.md`](kms/docs/dvt-tee-bls-custody-design.md)。

## 🌐 Live Service

**Production URL**: https://kms.aastar.io

```bash
# Quick health check
curl -s https://kms.aastar.io/health | jq

# Create a key
curl -X POST https://kms.aastar.io/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
```

## 📖 API Documentation

**Interactive API docs (Swagger UI) — served live by the KMS itself, always matching the deployed build:**

> 🔗 **https://kms.aastar.io/docs**

| Resource | Link |
|---|---|
| **Live Swagger UI** | <https://kms.aastar.io/docs> |
| **OpenAPI 3.1 spec** | <https://kms.aastar.io/openapi.yaml> · [in-repo](kms/docs/api/openapi.yaml) |
| **Test coverage matrix** | [kms/docs/API-TEST-MATRIX.md](kms/docs/API-TEST-MATRIX.md) |

38 endpoints — wallet lifecycle · signing (hash / message / transaction / EIP-712) · WebAuthn
ceremony · agent keys · grant sessions · P256 sessions · SuperPaymaster gasless signers ·
**DVT out-of-band confirm verify (`/verify-confirm-assertion`, #124)** · **contact binding
(`/contact/*`, #129)**. Every operation carries its test-coverage status (`x-tested`).
**Real-device E2E: 39/39 · unit: proto 39 + host 58.**

### 🔑 API Key 认证与管理

**敏感路由**（CreateKey / Sign / SignHash / DeleteKey / ChangePasskey / UnfreezeKey / ListKeys / DescribeKey / GetPublicKey / DeriveAddress / WebAuthn `Begin*`·`Complete*` / agent 端点 / `verify-confirm-assertion` (#124) / `contact/*` (#129)）需带 HTTP header **`x-api-key: <key>`**，否则 `401`。**开放只读端点**（`/health` `/version` `/stats` `/QueueStatus` `/RollbackCounter` `/attestation?nonce=` `/.well-known/*` `/docs` `/openapi.yaml`）无需 key。

```bash
curl -X POST https://kms.aastar.io/Sign \
  -H "x-amz-target: TrentService.Sign" \
  -H "x-api-key: kms_xxxxxxxx" -d '{ … }'
```

**强制规则**：DB 中存在 ≥1 个 key（或设 `KMS_API_KEY` / `KMS_REQUIRE_API_KEY=1`）即开启强制。生产 `kms.aastar.io` 已开启。

**密钥管理命令**（在 KMS 主机上运行，CLI 直接读写 SQLite，比暴露网络端点更安全）：

```bash
# 新建（自动 insert 并打印 key；v0.23.2 起诊断走 stderr，可脚本捕获）
KMS_DB_PATH=/data/kms/kms.db /path/to/api-key generate --label "service-name"
KEY=$(KMS_DB_PATH=/data/kms/kms.db /path/to/api-key generate --label svc)   # 干净捕获

# 列出（key 中段打码）
/path/to/api-key list

# 吊销
/path/to/api-key revoke kms_xxxxxxxx
```

- 新建/吊销**即时生效，无需重启**（`validate_api_key` 每请求查 DB）。
- key 明文仅存 DB、只在 `generate` 时打印一次，丢失只能 `revoke` 重发。
- ⚠️ 浏览器侧切勿把 key 打进前端 bundle；用 `浏览器 → 应用后端(持 key) → KMS` 的代理模式。
- 默认 DB 路径：`/data/kms/kms.db`（存在时）否则工作目录 `./kms.db`，可用 `KMS_DB_PATH` 覆盖。

### 🔍 日志与调试 (Logs & Debugging)

日志分两层：**CA（host）= 基础日志**，**TA（TEE）= 深度日志**。

**CA 日志 —— `/var/log/kms-api.log`（运行 KMS 的主机上，如 i.MX93 板）**
systemd 把 stdout+stderr 都 append 到这里。包含三类：
- **Access log**（v0.24.1+）：每请求一行 —— `[<时间> INFO kms::access] <ip> "<method> <path>" <状态码> "-" "<ua>" <耗时>`。**不含请求头，`x-api-key` 不会泄漏。**
- **Operation 日志**：`✅ SignHash OK addr=0x… webauthn=true 81ms`、`🔑 WebAuthn rpId resolved: localhost` 等。
- **错误**：`SignHash error: <TA 原始错误> …`（带 TEE 错误码，如 `0xffff0006`）。

```bash
# 主机 IP 会变（WiFi/DHCP）→ 先扫 192.168.2.0/24:22 找板子
ssh root@<host> 'tail -f /var/log/kms-api.log'                                   # 实时
ssh root@<host> 'grep -iE "access|error|mismatch|fail|0xffff" /var/log/kms-api.log | tail -50'  # 请求 + 错误
ssh root@<host> 'journalctl -u kms-api.service -n 100 --no-pager'               # systemd journal
```

**调日志级别**：服务读 `RUST_LOG`（默认 `info`，access log 在 `info` 即显示）。要更细：
```bash
# 在 /etc/airaccount/kms.env 加一行后 systemctl restart kms-api：
RUST_LOG=debug                 # 全部 debug（含 warp/hyper 内部）
RUST_LOG=kms=debug,warp=info   # 只把 KMS 自己调到 debug
```

**TA 深度日志（在 TEE 内，SSH/dmesg 看不到）**
TA 的 `trace_println!`（如 `[+] rpId hash verified in TA`）走 **OP-TEE 安全串口**，需物理串口线：
```bash
screen /dev/cu.usbmodem<XXXX> 115200    # macOS；见 screen-93.sh
```

> `/version` 的 `profile` 字段区分板子：`prod`（rpId 仅 aastar.io）/ `dev`（额外接受 localhost）。

## 🔒 Trust & Verifiability (信任增强)

**你不用"相信 AAStar 不作恶"。** AirAccount 把"我在跑哪个 TEE 程序"公开钉死在一个**谁都改不了、谁都能查的公共透明日志**里——想偷偷换成后门版本做不到、且会被立刻发现。三道叠加把"信任一家公司"降级成"信任公开的数学和记录"：

- **可复现构建** — 任何人用开源源码重算 TA 的 measurement，比对线上值（`scripts/ta-measurement.sh`）。
- **透明日志（Sigsum）** — 每份 measurement 清单都进公开 append-only 日志、多见证人共签；客户端经 `@aastar/attestation-verifier` 验"这份清单确实被公开登记过"。**已上线**：`/.well-known/attestation-measurements.json` + `…-proof.json`。
- **DVT 独立门限共签** — 大额操作要独立第三方节点共签，不依赖单点（也不依赖信任 AAStar）。

> 诚实边界：当前是**半去中心化、可独立验证**的信任模型（TOFU/可复现 + 透明 + DVT）；尚未锚定 NXP 硬件根（受 NDA 阻塞）。完整分析、判断、验证方法、运维方案见 **[docs/TRUST.md](docs/TRUST.md)**。

## 🏗️ System Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        CLI[CLI Tools]
        WebApp[Web Applications]
        SDK[Language SDKs]
    end

    subgraph "API Gateway"
        CF[Cloudflare Tunnel<br/>HTTPS Proxy]
        LB[Load Balancer<br/>Rate Limiting]
    end

    subgraph "KMS Service Layer"
        API[KMS API Server<br/>:8080<br/>AWS Compatible]
        Health[Health Monitor<br/>Service Status]
    end

    subgraph "Core Logic Layer"
        Core[KMS Core<br/>Cryptographic Logic]
        Proto[Protocol Definitions<br/>TEE Communication]
    end

    subgraph "TEE Layer (Secure)"
        Host[KMS Host<br/>TEE Interface]
        TA[Trusted Application<br/>Key Operations]
        Storage[Secure Storage<br/>Private Keys]
    end

    subgraph "Testing & Tools"
        MockTEE[Mock TEE<br/>Development]
        Tests[Test Suite<br/>API Validation]
        Scripts[Deployment Scripts<br/>Automation]
    end

    %% Connections
    CLI --> CF
    WebApp --> CF
    SDK --> CF
    CF --> LB
    LB --> API
    API --> Health
    API --> Core
    Core --> Proto
    Proto --> Host
    Host --> TA
    TA --> Storage

    %% Development connections
    Core -.-> MockTEE
    Tests -.-> API
    Scripts -.-> API

    %% Styling
    classDef secure fill:#ff6b6b,stroke:#333,stroke-width:3px
    classDef api fill:#4ecdc4,stroke:#333,stroke-width:2px
    classDef tool fill:#45b7d1,stroke:#333,stroke-width:1px

    class TA,Storage,Host secure
    class API,CF,LB api
    class Tests,Scripts,MockTEE tool
```

## 📁 Project Structure

```
├── README.md                    # Project overview and quick start
├── docs/                        # 📚 Documentation
│   ├── CLAUDE.md               # AI assistant instructions
│   ├── Changes.md              # Development changelog
│   ├── KMS-API-DOCUMENTATION.md # Complete API reference
│   ├── system-architecture.md  # Detailed architecture guide
│   ├── deploy-arm-kms.md       # TEE deployment guide
│   ├── deployment-guide.md     # Production deployment
│   ├── roadmap.md             # Development roadmap
│   └── quick-curl-*.md        # Testing references
├── scripts/                    # 🔧 Automation Scripts
│   ├── deploy-kms.sh          # One-click deployment
│   ├── test-kms-apis.py       # Complete API test suite
│   ├── test-all-apis-curl.sh  # Curl-based testing
│   ├── migrate-to-optee.sh    # OP-TEE migration tool
│   ├── setup-public-access.sh # Cloudflare tunnel setup
│   └── *.sh, *.py            # Various utility scripts
├── kms/                        # 🔐 KMS Core Implementation
│   ├── kms-core/              # Hardware-independent logic
│   ├── kms-api/               # HTTP API server (Axum)
│   ├── kms-host/              # TEE host interface
│   ├── kms-ta/                # Trusted Application
│   ├── kms-ta-test/           # Mock TEE for development
│   ├── proto/                 # Protocol definitions
│   └── bak/                   # Backup and legacy code
└── third_party/               # 🔗 External Dependencies
    ├── incubator-teaclave-trustzone-sdk/
    └── openssl_aarch64/
```

## ✨ Key Features

### 🔒 **Security First**
- **TEE-based Security**: Private keys never leave the secure execution environment
- **Hardware Isolation**: ARM TrustZone provides hardware-level protection
- **Secure Storage**: All sensitive data encrypted at rest in TEE storage
- **No Network Exposure**: Private keys never transmitted over network

### 🔗 **AWS KMS Compatibility**
- **Drop-in Replacement**: Compatible with existing AWS KMS client code
- **Standard APIs**: Full TrentService API implementation
- **Familiar Errors**: AWS-compatible error responses and status codes
- **Enterprise Ready**: Supports high-availability and load balancing

### ⚡ **Performance & Reliability**
- **Sub-300ms Response**: Average API response time under 300ms
- **High Throughput**: Supports concurrent key operations
- **24/7 Availability**: Production deployment with 99.9% uptime
- **Global CDN**: Cloudflare integration for worldwide access

### 🛠️ **Developer Experience**
- **Multiple Language Support**: SDKs for JavaScript, Python, Rust
- **Comprehensive Testing**: Complete test suite with real API validation
- **One-Click Deployment**: Automated deployment scripts
- **Rich Documentation**: API docs with real examples and responses

## 🔐 Security Architecture & Hardware Background

The KMS runs on OP-TEE TrustZone with the NXP i.MX93 **ELE (EdgeLock Enclave)** as hardware root of trust. The key crypto-hardware decisions and concepts are documented:

- **[secp256k1 Hardware Analysis](kms/docs/secp256k1-hardware-analysis.md)** — why Ethereum's secp256k1 signing stays in software, the SE05x external secure-element option, and a glossary.
- **[Build & Deploy on MX93](kms/docs/BUILD-MX93.md)** — cross-compile pitfalls + TA signing-key requirement.
- **[Release Plan](kms/docs/RELEASE-PLAN.md)** — Beta2/Beta3/mainnet feature gating.

**Key facts**

- **Ethereum private keys (secp256k1)** are managed in software (`k256`) inside the TEE secure world, protected by RPMB anti-rollback. The i.MX93 ELE hardware does **not** support secp256k1 — only NIST P-256/384/521, Ed25519, AES, HMAC, SM4 (实测确认). Hardware secp256k1 would need an external **SE05x** secure element over I2C.
- **ELE's role** = hardware root of trust: TRNG (wallet entropy), HUK (derives the secure-storage encryption key), P-256 (WebAuthn passkey verification), attestation. It is **not** the home of secp256k1 wallet keys.
- **Glossary**:
  - **I2C** (Inter-Integrated Circuit, 读作 "I方C") — a 2-wire chip-to-chip serial bus (SDA data + SCL clock); how external chips like SE05x attach to the SoC. Low bandwidth (≤3.4 Mbit/s).
  - **EAL6+** (Evaluation Assurance Level 6+) — a **Common Criteria** (ISO/IEC 15408) security-certification level (EAL1 lowest → EAL7 highest; higher = stricter third-party evaluation). Secure-element chips like SE05x are EAL6+. It measures *how rigorously security was evaluated*, not performance.

## 🚀 Quick Start

### 1. Test the Live Service
```bash
# Health check
curl -s https://kms.aastar.io/health

# Create a signing key
curl -X POST https://kms.aastar.io/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
```

### 2. Run Complete API Tests
```bash
# Python test suite (recommended)
python3 scripts/test-kms-apis.py --online

# Curl-based testing
./scripts/test-all-apis-curl.sh
```

### 3. Local Development
```bash
# Start KMS API server
cd kms/kms-api
cargo run --release

# Test locally
curl -X POST http://localhost:8080/health
```

### 4. Deploy Your Own Instance
```bash
# Mock TEE deployment (fast)
./scripts/deploy-kms.sh mock-deploy

# With public tunnel
./scripts/deploy-kms.sh mock-deploy -t

# Real TEE deployment (requires OP-TEE)
./scripts/deploy-kms.sh qemu-deploy
```

## 📊 API Reference

| Endpoint | Method | Purpose | Status |
|----------|--------|---------|---------|
| `/health` | GET | Service health check | ✅ Live |
| `/keys` | GET | List all keys | ✅ Live |
| `/` + `TrentService.CreateKey` | POST | Generate new key | ✅ Live |
| `/` + `TrentService.GetPublicKey` | POST | Retrieve public key | ✅ Live |
| `/` + `TrentService.Sign` | POST | Sign message | ✅ Live |

**Complete API Documentation**: [docs/KMS-API-DOCUMENTATION.md](docs/KMS-API-DOCUMENTATION.md)

## 🔧 Technology Stack

### Core Components
- **Rust**: Memory-safe systems programming
- **Axum**: High-performance async HTTP framework
- **secp256k1**: Ethereum-compatible ECDSA
- **UUID**: Cryptographically secure key identifiers

### Security Layer
- **OP-TEE**: Open-source Trusted Execution Environment
- **ARM TrustZone**: Hardware security features
- **Teaclave SDK**: Rust-based TEE development framework

### Infrastructure
- **Cloudflare Tunnel**: Secure public access without port forwarding
- **Docker**: Containerized development and deployment
- **QEMU**: ARM64 TEE simulation for development

## 🏗️ Architecture Phases

### **Current: Phase 7 (Production Ready)**
- ✅ Mock TEE implementation for rapid development
- ✅ AWS KMS compatible API service
- ✅ Global deployment via Cloudflare
- ✅ Complete test suite and documentation

### **Next: Phase 8 (Security Enhancement)**
- 🔄 Migration to real OP-TEE environment
- 🔄 Hardware security module integration
- 🔄 Advanced cryptographic features
- 🔄 Multi-tenant support

### **Future: Phase 9-10 (Enterprise Scale)**
- ⏳ High availability clustering
- ⏳ Hardware security module (HSM) integration
- ⏳ Compliance certifications (FIPS, Common Criteria)
- ⏳ Enterprise authentication and authorization

## 📚 Documentation

- **[Complete API Reference](docs/KMS-API-DOCUMENTATION.md)** - Full API documentation with examples
- **[System Architecture](docs/system-architecture.md)** - Detailed technical architecture
- **[Deployment Guide](docs/deployment-guide.md)** - Production deployment instructions
- **[Development Changelog](docs/Changes.md)** - Complete development history
- **[Phase Roadmap](docs/roadmap.md)** - Future development plans

## 🤝 Contributing

### Development Setup
```bash
# Clone with submodules
git clone --recursive https://github.com/AAStarCommunity/AirAccount.git

# Install Rust and dependencies
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and test
cd kms && cargo build --release
python3 scripts/test-kms-apis.py --online
```

### Testing
```bash
# Complete test suite
./scripts/test-all-apis-curl.sh

# Performance benchmarking
python3 scripts/test-kms-apis.py --compare

# Development with mock TEE
cd kms/kms-ta-test && cargo run
```

## 📈 Current Status

**Service Metrics** (Live Production):
- 🌐 **Uptime**: 24/7 availability
- ⚡ **Performance**: ~267ms average response time
- 🔑 **Capacity**: 35+ active keys managed
- 🛡️ **Security**: TEE-based private key protection
- 🌍 **Global**: Accessible via Cloudflare CDN

**Development Status**:
- ✅ Core cryptographic operations (Phase 1-7 complete)
- 🔄 Real TEE migration (Phase 8 in progress)
- ⏳ Enterprise features (Phase 9-10 planned)

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

---

**🚀 Try it now**: https://kms.aastar.io/health

## License

This project is licensed under the [Apache License, Version 2.0](LICENSE).  
Copyright 2024-present MushroomDAO Contributors.  
See [NOTICE](./NOTICE) · [TRADEMARK.md](./TRADEMARK.md) · [LICENSE-zh.md](./LICENSE-zh.md) · [TRADEMARK-zh.md](./TRADEMARK-zh.md) for details.
