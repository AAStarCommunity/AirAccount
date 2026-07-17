# aastar-node-setup — 社区节点自助配置向导(Phase 1）

社区上手程序 Phase 1 的 web 向导。见 [`../docs/community-onboarding-program.md`](../docs/community-onboarding-program.md)。

## 是什么
社区拿到(预刷好的)板 → 通电联网 → 浏览器打开 `http://<板子IP>:8088` → 填几项 → 板子自动：
1. **provision BLS 密钥**（调 KMS internal signer `/gen-key`，密钥在 TEE 内生成密封，**永不出板子**）
2. **生成 signer token**（KMS 与 DVT 共享密钥 → X-Signer-Token）
3. **写 config**（`/etc/airaccount/kms.env` + `dvt.env`，0600）
4. **链上注册**：模型 A(预充值 operator)一键 `registerWithProof`(经 KMS `/pop`)；未预置则回落手动登记指引

## 跑
```bash
# 板子上（kms-api 需带 KMS_BLS_PROVISIONING=1 才能 /gen-key）
python3 setup-server.py          # 启动时把 SETUP TOKEN 打到 console/串口
# 社区浏览器访问 http://<板子IP>:8088，表单里填那个 token
```
或用 systemd 首启：`cp aastar-node-setup.service /etc/systemd/system/ && systemctl enable --now aastar-node-setup`（已配置则 ConditionPathExists 自动不启）。

## 已做（本轮加固）
- ✅ **认证**：一次性 setup token（首启生成、打到 console/串口、只 root 可读；表单提交 constant-time 比对）→ 防同网他人配置。
- ✅ **幂等**：已写 `kms.env` 则拒（409），systemd 层也 `ConditionPathExists` 双保险。
- ✅ **校验**：operator 合法地址正则、rpId 非空/非 aastar.io、network 白名单、body ≤8KB、**KMS 可达性预检**（不可达给人话 503）。
- ✅ **首启 unit**：`aastar-node-setup.service`（首启拉起、配置后自动不启）。

## 首启编排（预刷板出厂状态）
1. 出厂：kms-api.service 带 `KMS_BLS_PROVISIONING=1`（drop-in `prov.conf`）；`aastar-node-setup` enabled。
2. 社区通电联网 → 向导跑（token 打到串口/日志）→ 社区填表 → provision + 写 config。
3. ✅ **finalize（已自动）**：向导成功后 `finalize()` 自动删 `prov.conf`（关 provisioning）→ `daemon-reload` → 重启 kms-api/dvt（读新 `/etc/airaccount/*.env`）→ disable 本向导。`SKIP_FINALIZE=1` 可跳过（测试）。

## 链上注册（模型 A：一键 `registerWithProof` 闭环）
早期 `registerPublicKey`（owner bootstrap 免质押）只有 validator owner 能调；但本 validator `requireStake=true`，走 **staked 自注册路径 `registerWithProof(publicKey, popPoint, popSig)`** —— **节点自己能注册**，前提是 operator 先质押 30 GToken。

向导 step 5（`attempt_onchain_register`）：
- **模型 A（预刷板）**：板已预置**预充值 operator key** `/etc/airaccount/dvt-operator.key`（AAStar 出厂充好 ETH+30 GToken）+ `ETH_RPC_URL` → 调 **`register-node.mjs`**（SDK `@aastar/operator` 的 `onboardDvtNode`，PoP 走 **KMS `/pop`** popSigner，key-less TEE 节点 BLS 私钥不出板）→ 一键 stake+`registerWithProof` → 返回 register tx，节点加入门限池。
- **未预置 / 注册失败** → 优雅回落：输出 `network/operator/blsPubkey` 让社区发 AAStar 登记（**不阻断**向导成功）。
- **模型 B/C**（AAstar 代付 / SP gasless）见 `../docs/community-node-register-model{B,C}-*.md`。

### `register-node.mjs` 依赖与打包（重要）
用 `@aastar/operator` + `@aastar/core` + `viem`，**DVT bare-node bundle 里没有这些**：
- **板侧**：预刷板/release bundle 必须把这三个包（含 `dist`）放到 `register-node.mjs` 能解析的 `node_modules`（CI/Phase 2 打包处理，见 T21）。
- **地址**：⚠️必须显式传 `VALIDATOR_ADDRESS/GTOKEN_ADDRESS/STAKING_ADDRESS`（Sepolia 实链值），别用 SDK canonical（Sepolia `aaStarBLSAlgorithm=0x0`、gToken 漂移会失配）。向导 `REGISTER_ADDRS` 已写死实链值。
- **已测**：`node register-node.mjs --dry-run` 对 Sepolia 实链跑通（读到 `minStake=30 GToken`、`requireStake=true`、出资计划）；`popSigner→/pop` 真机 E2E 待板 A 恢复。

