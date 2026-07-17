# 独立 DVT 节点上链注册 Runbook（以 dvt3 / B 板为例）

> 记录 2026-07-13 把 **dvt3（MX93 B 板，独立本地加密 keystore 节点，非 TEE 托管）** 从加电解锁到
> `registerWithProof` 链上注册闭环的**完整流程 + 踩的 4 个坑 + 解法**。
>
> 对照：dvt1（A 板）是 **KMS-TEE 双托管**节点（BLS 私钥 + operator EOA 都在 TEE），走 CC-37 的 keeper
> 模式，见 `deploy-runbook-3node.md` + CC-37。**本文专讲独立节点**（BLS 私钥在本地 EIP-2335 加密
> keystore、operator 是普通 EOA），流程和坑与 TEE 托管不同。

---

## 0. 节点画像 & 验收证据

| 项 | 值 |
|---|---|
| 节点 | dvt3 @ MX93 B 板（arm64），与 kms1 同板但**独立**（不依赖 kms1 的 TEE） |
| BLS 私钥 | 本地 **EIP-2335 加密 keystore**（`node_state.json` 的 `keystore` 字段，密码 = `~/Dev/.env` 的 `DVT3_SECRET`），明文只在 RAM |
| operator EOA | `0x18420702…`（fresh，私钥板内 `/etc/airaccount/dvt3-operator.key` 600 + 本地 scratchpad，**不进 git**） |
| nodeId | `0x9c1cc9bb…` = `keccak256(EIP-2537 128B pubkey)`（**不是** compressed 48B 的 keccak） |
| validator | `0x539B9681aFd5BFbCaa655Fe4c6BdcFe1fa7864bC`（algId 0x01，与 dvt1/dvt2 同一个） |
| register tx | `0xcee33d583a26a41657a937d3faf1f5242f45e9c960c66ad7b50c3d8796d31379` |
| stake tx | `0x9b653e279be07f0fe475669bb7d41288f837475d5431373077f6b8fdafea4727`（registerRole，30 GToken） |
| 结果 | `isRegistered=true` · `nodeOperator==operator` · `effectiveStake=30`（minStake 30） |

---

## 1. 加电后解锁 dvt3（每次断电都要做一次）

dvt3 的 keystore 密码走 **tmpfs `/run/dvt/pass`**（`dvt.service` 的 `EnvironmentFile`），断电即清 →
`dvt.service` **不自启**（设计如此，密码不落盘），需人工解锁：

```bash
# 密码从 ~/Dev/.env 取（DVT3_SECRET,带双引号要剥掉），走 stdin 写 tmpfs,不进 flash/不进进程参数/不回显
SECRET=$(sed -n 's/^DVT3_SECRET=//p' ~/Dev/.env | head -1); SECRET="${SECRET%\"}"; SECRET="${SECRET#\"}"
printf 'NODE_KEY_PASSPHRASE=%s\n' "$SECRET" | ssh mx93b '
  umask 077; mkdir -p /run/dvt; chmod 700 /run/dvt; cat > /run/dvt/pass; chmod 600 /run/dvt/pass'
unset SECRET
ssh mx93b 'systemctl --no-block start dvt'   # ← 见坑 0：必须 --no-block
# 验证
ssh mx93b 'systemctl is-active dvt; curl -s http://127.0.0.1:8080/health'
```

keystore 解密成功 = `/health` 返回 `{"status":"ok","version":"1.12.0",...}`（密码错会 crash-loop，不会静默）。

### 坑 0 — `systemctl start dvt` 阻塞
`dvt.service` 是 `Type=notify`（或启动慢），前台 `systemctl start dvt` 会**挂住直到超时**。
→ 用 `systemctl --no-block start dvt`，再单独 `is-active` + `/health` 轮询确认。

---

## 2. 为什么必须走 `registerWithProof`（staked），不能 owner-bootstrap

