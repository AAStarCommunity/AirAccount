# 社区节点更新:通知 + 一键应用(CLI / Web 管理台)设计稿

> 状态:设计草案 v1.1（已过一轮 Codex 对抗性评审，结论并入 §0.5）· 2026-08-02
> 作者:jason + Claude
> 关联(必读上游):
> - `kms/docs/auto-update-design.md` —— TUF-lite 签名元数据 / crash-safe 状态机 / 健康门 / 防回滚（本稿的**信任与应用底座**，不重复）
> - `kms/deploy/updater/aastar-node-updater.sh` —— 已实现的 updater(check/recovery/status)
> - `kms/deploy/updater/channels/stable.json.example` —— channel manifest schema
> - `kms/scripts/oob/serial-selfupdate.sh` —— **带外串口**一键升级(够不到板子时的 break-glass)
> - `kms/scripts/oob/serial-run.py` —— 串口执行器

---

## 0. TL;DR

在**已实现的 updater**(签名 manifest + minisign 验签 + sha256 + 健康门 + 自动回滚)之上，补三件：

1. **通知层**:updater 定时扫 GitHub（信息中心）→ 发现新版 → 推 **Telegram（AAStarMonitorBot）**，消息含 `版本 / 变动摘要 / 安全级别 / 是否含 TA`。默认 `notify-only`，**不自动改动签名节点**。邮件是后续通道。
2. **CLI 应用路径**:运维收到通知 → `ssh` 到自己的板 → 跑一条显式命令 `aastar-node-updater apply <ver>` → 验签 + 暂停 + 原子换 + 重启 + 健康门 + 失败回滚。
3. **Web 管理台**(可选、默认关):板上一个**仅本机/Tailscale 可达**的小管理页，管理员密码登录后可视化:看版本 diff / 安全级别 → 暂停 KMS(CA、可选 TA)→ 应用更新 → 实时进度 + 一键回滚。

**核心安全立场**:Web 管理台能替换 TEE 的 CA/甚至 TA，是**整套系统里权限最高的攻击面**。因此：
- **绝不**绑定到公网 cloudflared 域名；只听 `127.0.0.1` + Tailscale 接口。
- **验签永远在服务端做**，浏览器只发指令、拿状态；浏览器绝不经手二进制或信任决策。
- **TA 更新**默认不走 Web 一键（RSA-4096 签名 + secure storage/RPMB 迁移，风险量级不同），只显示「通知 + 走专门 OOB 流程」。
- 密码 argon2id 存 hash、会话短 TTL、CSRF、审计日志、失败限速、`enforce_admins` 式二次确认。

---

## 0.5 Codex 对抗性评审修正（v1 → v1.1）

一轮 Codex 评审（2026-08-02）。**总判:方向可保留，但不能按 v1 顺序做，尤其不能先做 Web 台。** 逐条并入:

**Critical(必须先解决，否则不写 Web)**
- **C1 root helper 而非 sudoers 跑脚本**:updater 行为受大量 env 控制（`AU_RESTART_CMD/AU_FETCH_CMD/AU_ROOT`）且会 `source updater.env`。Web 经 sudo 跑脚本 = 提权 RCE 面。**改**:写一个 root-owned privileged helper，**固定绝对路径 + 清空环境 + 固定 config + 严格 argv 白名单**，生产执行路径**禁止 hook env 注入**；Web 非 root，只能调 helper 的固定动词。
- **C2 先有可信 CLI 再有 Web**:现脚本只有 `check/recovery/status`。**改**:先实现并测 `apply <ver>` / `rollback` / `plan <ver>` 三个服务端可信命令，Web 只准调它们（单一可信实现）。
- **C3 TA 排除要“服务端能力级”**:不能只是 Web 不显示按钮。`apply_version` 会整包复制 `kms/`。**改**:CA-only apply 必须校验 bundle 内 TA hash==当前 TA（或直接丢弃 TA 文件）；TA 更新走独立 component role + 独立确认 + 独立流程，**服务端根本不提供 TA 在线热更能力**。
- **C4 兼容门用真实 TA 版本**:现兼容判断拿「CA 当前版本」当「TA 当前版本」比（`requires_ta_version` 误判）。**改**:从 `/version` 或本地 release manifest 读**真实** `ta_version/proto_version/storage_schema` 再比。

