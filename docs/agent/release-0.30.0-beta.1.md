# Release Plan — airaccount **v0.30.0-beta.1** (Beta7)

> 单一事实源。协调发布 **KMS + node + DVT** 三组件的 beta 预发布。
> 分支:`release`(off `main` @ 806239e)。GitHub 标记为 **pre-release**,不覆盖稳定线。
> 建档:2026-08-12 · 维护:pilot run 循环随进度更新本文件。

---

## 1. 决策(已拍板)

| 项 | 决定 |
|---|---|
| 发布性质 | **beta 预发布**(GitHub pre-release),tag 带 `-beta.1` 后缀 |
| 范围 | **KMS + node + DVT 三组件协调发布**,非单仓 |
| 内容闸门 | 等 **#195 + #196 + serial PR** 三者合并后,再从新 main 走 |
| node 版本 | `airaccount-node-v0.30.0-beta.1` |
| 安全门 | release 分支过一次**安全检查**,查出问题打 patch **一并纳入本 beta** |

## 2. 组件版本矩阵

| 组件 | 仓库 | 当前 | 本次 beta | 说明 |
|---|---|---|---|---|
| **node 整包** | 本仓 | `airaccount-node-v0.29.1`(pre) | **`v0.30.0-beta.1`** | 社区节点整包,统一入口 tag |
| **KMS 二进制** | 本仓 `kms/` | `airaccount-kms-v0.29.0` / code `0.29.0` | **待定**:若含 KMS API/TA 变更 → `v0.30.0-beta.1`;若仅 updater/脚本 → 复用 `0.29.x` | `#195/#196` 是 updater/admin 工具,**非** KMS API;是否 bump 待安全门+最终内容定 |
| **DVT** | `YetAnotherAA-Validator`(repo:dvt) | pin `v1.10.0` | **`v1.13.1`✅ 已定版并 pin** | GitHub release v1.13.1(Latest,2026-08-15);v1.13.0=guardian-slash + v1.13.1=fail-closed 硬化;本仓 pin 已升(release `7ce5d75`),向后兼容默认行为不变 |

> ⚠️ 版本对齐原则:node 是统一入口号;KMS/DVT 各自的 tag 按**是否有本体变更**决定 bump 与否,不为对齐而空转版本号。

## 3. 内容闸门(全绿才可打 tag)

| # | 闸门 | 状态 | 我能否推进 | 阻塞点 / 下一步 |
|---|---|---|---|---|
| G1 | **#195** Web 管理台增量1(feat/updater-phase2-web-admin @ `8eaf9d6`) | 🔴 BLOCKED · CHANGES_REQUESTED | ❌ 代码已全绿,不能自合(main 保护+需非作者 approve) | 重启 pr-daemon 复评 `8eaf9d6` **或** 人工 approve → auto-merge |
| G2 | **#196** 发版签名(feat/updater-release-signing @ `05251c2`) | 🔴 BLOCKED · CHANGES_REQUESTED | ❌ 同上,已全绿 | 重启 pr-daemon 复评 `05251c2` **或** 人工 approve → auto-merge |
| G3 | **serial-selfupdate.sh PR** | 🟢 **PR 已开 #201** | ✅ 已做 | 从最新 main 干净新分支 `feat/oob-serial-selfupdate` 只提取脚本+README(老 stale 分支会回滚半个仓库,已避开)。作者 2 轮自审(改一处注释过度声称),grade=B 余下对抗 review 交 pr-daemon。合并后 `docs/oob-serial-rescue` 可清理 |
| G4 | **DVT fixes + 定版** | 🟢 **已闭环** | ✅ 已做 | DVT **v1.13.1**(Latest)发布;CC-90 两行动项全做:①watcher env 生产 aggregator(`afcf2aa`)②pin v1.13.0→v1.13.1 fail-closed 硬化(`7ce5d75`)。DVT 确认交付无误。deploy-dvt.sh 零改动(密钥名未变)。**生产 aggregator = `0x174b60bB462b00550F0EC7Bc35Fe39dDB6310158`**(SP A' 4.3.0,**SP 24h apply 后生效**;v1.13.1 起非 Sepolia 须显式设否则拒启)。待 G5 前**真机验证 guardian watcher 通路**(opt-in,须 aggregator apply 后) |
| G5 | **release 分支安全检查** | ⬜ 待做(内容齐后) | ✅ 可发起 | 内容合齐后跑;查出问题 → 打 security patch 折入本 beta |

## 4. 发布执行步骤(闸门全绿后)

1. `release` rebase 到最新 main(含 #195/#196/serial 合并结果)。
2. **版本 bump**(仅当 KMS 本体有变更):
   - `kms/host/Cargo.toml` `version`
   - `kms/host/src/api_server.rs:4270` `KMS_VERSION`
   - 二者与 tag 严格一致(历史惯例:`/version` 自报对齐 tag)。
3. `kms/CHANGELOG.md` 新增 `0.30.0-beta.1 (Beta7)` 段:列 #195/#196/serial + 任何 DVT pin 变更 + 安全 patch。
4. **构建**:kms.aastar.io 板 CA 必须带 `MX93_DEV_RPID=1 MX93_STRICT_CHALLENGE=1`(否则 `/version` 翻 prod/transition)。CA-only 手动 scp + restart,勿用 mx93-deploy.sh(会推旧 TA)。
5. **打 tag**:`airaccount-node-v0.30.0-beta.1`(+ 若 bump 则 `airaccount-kms-v0.30.0-beta.1`)。tag 特定 commit 用 merge/rebase,**勿 squash**(会孤立);main 受保护,force-push 会孤立 approval。
6. **GitHub Release**:标 **pre-release**;门户 `/portal` 会过滤 prerelease,注意 installer 动态钉 tag 行为。
7. **部署 + 烟测**:`/version` 三项(rpid/challenge/mode)+ `/health` + `/RollbackCounter`;板 A/B 各测。
8. 更新 `docs/agent/progress.md`,关闭本 release 计划。

## 5. 现在可动的实事(不等闸门)

- **G3 serial PR**:`docs/oob-serial-rescue` 远程已 gone,需重新 push + 起 PR。起 PR 前走 pilot 纪律(preflight + grade-change 定评审轮数 + 自审/对抗 review)。**这是三道内容闸里唯一我能直接推的。**
- **G5 安全检查预备**:内容合齐前 release 分支 == main 无 diff,安全检查等 §3 合齐后跑才有对象;先占位为 gate。
- **#195/#196**:代码侧无事可做,纯等外部 approve/daemon。

## 6. 风险 / 备注

- DVT 是**跨仓协调**,本 beta 的实际发布时点受 DVT fix 进度制约,可能是最长的一条边。
- `airaccount-node` 上一版 `v0.29.1` 已是 pre-release;本 beta 延续 pre-release 语义。
- attestation(#200 已关)不在本 beta 范围,`attestation-measurements.body.next.json` 未跟踪文件搁置,待独立 attestation 流决策。
