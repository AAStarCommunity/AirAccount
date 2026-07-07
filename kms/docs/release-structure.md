<!-- Created: 2026-07-07 -->
# 发布结构:两个发布物 / Release Structure (two artifacts)

> 决策(2026-07-07):KMS 发布应有**两个独立发布物**,对应部署方案(见根 README「部署方案」)。
> KMS+DVT 合并版**不重编 DVT**,而是**依赖 DVT 仓库自己的最新 release 二进制**。

## 两个发布物

### ① `airaccount-kms` vX.Y.Z —— 独立 KMS(完整可运行)
只做密钥管理 / 签名服务(部署方案 ①)。自包含、独立可跑。
- **含**:签名 TA(`4319f351….ta`,生产 features `ree-fs-only,strict-challenge`)+ CA(`kms-api-server`)+ 安装脚本(`mx93-deploy.sh --first-run`)+ systemd units(`kms-api.service` + `dirf-repair.service`)+ `attestation-measurements.json`(**已签**)。
- **消费方**:SuperPaymaster / SuperRelay / SDK 直接调 AWS-KMS 兼容 API。
- **不含**:DVT 任何东西。

### ② `airaccount-node` vX.Y.Z —— KMS + DVT 合并(完整社区节点)
同板 co-located,DVT 的 BLS 私钥托管进 KMS TEE(部署方案 ③)。**完整可运行的社区节点包。**
- **含**:
  - ①的全部(KMS TA + CA + KMS systemd)
  - **DVT 二进制:取自 [YetAnotherAA-Validator](https://github.com/AAStarCommunity/YetAnotherAA-Validator) 仓库的最新 release 产物**(pin 一个 DVT release tag/version;**不在本仓重编 DVT**——DVT 的 build 依赖它自己的仓库)
  - co-location 配置 + 脚本:`community.toml`、`deploy/deploy-dvt.sh`、`deploy/dvt-unlock.sh`、DVT systemd unit
  - KMS 内部 signer 默认开(`:3100`),DVT 配 `RUST_SIGNER_URL=http://127.0.0.1:3100`
  - (未来 T10)`aastar-node-setup` TUI 向导 —— 傻瓜式一次性配置
- **DVT 依赖钉版**:`airaccount-node` 的 release note 明确「bundled DVT = YetAnotherAA-Validator vA.B.C」。DVT 升级 → 本仓换 pin + 重出 `airaccount-node`(KMS 二进制可不变)。

## 关键原则
1. **DVT 二进制来自 DVT 仓库的 release**(它 build 依赖自己的仓库),`airaccount-node` 只**打包引用**,不重编。
2. **两个发布物版本独立**:`airaccount-kms` 跟 KMS 代码走;`airaccount-node` = 某个 `airaccount-kms` + 某个 DVT release + 合并配置。
3. **签名/measurement**:两个发布物的 KMS TA 是同一个生产 TA(同 measurement);manifest 用 pinned publisher key 签一次。
4. **独立 DVT(方案②)不在本仓发布** —— 那是 DVT 仓库自己的 release(`RUST_SIGNER_URL` 不设即独立 keystore 模式)。

## 发布流程(Variant B merge 后执行)
1. merge PR #153(Variant B)+ 相关 → main。
2. 构建**生产 TA**(`ree-fs-only,strict-challenge`,无 dev-rpid)→ `ta-measurement.sh` 算 measurement → 更新 + **签** `attestation-measurements.json`(publisher key)→ Sigsum cosign。
3. 出 `airaccount-kms vX.Y.Z`(TA + CA + 脚本 + 已签 manifest)。
4. 取 DVT 最新 release 二进制 + pin 版本 → 组 `airaccount-node vX.Y.Z`(①+ DVT + 合并配置 + 向导)。
5. 生产板(kms.aastar.io)刷生产 TA + measurement。
