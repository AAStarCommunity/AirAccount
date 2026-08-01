# 社区节点自动更新设计(Auto-Update Design)

> 状态:设计草案 v2(已过一轮 Codex 对抗性评审,对标 TUF/RAUC/Mender/OSTree/Sigstore) · 2026-08-01
> 关联:Task #21(整盘镜像 + CI release 流水线,验签/元数据的上游)、Task #13(CA/TA 一致性门)、
> `kms/deploy/aastar-node-installer.sh`、`kms/docs/community-node-image-ci-design.md`、
> `kms/docs/RELEASE-CHECKLIST.md`、`kms/docs/out-of-band-console/`(远程救板)

## 0. TL;DR(评审后修正结论)

方向对,但 v1 **不完备,不能作为默认开启的 TEE/KMS 自动更新方案**。最核心的缺口:
只签了单个 tarball,**没有 TUF 式的更新元数据新鲜度/防回滚模型**;TA/CA/TEE 存储状态**没有真正原子绑定**;
签名根与 CI 泄露**爆炸半径没收住**;灰度只是 jitter、**不是熔断式 rollout**。

**MVP 可做,但必须收窄范围 + 先补 5 项地基**(见 §9)。在执行私钥签名的社区节点上,缺任一项都不建议默认开。

## 1. 问题

社区节点是**远程、NAT 后、难物理触达、且执行签名**的 KMS/DVT 设备。发新 release 后它们怎么拿到更新?
朴素方案"轮询→有新版就拉→重部署"方向对,但对签名底座有一批坑,必须系统性补齐(见 §3–§8)。

更新单元是 **CI 产出、可复现、已签名的 release tarball**(`airaccount-node-$VERSION.tar.gz`,含 host CA
`kms-api-server` + OP-TEE TA + systemd unit),**不是板上重编**(交叉编译慢且脆)。这是 Task #21 的下游。

## 2. 决策(评审后修正)

- **MVP 默认 `notify-only`(不是 `on`)**。首启向导显式 opt-in 才开自动应用;安全补丁的自动应用**先只对 canary ring 开**,观察通过再放量。
  (v1 原写默认 `on + security-only`,Codex M1:对 MVP 太激进——远程签名节点先保守。)
- **TA 自动更新默认关**:TA 变更(RSA-4096 签名、牵动 secure storage/RPMB)风险高于纯 CA,默认只通知。
- **MVP 只做 CA-only 自动路径**;TA/proto/storage schema 变更走"通知 + 人工/OOB"。
- **更新策略**(opt-in 后):`security`(仅被签名元数据标注的安全 patch 自动)/ `minor` / `all` / `off`,社区可控。
- **先补地基再默认开**(§9)。

## 3. 架构:独立 updater + crash-safe 状态机

独立 systemd oneshot + timer,**不**让 KMS 签名进程改自身二进制(最小权限、职责分离、崩了不挡更新)。

```
airaccount-updater.timer   每 CHECK_INTERVAL(默认 6h)± RandomizedDelaySec 触发
        │
airaccount-updater.service (Type=oneshot) 执行 aastar-node-updater.sh:
  1. flock 防并发;读 /etc/airaccount/updater.env
  2. 拉 signed 更新元数据(TUF-lite,§4)→ 校验新鲜度 + 阈值签名 + 防回滚
  3. 依 targets metadata 选候选版本;按 policy + rollout cohort(§7)判定是否应用
  4. 兼容性门(§6):requires_ta_version / proto_version / storage_schema / rollback_floor
  5. 下载 tarball;✅ 验 sha256 + ✅ 验签(cosign,pin identity,§5)+ ✅ 比对 metadata 中 hash
  6. 写状态机 pending → 落盘 releases/<ver>/(.new + fsync + rename,§3.1)
  7. 原子切换 current;systemctl restart;健康门(§3.2 深度门)
  8. ✅ 健康 → 状态机 committed + 更新 last-good;❌ → rollback last-good + restart + 告警
  9. boot-time recovery:开机若发现 pending 未 committed → 自动回滚(治掉电中断)
```

### 3.1 版本化目录 + crash-safe 原子切换(修 S2 / I6)

