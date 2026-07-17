<!-- Created: 2026-07-17 -->
# 模型 B — AAstar 运营方「可选代付注册服务」详细设计

> **派发对象**：`repo:dvt`（YetAnotherAA-Validator / AAstar 运营方节点工具链）
> **发起**：`repo:airaccount`（KMS/社区节点向导）
> **性质**：**可选加载模块**。这是 **AAstar 运营方**跑的服务，**社区板子不安装**；未部署时社区回落模型 A（自备/预充值 operator）。
> **状态**：需求 + 规格 + 接口设计（本文）→ 请 DVT 仓评估并实现该可选模块。

---

## 1. 背景与三模型

社区节点一键链上注册（AirAccount task #19）。合约 `AAStarBLSAlgorithm.registerWithProof` 要求 operator **先质押 30 GToken**（`requireStake=true`），**gasless 只能代付 gas、代付不了质押**。三种"谁出钱"模型：

| 模型 | 谁出 ETH+GToken | 需要后端 | 归属 |
|---|---|---|---|
| **A 预充值**（AirAccount 正在做） | AAstar 预刷板时 bake 预充值 operator key | 否 | AirAccount 板侧 CLI |
| **B 代付服务**（本文） | AAstar `funderWallet` 按需转给板子的 operator | **是（本模块）** | **DVT 仓（运营方服务）** |
| C SP gasless（最终形态） | SP paymaster + sponsored-stake/信用 | 是 | SP + SDK |

**模型 B 的价值**：社区**零持仓**——不用预充值、也不用自己买 GToken/ETH，板子上电后向 AAstar 服务申请，AAstar 出资，板子自注册。

---

## 2. 设计要点（关键：msg.sender 必须是社区 operator）

`registerWithProof` 断言 `nodeOperator == msg.sender`。若 AAstar 服务**代替**签名，链上 operator 就成了 AAstar，社区节点身份不独立。所以：

> **AAstar 只出资（转账），不代签。板子的 operator EOA 自己签 `registerRole` + `registerWithProof`。**

这把模块简化成一个**受控出资水龙头（funding faucet）**：SDK `onboardDvtNode` 的 `funderWallet` 语义正是"给 operator 补 ETH+GToken"，本模块把它**服务化**。板侧注册逻辑**复用模型 A 的 CLI**（`register-node.mjs`），只是资金来源从"预充值"换成"B 服务按需补"。

---

## 3. 需求（给 DVT 仓）

### 3.1 功能需求
1. 提供一个**可选启用**的 HTTP 服务模块（AAstar 运营方部署，默认不启）：`POST /operator/fund-node`。
2. 校验请求合法性（见 §5 鉴权），通过后用 **AAstar `funderWallet`**（持 ETH + GToken）：
   - 若板子 operator 的 ETH < 阈值 → 转 gas（默认 0.03 ETH）。
   - 若 operator 的 GToken < `minStake + ticketPrice + headroom` → 转足额 GToken。
   - **仅转账，不调 registerRole/registerWithProof**（那两步板子自己做）。
3. 返回出资 tx 哈希 + 板子应达到的目标余额，供板侧确认后继续注册。
4. **幂等**：同一 operator 已被资助且余额足够 → 短路返回，不重复转账。
5. **限额/风控**：单 operator 单次上限、总额度、频率限制（防滥用把 funder 掏空）。

### 3.2 非功能
- **可选/可卸载**：模块不影响 DVT 节点核心；未加载时零副作用。
- **funderWallet 私钥**：只在 AAstar 服务侧，走 KMS/HSM/env-tmpfs，**绝不下发到社区板**。
- **审计**：每次出资记日志（operator、金额、tx、请求来源）。

---

## 4. 接口规格（建议）

```
POST /operator/fund-node
Authorization: <见 §5>
{
  "network": "sepolia",
  "operator": "0x…",                 // 板子的 operator EOA(板侧生成,私钥在板内)
  "blsPubkey": "0x…(128B EIP-2537)", // 节点身份,用于风控/去重
  "pop": { "popPoint":"0x…", "popSig":"0x…" },  // KMS /pop 产出,证明持有该 BLS key(防拿别人 pubkey 冒领资助)
  "nodeMeta": { "rpId":"kms.community.org", "community":"…" }
}
→ 200
{
  "funded": true,
  "operator": "0x…",
  "hashes": { "fundEth":"0x…", "fundGToken":"0x…" },  // 未补的项省略
  "targetBalances": { "eth":"0.03", "gToken":"32" },
  "next": "板子现在可自行 registerRole + registerWithProof(用模型A CLI)"
}
```

**出资金额来源**：读链上 `minStake()` + ROLE_DVT `ticketPrice` + headroom（与 `onboardDvtNode` 同口径），不写死。
**地址**：Sepolia 用实链值（validator `0x539B96…`、gToken `0x4c09aE57…`、staking `0x472297B5…`）——⚠️**别用 canonical addresses.ts 的漂移地址**（gToken 0x8d6Fe002 / validator 0x0）。

---

## 5. 鉴权（防白嫖 funder）

出资前必须确认"请求者是 AAstar 认可的真实节点"。建议二选一或叠加：
- **PoP 校验**：服务侧用 `blsPubkey` 验 `pop`（proof-of-possession），证明请求方确实持有该 BLS 私钥（板子/TEE 内）。防止拿他人 pubkey 冒领。
- **准入名单/邀请码**：AAstar 给认可社区发一次性 onboarding token，请求带上。
- （可选）与 SuperPaymaster 角色/信用系统联动做额度控制。

---

## 6. 与板侧（AirAccount 模型 A CLI）的衔接

```
板子(社区)                              AAstar B 服务(DVT 仓,可选)
  ├─ KMS /gen-key → BLS key(TEE)
  ├─ KMS /pop     → pop(popPoint,popSig)
  ├─ 生成 operator EOA(板内 600)
  ├─ POST /operator/fund-node ──────────▶ 验 pop/token → funderWallet 转 ETH+GToken → tx
  │   ◀───────────────────────────────── 200 {hashes, targetBalances}
  ├─ 轮询确认 operator 余额到账
  └─ 复用模型A CLI: registerRole + registerWithProof(operator 自签) → 节点上链
```

板侧 CLI（`register-node.mjs`，AirAccount 出）已支持 `popSigner`→KMS `/pop` 与显式地址；模型 B 只在它前面插一步"调 B 服务补钱"。**板侧代码 A/B 共用，差异只在资金来源。**

---

## 7. 请 DVT 仓确认/决策
1. 该可选模块放哪：DVT operator 工具链内新 package / 独立 service？
2. funderWallet 密钥托管方式（KMS-TEE？HSM？）。
3. 鉴权取 PoP 校验 / 准入 token / 两者叠加？
4. 风控参数（单次上限、总额度、频率）默认值。
5. 是否复用 SP v5 角色/信用系统做额度（与模型 C 有交集）。

> 交付物：DVT 仓提供该可选服务 + 接口文档；AirAccount 侧板 CLI 增加"调 B 服务"分支并联调。