**High**
- **H1 Web apply/rollback 强制二因子**:密码只能是第一因子，优先 WebAuthn/FIDO2 或 Tailscale identity + device posture + TOTP。
- **H2 暴露面启动即自检**:服务启动时枚举监听地址 + cloudflared/frp/nginx 配置 + systemd socket，**发现 8788 被代理立即拒启动**；再加 nftables 只放 loopback + tailscale0。
- **H3 CSRF/XSS 写死规则**:所有写操作校验 CSRF token + **精确 Origin + 精确 Host**，全关 CORS，拒绝未知 Host（防 DNS rebinding 经 localhost/tailnet 打进来），登录 POST 也查 Origin。
- **H4 version 索引的 TOCTOU**:schema 强制 version 唯一;Web 生成**服务端 update plan**，绑定 `manifest digest + metadata_version + target hash`，apply 前 digest 变了必须重新展示确认。
- **H5 OOB 也纳入 TUF 信任模型**:`serial-selfupdate.sh` 现只验 tarball minisig+sha256，不看 signed manifest/floor → 可被用来装「旧但合法签名的脆弱 CA」。**改**:OOB 默认也读签名 manifest、拒绝 < rollback_floor;只有显式 `--break-glass-allow-downgrade` 才放行并审计 + Telegram 留痕。
- **H6 freeze/单点先做**:timestamp 过期告警 + ≥2 metadata mirror + 持久化「最后一次见到新鲜 timestamp 的时间」，超 SLA 红色告警——不能放「后续」。
- **H7 DVT 门限保护**:apply 前查 DVT 成员健康与门限余量;加**社区级分布式更新锁**，无锁时 Web 只允许单节点手动维护模式。
- **H8 先下载后停机**:下载/验签/解包/预健康全部在服务运行时做，**只在最后 symlink 切换 + restart 前短暂停机**（缩小签名中断窗口）。

**Medium/Low**:深度健康门（§3.2 那套）必须落成默认，不允许 hook 口头替代;审计除本地链式 hash 外关键事件必须远端 append-only 留痕;通知去重 key 用 `version+hash+severity+ta_changed+metadata_version`;生产 fetch 只允许 `https://github.com/AAStarCommunity/...` 或配置 mirror，`file://` 仅测试;**Web 台本身是 Phase 2**，Phase 1 不做（已有 SSH+OOB，Web 只是省敲命令、却加最高权限常驻面）。

> 落地顺序据此重排 → 见 §8（已更新）。**本仓另有一处待修**:板上 userland 无 `jq`（实测 mx93b），而 updater `need jq` 会直接 die —— 真机部署前必须装 jq 或把 updater 改 jq-free。

---

## 1. 目标 / 非目标

**目标**
- GitHub 为唯一信息中心；节点自扫、发现即通知，**人来拍板**。
- 收到通知后，运维有两条低摩擦的「应用」路径:纯命令行(ssh)、可视化(Web)。
- 全链保持 `auto-update-design.md` 的密码学保证:签名 manifest 新鲜度/防回滚 + tarball 验签 + 健康门 + 自动回滚 + boot recovery。
- 通知内容让人能**据以决策**:版本、变动摘要、安全级别(none/low/high/critical)、是否含 TA、是否兼容当前 TA。

**非目标(本稿)**
- 不做「无人值守自动应用」的放量策略(那是 `auto-update-design.md` §7 灰度熔断的活)。本稿默认人来点。
- 不做 TA 的 Web 一键热更(明确排除，见 §6.4)。
- 不替代带外串口救板(`serial-selfupdate.sh`)——那是网络/服务全挂时的 break-glass，两者并存。

---

## 2. 总体架构

