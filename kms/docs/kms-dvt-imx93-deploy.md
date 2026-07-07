<!-- Created: 2026-07-06 -->
# KMS + DVT 二合一部署 runbook(imx93 · 单社区单板)

> 目标:一块 NXP FRDM-IMX93 上同时跑 **KMS(AirAccount)+ DVT(aNode)**,为一个小社区提供
> TEE 私钥管理 + BLS 二签。统一入口:**KMS 侧脚本起头 → 拉起 DVT 从源码自 build**。
> 实测基线见 [`kms-dvt-imx93-baseline.md`](./kms-dvt-imx93-baseline.md)。分支 `feat/kms-dvt-imx93-colocation`。

## 版本(基于最新 release)
- KMS **v0.27.4**(已有部署走 `scripts/mx93-build.sh` / `mx93-deploy.sh`)
- DVT **v1.9.0**(YetAnotherAA-Validator,本 runbook 从源码 build)

## 为什么 DVT 走 bare-node 而不是 Docker
DVT 官方部署路径是 Docker,但**嵌入式板的 Docker 常残缺**。imx93 实测三处不通:
1. legacy builder 在 ext4 上 `xattr` 导出层失败;
2. `buildx` 组件未装(BuildKit 起不来);
3. 内核无 iptables `raw` 表 → 桥接网络起不来。

→ 改用 **bare-node**:官方 glibc arm64 Node + `npm ci && npm run build` 从源码编译,systemd 裸跑。
更省内存(无容器层)、对受限板更 robust,契合"极致省内存"。DVT 纯 JS(@noble/curves BLS + NestJS + ethers),无 native dep,跨编无坑。

## 前置
- 操作机能 SSH 到板(Tailscale 固定 IP 优先);板子有外网(下 node + npm ci)。
- 板子:glibc(实测 2.42)、2GB RAM、python3(KMS 侧测试用)。
- DVT repo 本地干净(`git archive` 该 tag)。

## 一键部署
```bash
cp kms/deploy/community.toml.example community.toml   # 填 board/域名/RPC/validator
kms/deploy/deploy-dvt.sh --config community.toml --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator
```
脚本步骤(全部实测跑通):
1. 板上装 glibc Node20（缺则下）
2. `git archive vX.Y.Z` DVT 源码 → 上板解压（`--exclude node_state.json`，见下 footgun）
3. `npm ci && npm run build`（→ dist/main.js）
4. **手动输入 `NODE_KEY_PASSPHRASE`** → 只进 tmpfs `/run/dvt/pass`（内存，绝不落盘）
5. 生成**独立 BLS12-381** 密钥 → **EIP-2335 加密 keystore**（scrypt，明文即刻 `shred`；已加密则跳过）
   - ⚠️ **DVT 仓库 committed 了一个 `node_state.json` 测试 fixture**（`bls-node-001`，公共 BLS_TEST 键，"谁都能签"）。`--exclude` 排除它,否则覆盖独立密钥 → 跑在公共测试键上（严重）。已回报 @repo:dvt 建议 gitignore。
6. 写 `dvt.env`（非秘密，600）；合约地址取 `contracts.active`（testnet/mainnet 切换）
7. 装**加固** `dvt.service`（`ProtectSystem=strict`/`NoNewPrivileges`/... ，`EnvironmentFile=/run/dvt/pass`，**禁开机自启**，崩溃从 tmpfs 自动重启，不碰 `kms-api.service`）
8. 验收：DVT `/health` + `/node/info`（keystore 解密） + KMS `/health` + RAM

## 生产安全模型（实测验证）
- **BLS 私钥盘上只密文**（EIP-2335 keystore，scrypt）。`NODE_KEY_PASSPHRASE` 只在 tmpfs（内存）。
- **硬盘离线拿不到私钥**：只有密文 + scrypt，无密码。实测:清掉 `/run/dvt/pass` → 服务 fail-closed 起不来。
- **崩溃 vs 断电**:进程崩溃 → systemd 从 tmpfs 自动重启（无需重输）；断电/重启 → tmpfs 清空 → 运维跑 `dvt-unlock.sh` 重输密码。
- **重启恢复**:`ssh <board> 'bash /opt/dvt-build/dvt-unlock.sh'`（交互输密码 → 启动 → 验证）。
- **KMS 私钥本就在 TEE**（永不出）；两服务只绑 127.0.0.1，公网仅 Cloudflare Tunnel。
- **秘密清单**（全不入库，手动 provision）:keystore 密码 · operator key(ETH_PRIVATE_KEY，注册+gas) · KMS API key · tunnel token · x402 secret。

## 链上注册（E2E 前置，用 DVT 现成路径，不新增功能）
配 `ETH_PRIVATE_KEY`（= validator owner）→ `curl -XPOST 127.0.0.1:8080/node/register` 把节点 BLS 公钥
注册到 validator（`registerPublicKey`，付 gas）。注册后才能跑 `realnode-e2e.mjs` 的链上 `validate=0`。
p2p/DHT 自动发现以后再补。

## 合约地址配置化（测试↔生产切换）
`community.toml` 的 `[contracts_testnet]` / `[contracts_mainnet]` 两组 + `contracts.active` 选择器。
生产合约部署后填 `contracts_mainnet` → 改 `active=mainnet` → 重跑部署/E2E,不动代码。

## §E BLS 功能验证（部署后自证节点"真能签"）
节点 = 独立 BLS12-381 密钥,`/signature/sign` 走 owner 闸门(ERC-1271 风格,fail-closed)。验证 BLS 密码学(不依赖链上注册):
```bash
# 板上:用节点 BLS 私钥按 DVT DST 签 userOpHash,喂给服务 /signature/verify
# DST = BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_（noble 默认 _NUL_ 必须覆盖）
# 期望:本地 sigs.verify=true + HTTP /signature/verify valid=true;错 message→false
```
实测:节点私钥⇔公钥自洽、本地 verify=true、服务 `/signature/verify` valid=true、坏 ownerAuth→403。

## ⚠️ 已知跨仓阻塞(全链路 owner-gated sign)
DVT v1.9.0 的 owner 闸门调账户 selector **`0xa0cf00cf`**(非标准 ERC-1271),需 airaccount-contract 生产
AAStarAirAccountV7 实现。sepolia 旧测试账户 `0x45Dfe3`(只有标准 `isValidSignature` `0x1626ba7e`)→ revert。
→ 已在 Cooperation-Center 任务 @repo:dvt @repo:airaccount-contract 求权威账户地址。**不影响本地 BLS 签名/验证**;
链上注册 `registerBLSPublicKey`(发现层)本阶段先不做。

## 回滚(可随时恢复)
```bash
ssh <board> 'systemctl disable --now dvt.service && rm -rf /opt/dvt-build /opt/node20 /etc/systemd/system/dvt.service && systemctl daemon-reload'
# KMS 全程未受影响。
```

## 复制到其他社区
换一份 `community.toml`(新域名=新 rpId=新 passkey、新 RPC、新 validator、新板)即可。KMS 侧
passkey rpId 锚定各自域名(半去中心化);DVT node_state 每板独立生成,不复用。
