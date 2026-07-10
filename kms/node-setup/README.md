# aastar-node-setup — 社区节点自助配置向导(Phase 1）

社区上手程序 Phase 1 的 web 向导。见 [`../docs/community-onboarding-program.md`](../docs/community-onboarding-program.md)。

## 是什么
社区拿到(预刷好的)板 → 通电联网 → 浏览器打开 `http://<板子IP>:8088` → 填几项 → 板子自动：
1. **provision BLS 密钥**（调 KMS internal signer `/gen-key`，密钥在 TEE 内生成密封，**永不出板子**）
2. **生成 signer token**（KMS 与 DVT 共享密钥 → X-Signer-Token）
3. **写 config**（`/etc/airaccount/kms.env` + `dvt.env`，0600）
4. **[Phase 1 stub]** 给出链上注册指引（SP gasless）；Phase 3 做成一键闭环

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

## 链上注册（节点不能自注册）
`AAStarBLSAlgorithm.registerPublicKey` 只有 **validator owner** 能调 —— 第三方节点**无法自注册**（DVT `deploy/README.md §3`）。所以向导把 `network/operator/blsPubkey` 输出在 `next_steps`，社区发给 AAStar 登记（AAStar 可 SP gasless 代付）。**这是 Phase 1 的正确模型**；Phase 3 把"提交给 AAStar + gasless 注册"做成向导内一键闭环。

## 仍待做
- **Phase 3**：链上注册一键闭环（向导内提交 + gasless）。
- **Low（Phase 2）**：service 加 `User=`(非 root)——现以 root 跑是为了 finalize 调 systemctl;拆成"向导 nobody + finalize 走 sudo 白名单/单独特权 helper"。

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
