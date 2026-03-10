# KMS 系统架构详细文档
# KMS System Architecture Documentation

*文档创建时间: 2025-09-28 11:34:18*

## 概述 / Overview

AirAccount KMS 是一个基于 TEE (Trusted Execution Environment) 的密钥管理系统，提供与 AWS KMS 兼容的 API 接口。系统采用分层架构设计，支持从开发阶段的 Mock TEE 到生产环境的真实 TEE 的平滑迁移。

## 系统架构层次 / System Architecture Layers

### 1. 客户端层 (Client Layer)

**组件:**
- CLI 工具 (curl, custom clients)
- Web 应用程序
- 语言 SDK (JavaScript, Python, Rust)

**职责:**
- 发送符合 AWS KMS 格式的 API 请求
- 处理响应和错误
- 管理客户端认证凭据

**通信协议:**
- HTTPS over Cloudflare Tunnel
- AWS KMS TrentService API 格式
- JSON payload with X-Amz-Target headers

### 2. API 网关层 (API Gateway Layer)

**Cloudflare Tunnel:**
- 功能: HTTPS 代理，无需端口转发
- 优势: 全球 CDN 加速，DDoS 防护
- 配置: 自动 SSL/TLS 终端

**负载均衡器:**
- 速率限制 (Rate Limiting)
- 流量分发
- 健康检查

### 3. KMS 服务层 (KMS Service Layer)

**KMS API 服务器 (kms-api):**
- 框架: Axum (Rust 异步 HTTP 框架)
- 端口: 8080 (默认)
- 功能:
  - AWS KMS API 兼容性
  - 请求路由和验证
  - 错误处理和响应格式化

**健康监控:**
- 端点: `/health`
- 响应: 服务状态、时间戳、版本信息
- 监控指标: 响应时间、成功率

### 4. 核心逻辑层 (Core Logic Layer)

**KMS Core (kms-core):**
- 语言: Rust
- 特性: 硬件无关的密码学逻辑
- 功能:
  - 密钥生成策略
  - 签名算法实现
  - 密钥生命周期管理

**协议定义 (proto):**
- TEE 通信协议
- 序列化/反序列化
- 类型安全保证

### 5. TEE 层 (Secure Layer)

**KMS Host (kms-host):**
- 功能: TEE 接口适配层
- 职责:
  - 安全上下文管理
  - TEE 会话建立
  - 命令转发

**Trusted Application (kms-ta):**
- 环境: TEE 安全执行环境
- 功能:
  - 私钥生成和存储
  - 数字签名操作
  - 密钥访问控制

**安全存储:**
- 位置: TEE 内部安全存储
- 加密: 硬件级加密
- 特性: 私钥永不离开 TEE

### 6. 测试和工具层 (Testing & Tools Layer)

**Mock TEE (kms-ta-test):**
- 用途: 开发和测试阶段
- 特性: 完整 API 兼容性
- 优势: 快速迭代开发

**测试套件:**
- Python 测试脚本: `scripts/test-kms-apis.py`
- Curl 测试脚本: `scripts/test-all-apis-curl.sh`
- 性能基准测试

**部署脚本:**
- 一键部署: `scripts/deploy-kms.sh`
- 公网访问设置: `scripts/setup-public-access.sh`
- OP-TEE 迁移: `scripts/migrate-to-optee.sh`

## 数据流架构 / Data Flow Architecture

### 密钥创建流程 (Key Creation Flow)

```
1. Client → Cloudflare Tunnel
   POST / with X-Amz-Target: TrentService.CreateKey

2. Cloudflare → KMS API Server (port 8080)
   Proxy request with SSL termination

3. KMS API → KMS Core
   Parse request, validate parameters

4. KMS Core → KMS Host
   Initiate secure key generation

5. KMS Host → Trusted Application
   Execute in TEE secure context

6. TA → Secure Storage
   Generate private key, store securely

7. Response path (reverse)
   Public key + metadata returned
```

### 签名操作流程 (Signing Flow)

```
1. Client sends message + KeyId
2. API validates request format
3. Core logic prepares signing context
4. Host establishes TEE session
5. TA retrieves private key from secure storage
6. TA performs ECDSA signature
7. Signature returned to client
```

## 安全架构 / Security Architecture

### 安全边界 (Security Boundaries)

