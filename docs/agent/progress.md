# AirAccount — Progress

> pilot 运行态日志。最新在上。

## 2026-08-04 — pilot 接管 + 仓库整理

- **pilot 初始化**:建 `.pilot.yml`(base=main / integration=main,匹配现有 PR→main 流)+ `docs/agent/`。
  - ⚠️ **合并终点**:本仓 integration=main 是受保护 trunk,`git-guard.sh merge-pr` 硬拒直合主干,
    且主干需**非作者** approve —— 故 `pilot run` 在**「PR 已开、等外部 approve」即终止**,
    合并由 pr-daemon/人工在 GitHub 上完成,pilot 不代劳自动合并。这是刻意设计,非缺陷。
- **分支审核**:12 → 4。删 8 个(1 个 -d 干净合并 + 7 个 squash 合并/评审快照,逐个核过内容在 main)。
  保留:main、feat/updater-phase2-web-admin(#195)、feat/updater-release-signing(#196)、docs/oob-serial-rescue(有未合并 serial-selfupdate.sh,待 PR-A 后删)。
- **仓库卫生**:73MB NXP PDF(`imx93-docs/`)+ WiFi PSK 明文密钥(`kms/docs/dk2-school-wifi/`)加 .gitignore,挡在 git 外。
- **历史散件批 PR**:PR-A(oob 工具)/ PR-B(文档)/ PR-C(本 hygiene)/ PR-D(attestation seq3)。
- **当前焦点**:updater 自动更新系统(M1)—— #194 已合并,#195/#196 评审中,真机 105 断言 + 真实密钥端到端已通。

## 正在开发的 Feature/Task

- F1.2 发版签名(#196,review)、F1.3 Web 管理台增量1(#195,review)。
- 下一步 READY:PR-A/B/D 历史散件收口 → 然后 F1.4 Web 增量2 / F1.5 板上生产部署。

## 阻塞

- T-liveapply:真机 live apply 需交叉编译真 tarball + 动生产服务,择窗口。
- 板 B 依赖手机热点/校园网,联网不稳(非代码问题)。
