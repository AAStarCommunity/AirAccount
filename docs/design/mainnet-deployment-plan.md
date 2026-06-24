<!-- Created: 2026-06-24 -->
# Mainnet 部署规划（Alpha）— AirAccount KMS

> 关联：#99（生产硬化总纲）· #50（RPMB）· #63（strict，dev 已翻）· `docs/TRUST.md` §6（硬件安全六条）· `RELEASE-CHECKLIST.md`
> 状态：规划。2026-07 两块 MX93 到货后执行。

---

## 0. 一句话：mainnet ≠ "换块板子"

主网部署是**一套不同的 build profile + 一组不可逆的硬件熔丝操作 + 全新的域名/密钥/measurement + 真实资金等级的运维**。换板子只是最表层的一步。下面把"还有哪些变化"逐条列清。

---

## 1. 两板拓扑（2026-07）

| | **Board-A（现有板）** | **Board-B（新板）** |
|---|---|---|
| 角色 | 测试 / Beta | 主网 / **Alpha** |
| 链 | Sepolia（testnet）| Ethereum mainnet |
| build profile | `dev-rpid` + `strict-challenge`（当前配置，保持）| **prod**（无 dev-rpid）+ `strict-challenge` + RPMB + secure boot |
| `/version` | `profile=dev, challenge_mode=strict` | `profile=prod, challenge_mode=strict` |
| rpId | aastar.io + localhost | aastar.io（仅，无 localhost）|
| 上线时间 | 持续（保持当下）| 内部测试 **1–2 周 ~ 1 月**后部署 |

Board-A = **现在这块板**，配置不动，继续当 Sepolia Beta 测试环境。Board-B = 新板，按下面的生产配置 provisioning。

---

## 2. mainnet 相对 testnet 的全部变化（清单）

| # | 维度 | testnet（Board-A） | mainnet（Board-B） | 不可逆? |
|---|---|---|---|---|
| A | **build profile** | dev-rpid + strict | **prod（无 dev-rpid）+ strict** | 否（重编） |
| B | **RPMB 防回滚（#50）** | `ree-fs-only`（RPMB key 未烧）| **烧 RPMB key → 关 ree-fs-only → 硬件 anti-rollback** | ⚠️**是** |
| C | **Secure Boot（AHAB/ELE）** | lifecycle 开放（未强制）| **烧 SRK + 闭锁 lifecycle**（评估后决策）| ⚠️**是** |
| D | **TA 签名密钥** | NXP **公开** dev key（`mx93_ta_sign_lf6.18.pem`）| **自己的私有签名 key** + SRK 对应（secure boot 下用公开 key = 零安全，谁都能签 TA）| key 一次定 |
| E | **HUK** | 确认是真设备熔丝（非默认值）| 同左，确认存储加密真设备绑定 | — |
| F | **ELE TRNG** | U-Boot SPL 启动（崩板根因，已知）| 同左，prod 必须正确 | — |
| G | **域名 / passkey rpId** | aastar.io（+localhost）| **见 §4 决策**（passkey 绑域名，跨域名不通用）| 影响 social recovery |
| H | **API keys** | dev key（kms_7e05…）| **全新 prod keys**（dev key 不上 prod）| — |
| I | **measurement / 透明日志** | dev TA measurement（不进 prod 日志）| **prod TA measurement 发布到 Sigsum 透明日志**作权威值 | — |
| J | **合约地址** | Sepolia SessionKeyValidator / SuperPaymaster | **mainnet 合约地址**（grant verifyingContract 等）| — |
| K | **备份 / DR** | dev（可丢）| **prod kms.db + TEE secure storage 备份策略；丢板=丢钥**（除非备份 / social recovery）| — |
| L | **监控 / 运维** | 基础 | **uptime + transparency monitor（B-4）+ 日志 + 告警** | — |

> KMS 本身对链是中立的（它签 hash/tx，chainId/合约由请求带）。所以"mainnet"在 KMS 侧主要体现为：客户端发 mainnet chainId/合约 + grant 的 verifyingContract 换 mainnet 地址 + **真实资金 = 任何 bug 代价陡增**。