```
┌──────────────────────── 信息中心:GitHub ────────────────────────┐
│  Releases: airaccount-node-vX.Y.Z.tar.gz (+ .sha256 + .minisig)  │  ← 二进制托管
│  Repo    : kms/deploy/updater/channels/stable.json (+ .minisig)  │  ← 签名 manifest(信任根/新鲜度/变动/安全级别)
└──────────────────────────────────────────────────────────────────┘
        │ (1) 定时拉取 signed manifest + 验签              ▲
        ▼                                                  │ (发布侧:CI 生成 release + 签 manifest)
┌───────────────────────── 板上:社区节点 ────────────────────────┐
│  airaccount-updater.timer  → aastar-node-updater.sh check       │
│      │  发现新版 & 通过 schema/新鲜度/防回滚                     │
│      ├─(2) 策略=notify-only → 组装通知 → AU_NOTIFY_CMD          │
│      │         └──→ notify-telegram.sh → AAStarMonitorBot ──────┼──→ 运维手机 Telegram
│      │                                                          │        │
│      │                                            (3a) ssh 进板 ◀┼────────┤ 运维决定应用
│      │                                            aastar-node-updater apply <ver>
│      │                                                          │        │
│      └─ Web 管理台(可选,127.0.0.1 + Tailscale)◀───────────────┼──(3b)──┘ 浏览器登录操作
│              admin-web → 复用 aastar-node-updater.sh 同一套应用逻辑
│                         (验签/暂停/换/重启/健康门/回滚 全在服务端)
└──────────────────────────────────────────────────────────────────┘
```

**要点**:通知路径与应用路径**共用同一套已验签的 manifest 与同一套 apply 逻辑**（`aastar-node-updater.sh`）。ssh 和 Web 只是两个「触发器」，密码学与状态机不重复实现——**单一可信实现，双前端**。

---

## 3. 通知层（GitHub → Telegram）

### 3.1 触发与内容

`aastar-node-updater.sh` 的 `decide_action` 已把「有新版但策略不自动应用」归到 `notify`。现状通知文本过简，扩为**决策级信息**。manifest 每个 release 增补(可选)字段：

```jsonc
{
  "version": "0.30.0",
  "security": true,
  "severity": "high",              // none | low | medium | high | critical
  "ta_changed": false,             // 是否含 TA 变更
  "requires_ta_version": "0.29.0", // 需要的最低 TA
  "notes": "portal 三语 + 修 stats UTF-8 panic",   // 一句话变动摘要(人看)
  "notes_url": "https://github.com/.../releases/tag/airaccount-node-v0.30.0",
  "tarball": "https://github.com/.../airaccount-node-v0.30.0.tar.gz",
  "sha256": "…"
}
```

> 注意:`severity/notes/ta_changed` 全部在**签名的 manifest 里**，不是可被人随手改的 GitHub release body（防伪造安全级别，见 `auto-update-design.md` §5 S5）。`notes` 只是展示文案，**不参与任何信任判定**。

通知消息模板(Telegram，MarkdownV2 转义)：

```
🔔 AAStar 节点有更新  (node: mx93b-kms1)
版本  : 0.29.1 → 0.30.0
安全  : 🔴 high（安全补丁）
变动  : portal 三语 + 修 stats UTF-8 panic
TA    : 否（纯 CA，可在线应用）
兼容  : 需 TA ≥ 0.29.0，当前 0.29.0 ✓
应用  : ssh 进板跑  aastar-node-updater apply 0.30.0
        或 Web 管理台 https://mx93b-kms1.<tailnet>.ts.net:8788
详情  : <release notes url>
```

含 TA 变更时，`TA: 是 ⚠️ 走专门流程（不建议在线一键）`，并**不给** Web 一键入口。

### 3.2 通知通道实现

- 复用现成基建:`~/Dev/.env` 的 `AAstarMonitorBot_TOKEN` + `TELEGRAM_CHAT_ID`（memory:node-healthcheck.timer 已在用同一 bot 推巡检告警）。
- 新增 `kms/deploy/updater/notify-telegram.sh`:读 `/etc/airaccount/updater.env` 的 `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID`，`curl` 打 `api.telegram.org/bot<token>/sendMessage`。作为 `AU_NOTIFY_CMD` hook 注入(updater 已支持 hook 注入)。
- **去重/免疲劳**:updater 持久化「已通知过的最高版本」，同版本不重复推(除非 severity 升级)。网络不可达=静默重试(updater 已有 `give_up_quiet`)。
- 邮件通道(后续):同一 `notify` hook 加 `notify-email.sh`(SMTP / SES)，与 Telegram 并列。

