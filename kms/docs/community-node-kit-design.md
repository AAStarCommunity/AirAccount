<!-- Created: 2026-07-06 -->
# AAStar Community Node Kit — 傻瓜式 mx93 部署方案(设计稿)

> 问题:社区能买到 mx93 板,但**不会编译/部署**。DVT 门限需 **≥3 节点**(AAstar 1 个 +
> ≥2 个社区各 1 个)。要让非技术社区"买到板 → 填点配置 → 自动跑起 KMS+DVT"。
> 目标:**至少 2 个社区**近期用此方案上线。

## 核心洞察

**编译是唯一技术壁垒**(交叉编译 TA+CA、签 TA、build DVT)。→ **AAstar 编一次、发产物;社区只做配置 + provision 秘密**。社区侧**不需要 Docker、不需要编译、不需要命令行**。

## 四层架构

### Layer 1 · AAstar 预构建 & 发布(一次性,CI 做)
每个版本 CI 产出并发到 GitHub Releases:
- **KMS**:预签名 TA(RSA-4096)+ CA 二进制(aarch64)+ CLI(api-key/kms-admin)
- **DVT**:自包含 bare-node bundle(dist + node_modules + **内置 node**,arm64)→ 社区侧**零 npm build**
- **installer**:单个 `aastar-node-setup`(交互式)
- (可选)**整盘 eMMC/SD 镜像**:OP-TEE + KMS + DVT 全烤进去

### Layer 2 · 社区拿到可用板(三档傻瓜度)
| 档 | 方式 | 傻瓜度 |
|---|---|---|
| **A 预刷板(最傻瓜,推荐)** | AAstar 刷好板子直接寄。社区通电 → 联网 → 开浏览器填表 | ★★★ 零刷机 |
| **B 刷提供的镜像** | AAstar 发 eMMC 镜像,社区用 **balenaEtcher**(GUI,跨平台,拖拽即刷)| ★★ 有 GUI |
| **C 网络 provision** | 板子已带基础 OP-TEE 镜像 → installer 下载 AAstar 预构建产物 + 配置 | ★ 需一条命令 |

> ⚠️ `uuu` over USB 不傻瓜(CLI + 时序坑)。**首选 A(预刷板)或 B(balenaEtcher)**。

### Layer 3 · 交互式 setup 向导("填点配置"那步)
首次开机 → 板子跑向导,两种形态:
- **Web 向导(推荐)**:板子在 LAN/Tailscale IP 上开一个 setup 页,社区**开浏览器填表**提交。
- **TUI 向导**:`aastar-node-setup` SSH/串口交互提示。

社区**只需提供这几项**(其余全自动):
1. 社区名 + **域名**(= KMS passkey rpId 锚点)
2. 链 RPC(或用 AAstar 共享默认)
3. **keystore 密码**(现场输两遍 → BLS keystore + tmpfs,永不落盘)
4. Cloudflare tunnel(AAstar 可代发子域名 / 或社区自己的)
5. operator 钱包(注册用;或 **AAstar 用 SuperPaymaster gasless 代付注册** → 社区连 ETH 都不用有)

向导自动做:
- 生成 `community.toml`
- 装预构建 KMS(TA+CA)+ provision API key
- 部署 DVT bundle + **板上生成独立 BLS 密钥 → 立即 EIP-2335 加密**(pbkdf2)
- **链上注册:调 SDK v0.38.0 `dvtOperatorActions.registerWithProof`(PoP)** → 加入 ≥3 节点门限
- health 检查 → 显示「你的节点已上线」+ 公网 URL + BLS 公钥

### Layer 4 · ≥3 节点门限 & 发现
- **链上注册 = 发现**(不需 p2p/DHT):每个社区节点 `registerWithProof` 注册到 validator 合约,合约即 registry。
- 客户端 SDK 从 ≥3 个已注册节点收集门限共签。
- 新社区上线 = 其节点注册 → 门限池扩大。AAstar 维护已知节点清单。

