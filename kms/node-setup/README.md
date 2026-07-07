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
# 板子上（需先让 KMS 允许 provisioning）
KMS_BLS_PROVISIONING=1 KMS_BLS_SIGNER_TOKEN=<可选> python3 setup-server.py
# 社区浏览器访问 http://<板子IP>:8088
```

## Phase 1 骨架边界（生产前 TODO）
- **认证**：setup 页现无认证 —— 生产要加一次性 setup token（防同网他人配置）。
- **幂等**：重复提交应检测已配置状态。
- **校验**：operator/rpId 格式、KMS 可达性、provisioning 是否已开。
- **首启集成**：systemd 首启拉起本向导；配置完成后自动关闭 provisioning 开关 + 重启服务。
- **链上注册**：接 SDK `dvtOperatorActions.registerWithProof`（Phase 3 一键）。

## 只用来 web 服务的子域名（可选）
AAStar 可给社区一个 `<community>.aastar.io` **仅用于 web 访问向导/面板**；passkey 的 rpId **必须**是社区自己的域名（身份独立）。