`AAStarValidator.registerPublicKey`（owner bootstrap 免质押路径）合约里有：
```solidity
require(!requireStake, "Staking on: use registerWithProof");
```
本 validator `requireStake=true` → **bootstrap 路被合约堵死**。而且 validator 的 owner
（`0xb5600060`）**已锚定另一个节点**（`operatorNode(owner)!=0`），one-node-per-operator 也挡住 owner 再注册。

**推论**：
- 节点自带的 `POST /node/register` 端点内部只调 `registerPublicKey`（bootstrap）→ **staked 模式下没用**。
- 独立节点唯一入口 = **fresh operator EOA + 质押 minStake + `registerWithProof(pubkey, popPoint, popSig)`**（带 BLS PoP）。
- PoP 需要 BLS 私钥。keystore 里的 key 只在 RAM → 用 SDK `onboardDvtNode`，把**内存解密**出的 `blsSecretKey` 喂进去（明文绝不落盘）。

---

## 3. 一键注册（SDK `onboardDvtNode`，funder 代付）

用 `@aastar/operator onboardDvtNode`：一把做完 `fund operator ETH+GToken（JASON 代付）→ approve →
registerRole(ROLE_DVT, 锁 minStake) → registerWithProof`，`nodeId=keccak256(pubkey)`。

一次性脚本落在 **`aastar-sdk/tests/regression/onchain-evidence/dvt3-register.ts`**，要点：

1. 从板上取 `node_state.json`（**只含密文 keystore + 公开信息，明文私钥 has_privkey=false**，安全）。
2. **内存**解密 EIP-2335（pbkdf2 c=262144 / aes-128-ctr / sha256 checksum）→ 32B BLS scalar（只在 RAM）。
3. 断言 `keccak256(EIP-2537(pubkey)) == 板上 node_state.nodeId`（防止注册一个节点不 serve 的 nodeId）。
4. `onboardDvtNode({ publicClient, operatorWallet, funderWallet, blsSecretKey, ...覆盖 })`。
5. 链上复核 `isRegistered && nodeOperator==operator`。

运行（密码/私钥走 env，不进 argv）：
```bash
cd ~/Dev/aastar/aastar-sdk
SP=<scratchpad>
SECRET=$(sed -n 's/^DVT3_SECRET=//p' ~/Dev/.env|head -1); SECRET="${SECRET%\"}"; SECRET="${SECRET#\"}"
env SDK_DIR="$PWD" DVT3_KEYSTORE_JSON="$SP/dvt3-node_state.json" DVT3_SECRET="$SECRET" \
    DVT3_OPERATOR_PK="$(cat "$SP/dvt3-operator.key")" \
    SEPOLIA_RPC_URL="<sepolia rpc>" \
    pnpm exec tsx tests/regression/onchain-evidence/dvt3-register.ts
```

> operator EOA 先 `cast wallet new` 生成 → **先持久化**（板内 `/etc/airaccount/dvt3-operator.key` 600 +
> scratchpad）**再** onboard，防中途中断把质押打进一个丢了私钥的地址。

---

## 4. 四个坑 & 解法（按踩到顺序）

### 坑 1 — keystore 存的是**未 mod r 的原始 32B scalar**
`onboardDvtNode` 内部 `buildDvtPop` 严格校验 `sk ∈ [1, r-1]`，直接喂解密出的 32B 会报：
```
buildDvtPop: BLS secret key must be a scalar in [1, r-1] (r = BLS12-381 curve order)
```
**根因**：keystore 存的是原始 32 字节，可能 ≥ r。noble 在签名/`getPublicKey` 时内部会 `mod r`，
所以 pubkey/nodeId 不受影响，但 buildDvtPop 不自动约简。
**解法**：喂 PoP 前先约简 —— `blsSecret = toHex(BigInt(rawSecret) % bls.params.r, {size:32})`。
同一有效私钥、同 pubkey、同 nodeId，可证等价。

