# 上线收尾计划 — dvt3(板B)+ dvt2(DK2)

> 承接 `deploy-runbook-3node.md` 的拓扑。本文记录 **2026-07-13 实际上线状态** + 剩余两个 DVT 节点(dvt3 / dvt2)的落地步骤。
> 你在 mac mini 上 follow 最新版后,照本文 §3(DK2)操作即可。

## 0. 拓扑与当前状态(真实)

| 节点 | 硬件 | 角色 | 状态(2026-07-13) |
|---|---|---|---|
| **node1** | MX93-A | KMS 生产(kms.aastar.io)+ DVT1(TEE 托管) | ✅ **已上线并送机房**;CC-37 链上注册闭环(register tx `0x7559c8d7…`);CA 0.29.0;加电自启 |
| **node3** | MX93-B | **kms1(KMS 测试,kms1.aastar.io)** + **dvt3(独立)** | 🔄 **kms1 已完成**(真 TEE + CA 0.29.0 + tunnel LIVE + 加电自启);**dvt3 待部署**(见 §2) |
| **node2** | **DK2** | **dvt2(独立,armv7)** | ⏳ **待部署**(见 §3,mac mini + 串口) |

DVT 门限 **2-of-3**(dvt1@node1 TEE + dvt2@DK2 本地 + dvt3@node3 本地),容忍 1 挂。

**关键区别 — dvt2/dvt3 是「独立 DVT」**:各自持有**本地 EIP-2335 加密 keystore** 的 BLS 私钥(不走 KMS-TEE 的 `RUST_SIGNER_URL`)。keystore 存板内、600、密码保密。只有 dvt1@node1 是 TEE 托管。

---

## 2. dvt3 @ 板B(独立,arm64)

板 B 已联网(Tailscale `100.69.249.7`)+ kms1 已跑。dvt3 与 kms1 **同板但独立**(不依赖 kms1 的 TEE)。

```bash
# 板 B 上(ssh mx93b):
# 1) 取 DVT aNode(bare-node 方式,纯 JS 无 native dep)
#    git clone YetAnotherAA-Validator 或用 deploy bundle
# 2) 生成独立 BLS12-381 密钥 + 加密成 keystore(密码 = ~/Dev/.env 的 DVT3_SECRET)
#    node scripts/gen-node-state.mjs        # 出 nodeId + pubkey
#    KDF=pbkdf2 node scripts/encrypt-node-key.mjs   # → EIP-2335 keystore
#    install -m600 keystore.json /etc/airaccount/dvt3-keystore.json   # 板内 600 保密
# 3) dvt.env(独立模式,不设 RUST_SIGNER_URL):
#    ETH_RPC_URL=<Sepolia>  VALIDATOR_CONTRACT_ADDRESS=0x539B9681aFd5BFbCaa655Fe4c6BdcFe1fa7864bC
#    ENTRY_POINT_ADDRESS=0x0000000071727De22E5E9d8BAf0edAc6f37da032  PORT=8080
#    NODE_KEY_PASSPHRASE 走 tmpfs / 手动输入(密码不落盘)
# 4) systemd 起 dvt.service(独立、Restart=on-failure)→ /health
```

**dvt3 上链注册**:dvt3 自己的 operator EOA `registerBLSPublicKey`(或 SDK `onboardDvtNode` 本地 key 路径,operator 质押 30 GToken)。operator 付 gas、须是 validator owner 或走质押注册。**独立 tx,与 node1 无关。**

---

## 3. dvt2 @ DK2(独立,armv7)—— ⭐ 你在 mac mini 要做的

DK2 = STM32MP157F,**ARMv7 32-bit / 512MB**,`deploy-dvt.sh` 的 arm64 路径跑不了。用 **DVT 仓 CC-32 的 DK2 专用脚本**(`YetAnotherAA-Validator/deploy/dk2/`,PR #206 已合并)。

### 你在 mac mini 上的步骤(follow 最新版后)

```bash
# 前置:
#  - DK2 MAC 24:cd:8d:4e:4f:28 已在图书馆门户注册(WiFi 同学校网络)
#  - mac mini 装 Node 20 + 能连 DK2 串口(/dev/tty.usbserial* 115200)

# ① Mac 交叉打包 armv7l bundle(板上永不 build,只解包 → 绕开 512MB)
cd YetAnotherAA-Validator
./deploy/dk2/build-bundle-dk2.sh          # 产出 linux-armv7l bundle(48M,含 ELF 32-bit ARM node)

# ② 串口/网络传到 DK2 + 安装(install-dk2.sh 自带 arch 守卫,拦错架构)
./deploy/dk2/install-dk2.sh <bundle> node2

# ③ DK2 上生成独立密钥 + keystore(密码放 DK2 的 .env,约定名 NODE_KEY_PASSPHRASE)
node scripts/gen-node-state.mjs           # 独立 BLS key,勿用仓库任何键
KDF=pbkdf2 node scripts/encrypt-node-key.mjs   # A55/armv7 用 pbkdf2 更稳
#   keystore 存 DK2 内、600 保密;密码写 DK2 /etc/dvt2.env 的 NODE_KEY_PASSPHRASE

# ④ dvt2.env(独立,不设 RUST_SIGNER_URL):
#    ETH_RPC_URL / VALIDATOR=0x539B96 / ENTRY_POINT / PORT=4002
#    systemd MemoryMax=320M + NODE_OPTIONS=--max-old-space-size=256(512MB 调优)

# ⑤ 起 systemd(deploy/dk2/aastar-dvt@.service)→ curl /health

# ⑥ dvt2 上链注册:dvt2 operator registerBLSPublicKey(独立 tx)
```

### DK2 应该做的事(清单,照勾)
- [ ] WiFi 入网(图书馆 PSK,MAC 已注册,同板A/B 的双网络重试规则)
- [ ] 装 armv7l DVT bundle(Mac 交叉打包,板上不 build)
- [ ] 生成**独立** BLS keystore(密码 = DK2 .env 的 `NODE_KEY_PASSPHRASE`,600 保密,勿用仓库测试键)
- [ ] dvt2.env(独立模式,validator 0x539B96,PORT 4002)
- [ ] systemd 512MB 调优 + Restart=on-failure + 开机自启
- [ ] `/health` 绿
- [ ] 上链 `registerBLSPublicKey`(dvt2 自己的 operator/pubkey)
- [ ] 加电自启验证(断电重启,自动重连 + 服务恢复)

---

## 4. 2-of-3 生效验收
三个节点各自注册 BLS pubkey 后,链上 `AAStarBLSAlgorithm.validate()==0` + slash 门限编排跑通即 2-of-3 生效。node1(dvt1,TEE pubkey)/ node2(dvt2,本地)/ node3(dvt3,本地)三把独立 pubkey。

## 5. 安全红线
- keystore **只存板内、600、密码保密**,绝不进 github。
- 三节点密钥**各自独立**(TEE 密封的 dvt1 + 本地加密的 dvt2/dvt3),一把泄露只塌一个节点。
- 密码走 tmpfs / 手动输入,不落盘;断电需人工重输(安全 vs 可用取舍,已接受)。
