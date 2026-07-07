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