**Level 1: 网络安全**
- Cloudflare DDoS 防护
- HTTPS 强制加密
- 速率限制

**Level 2: 应用安全**
- 输入验证
- 错误处理
- 访问日志

**Level 3: TEE 安全**
- 硬件隔离
- 安全存储
- 认证启动

### 威胁模型 (Threat Model)

**保护对象:**
- 私钥材料
- 签名操作
- 密钥元数据

**攻击面:**
- 网络攻击 → Cloudflare 防护
- 应用漏洞 → Rust 内存安全
- 侧信道攻击 → TEE 硬件保护
- 物理攻击 → 安全存储加密

## 性能特性 / Performance Characteristics

### 响应时间指标

- 健康检查: < 50ms
- 密钥创建: < 300ms
- 公钥获取: < 100ms
- 消息签名: < 200ms
- 密钥列表: < 150ms

### 吞吐量

- 并发连接: 100+
- 每秒请求: 50+ TPS
- 批量操作: 支持

### 可扩展性

- 水平扩展: 支持多实例
- 负载均衡: Cloudflare 自动
- 缓存策略: 公钥缓存

## 部署架构 / Deployment Architecture

### 开发环境 (Development)

```
Developer Machine:
├── Mock TEE (kms-ta-test)
├── KMS API Server (localhost:8080)
├── Test Scripts
└── Local Cloudflare Tunnel (optional)
```

### 生产环境 (Production)

```
Physical Hardware:
├── Raspberry Pi 5 / ARM64 Server
├── OP-TEE Secure OS
├── KMS Services
└── Cloudflare Tunnel Client

Cloud Infrastructure:
├── Cloudflare Global Network
├── DDoS Protection
├── SSL/TLS Termination
└── Global CDN
```

## 技术栈 / Technology Stack

### 核心技术

- **Rust**: 系统编程语言，内存安全
- **Axum**: 高性能异步 HTTP 框架
- **secp256k1**: 以太坊兼容的 ECDSA 算法
- **UUID**: 密码学安全的密钥标识符

### 安全技术

- **OP-TEE**: 开源可信执行环境
- **ARM TrustZone**: 硬件安全特性
- **Teaclave SDK**: Rust 基 TEE 开发框架

### 基础设施

- **Cloudflare Tunnel**: 安全公网访问
- **Docker**: 容器化开发部署
- **QEMU**: ARM64 TEE 模拟器

## 质量保证 / Quality Assurance

### 测试策略

**单元测试:**
- Rust 测试框架
- 覆盖核心逻辑

**集成测试:**
- API 端到端测试
- TEE 集成验证

**性能测试:**
- 负载测试
- 并发测试
- 响应时间监控

### 监控和日志

**应用监控:**
- 健康检查端点
- 响应时间指标
- 错误率统计

**安全监控:**
- 访问日志
- 异常检测
- 威胁情报

## 演进路线图 / Evolution Roadmap

### Phase 7 (当前 - Current)
- ✅ Mock TEE 完整实现
- ✅ AWS KMS API 兼容
- ✅ Cloudflare 全球部署
- ✅ 完整测试套件

### Phase 8 (下一步 - Next)
- 🔄 真实 OP-TEE 环境迁移
- 🔄 硬件安全模块集成
- 🔄 高级密码学特性
- 🔄 多租户支持

### Phase 9-10 (未来 - Future)
- ⏳ 高可用集群
- ⏳ 合规认证 (FIPS, Common Criteria)
- ⏳ 企业级认证授权
- ⏳ 零知识增强

## 故障排除 / Troubleshooting

### 常见问题

**连接问题:**
- 检查本地服务: `curl localhost:8080/health`
- 检查隧道状态: `scripts/setup-public-access.sh status`

**API 错误:**
- ValidationException: 检查请求格式
- NotFoundException: 验证密钥 ID
- 网络超时: 检查 Cloudflare 状态

**性能问题:**
- 高延迟: 检查 TEE 资源使用
- 连接池耗尽: 调整并发限制

### 调试工具

- 完整 API 测试: `python3 scripts/test-kms-apis.py --online`
- 快速验证: `scripts/test-all-apis-curl.sh`
- 性能对比: `python3 scripts/test-kms-apis.py --compare`

---

**文档版本:** v1.0
**最后更新:** 2025-09-28
**维护者:** AirAccount KMS Team