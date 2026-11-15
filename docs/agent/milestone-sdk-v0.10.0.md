# Milestone — teaclave-trustzone-sdk 升级到 v0.10.0

> 单一事实源。建档 2026-11-13 11:20 +07（本机时间）。GitHub Milestone：**SDK v0.10.0 upgrade**。
> 原则：稳步推进，每一步都有真机证据；生产板（板 A / kms.aastar.io）只在最后一步才碰。

---

## 0. 背景与现状

| 项 | 现状 |
|---|---|
| 子模块 | `third_party/teaclave-trustzone-sdk` → fork `AAStarCommunity/teaclave-trustzone-sdk`，钉在 **`aastar-rpmb-patches`**（不是 `main`） |
| 基点 | `c2da817`（2025-09-26，介于 v0.6.0 和 v0.7.0 之间），落后 v0.10.0 共 133 个 commit |
| 我们的补丁 | 4 个：`b21d6be` RPMB backend 参数化 · `a75d631` RPMB 迁移崩溃安全 · `eefa483` REE-FS fallback / TLS-safe 计数 / hashbrown · `96c823c` RPMB ID 修正 |
| 上游 v0.10.0 | 2026-10-22，对应 **OP-TEE 4.11.0**；板子跑的是 **OP-TEE 4.8** |
| fork `main` | 纯镜像，同步走 fork PR #1，对构建没有影响 |

### 我们的补丁到底改了什么（以后要在 fork 里自己维护的部分）

上游的 `secure_db` 只支持 REE-FS（`TEE_STORAGE_PRIVATE`，文件放在 Linux 的 `/var/lib/tee`，root 可以拷走旧文件再放回来，也就是**回滚攻击**）。我们加了：

1. **存储后端参数化**：`SecureStorageDb` / `SecureStorageClient` 多了 `storage` 字段，`open_rpmb()`、`open_with_storage()`；底层 save/load/delete 都接受 `ObjectStorageConstants`。
2. **REE-FS → RPMB 一次性迁移**（`open_rpmb_migrating`）：分三阶段，每条幂等拷贝 → 写完成标记 `__rpmb_migration_complete_v1` → 最后才清 REE-FS，中途崩溃也不会丢钱包。
3. **RPMB 不可用时自动退回 REE-FS**：开箱的 i.MX93 没烧 eMMC RPMB key，先探测 `__rpmb_probe_v1` 能不能写，写不了就继续用 REE-FS。
4. **TA 内稳定性修复**：按内存里的 key list 计数（避免每条一次存储 syscall 把 TLS 寄存器 `tpidr_el0` 写坏）；`std::HashMap` 换成 `hashbrown`（`RandomState` 在 TA 里调 getrandom 会 panic）。
5. **`ObjectStorageConstants::Rpmb`**（值已修正为 `0x80000100`）。

KMS TA 本身（`kms/ta/src/main.rs`）另外有一个 **RPMB 单调计数器** `RPMB_COUNTER_ID`：每个钱包写入时带 `rollback_epoch`，和 RPMB 里的计数器比对，旧快照放回来就会被拒绝。生产用 `ree-fs-only` 编译，这整套目前**全部是关闭的**；真正开启是 #50 的事，不在本里程碑范围内。

上游 v0.10.0 已经有 `PrivateRpmb` 常量，第 5 项可以直接用上游的；第 1–4 项上游没有，要继续带在 fork 里。

---

## 1. 前置条件（开工前需要 jason 提供）

| # | 需要 | 原因 |
|---|---|---|
| P1 | **一块测试用 MX93**（建议板 B，或者一块不跑生产的板），网络 + 串口都接好 | 所有 TA 改动都要真机跑；板 A 是生产，不拿来做开发 |
| P2 | 确认测试板上的数据能不能动 | T7 要测"老 TA 写的钱包，新 TA 能不能读"，需要先备份 |
| P3 | 确认 TA 签名 key 和 devkit 的来源 | 本机 Docker **找不到 `teaclave_dev_env` 容器和镜像**；`BUILD-MX93.md` 写的是 vexpress devkit + `default_ta.pem`，要先确认板子的 OP-TEE 实际认哪把 key |
| P4 | minisign 发版私钥（只在 T10 需要） | 签 release 包和 measurement manifest |

---

## 2. 任务拆分