---

## 4. 应用路径 A:CLI（ssh）

新增 `apply <ver>` 子命令到 `aastar-node-updater.sh`（当前只有 `check/recovery/status`）。它是「显式、指定版本」的应用，绕过策略门（人已决定），但**不绕过任何密码学门**：

```
aastar-node-updater apply 0.30.0
  1. flock;读 manifest（同 check:拉取 + minisign 验签 + schema + 新鲜度 + 防回滚）
  2. 断言 0.30.0 在 manifest 且 version>current 且 >=rollback_floor 且 min_version 满足
  3. 若 ta_changed=true → **一律拒绝**(决策 D:TA 在线一键=能力级砍掉,无 bypass;走 OOB/专门流程)
  4. 下载 tarball → sha256==manifest → 可选 tarball minisig → tar 加固
  5. 暂停:systemctl stop kms-api（DVT 同理可选）
  6. crash-safe 落盘 releases/0.30.0 + 原子切 current（复用 apply_version）
  7. 重启 + 深度健康门（auto-update-design §3.2）→ 通过则 commit，否则自动回滚 last-good
  8. 结果回推一条 Telegram（成功/回滚）
```

**这是最简、最稳、攻击面最小的路径**——只多了一个 ssh 会话，没有新增常驻网络服务。**推荐作为默认应用方式**；Web 台是「不想敲命令」的人的可选糖。

---

## 5. 应用路径 B:Web 管理台

### 5.1 定位与边界

一个**板上常驻的小管理服务**（`airaccount-admin`），给不想用命令行的运维一个可视化入口。它**不是**面向终端用户的页面，是**运维私有**的。硬边界:

- **监听**:`127.0.0.1:8788` + **Tailscale 接口地址**（100.x）。**绝不** `0.0.0.0`，**绝不**进 cloudflared ingress。默认甚至只 `127.0.0.1`，要 Tailscale 访问需显式开 `ADMIN_BIND_TAILSCALE=1`。
- **TLS**:Tailscale 内已加密；本机 loopback 明文可接受。若开 Tailscale 访问，用 `tailscale cert` 签的证书。
- **它不自己实现更新逻辑**:所有「验签/换/重启/回滚」都 `exec` 同一个 `aastar-node-updater.sh`（或调用其函数），Web 只做「展示 + 发指令 + 轮询状态」。

### 5.2 页面（5 屏）

**① 登录页**
```
┌─────────────────────────────────────┐
│  AAStar 节点管理台   mx93b-kms1       │
│                                       │
│  管理员密码 [ ____________________ ]  │
│            [ 登录 ]                    │
│  · 仅本机/Tailscale 可访问            │
│  · 5 次失败锁定 15 分钟                │
└─────────────────────────────────────┘
```

**② 仪表盘(登录后主页)**
```
┌───────────────────────── 节点状态 ──────────────────────────┐
│ 当前版本 0.29.1   服务 kms-api ● active   TA real   健康 ✓   │
│ 上次检查 2m ago   通道 stable   策略 notify-only            │
├───────────────────────── 可用更新 ──────────────────────────┤
│  ▶ 0.30.0   🔴 high 安全补丁   纯 CA                          │
│    变动: portal 三语 + 修 stats UTF-8 panic                  │
│    兼容: 需 TA≥0.29.0 当前 0.29.0 ✓                          │
│    [ 查看详情 ]            [ 应用此更新 → ]                    │
├───────────────────────── 回滚点 ────────────────────────────┤
│  last-good 0.29.0   [ 回滚到 0.29.0 ]                        │
└──────────────────────────────────────────────────────────────┘
[ 立即检查更新 ]   [ 审计日志 ]   [ 退出 ]
```

