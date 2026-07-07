# AirAccount — CLAUDE.md

## Mycelium Protocol 生态上下文

@/Users/jason/Dev/Brood/protocol/MISSION.md
@/Users/jason/Dev/Brood/protocol/PGL/CONTEXT.md
@/Users/jason/Dev/Brood/orgs/aastar/PROFILE.md
@/Users/jason/Dev/Brood/orgs/aastar/INTERFACES.md

## 十词定位
TEE私钥管理 · WebAuthn无密码认证 · AWS KMS兼容API

## 生态角色
**组织**: AAstar（区块链基础设施层）
**协议**: Mycelium Protocol — https://www.mushroom.cv
**定位**: 整个生态的**身份与密钥底层**。SuperPaymaster 依赖它做账户验证，SuperRelay 依赖它做 TEE 双签，Sin90 依赖它做用户隐私保护。

## 当前状态
- **当前分支**: main（活跃开发）
- **版本**: v0.27.3
- **架构**: TEE (Trusted Execution Environment) + WebAuthn + AWS KMS 兼容 API
- **生产 URL**: https://kms.aastar.io（Cloudflare Tunnel → NXP FRDM-IMX93）
- **硬件**: NXP FRDM-IMX93 (aarch64 Cortex-A55, OP-TEE 4.8, RSA-4096 TA signing)

## 核心架构（四层）
```
Client Layer     → CLI / Web / SDK（发 AWS KMS 格式请求）
API Gateway      → HTTPS over Cloudflare Tunnel
TEE Layer        → OP-TEE TrustZone（真实硬件）/ Mock TEE（开发）
Storage Layer    → Secure Storage in TEE
```

## 关键接口
- `POST /kms/CreateKey` — 创建密钥（TEE 内生成，私钥不出 TEE）
- `POST /kms/Sign` — 签名（私钥不暴露）
- `GET  /health` — 服务状态
- `GET  /stats` — TX 历史统计
- WebAuthn 注册/认证端点（无密码登录）

## 开发约定
- 硬件部署：NXP FRDM-IMX93 (aarch64) + DK2 (armv7)
- 当前硬件：MX93 board（主力），DK2（备用）
- 测试：`./kms/test-full-api.sh localhost:3000`（本地）或 `./kms/test-full-api.sh kms.aastar.io`（公网）
- 关键 API header：`x-amz-target: TrentService.<Operation>`（AWS KMS 格式，缺少返回 500）
- CreateKey 必填：KeySpec + KeyUsage + Description + Origin + PasskeyPublicKey（65字节 P256 uncompressed hex）

## 关联任务
- Brood TASK-12: AirAccount 隐形账户（70%，In Progress）
- 进度追踪：https://www.mushroom.cv

## 中国大陆社区部署（China Community Deployment）

中国大陆社区成员运行自己的 KMS 节点时，Cloudflare Tunnel 被 GFW 封锁，
需通过**香港 VPS + frp 中继**替代。完整部署指南：

@/Users/jason/Dev/Brood/research/global-network/china-kms-tunnel-setup.md

架构分析：
@/Users/jason/Dev/Brood/research/global-network/cloudflare-tunnel-global-availability.md

## 不要做的事
- 不要在 TEE 之外存储或打印私钥
- 不要绕过 WebAuthn 验证流程
- 不要修改 TA（Trusted Application）接口而不更新对应 SDK