### 坑 2 — canonical gToken 与 validator 实际质押 token **漂移**（Sepolia）
`onboardDvtNode` 默认用 `CANONICAL_ADDRESSES[11155111].gToken`，当前是 **`0x8d6Fe002…`**（funder JASON
余额 0）。但本 validator `0x539B96` 的 registry `0xf5Bf37ca` 实际收的是 **`0x4c09aE57…`**（symbol GToken，
**owner = JASON `0xb5600060`、余额 1523**，dvt1 当年就是用它质押的）。默认 canonical → fund 错 token →
报余额不足。
**解法**：给 `onboardDvtNode` 传覆盖，钉到 dvt1 同款链上 setup：
```ts
onboardDvtNode({ ..., gToken: '0x4c09aE57503Aa1E2A43b05621A38DbdD43b0Aa08',
                      registry: await validator.registry() /* = 0xf5Bf37ca */ })
```
**排错法**：解剖 dvt1 的 stake tx（`cast receipt <staketx>` 看 Transfer 事件来自哪个 token 合约 +
`cast tx <approvetx>` 看 approve 的 `to`）→ 那个就是真质押 token。
> ⚠️ `GToken.mint` 是 `onlyOwner`；真 token `0x4c09aE57` 的 owner 就是 JASON（我们有 key），**全程 Sepolia
> 自助**，不用找人要，也别跟 OP mainnet 那套 deployer `0x51Ac6949` / `0x8d6Fe002` 混。

### 坑 3 —（最坑）SDK 把「operator ETH 不够付 gas」**误映射**成「Insufficient token balance」
`onboardDvtNode` 报：
```
Insufficient token balance for this operation      ← AAStarError E2002
```
但把 `e.cause` 打出来，真身是：
```
The total cost (gas * gas fee + value) of executing this transaction exceeds the balance of the account.
```
即 **operator ETH 不够付 registerRole 的 gas**（`registry.ts:364` 把 viem 的 gas-不足错误映射错了）。
operator 默认只被充到 `minOperatorEth=0.015`，而 `registerRole`（`lockStakeWithTicket`）是重调用、
gas 估算超过 operator 手上的 0.028 ETH → 卡死。GToken **从头就够**，被这个错误串带偏查了半天。
**解法**：给 operator 多充 ETH：
```ts
onboardDvtNode({ ..., minOperatorEth: parseEther('0.1'), topUpEth: parseEther('0.1') })
```
**教训**：**下次见 "Insufficient token balance" 先 `cast balance <operator>` 查 ETH，别急着查 GToken。**
排错时务必把 `e.cause` / `e.cause.cause` 打出来，别只看 `shortMessage`。

### 坑 4 — 生成的 nodeId 编码
`nodeId = keccak256(EIP-2537 128B pubkey)`，**不是** `keccak256(compressed 48B pubkey)`。
两者都能算出来，但只有 128B 版与 `onboardDvtNode` / 合约一致。脚本里用**两种编码都算一遍 + 断言等于板上
`node_state.nodeId`**，不一致直接 abort（防止注册一个节点不 serve 的 nodeId）。

---

## 5. 收尾

1. 独立 `cast` 复核：`isRegistered(nodeId)==true` / `nodeOperator(nodeId)==operator` / `effectiveStake>=minStake`。
2. 更新板内 `node_state.json` 的 `registeredAt` + `operator`（反映真实，节点侧幂等）。
3. **2-of-3 门限**：dvt1（TEE 托管）+ dvt3（本地 keystore）两个独立节点上链即满足 2-of-3，容忍 1 挂；
   dvt2 / DK2 是第三个，非门限阻塞。

## 6. 安全红线（本流程恪守）
- BLS 明文私钥**只在 RAM**（解密在内存、`mod r`、喂 onboardDvtNode），**绝不落 flash / 不进 git / 不回显**。
- keystore（密文）板内 600；密码走 **tmpfs `/run/dvt/pass`**，断电即清，人工重输（安全 vs 可用取舍，已接受）。
- operator EOA 私钥板内 `/etc/airaccount/dvt3-operator.key` 600 + scratchpad，**不进 git**；它只是 gas 付款人 +
  注册 msg.sender，泄露不动 BLS 密钥（价值在 BLS，那把在加密 keystore）。
- RPC key / DVT3_SECRET / operator key 全走 env / stdin，不进 argv（`ps` 可见）、不提交仓库。