**③ 应用确认页(点「应用此更新」)**
```
┌────────────────── 确认应用 0.30.0 ──────────────────┐
│ 将执行:                                             │
│   1. 暂停 kms-api（签名服务将短暂中断 ~10s）        │
│   2. 下载 + 验签(minisign)+ sha256 校验            │
│   3. 原子替换 CA → 重启 → 健康门                    │
│   4. 不健康自动回滚到 0.29.0                        │
│                                                     │
│ ☑ 我已知悉签名服务将短暂中断                        │
│ 输入版本号确认:  [ 0.30.0 ]                         │
│              [ 取消 ]     [ 确认应用 ]              │
└─────────────────────────────────────────────────────┘
```
（含 TA 的更新此页改为红条警告「TA 变更不支持 Web 一键，请走 OOB 流程」，无「确认应用」按钮。）

**④ 实时进度页(SSE 流)**
```
┌──────────── 应用 0.30.0 进行中 ────────────┐
│ ✓ 拉取 manifest + 验签                      │
│ ✓ 下载 tarball  sha256 OK                   │
│ ✓ 暂停 kms-api                              │
│ ✓ 原子切换 current → 0.30.0                 │
│ ⟳ 重启 + 健康门检查中…                       │
│ ─ 日志 ─                                     │
│ [updater] restart: systemctl restart kms-api│
│ [updater] 健康门检查…                        │
└──────────────────────────────────────────────┘
```
终态:✅ 成功(显示新版本 + 健康)/ ↩ 已自动回滚(显示原因 + 回到 0.29.0)。

**⑤ 审计日志页**
```
时间                 事件        操作者    版本         结果
2026-08-02 14:30    apply       admin     →0.30.0     success
2026-08-02 10:12    rollback    system    →0.29.0     auto(health)
2026-08-01 19:00    login       admin     -           ok
```

### 5.3 API（管理台后端 ↔ 前端）

| 方法 | 路径 | 作用 | 认证 |
|---|---|---|---|
| POST | `/api/login` | 密码 → 会话 cookie | 密码 |
| POST | `/api/logout` | 注销 | 会话 |
| GET  | `/api/status` | 当前版本/服务/健康/通道/策略 | 会话 |
| POST | `/api/check` | 立即跑 updater check（拉 manifest 验签） | 会话+CSRF |
| GET  | `/api/candidates` | 可用更新列表（来自已验签 manifest） | 会话 |
| POST | `/api/apply` | body `{version, confirm}` → 触发 apply | 会话+CSRF+二次确认 |
| GET  | `/api/apply/stream` | SSE 实时进度 | 会话 |
| POST | `/api/rollback` | 回滚 last-good | 会话+CSRF+二次确认 |
| GET  | `/api/audit` | 审计日志 | 会话 |

后端**每个写操作都重新做完整验签**（不信任前端传来的任何 hash/URL；version 只作为「在已验签 manifest 里选哪条」的索引）。

### 5.4 实现选型

- **同进程 vs 独立进程**:独立 `airaccount-admin`(**Rust / axum**,与 kms 同栈)。**不塞进 kms-api**——职责分离，管理台崩了不影响签名服务，签名服务崩了管理台还能救。
- **权限(评审 C1 修正:root helper,不是 sudoers 跑脚本)**:管理台**非 root** 跑;不给它 `sudo aastar-node-updater …`(该脚本行为受大量 env 控制且 source updater.env,sudo 跑脚本 = 提权 RCE 面)。改成一个 **root-owned privileged helper**:固定绝对路径、**清空环境**、固定 config、**严格 argv 白名单**(只认 `apply <semver>` / `rollback` / `check` 这几个定长动词,version 先过 `^v?\d+\.\d+\.\d+$` 校验),生产执行路径**禁止任何 hook env 注入**。管理台只能调 helper 的固定动词,拿不到裸 root、也改不了 updater 的 env/config。
- **状态**:复用 updater 的 `state.json`；审计日志 append-only 文件 + 每条带前一条 hash（防篡改，见 §6.5）。

---

## 6. 安全建议（重点）

> 一个能换 TEE CA/TA 的网页 = 拿下它就能给这台 KMS 装任意签名逻辑。安全预算优先砸这里。

