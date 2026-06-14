# #63 — WebAuthn Challenge Binding 强制（strict flip，闭合 V2 重放后门）

> 创建时间：2026-06-14 15:30 +07（本机时间）
> 关联：威胁模型 `threat-model-ca-adversary.md` 向量 V2 · 安全路线图 `security-roadmap.md` A 线 · issue #49（challenge binding 实现）/ #58（SDK 升级）

## 背景：V2 与 legacy 后门

#49 已实现 TA 侧 challenge binding：客户端先 `GetChallenge` 拿一次性 nonce，签名时带上 `clientDataJSON`，TA 校验 `SHA-256(clientDataJSON)==client_data_hash` 并比对 nonce（一次性、限时）→ 防重放（V2）。

**但带 `clientDataJSON` 的请求才走严格校验**；不带的 legacy 请求在 **transition 模式**下只警告、放行（走旧 ECDSA-only 路径）。这条 legacy 路径就是 **V2 的可重放后门**：一个被攻陷的 CA 可重放用户旧的 assertion。`threat-model` 标注「mainnet 必须 flip `ENFORCE_TA_CHALLENGE=true`」。

## 本 PR 做了什么

strict 校验路径 #49 早已完整实现且正确（含 grant-session：`sign_grant_session` / `sign_p256_grant_session` 都经 `verify_passkey_for_wallet` → `verify_challenge_binding`，已覆盖）。本 PR **不改安全逻辑**，只把模式从硬编码常量改成**可控的 cargo feature**，使「翻转」成为一次构建选择而非改源码：

- `kms/ta/Cargo.toml` 增 feature `strict-challenge`（默认关）。
- `kms/ta/src/main.rs`：`ENFORCE_TA_CHALLENGE` 由 `#[cfg(feature = "strict-challenge")]` 选择（on→true / 默认→false）。
- `scripts/mx93-build.sh`：`MX93_STRICT_CHALLENGE=1` 构建 strict 镜像（默认 transition）。

**默认仍是 transition** —— 保证生产 `kms.aastar.io` 在 SDK（#58）就位前继续接受未迁移客户端，**本 PR 合入不改变生产行为**。

## 翻转流程（⚠️ 必须按序，否则打断生产）

```
前置：#58 aastar-sdk 升级上线(GetChallenge+clientDataJSON),
      确认所有活跃客户端都走新流程(观察 transition 警告日志归零)
  ↓
1. 构建 strict 镜像:  MX93_STRICT_CHALLENGE=1 ./scripts/mx93-build.sh ta
2. 选维护窗口部署:    MX93_BOARD_IP=<ip> ./scripts/mx93-deploy.sh
3. E2E 验证:
   ✓ 新流程(GetChallenge→带 clientDataJSON 签名)通过
   ✗ legacy 请求(不带 clientDataJSON)被拒:"strict mode: assertion missing clientDataJSON"
   ✗ 重放旧 assertion 被拒
4. 回滚预案:重新部署默认(transition)镜像即可
```

**不要在 #58 上线前用 `MX93_STRICT_CHALLENGE=1` 部署生产** —— 会拒掉所有未迁移客户端。

## 测试

- 编译：默认（`ree-fs-only`）+ strict（`ree-fs-only,strict-challenge`）两变体均通过（本 PR 已验证）。
- 行为（strict 镜像，E2E，需非生产板或维护窗口）：带绑定通过 / 不带绑定拒 / 重放拒。TA 为 no_std 无 cargo test，依赖真机 E2E（见 MEMORY）。