## 关键:注册用 SDK v0.38.0(已链上验证)
SDK `buildDvtPop` + `dvtOperatorActions.registerWithProof`(#288/#289,tx `0x216a7ed5…` LIVE PASS)=
**PoP 证明 operator 持有该 BLS 私钥**(防注册他人的键)。向导直接调它,不手撸合约 —— 干净、已测。

## 安全(傻瓜流程里不打折)
- keystore 密码现场输、只进 tmpfs、永不落盘(向导也守此规矩)。
- BLS 私钥**板上生成、永不出板**、即刻加密。
- 注册 gas:社区自付 **或 AAstar SuperPaymaster 代付**(社区无需持 ETH → 更傻瓜)。
- rpId 绑社区自己域名(半去中心化,各社区独立身份)。

## AAstar 维护(一次性 + 版本迭代)
- **CI**:交叉编译 KMS(复用 mx93-build.sh)+ 打 DVT bundle(复用 build-bare-node.sh)+ 组镜像/installer → 发 Release。
- **版本更新**:社区跑 `aastar-node-update` → 下新产物 → 重部署(**keystore 身份保留**,复用 deploy-dvt.sh 的 `--exclude`)。

## 已定案(2026-07-07)
1. **傻瓜度**:MVP = **预刷板 + SSH 跑一次 TUI 向导**。服务 2 个起步社区足够。
2. **交付**:AAStar 预刷板 → **邮寄**。
3. **注册 gas**:**接 SuperPaymaster 代付**(gasless `registerWithProof`,社区无需持 ETH)。
4. **域名(关键区分)**:AAStar 发子域名 `<community>.aastar.io` —— **仅托管 web 向导/UI/仪表盘**;
   社区**必须设自己的 rpId 域名**(passkey 锚点)。**子域名 ≠ rpId,两者无关** —— 保持各社区身份独立(半去中心化)。

## MVP · `aastar-node-setup`(TUI 向导,板上 SSH 跑一次)

预刷板已烤入:KMS(RSA-4096 签名 TA + CA)+ DVT bare-node bundle(含 node)+ aastar-sdk + installer。
**社区拿到板 → 联网(Tailscale/WiFi)→ SSH 跑一次:**

```
$ aastar-node-setup
=== AAStar Community Node Setup（一次性）===
[1/6] 社区名: mycommunity
[2/6] 你自己的 rpId 域名（passkey 锚点，必须是你自己的域名）: kms.mycommunity.org
      ↳ AAStar 另给你 web 子域名 mycommunity.aastar.io（仅 UI 用），与此 rpId 无关
[3/6] 链 RPC [回车用 AAStar 共享默认]: …
[4/6] BLS keystore 密码（输两遍，永不落盘）: ******  确认: ******
[5/6] Cloudflare tunnel: [a] AAStar 代发  [b] 自己 token  → a
[6/6] 注册 gas: [a] SuperPaymaster 代付（免费/推荐）  [b] 自己 operator key  → a

Setting up…
 ✓ community.toml 生成（rpId=你的域名；web=子域名）
 ✓ KMS: 装 TA + 起 CA + provision API key → healthy
 ✓ DVT: 部署 bundle + 板上生成独立 BLS 密钥 + EIP-2335 加密(pbkdf2) + tmpfs 密码
 ✓ 链上注册: SDK registerWithProof 经 SuperPaymaster gasless → tx 0x… ✓ 加入门限池
🎉 节点已上线  KMS: https://kms.mycommunity.org  |  Web: https://mycommunity.aastar.io
⚠️ 重启后重输密码: 跑 `aastar-node-unlock`
```

向导 = 现有 `deploy-dvt.sh` + KMS 部署 + SDK `dvtOperatorActions.registerWithProof`(经 SP paymaster)的**交互封装**。
输入仅 6 项,其余全自动。密码守 tmpfs 规矩。

## Web UI 向导(未来,同一后端)

同样 6 项输入,改成**板子在 LAN/Tailscale IP 开一个 setup 页**,社区**开浏览器填表**,免 SSH:
- **前端**:单页表单(6 项)+ 实时进度(SSE/WS 显示"装 KMS… 部署 DVT… 注册中…")+ 完成后**状态仪表盘**(节点健康 / BLS 公钥 / 门限池成员 / 签名统计)。
- **托管**:跑在 `<community>.aastar.io` 子域名(AAStar tunnel)—— 这正是子域名的用途。
- **后端**:同一套 installer 动作(TUI 与 Web 共用);密码经 https 表单 → 直接进 tmpfs,不落盘。
- **安全**:setup 页仅首次未初始化时可访问(初始化后关闭 / 转仪表盘);密码字段 type=password + 不回显不记录。
- **复用**:与 TUI 向导同一 `node-provision` 核心(TUI=终端前端,Web=浏览器前端),避免两套逻辑。

## 落地顺序
1. **MVP**:`node-provision` 核心 + TUI 前端 → 预刷 2 块板 → 邮寄 → 社区 SSH 跑。
2. **接 SP**:registerWithProof 走 SuperPaymaster paymaster(gasless)。
3. **Web UI**:同后端加浏览器前端 + 仪表盘 + balenaEtcher 镜像(免 SSH/预刷)。