### 6.1 网络暴露面
- **默认只 `127.0.0.1`**；Tailscale 访问显式 opt-in，且只绑 tailscale0 地址。
- **硬阻断公网**:启动时校验绑定地址不是公网 IP；文档明确「**绝不**把 8788 放进 cloudflared / frp ingress」。
- Tailscale ACL 收窄到运维自己的设备(tag)。

### 6.2 认证与会话
- 管理员密码 **argon2id** 存 hash（非明文/非弱 hash）；首启向导设，或 `/etc/airaccount/admin.hash`。
- 会话 cookie:`HttpOnly` + `Secure`(Tailscale)+ `SameSite=Strict`；短 TTL(如 30min)+ 空闲超时。
- **失败限速 + 锁定**（5 次/15min）；`fail2ban` 式。
- **二因子(已定):`apply/rollback` 强制**。第一版用 **Telegram 作 OOB 确认信道** —— 发起操作 → 服务端推一次性确认码 + 操作摘要到 AAStarMonitorBot → 运维回码/点按钮才放行;确认码与 `{manifest digest, target version+hash, nonce, 短 TTL}` 绑死(防重放/防 TOCTOU)。后续升级 WebAuthn/FIDO2。

### 6.3 CSRF / 点击劫持 / XSS
- 所有写操作要 CSRF token（double-submit 或 SameSite=Strict + Origin 校验）。
- `Content-Security-Policy` 严格、`X-Frame-Options: DENY`（防 clickjacking 诱导点「应用」）。
- 前端纯静态、无第三方脚本；`notes` 等展示文本**转义**后渲染（防 manifest 里塞 XSS，虽已验签仍纵深防御）。

### 6.4 CA vs TA —— 分级
- **CA(host `kms-api-server`)**:可在线一键(暂停 kms-api → 换 → 重启 → 健康门 → 回滚)。中断 ~10s，可回滚，风险可控。
- **TA(OP-TEE `<uuid>.ta`)**:**不走 Web 一键**。理由:RSA-4096 TA 签名链、secure storage/RPMB 迁移、掉电落在半途不可预测(auto-update-design §3.1)、回滚更难。Web 台对 TA 变更只**通知**并引导到 OOB/专门流程。未来若要支持，必须先落地「TA 版本化 + RPMB 防回滚 floor」(auto-update-design §9 未完项)且额外物理/带外确认。

### 6.5 完整性 / 供应链 / 审计
- **验签永远服务端**;前端零信任。version 是索引，hash/URL 一律取自已验签 manifest。
- **保留 `auto-update-design.md` 全部保证**:manifest 新鲜度(expires 防 freeze)、metadata_version 单调 + rollback_floor(防回滚)、tar 加固、健康门、boot recovery。Web/CLI 都不得绕过。
- **GitHub 单点**:GitHub 挂/被投毒时，签名 manifest + pin 公钥能挡「投毒」(验签不过即拒)，但挡不了「可用性」。缓解:manifest 可多 mirror(auto-update-design §4)；应用前的 tarball 必过 sha256+验签。
- **审计日志防篡改**:append-only + 逐条链式 hash;关键事件(apply/rollback/login/失败)同时推一条 Telegram(异地留痕)。
- **最小提权**:管理台非 root + sudoers 白名单;绝不 `system()` 拼接用户输入(version 先用 `^v?\d+\.\d+\.\d+$` 白名单校验再用)。

### 6.6 可用性 / 故障
- **暂停即中断签名**:apply 期间 kms-api 停 ~10s。若该节点是 DVT 门限的一员(2-of-3)，单节点短暂下线不影响门限——但**避免多节点同时 apply**(通知里提示错峰;未来可加「本社区同时最多 1 台 apply」协调锁)。
- **回滚兜底**:健康门不过自动回 last-good;仍失败 → Telegram 告警 → 转 `serial-selfupdate.sh` 带外救。
- Web 台自身挂掉不影响已跑的 kms-api（独立进程）。

---

## 7. 与带外串口工具的关系