依赖关系：`T1 → T2 → (T3 ∥ T4) → T5 → T6 → T7 → T8 → T9 → T10`

| # | 任务 | 产出 / 完成标准 | 需要板子 |
|---|---|---|---|
| **T1** | **构建环境盘点与恢复** | 找回或重建 `teaclave_dev_env`（4.8 devkit + 签名 key）；能在本机重新编出**当前** TA，并把 sha256 记下来 | 否（P3） |
| **T2** | **基线** | 用当前 main（含 RPMB ID 修复）编 TA+CA → 部署测试板 → `run-full-e2e` 41/41 → 记录 TA measurement。顺带确认：`ree-fs-only` 下修 RPMB ID 前后，TA 二进制是否逐字节相同 | **是** |
| **T3** | **fork 补丁 rebase** | fork 新分支 `aastar-v0.10.0-patches`（基于 `v0.10.0`），4 个补丁 rebase 上去；`Rpmb` 改用上游 `PrivateRpmb`；`secure_db/db.rs` 冲突合并；`secure_db` 对上游新的对象 API（可变性、不再返回 raw handle）做适配 | 否 |
| **T4** | **新构建环境** | 工具链 `nightly-2026-08-05`；xargo 换成 `cargo -Z build-std`；**devkit 仍用板子的 OP-TEE 4.8**；离线 crate 缓存重新准备；更新 `scripts/mx93-build.sh`、`kms/docs/BUILD-MX93.md`、`.github/workflows/optee-build.yml` | 否 |
| **T5** | **TA / CA 代码适配** | 改 `kms/ta/Cargo.toml` 和 `kms/host/Cargo.toml` 里的 `crates/` 路径；对象 API 加 `mut`；处理 deprecated 参数 API 的警告（先保留旧 API 让它编过，迁移到 typed 参数 API 单独一个 PR）；TA 所有依赖在新 nightly 上编过；CA 适配 optee-teec 改动（`Send`、login code） | 否 |
| **T6** | **OP-TEE 4.8 兼容验证** | 对新 TA 的 undefined symbols 和 4.8 devkit `libutee` 做比对，不能引用 4.11 才有的符号（如 PBKDF2）；真机能加载 TA（`open_session` 成功） | **是** |
| **T7** | **数据兼容 + 回滚演练** | 测试板先用旧 TA 建几个钱包 → 换新 TA → 全部能读、能签；再换回旧 TA 也能读（回滚路径） | **是** |
| **T8** | **全量验证** | `run-full-e2e` 41/41；`ca-ta-consistency` CI 通过（含 `bls_dst_check`）；CC-103 字节 golden；`/version`、`/health`、`/RollbackCounter` 烟测 | **是** |
| **T9** | **评审** | 按 CLAUDE.md 的评审链：Codex 严格评审 + pr-daemon；重点是 secure_db rebase、memref 安全、构建链 | 否 |
| **T10** | **发布** | 新 TA measurement → 重签 attestation manifest；`airaccount-kms` 版本 bump；updater 灰度：测试板 → 板 B → 最后板 A | **是**（P4） |

### 不在本里程碑范围
- 开启 RPMB 防回滚（#50）：要烧 eMMC RPMB key，**这一步不可逆**，必须单独决策。
- 迁移到上游 typed 参数 API：等 T8 全绿以后单独做。
- 升级板子的 OP-TEE OS 到 4.11。

---

## 3. 风险

| 风险 | 影响 | 应对 |
|---|---|---|
| 构建容器丢失 / 签名 key 不明 | T1 卡住，整个里程碑卡住 | 先做 T1；必要时从 NXP BSP 重新 export devkit |
| SDK 4.11 绑定 vs 板子 4.8 | TA 加载失败或运行时出错 | T6 做符号比对 + 真机加载；必要时把 `optee-version` 钉回 4.8 头文件 |
| 新 nightly 下 TA 依赖编译失败 / 行为变化 | 签名结果变了 | T8 里 golden 逐字节比对 |
| 换 TA 后旧钱包读不出 | 用户资产风险 | T7 先备份 + 双向演练；生产最后才升 |
| 新 measurement 没重签 | 远程证明失败 | T10 发布清单里强制 |

---

## 4. 进度

| 日期 | 进展 |
|---|---|
| 2026-11-13 | 建档。fork `main` 同步 PR #1；RPMB ID 修复（fork PR #2 / 本仓 PR）。等 P1–P3 |
