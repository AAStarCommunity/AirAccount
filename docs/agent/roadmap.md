# AirAccount — Roadmap (Milestone → Feature)

> pilot 规划层。M=里程碑,F=Feature。Task 级见 `tasks.md`,实时进度见 `progress.md`。
> 维护:每推进一步同步更新;宁慢勿让文档与仓库脱节。

## M1 — 社区节点自动更新系统 ★进行中

远程/NAT 后/难物理触达的签名节点安全自更新:验签拒毒 + 健康门回滚 + 掉电恢复 + 错峰。

| Feature | 状态 | 说明 |
|---|---|---|
| F1.1 Phase1 updater 核心 | ✅ 已合并 #194 | check/apply/rollback/recovery + minisign 验签 + 健康门 + 自动回滚 + 掉电恢复 + Telegram 通知,105 断言 |
| F1.2 发版签名链路 | 🔵 review #196 | release-sign.sh 组装+签名 manifest + 生产公钥入库(key ID B4D4EC2546A19EB2) |
| F1.3 Web 管理台(增量1) | 🔵 review #195 | warp 服务 + 安全地基 + 会话/CSRF/Origin + Telegram 2FA + 非 root helper |
| F1.4 Web 管理台(增量2) | ⬜ 待办 | SSE 实时进度 + 审计链式 hash 持久化 + apply/进度/审计三屏 |
| F1.5 板上生产部署 | ⬜ 待办 | updater 装 systemd timer/recovery + flat→版本化目录迁移 + 真机 live apply(需交叉编译真 tarball) |

**真机验证(2026-08-04 板 B):** 105 断言测试套件全绿;真实生产密钥端到端(Mac 签→板拉→真公钥验签→通知)闭环。

## M2 — 带外运维工具链(OOB)

板子上电但 SSH/Tailscale 不通时的救板/升级能力。

| Feature | 状态 | 说明 |
|---|---|---|
| F2.1 串口命令执行器 | ✅ 已合并 #193 | serial-run.py(nonce 框定输出 + flock 独占);⚠️坑:慢命令需 --read-secs |
| F2.2 串口自拉升级 | ⬜ PR 待提 | serial-selfupdate.sh(Mac 验签→板拉 release→换 CA→烟测→回滚);待更新到新签名公钥 |
| F2.3 板级电源/控制台工具 | ⬜ PR 待提 | poweroff wrappers + mac-mini 常驻控制台 |
| F2.4 WiFi provisioning | 🔒 本地 only | 含 PSK 密钥,gitignore 不入库 |

## M3 — 社区节点分发(Community Node Kit)

预刷板邮寄 + 一次 SSH 跑向导 + 链上注册。

| Feature | 状态 | 说明 |
|---|---|---|
| F3.1 aastar-node-setup TUI 向导 | 🔵 进行中 | 见旧 task #10;含链上注册一键闭环(#19 已完成) |
| F3.2 整盘 .wic 镜像 + CI release | 🔵 进行中 | 见旧 task #21 |
| F3.3 社区节点下载门户 | ⬜ 待办 | 见旧 task #22 |

## M4 — 运维文档与治理

| Feature | 状态 | 说明 |
|---|---|---|
| F4.1 运维/设计文档补全 | ⬜ PR 待提 | community-node-init / sd-card-offline / security-audit |
| F4.2 attestation 测量登记 | ⬜ PR 待提 | measurements sequence 3 |

## 生态背景

AirAccount 是 Mycelium/AAstar 生态的身份与密钥底层(TEE 私钥 + WebAuthn + AWS KMS 兼容 API)。
硬件:NXP FRDM-IMX93(aarch64,OP-TEE 4.8)。生产 URL kms.aastar.io。