---

## 3. 主网板 provisioning 步骤（顺序，#99 一趟做）

1. **硬件到货 + 基础刷机**：uuu 刷镜像，确认 **U-Boot SPL 启动 ELE TRNG**（崩板根因）。
2. **生成 prod TA 签名密钥**（RSA-4096 私钥，安全保管，**绝不进仓库**）+ 对应 SRK hash。
3. **烧 SRK + 闭锁 secure boot lifecycle**（评估后）。⚠️**不可逆**。
4. **烧 RPMB 认证 key** → build 去掉 `ree-fs-only` → 硬件 anti-rollback 生效。⚠️**不可逆**。确认 HUK 真熔丝。
5. **构建 prod TA+CA**：`MX93_STRICT_CHALLENGE=1 ./scripts/mx93-build.sh all`（**不带** `MX93_DEV_RPID`）+ 用 prod 签名 key + 去 ree-fs-only。→ profile=prod, challenge_mode=strict。
6. **部署** + 配 **prod API keys** + prod RP config（`KMS_RP_ID=aastar.io` 仅，无 localhost）。
7. **重算 + 发布 prod TA measurement 到透明日志**（Sigsum），作主网权威 measurement。
8. **E2E**：mainnet chainId/合约的签名 + mint + 非空 grant + **RPMB 回滚测试拒** + **strict 拒 legacy/裸 nonce** + 不回归。
9. **Cloudflare tunnel → prod URL**（见 §4）。
10. **监控上线**（uptime + transparency monitor + 告警）。

### 3.1 两把 key 澄清（别混）

provisioning 涉及**两把不同的 key**，作用完全不同：

| key | 是什么 / 从哪来 | 管什么 | 为什么重要 |
|---|---|---|---|
| **TA 签名密钥**（RSA-4096 私钥）| **你自己生成**（`openssl`），保密、绝不进仓库。其公钥哈希 = **SRK** 烧进芯片 | secure boot 闭锁后，**只有用这把私钥签的 TA 能加载** | **这是"能否重刷生产板"的唯一入口**：没这把 key 签的 TA，板子拒绝加载 → 刷不进。丢=不能再更新此板；泄漏=攻击者能签恶意 TA |
| **RPMB key** | eMMC RPMB 分区的认证 key，一次性烧 | **存储防回滚**（防 secure storage 回退旧态）| 跟"能不能刷 TA"无关，是另一条线 |

> dev 板现在用的是 NXP **公开** dev key（仓库 `keys/mx93_ta_sign_lf6.18.pem`，谁都有）→ 能随便刷，无安全意义。生产板必须换**自己的私钥** + 烧对应 SRK，否则 secure boot 形同虚设。**重刷窗口（#99）= 你持签名 key + 排期 + 备份 + 授权**才动生产板。

---

## 4. 域名 / passkey rpId 策略（已定）

**决策（2026-06-24）**：
- **主网板（Board-B）= `kms.aastar.io`**，承接权威身份 + **rpId = `aastar.io`**（现有 121+ 用户的 passkey 直接归主网板）。
- **测试板（Board-A）域名改为 `kms1.aastar.io`**，让出 `kms.aastar.io` 给主网板。

实施要点：
- **⚠️ 先确认旧 `kms1.aastar.io` 实例可下线**：部署 memory 记 `kms1.aastar.io` 现指向一个独立旧实例（v0.16.8，不同 tunnel/DB）。把测试板重指到 kms1 前，先确认旧实例退役/可覆盖，避免冲突。
- **测试板 rpId 建议设为 `kms1.aastar.io`**（而非 aastar.io）→ 测试 passkey 与生产**隔离**，测试板碰不到生产用户的凭证语义。代价：测试要重新注册 passkey（测试环境本就该这样）。若图省事用 aastar.io rpId 则两板 passkey 通用——**不建议**（真资金板不该和测试板共享凭证锚点）。
- **Cloudflare tunnel** 重新映射：`kms.aastar.io` → 主网板，`kms1.aastar.io` → 测试板。**注意 origin 白名单 ≠ rpId**（部署 memory：origin 精确匹配、子域靠 `*.aastar.io` 通配；rpId 后缀匹配）。