v1 说"改软链 = 原子",但只覆盖 CA;现 installer 直接覆盖 `/lib/optee_armtz/<uuid>.ta` 和 CA
(`aastar-node-installer.sh:82-83`),掉电落在"TA 已换 / CA 未换"之间会得到不可预测组合,软链回滚也回不了 TA。

修正:**TA 也版本化**,和 CA 一起纳入 `current` 切换。

```
/opt/airaccount/
  releases/0.28.1/  { kms-api-server, <uuid>.ta, kms-api.service, attestation-measurements.json, manifest.json }
  releases/0.29.0/  { ... }
  current   -> releases/0.29.0      # systemd ExecStart 指 current/;OP-TEE TA 路径固定指 current/<uuid>.ta
  last-good -> releases/0.28.1      # 回滚目标
/var/lib/airaccount/updater/state.json   # { pending, committed, last_good } 状态机
```

- TA 路径:若 OP-TEE 支持跟随符号链接,`/lib/optee_armtz/<uuid>.ta` 固定软链到 `current/<uuid>.ta`;否则用 `.new + fsync + rename` 两阶段替换(rename 原子)。
- **所有 symlink/rename 后 fsync 父目录**;`flock` 防并发。
- **顺序纪律(修 I6)**:pending 成功且健康门通过前,**绝不更新 last-good**;committed 后才 vacuum 旧版本;保留 N=`KEEP_RELEASES` 个,清理不得删掉唯一可回滚版本。
- **boot recovery**:开机检查 state.json,发现 pending 未 committed(=上次更新中途掉电)→ 自动回滚到 last-good。

### 3.2 健康门要够深(修 I5)

v1 的 `/health + /version + 一次签名自检`太浅,测不出延迟崩溃 / secure storage 迁移失败 / 队列熔断 / profile 错。
健康门至少检查:
- `/version` 的 `profile=prod`、`ta_mode=real`(不是 mock)
- `/RollbackCounter` 可读(TEE storage 正常)
- `/QueueStatus` 非熔断
- attestation measurement 与 manifest 声明一致
- 真实 API key 鉴权路径通(不是 open mode 误开)
- 一次真实签名自检

## 4. 更新元数据安全模型:TUF-lite(修 S1 / I2)

v1 只签 tarball,按 GitHub Releases API 选版本——**旧版合法签名 tarball 仍会通过**,回滚攻击 / freeze attack /
mix-and-match 全部成立:攻击者只要让设备长期看不到新 timestamp,或投喂旧 metadata,就能把节点卡在有已知漏洞的旧版。

修正:引入 TUF(或 TUF-lite)四类元数据:

| 元数据 | 作用 | 签名 |
|--------|------|------|
| **root** | 信任根,声明各角色公钥与阈值 | 离线阈值签名(§5) |
| **targets** | 每个 release 的版本/hash/security/min_version/rollout/兼容性字段 | targets role |
| **snapshot** | 绑定 targets 的一致快照(防 mix-and-match) | snapshot role |
| **timestamp** | 短期过期(如 24h),证明"这是最新视图"(防 freeze) | timestamp role(可在线) |

客户端(updater)规则:
- **持久化"见过的最高版本 / metadata 版本号"**,拒绝任何更旧的 metadata 和 target(防回滚 / mix-and-match)。
- **timestamp 过期 = 拒绝更新 + 告警**,并区分"正常无更新" vs "拿不到新鲜 timestamp"(后者是 freeze 攻击信号,I2)。
- 支持**多 mirror**,不把 GitHub 当唯一新鲜度来源(修 I2 单点)。

`security` / `min_version`(rollback floor) / `rollout` 全部进入**签名的 targets metadata**,而不是可编辑的 GitHub release body。

## 5. 签名与密钥:角色分离 + 收爆炸半径(修 S4 / S5)

v1 把私钥放 CI secret/OIDC,无阈值、无角色分离、无撤销——**一个 CI secret 或 workflow 身份被拿下就能给全网签恶意 KMS/TA**。

