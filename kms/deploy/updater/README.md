# 社区节点自动更新器(MVP / increment 1)

> 设计与威胁模型:[`kms/docs/auto-update-design.md`](../../docs/auto-update-design.md)(已过一轮 Codex 对标 TUF/RAUC/Mender/OSTree/Sigstore 评审)
> 本目录是**增量 1**:CA-only 自动路径、默认 `notify-only`、可 100% 本地测试。

## 它做什么

节点周期性(systemd timer)拉一份**签名的 channel manifest**,校验后按策略**自动应用安全补丁**或**只通知**,应用走 **crash-safe 版本化目录 + 原子软链 + 健康门 + 自动回滚**,掉电中断由 **boot recovery** 兜底。

**Phase 1(通知 + CLI 手动应用,本 PR)**:默认 `notify-only`,发现新版推 **Telegram**(富通知:版本 / 变动 / 安全级别 / 是否含 TA + 去重);运维收到后 `ssh` 进板跑 `aastar-node-updater apply <ver>` 显式应用(越过 policy 门,但**不越过**验签/防回滚/兼容/TA 门)。**TA 变更默认拒绝在线应用**(`--allow-ta` 仅限带外/专门流程)。设计与安全评审见 [`auto-update-web-admin-design.md`](../../docs/auto-update-web-admin-design.md)。

```bash
aastar-node-updater check                 # 定时器调用:拉 manifest→校验→按策略应用/通知
aastar-node-updater apply 0.30.0          # 手动应用指定版本(收到通知后 ssh 进板跑)
aastar-node-updater apply 0.31.0 --allow-ta   # 强制放行 TA 变更(慎用,带外场景)
aastar-node-updater status                # 打印当前状态
```

远程、NAT 后、难物理触达的签名节点最怕"坏版本自动铺开变砖" —— 本设计的核心保险就是**验签拒毒 + 健康门回滚 + 掉电恢复 + 错峰**。

## 安全不变量(测试逐条覆盖)

| 不变量 | 机制 | 测试 |
|--------|------|------|
| 只接受可信来源 | minisign 离线验签(公钥编入节点) | T2 |
| 防 freeze attack | manifest `expires` 过期即拒 | T3 |
| 防回滚攻击 | `metadata_version` 持久化单调不降(候选亦不得低于 `rollback_floor`) | T4 |
| 不轻易变砖 | 健康门失败 → 自动回滚到上一版 | T5 |
| 掉电可恢复 | `pending` 状态标记 + boot recovery | T6 |
| 保守默认 | `notify-only` 默认,安全补丁才自动、canary 先行 | T1/T7/T8 |
| security 策略不被架空 | 遍历所有候选选「可自动应用的最高版」,更高的非安全版不挡安全 patch | T15 |
| 总开关优先于 PIN | `PIN_VERSION` 受 `AUTO_UPDATE` 约束,notify-only 下不自动应用 | T16/T17 |
| CA/TA 兼容性门 | `requires_ta_version` > 当前且本次不换 TA → 只通知 | T18 |
| 健康门核对版本 | 内置健康门比对 `/version` == 刚部署版本(防跑旧二进制误判) | 内置 `AU_EXPECT_VERSION` |

## 文件

| 文件 | 作用 |
|------|------|
| `aastar-node-updater.sh` | 更新器主程序(`check` / `apply <ver>` / `recovery` / `status`) |
| `notify-telegram.sh` | 通知 hook → Telegram(AAStarMonitorBot);作 `AU_NOTIFY_CMD` 注入,fail-safe |
| `sign-channel.sh` | CI/测试:给 channel.json 签名(minisign) |
| `channels/stable.json.example` | manifest 模板(真实文件由 CI 生成+签名) |
| `updater.env.example` | 社区可改的策略配置(总开关/通道/策略/pin) |
| `airaccount-updater.{service,timer}` | 周期检查 + 错峰(RandomizedDelaySec) |
| `airaccount-updater-recovery.service` | boot 时先于 kms-api 跑,回滚未提交更新(只切 symlink+state,不 restart) |
| `kms-api.service.d/10-airaccount-current.conf` | drop-in:ExecStart 指向 `current/`,**updater 生效前提** |

## 目录布局(节点上)

```
/opt/airaccount/
  releases/<ver>/   { kms-api-server, <uuid>.ta, kms-api.service, ... }
  current   -> releases/<新版>      # systemd ExecStart 指向 current/
  last-good -> releases/<回滚目标>
  updater/aastar-node-updater.sh
/var/lib/airaccount/updater/state.json   # {seen_metadata_version,current,previous,pending}
/etc/airaccount/updater.env              # 策略
/etc/airaccount/updater-pubkey.pub       # minisign 验签公钥
```

## 本地测试(无需硬件)

```bash
brew install minisign jq       # mac;板子/CI 用 opkg/apt
bash kms/tests/updater/test-updater.sh
```

所有外部副作用(restart / 健康门 / 通知 / 下载)都走可注入 hook(`AU_RESTART_CMD` / `AU_HEALTH_CMD` / `AU_NOTIFY_CMD` / `AU_FETCH_CMD`),manifest/tarball 用 `file://`,故整套在开发机上闭环跑。

## 生产接线(评审通过后)

1. CI:发版时生成 `channels/<channel>.json`(含 sha256/security/兼容字段),用 minisign 私钥(CI secret / 离线)签名 → `.minisig`;发布到节点可拉的稳定 URL(`AU_MANIFEST_BASE`)。
2. 板子:installer 落地 `updater.env` + 验签公钥 + 版本化目录 + enable timer & recovery unit。
3. 监控:`AU_NOTIFY_CMD` 接 `AAstarMonitorBot`(telegram);`monitor.html` 加版本/更新状态列。

## 本增量**未做**(见设计文档 §9,真机/CI 阶段接)

- RPMB 内 `min_ta_version` 防回滚 floor(需真机 TEE)
- cosign keyless + Rekor 透明日志(CI 侧,叠加在 minisign 之上)
- TA 固定路径原子切换 + 全量 TA 自动更新(默认关)
- rollout cohort 失败熔断(<15 节点先 canary + 人工暂停)
- `rollback_floor` 目前只在**前向候选选择**时强制(不选低于 floor 的版本)+ 越界告警;**回滚/recovery 仍可回到 floor 以下**(强制回滚 floor 需 RPMB,见 RPMB floor 推迟项)
- 真机深度健康门(profile=prod / ta_mode=real / attestation measurement 比对)
- installer 加固(禁 raw main fallback、真 latest、版本化目录迁移)