## 仍待做
- **模型 A 收尾**：`popSigner→/pop` 真机 E2E(待板 A 恢复)；板侧 `@aastar/operator+core+viem` 打包(T21/Phase2 CI)；operator key 预充值出厂流程。
- **模型 B/C**：AAstar 代付服务(CC-49 → repo:dvt) / SP gasless(CC-50 → repo:sp+sdk),已派协同中枢,反馈定稿后开发。
- **降权（#23，已提供 opt-in，待板上验证）**：`finalize()` 里唯一需 root 的动作已拆成特权 helper `finalize-helper.sh`（经 `aastar-node-setup.sudoers` NOPASSWD 白名单）。向导优先 `sudo -n helper`、失败**回落**直接 systemctl（root 部署行为不变）。启用非 root 见下「降权启用」。

## 降权启用（#23，非 root 跑向导，opt-in）
向导默认以 root 跑（`finalize` 直接 systemctl）。要把它降权、收窄 root 面：
1. `useradd -r -s /usr/sbin/nologin aastar-setup`
2. 装 `finalize-helper.sh` → `/opt/aastar/node-setup/`（0755 root:root）+ `install -m440 aastar-node-setup.sudoers /etc/sudoers.d/aastar-node-setup`（`visudo -c` 校验）
3. `/etc/airaccount` 归 `root:aastar-setup` 0770；**operator key `0640 root:aastar-setup`**（向导要读它注册——⚠️降权不改变"向导进程能读该 key"这一事实，收窄的是 systemctl/其它 root 面，非 key 本身）
4. `.service` 取消注释 `User=/Group=aastar-setup`，并把 **`NoNewPrivileges=no`**（否则 sudo 被挡死）
5. 首启验证：token 认证→provision→写 config→`sudo helper` 收尾全绿

**未做上面步骤时**：向导仍以 root 跑，`sudo -n helper` 失败即回落直接 systemctl，**行为与之前一致**。故本改动向后兼容、可安全合入，非 root 路径需在真板首启验证后再切。

## 只用来 web 服务的子域名（可选）
AAStar 可给社区一个 `<community>.aastar.io` **仅用于 web 访问向导/面板**；passkey 的 rpId **必须**是社区自己的域名（身份独立）。

---

## Hands-off 自运行（`aastar-kms-selfinit.sh` + `.service`）—— 无 web 表单的加电自初始化

上面的 web 向导是**社区成员交互配置**用；调试台 / 预刷板要的是**"加电即自初始化、无人值守、幂等"**——这条走 `aastar-kms-selfinit`（这是"几种加入/启动方式"里的自运行台那种）。

**它做什么（KMS 侧，幂等）**：
1. 生成/复用共享 signer token（`KMS_BLS_SIGNER_TOKEN`）。
2. **BLS provision**：`/gen-key` → 记 `KMS_BLS_KEY_ID/PUBKEY` 进 `kms.env`（已有则跳）。TEE 内已有 singleton 但 key_id 未记录 → **明确报错停**（不瞎猜；需人工恢复 key_id 或移除旧 key）。
3. **keeper provision**（opt-in `KMS_KEEPER_ENABLE=1`，CC-34）：`/kms/gen-keeper-eoa` → 记 `KMS_KEEPER_KEY_ID/ADDRESS`（二进制无此端点则 WARN 跳过，不阻断 BLS）。
4. **写 DVT handoff** `/etc/airaccount/dvt-handoff.env`（`RUST_SIGNER_URL/TOKEN/REQUIRED` + BLS pubkey + keeper）——**KMS 只产出，@repo:dvt 消费进它自己的 `node_state`/`dvt.env`**（不越界写 DVT install 目录 / 不建 node_state）。
5. **关 provisioning gate + 重启 kms-api**（读进已记录的 key_id）→ 写 marker `.kms-selfinit-done`。

**幂等**：marker 在 → service `ConditionPathExists` 不启；脚本内每步 skip-if-done；重跑安全。

**首启编排**（预刷板出厂）：
```
出厂 kms.env: KMS_BLS_PROVISIONING=1 (+ KMS_KEEPER_ENABLE=1/KMS_KEEPER_PROVISIONING=1 若要 keeper)
enable: kms-api.service + aastar-kms-selfinit.service
加电 → kms-api 起(gate on) → selfinit oneshot 跑 → provision + handoff → 关 gate + 重启 kms-api → marker
之后每次加电: marker 在 → selfinit 不跑, kms-api 以已 provision key 直接自启
```

**边界**：本脚本纯 KMS 侧——provision KMS 自有 TEE key + 出 handoff。DVT 侧（v1.11.0 key-less 部署、写 `node_state.json`、`dvt.env`、`dvt.service After=kms-api`）由 **@repo:dvt** 消费 handoff 后自己落（见 CC-24）。

**装**：`cp aastar-kms-selfinit.{sh,service} → 板上 /opt/aastar/node-setup/ 与 /etc/systemd/system/`（路径见 service 注释）。