修正:
- **root key 离线,2-of-3 或 3-of-5 阈值**;在线只持 targets/timestamp 委托 key,权限最小。
- **角色分离**:TA 签名 key ≠ release metadata key ≠ attestation manifest key,互不通用。
- **cosign keyless**:必须 pin `certificate-identity`(具体 workflow ref)+ issuer,并保留 **Rekor inclusion proof / bundle**(透明日志,契合"可复现+透明"信任根战略)。
- **公钥/root 轮换与撤销**:root metadata 支持 key rotation;吊销走 root 重签下发。
- **`security=true` 由 security role 断言**(最好阈值),字段含 `severity / GHSA/CVE / fixed_from / auto_apply_allowed / rollback_floor`——不可被单一发布 key 或 GitHub body 伪造(修 S5)。

## 6. 防回滚 floor + 兼容性门(修 S3 / S4)

### 6.1 软件版本防回滚接进 TEE(修 S3)

`/RollbackCounter` 是**钱包状态**防回滚诊断,不保护"软件版本";而 TA 在 RPMB 不可用时还会降级为 absent
(`ta/src/main.rs`),意味着可降级到旧 TA/CA 继续用当前密钥签名。

修正:TA 内维护 **monotonic `min_software_epoch` / `min_ta_version`**,存 RPMB 或等价不可回滚介质。
安全修复合入后提升该 floor;manifest 声明 `rollback_floor`。**floor 提升后,旧的 last-good 不再可回滚**,必须走 OOB(见 §8)。

### 6.2 CA/TA/proto 兼容性门(修 I4)

`RELEASE-CHECKLIST.md` 已规定 proto 变更必须 host+TA 同部署;但 v1 updater 只看"TA 字节变没变",
CA 自动升、TA 留旧 → 运行时协议错配。

修正:manifest 增加 `requires_ta_version / proto_version / storage_schema`;updater 做**兼容性门**,不满足只通知不自动应用。

## 7. 灰度要熔断,不只是 jitter(修 I1)

v1 的 jitter 只错开时间,坏版本仍会在 6h 内铺完。

修正:**signed rollout metadata**(在 targets 里):`cohort_hash / percentage / canary_ring / pause / withdraw` 字段。
- 节点按稳定 cohort hash 判定自己在第几批。
- 监控失败率(节点上报 + 巡检盘),超阈值**自动暂停/撤回 rollout**(熔断)。
- 节奏对标 Tailscale:安全更新更快,普通更新先观察数日。

## 8. 失败兜底与 OOB 衔接(修正版)

| 场景 | 行为 |
|------|------|
| 网络/GitHub 不可达 | 静默退出重试;**但 metadata 过期要告警**(区分无更新 vs freeze)。 |
| 验签/元数据校验失败 | 中止,不应用,告警(疑似投毒/损坏/freeze)。 |
| 兼容性门/floor 不满足 | 只通知,不自动应用。 |
| 健康门不过 | 自动回滚 last-good + restart + 告警。 |
| 回滚后仍不健康 / floor 已越过无法回滚 | 停机红色告警,转 **OOB 控制台**(`kms/docs/out-of-band-console/`)人工救板。 |
| 掉电中断更新 | boot recovery 自动回滚 pending。 |
| `PIN_VERSION` 已设 | updater 不动;**但若 pin 低于 security floor,持续红色告警**(修 M2)。 |
| 磁盘不足 | 应用前预检,不足则中止+告警,不动 current。 |

## 9. 上线前必补的 5 项地基(评审硬门槛)

默认开启前,以下缺一不可(对应 Codex 5 个严重项):

1. **TUF-lite 签名元数据 + 过期/防回滚检查**(§4,修 S1)。
2. **crash-safe symlink/state machine + boot recovery + TA 固定路径原子性**(§3.1,修 S2)。
3. **CA/TA/proto 兼容性 manifest + 防回滚 floor 接 RPMB**(§6,修 S3/I4)。
4. **签名 key 角色分离 + 阈值 + 轮换/撤销 + cosign identity pin + Rekor**(§5,修 S4/S5)。
5. **灰度 cohort + 失败熔断 + OOB 状态衔接**(§7/§8,修 I1)。

在此之前:MVP 只做 **CA-only + TA 自动关 + 默认 notify-only / canary opt-in**。

## 10. 现状 installer 的供应链问题(先修,修 I3 + 次要项)

updater 再安全,首装阶段裸奔也白搭:

- installer 下载 release 后直接解包安装(`aastar-node-installer.sh:71`),还从 `raw.githubusercontent.com/main`
  拉 fallback(`:96-102`)—— GitHub/main 分支/MITM 都能在首装阶段注入。**installer 也必须验 signed manifest;禁 raw main fallback**;DVT tag/commit、node-setup、systemd unit 全纳入 bundle hash。