> 依据 `project_decentralization_model`：换实例 = 换 rpId = 换 passkey = social recovery 边界。主网板拿 aastar.io = 现有用户无缝；测试板独立 rpId = 干净隔离。

---

## 5. 对"我"（执行部署的 assistant）有什么变化

不是"换个 IP 重刷"那么简单，操作纪律升级：

1. **两块板、两个 IP** → 每次部署前**必查 `/version` 的 `profile`+`challenge_mode` 确认是哪块板**。`profile=prod` = 生产板，要格外谨慎。
2. **绝不把 dev-rpid build 刷到生产板**（RELEASE-CHECKLIST §0.5 + build 时 `⚠️ DEV-RPID` 警告 + /version profile 三重防。dev-rpid 接受 localhost = 安全面扩大）。
3. **生产板 secure-boot-lock 后，重刷受限**：只有用**真签名 key** 签的 TA 能加载。没有签名 key 就刷不了 → 这是 #99「重刷窗口」的本质：**生产板不能随便刷**，重刷要排窗口 + 备份 + 授权。
4. **不可逆操作（RPMB key / SRK / secure boot 闭锁）我不自主执行** —— 必须**你授权 + 在场**，因为烧错 = 砖板/不可恢复。这些是单独决策、双确认。
5. **真实资金等级的核实纪律**（吸取本月教训）：每步 git/部署/版本都**核实落地**（`git log`/`ls-remote`/`/version`），杜绝 detached-HEAD/未核实推送那类事；改 TA payload/host delegate 走**一致性门**（build/CI 已强制）。
6. **备份比 dev 严格**：生产板任何部署/重刷前必备份（kms.db + 关键状态到 Mac/异地），prod kms.db 不能只在板上。
7. **不混淆 measurement**：dev 板的 measurement 绝不进生产透明日志（§2-I）。

---

## 6. 时间线

```
2026-07  两块 MX93 到货
         ├─ Board-A：保持当前配置（dev-rpid+strict，Sepolia Beta）—— 不动
         └─ Board-B：按 §3 provisioning（prod profile + RPMB + secure boot）
            └─ 内部测试 1–2 周 ~ 1 月（mainnet 合约 E2E + 回滚/strict 验收）
               └─ 通过 → mainnet Alpha 上线
```

---

## 7. 验收（mainnet Alpha 上线门槛）

- [ ] `/version` = `profile=prod, challenge_mode=strict`
- [ ] RPMB 回滚测试：回退存储被拒
- [ ] strict：legacy/裸 nonce assertion 被拒（#68 WYSIWYS）
- [ ] mainnet chainId/合约的签名 + mint + 非空 grant E2E 全过、不回归
- [ ] prod TA measurement 在 Sigsum 透明日志（可外部验）
- [ ] prod API key 生效、dev key 不在板上
- [ ] 域名/rpId 按 §4 决策落实，passkey 流程验通
- [ ] 备份 + 监控 + 告警上线
- [ ] secure boot（若启用）：未签名/篡改 TA 不加载

---

## 8. ⚠️ 不可逆操作清单（需授权 + 在场 + 双确认 + 先备份）

| 操作 | 后果 | 错误代价 |
|---|---|---|
| 烧 RPMB 认证 key | 启用硬件 anti-rollback | 烧错/丢 key = 存储不可访问 |
| 烧 SRK | 锚定 TA/boot 签名信任根 | 错 = 只能用错 key 签的镜像 |
| 闭锁 secure boot lifecycle | 强制只跑签名镜像 | **砖板**（若签名链不对）|

这三项是 #99 的核心、也是 mainnet 与 testnet 的根本分界。**做之前：硬件定版 + 签名密钥就绪 + 完整备份 + 你授权 + 双人确认。**
