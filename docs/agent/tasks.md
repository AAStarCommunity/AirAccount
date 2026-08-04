# AirAccount — Tasks

> pilot 任务账本。状态:READY(可做) / IN_PROGRESS / BLOCKED / IN_REVIEW / DONE。
> 一个 task = 一个分支 = 一个 PR。字段:ID · 标题 · Feature · 状态 · 分支/PR · 备注。

## 进行中 / 评审中

| ID | 标题 | Feature | 状态 | 分支/PR |
|---|---|---|---|---|
| T-196 | 发版签名工具 release-sign.sh + 公钥入库 | F1.2 | IN_REVIEW | feat/updater-release-signing · #196 |
| T-195 | Web 管理台增量1(安全地基) | F1.3 | IN_REVIEW | feat/updater-phase2-web-admin · #195 |

## READY(下一步可挑)

| ID | 标题 | Feature | 状态 | 备注 |
|---|---|---|---|---|
| T-A | OOB 工具补全 + serial-selfupdate.sh(改新公钥) | F2.2/F2.3 | READY | 历史散件批 PR-A |
| T-B | 运维/设计文档补全 | F4.1 | READY | 批 PR-B |
| T-D | attestation 测量 sequence 3 入库 | F4.2 | READY | 批 PR-D |
| T-web2 | Web 管理台增量2(SSE 进度+审计链+三屏) | F1.4 | READY | #195 合并后接 |
| T-deploy | updater 板上生产部署(systemd+目录迁移) | F1.5 | READY | 有真机风险,需谨慎窗口 |

## BLOCKED

| ID | 标题 | 阻塞原因 |
|---|---|---|
| T-liveapply | 真机 live apply 端到端 | 需交叉编译真实新版节点 tarball;且动 live 生产服务有风险,择窗口 |

## 已完成(近期)

| ID | 标题 | PR |
|---|---|---|
| T-194 | Phase1 updater 核心 | #194 ✅ |
| T-hygiene | 仓库卫生:pilot 初始化 + gitignore(73MB PDF/PSK 密钥)+ goutou 配置 | 本 PR |

> 旧的会话级 task(#1–#25,co-location/DVT/向导等)见 git 历史与 progress.md,已大部完成。