- **M3**:`latest` URL 实际硬编码 `v0.29.0`(`:65-68`),不是真 latest,会版本漂移 → 改为查 metadata。
- **M4**:combined profile 下 `deploy-dvt.sh` 非零只 log 不 fail(`:121-124`)→ 应 fail-loud,否则节点"完成"但 DVT 半残。
- **M5**:`UPDATE_ONLY=1` 必须保证**不重写** `KMS_BLS_PROVISIONING=1`(首装才写 prov.conf,`:134-135`),否则更新会误触发重新 provisioning。

## 11. 实现清单(评审通过后)

- [ ] Task #21:CI 发版加 TUF-lite 元数据 + cosign 签名(pin identity)+ Rekor bundle;公钥/root 落地。
- [ ] `RELEASE-CHECKLIST.md`:加 security/severity/rollback_floor/requires_ta_version 标注步骤。
- [ ] installer 加固:验 signed manifest、禁 raw main fallback、真 latest、DVT fail-loud、`UPDATE_ONLY=1` 不碰 prov。
- [ ] installer 版本化目录 + `current`/`last-good` 软链 + crash-safe rename/fsync + 一次性迁移脚本。
- [ ] `aastar-node-updater.sh`:元数据校验 / 兼容性门 / 下载验签 / 状态机 / 深度健康门 / 回滚 / boot recovery。
- [ ] `airaccount-updater.{service,timer}`(oneshot + RandomizedDelaySec + boot-recovery unit)。
- [ ] TA 内 `min_software_epoch/min_ta_version` 防回滚 floor(RPMB)。
- [ ] rollout cohort + 失败熔断;监控:probe 采版本/更新状态、`monitor.html` 加列、telegram 告警。
- [ ] 测试(Task #16 虚拟 DVT harness):升级成功 / 验签失败中止 / freeze(过期 metadata)拒绝 / 回滚攻击拒绝 / 健康门失败回滚 / 掉电 boot recovery 六条路径。
- [ ] 文档:社区侧"开关/pin/更新失败怎么办/OOB 救板"。

## 12. 待评审决策点

1. **TUF 全量 vs 自研 TUF-lite**:全量 TUF(python-tuf/go-tuf)成熟但重;TUF-lite 轻但要自己保证不漏 freeze/rollback。节点规模小,倾向精简但严格实现四类元数据的核心不变量。
2. **cosign keyless vs minisign + 自管 root**:keyless 免管私钥 + Rekor 透明日志,契合信任根战略;但依赖 Fulcio/Rekor 在线。倾向 keyless + 离线 root 元数据兜底。
3. **灰度**:小规模(个位数~十几)是否值得完整 cohort/熔断,还是先 canary ring(手动指定 1–2 台)+ 失败告警人工暂停。
4. **installer 版本化目录迁移**:现有已部署板直接装在 `$TA_DIR/$CA_DIR`,首次引入软链布局需一次性迁移脚本 + 回退预案。

---

### 附:Codex 评审对标业界(2026-08-01,一轮)

- **TUF**:role separation / threshold / rollback / freeze / snapshot+timestamp 新鲜度 —— v1 只做了 target 签名,缺 root/snapshot/timestamp 这一半 → §4/§5。
- **RAUC / Mender / OSTree**:先完整写备用槽 → 激活 → mark-good/commit,失败由 bootloader/watchdog 回滚 —— 软链模型可作应用层简化版,但必须补 fsync/pending state/boot recovery/TA 原子性 → §3.1。
- **Sigstore/cosign + Rekor**:适合 CI keyless,但必须 pin identity + 留 Rekor bundle → §5。
- **apt-secure / systemd-sysupdate**:签名索引/元数据链 + 多资源整体更新,不是只签单包 → §4。
- **Chrome/Omaha / Tailscale**:staged rollout + rollback 熔断;安全更新更快、普通更新观察数日 → §7。

参考:TUF spec、RAUC mark-good/mark-bad、Mender rollback/commit states、OSTree atomic upgrades、apt-secure、Sigstore keyless/Rekor、systemd-sysupdate、Tailscale auto-update。