| 场景 | 用什么 |
|---|---|
| 板子在线、有网、想升级 | **CLI `apply`**（默认）或 **Web 台** |
| 想被动收到「有更新」提醒 | **Telegram 通知**（默认 notify-only） |
| 板子 SSH/tailscale 全不通(掉线/网挂) | **`serial-selfupdate.sh` 带外串口**（break-glass） |
| 需要暂停/关机/改网 | `serial-run.py` / `mx93b-serial-poweroff.sh` |

三者共用同一密码学信任根(pin 的 minisign 公钥 + 签名 manifest/release)与同一 release 制品。

---

## 8. 落地顺序（增量，每步可独立测）

1. **通知 MVP**:`notify-telegram.sh` + manifest 增补 `severity/notes/ta_changed` + updater `notify` 文本升级 + 去重。（纯软件，本地 + 板上可测）
2. **CLI `apply <ver>`**:加子命令 + 结果回推;含 TA 变更一律拒绝(决策 D,无 bypass)。（复用 apply_version，风险低）
3. **签名 manifest 上线**:CI 生成 `stable.json` + 签名（`sign-channel.sh`），发到 repo；节点 `updater.env` 配 pin 公钥。
4. **板子版本化目录迁移**:把现在 flat 的 `/opt/airaccount/kms-api-server` 迁到 `releases/<ver>/ + current 软链 + kms-api ExecStart drop-in`（auto-update-design §9 未完项，是 Web/CLI apply 在真机生效的前提）。
5. **Web 管理台**（Phase 2）:独立 `airaccount-admin`(Rust/axum) + 5 屏 + **root helper**(非 sudoers) + Telegram 二次确认 + 启动自检拒公网 + Host/Origin/CORS。CLI 永久保留兜底。
6. **邮件通道 + 多 mirror + WebAuthn 升级 + TA 版本化**（后续）。

---

## 9. 决策（2026-08-02 jason 已拍板）

- **A. Web 台 = Phase 2** ✅。先把「通知 + CLI apply」跑顺(覆盖 ~90% 需求、攻击面最小)。**CLI 模式永久保留，作为 Web 的兜底备份**(Web 崩/被停用时仍能 ssh 应用)。
- **B. Web 语言 = Rust / axum** ✅。kms 本身已用 axom 组件,同栈复用。
- **C. `apply/rollback` 强制二因子** ✅。**第一版用 Telegram 作 OOB 确认信道**:Web/CLI 发起 apply → 服务端推一条带**一次性确认码 + 操作摘要**到 AAStarMonitorBot → 运维在 Telegram 回复该码(或点 inline 按钮)才放行。复用现成 bot、无需先上 WebAuthn/TOTP;后续可平滑升级到 WebAuthn/FIDO2。绑定:确认码与 `{manifest digest, target version, target hash, nonce, 短 TTL}` 绑死,防重放/防 TOCTOU。
- **D. TA 在线一键 = 不做(服务端能力级砍掉)** ✅。updater/admin 对 TA 变更**只检测 + 只通知**;apply 路径拒绝含 TA 变更的 bundle(校验 bundle TA hash==当前,或丢弃 TA 文件)。TA 更新走带外串口 + 物理在场 + 额外确认。理由见 §6.4。
- **E. 通知去重 key = `version + target_hash + severity + ta_changed + metadata_version`** ✅。任一变化(含 severity 升级、TA 标记修正)重推。

---

## 附:本稿如何满足原始需求

| 你的需求 | 对应章节 |
|---|---|
| GitHub 为信息中心、updater 定期扫、发现变动通知 | §2、§3 |
| 通知加 AAStarMonitorBot 到 Telegram（token 在 ~/Dev/.env） | §3.2（复用现成 bot） |
| 未来加邮件 | §3.2、§8.6 |
| 收到通知 → ssh 到板跑 updater 命令 | §4（`apply <ver>`） |
| 管理员密码登录 updater 网址、网页操作 | §5.1、§5.2 |
| 暂停 kms CA 和 TA | §5.2③、§6.4（CA 一键；**TA 只通知**，安全考量） |
| 下载/覆盖/重启 | §4、§5.3（服务端 apply，全程验签） |
| 自动校验签名保障安全 | §4.4、§6.5（服务端 minisign + sha256，前端零信任） |
| 安全建议 | §6 全章 |
