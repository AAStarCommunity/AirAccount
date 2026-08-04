<!-- Created: 2026-07-18 -->
# 社区节点初始化配置设计：哪些 key / 在哪生成 / 怎么进镜像 / 刷板

> 目标（jason 提）：把「节点初始化需要哪些 key、怎么**在用户本地**帮他生成、怎么加进部署镜像、再刷到板子」讲清楚。
> 本文 = 该问题的**独立分析 + 推荐方案**。关联：`community-node-image-ci-design.md`(#21)、`phase2-image.md`、
> `kms-dvt-production-init.md`、`setup-server.py`(web 向导)、`community-profiles/`(4 种情况)。

---

## TL;DR（我的建议，先说结论）

> **「本地生成 key 注入镜像」这个直觉对一半：config 可以本地准备+注入（推荐），但私钥不能。**
> 私钥必须**首启在板/TEE 内生成**，否则同时破坏两件事：① TEE 保护模型（KMS 私钥物理上不可导出），
> ② 共享金镜像的安全（portal 上是公开下载，任何烤进金镜像的私钥 = 所有下载者拿到同一把）。
>
> **正确形态 = 本地工具准备 config（无私钥）→ 注入镜像的 config 区 → 刷板 → 私钥全部首启在板生成。**

---

## 1. 初始化需要的 key / config 全清单

| # | 项 | 类型 | 秘密? | 哪些情况 | 本地能生成? | 能烤进共享金镜像? | **推荐生成位置** |
|---|---|---|---|---|:--:|:--:|---|
| 1 | rpId / 社区域名 | config | 否(公开) | 全部 | ✅ | ✅ | **本地准备→注入 config** |
| 2 | 链 RPC URL | config | ⚠️含 API key | 全部 | ✅ | ❌(含key) | 本地准备→注入(每社区自己的) |
| 3 | 合约地址(validator/gToken…) | config | 否 | 2/3/4 | ✅ | ✅ | 烤进镜像(公开定值) |
| 4 | **KMS BLS 私钥** | 密钥 | **是** | 1/3 | ❌**不可能** | ❌**绝不** | **首启·板 TEE 内生成** |
| 5 | **KMS keeper operator EOA** | 密钥 | **是** | 3(keeper 走 KMS) | ❌ | ❌ | 首启·板 TEE 内(`/kms/gen-keeper-eoa`) |
| 6 | **DVT BLS 私钥(独立 keystore)** | 密钥 | **是** | 2/4 | ✅技术可 | ⚠️危险 | **首启·板上生成 EIP-2335** |
| 7 | **operator EOA(独立/模型A)** | 密钥 | **是** | 2/4(+模型A) | ✅ | ⚠️危险 | 首启·板上;模型A=AAstar 预充值 |
| 8 | keystore 密码 | 密钥 | 是(用户定) | 2/4 | 用户输入 | ❌绝不(走 tmpfs) | 首启·用户当场输入 |
| 9 | KMS API key | 密钥 | 是 | 1/3(有 KMS) | ✅ | ❌ | 首启·板上生成 |
| 10 | signer token(KMS↔DVT 共享) | 密钥 | 是 | 3(co-custody) | — | ❌ | 首启·板上生成 |
| 11 | 隧道 token(cloudflared/frp) | 密钥 | 是 | 全部 | AAstar 发/社区备 | ❌ | 本地准备→注入(每社区自己的) |

**看这张表就懂**：只有 **1、3（公开 config）**能安全烤进**共享金镜像**；**2、11**是每社区自己的 config（可本地准备注入，但不能进公开金镜像）；**4–10 全是私钥/密码，一律首启生成，绝不烤**。

---

## 2. 为什么不能简单「本地生成全烤进去」——三条硬约束

**A. TEE 私钥物理不可导出。** KMS 的 BLS/keeper 私钥在 TrustZone 内生成、用 SoC 硬件唯一密钥(HUK)密封。
本地生成 = 私钥在普通电脑明文存在过 = **破坏整个 TEE 卖点**；且技术上你也**导不出** TA 里的密钥去"烤进镜像"。

**B. 共享金镜像是公开、多人下载的。** portal 上一个镜像给 N 个社区下载。**任何**烤进金镜像的私钥
= N 个社区拿到**同一把私钥** = 灾难（互相能签、能盗）。金镜像**只能含公开 config**。

**C. 私钥碰用户笔记本 = 扩大攻击面。** 即便"每社区本地生成"（不是共享），私钥也在用户的普通电脑上落过盘，
比"永不出板/出 TEE"弱一档。能首启在板生成的，就别在本地生成。

---

## 3. 两个模型对比

| | 模型 I · 首启在板生成（现向导/selfinit） | 模型 II · 本地生成+注入（你提的直觉） |
|---|---|---|
| 私钥在哪生成 | 板 TEE / 板上 keystore | 用户电脑 |
| 金镜像含密钥? | 否(零密钥) | 若烤进=泄密;只能每社区自己的镜像副本 |
| 适用 | **全部私钥** | 仅 config + 非 TEE 密钥(且不进共享镜像) |
| TEE 卖点 | ✅保持 | ❌KMS 私钥无法这样做 |
| 安全 | 最强 | 弱一档(私钥落用户电脑) |

---

## 4. 推荐方案：本地个性化工具（只碰 config）+ 私钥首启在板

```
┌─ 共享金镜像(portal 下载,零密钥) ─────────────────────────┐
│  OS + OP-TEE + airaccount-node 组件 + 首启 provisioning 武装 │
│  + 一个空的 config 分区(FAT,cloud-init 风格)               │
└───────────────────────────────────────────────────────────┘
                    │ 社区在自己电脑跑本地工具
                    ▼
   aastar-flash.sh(本地个性化,只生成 config 不生成私钥):
     输入: 社区域名(rpId) · 链 RPC · 选情况(1-4) · (可选)隧道 token
     生成: community.toml / 首启参数(rpId、公开地址、选的 profile) —— 全是 config,零私钥
     动作: 把 config 写进金镜像副本的 config 分区 → 刷到板(balenaEtcher/uuu)
                    │
                    ▼
   板首启(读注入的 config):
     · KMS BLS/keeper → TEE 内生成密封(永不出板)
     · 独立 DVT → 板上生成 EIP-2335 keystore(用户当场输密码,走 tmpfs)
     · API key / signer token → 板上生成
     · web 向导只补「域名/密码/operator」等交互项(已注入的免问)
```

**这样同时满足你的诉求和安全**：用户**在本地准备了 config 并注入镜像**（✅ 你要的流程），但**私钥永远首启在板/TEE 生成**（✅ 不破坏 TEE、金镜像零泄密）。
**例外**：模型 A 预充值 operator——那把 operator 私钥由 **AAstar 侧**生成+充值+预置（不是社区本地），见 `community-node-register-modelB-funding-service.md` / 模型 A。

---

## 5. 四种情况的 key 矩阵（首启各生成什么）

| 情况 | 首启在板/TEE 生成 | 本地注入的 config | 用户当场输入 |
|---|---|---|---|
| 1 独立 KMS | KMS BLS(TEE) · API key | rpId · RPC | — |
| 2 独立 DVT | DVT BLS(keystore) · operator EOA | rpId · RPC · validator | keystore 密码 |
| 3 联合·KMS 托管 | KMS BLS(TEE) · keeper EOA(TEE) · signer token · API key | rpId · RPC · validator | — |
| 4 联合·各自独立 | KMS BLS(TEE) · DVT BLS(keystore) · operator EOA · API key | rpId · RPC · validator | keystore 密码 |

---

## 6. 注入镜像的技术手段（config 区）

- **cloud-init 风格 config 分区**（推荐）：金镜像留一个小 FAT 分区，本地工具写 `community.toml` + `first-boot.env`；
  板首启脚本挂它、读 config、跑 provisioning。跨刷机方式(balenaEtcher/uuu)通用。
- 或 **installer 传参**（phase2 路径 A）：刷 NXP 基础镜像后，`aastar-node-installer.sh --config community.toml` 注入。
- 两者都**只注入 config，不注入私钥**。

---

## 7. 待办 / 落地
- [ ] `aastar-flash.sh` 本地个性化工具（生成 config + 注入镜像 config 区 + 调 balenaEtcher/uuu）。
- [ ] 金镜像留 config 分区 + 首启脚本读它（或 installer --config 传参）。
- [ ] 4 种情况的首启 provisioning 各自脚本（复用 selfinit + 向导，按 profile 分支）。
- [ ] 与 #21 镜像流水线合流：金镜像零密钥 + config 区。

> **一句话给 jason**：你的流程对，只要把「本地生成的」限定成 **config（无私钥）**，私钥留给首启在板/TEE。
> 这样既是"本地准备+注入镜像+刷板"，又不砸 TEE 卖点、不让公开金镜像泄私钥。
