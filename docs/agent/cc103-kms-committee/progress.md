# CC-103 KMS committee 集成 — 进度台账

> 实时状态。每推进一步更新。协同源:Seeder CC-103(b8f3441f)。

## 当前状态:开工中(T1/T3 done,T2/T4 待续)

**第一轮(2026-08-18,feat/cc103-kms-committee-audit)**:
- **T1 B2 审计 ✅** — A1 accountId 构造/注入 = 0 处;A2 唯一 "account" 命中 = session 0x08 的 20B 账户绑定(防跨账户重放,非 B2 的 32B committee accountId,不违反 B2);B1 DST 在位。结论落 `kms/docs/committee-signing-boundary.md` §3。
- **T3 边界文档 ✅** — `kms/docs/committee-signing-boundary.md` 已写(不透明签名机 + BLS 原像=userOpHash + DST 三方一致 + 两种 account 区分 + 方向红线)。
- **T2 DST golden KAT** — 待续(下轮:找 kms-ta/host crate 加 KAT)。
- **T4 legacy 集成盘点** — 待续。

- **规范定稿** 2026-08-18:airaccount-contract + dvt 逐字节答复,KMS = **签名代码零改动**(SPEC.md)。
- CC-103 已发 [repo:kms] 确认帖(commentId 4e96e9dc):接受 dvt 两处纠正(下限 30 非 17;N=3 可激活),声明 KMS 零改动。
- 协同轮询(cron)已停(规范锁定)。

## Tasks

| Task | 说明 | 状态 | 验收 |
|---|---|---|---|
| T1 | B2 审计(签名路径零 accountId) | READY | ACCEPTANCE §A |
| T2 | DST golden-vector 回归 KAT(唯一新增代码) | READY | ACCEPTANCE §B |
| T3 | 边界设计说明 `kms/docs/committee-signing-boundary.md` | READY | ACCEPTANCE §D |
| T4 | legacy 集成盘点(新 Sepolia 地址) | READY(E2E 或 BLOCKED-testnet) | ACCEPTANCE §C |

## 阻塞 / 待决

- T4 E2E 需 testnet + keeper;若不可用则标 BLOCKED,不影响 T1/T2/T3。
- T2 golden fixture 三方对齐依赖 dvt 提供 golden vector;拿不到则先锁 KMS 自产 KAT + 挂跟进项。

## 跨仓依赖(非 KMS,记录防误判)

- SDK:per-signer wire 组装 + 链上读 TREE_DEPTH + enroll 流程。
- dvt:翻 setEpochLength 激活 committee(CC-104)。
- airaccount-contract:账户侧 perSigner=512 硬编码耦合。

## 决策记录

- KMS 不在 TEE 加 accountId 绑定(dvt 纠正:破坏不透明边界 + TEE↔链上漂移)。
- committee 正向 E2E 阻塞在 SDK per-signer wire + dvt 翻转,**不在节点数**(N=3 即可)。
