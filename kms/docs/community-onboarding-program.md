<!-- Created: 2026-07-07 -->
# AAStar 社区自助上手程序 / Community Self-Service Onboarding

> **阶段目标(2026-07 起)**:一个新社区买了自己的 MX93 主板,我们提供**在线下载流程 + 引导**,让他**自助启动自己的 KMS+DVT 节点**,加入 ≥3 节点门限网络。
>
> 本文档是该阶段的目标 + 路线图。进度在此持续更新。

## 终态(一句话)
社区买板 → 打开一个网页照做 → 板子跑起 KMS+DVT → 链上注册(gasless)→ 成为门限网络的一个独立节点。**不需要懂 CLI/编译/Git。**

## 现状盘点(2026-07-07)
| 已有 ✅ | 缺 ❌ |
|---|---|
| 部署脚本(mx93-deploy / deploy-dvt / dvt-unlock) | **可下载 flashable image**(整盘 .wic) |
| 发布 bundle(airaccount-kms / airaccount-node v0.28.0) | **setup 向导(web/TUI)实现** |
| 设计稿(community-node-kit-design 四层架构) | **下载门户 / 在线图文引导** |
| SDK v0.38.0 `dvtOperatorActions.registerWithProof` | **CI release 流水线**(自动出 image+bundle) |
| co-location 配置(community.toml) | SDK/API 自助注册**接线**(向导调 registerWithProof + BLS provisioning) |

## 三阶段计划(按社区傻瓜度递进 · 我方从难到易实现)

### Phase 1 — 预刷板 + 极简 web setup(MVP,最快让首个社区上线)
- **社区体验**:收到预刷好的板 → 通电 → 联网 → 浏览器打开 setup 页 → 填几项 → 完成。★★★ 零刷机。
- **我方要做**:
  - `aastar-node-setup` **web 向导**(首启在 LAN/Tailscale IP 开一个 setup 页)—— **本阶段核心新增**。
  - 向导收:社区域名/rpId、operator 地址、合约网络(test/main)。
  - 向导做:provision BLS 密钥(`/gen-key`)、生成 signer token、写 config、SP gasless 链上注册。
- **不需要 image 发布**(预刷板绕过)。用现成 bundle。
- **状态**:🚧 进行中(骨架起于 2026-07-07)。

### Phase 2 — 可下载镜像(社区自刷板)· 🚧 进行中(路径 A 起于 2026-07-07)
- **社区体验**:下 `.wic` 镜像 → balenaEtcher 拖拽刷 → 开机 → 同 Phase 1 的 setup 页。★★ 有 GUI。
- **我方要做**:
  - CI/脚本把 OP-TEE + KMS TA/CA + DVT + 部署工具 + node-setup **烤成整盘 image**。
  - 发 GH release + 下载页 + 刷机图文(uuu / balenaEtcher 跨平台)。

### Phase 3 — 全自助门户
- **社区体验**:官网下载门户一站式:选板型 → 下镜像 → 图文向导 → 一键注册。★ 最丝滑。
- **我方要做**:下载门户网站 + web 向导完善(错误说人话、进度可视)+ 链上 gasless 注册闭环 + SDK 集成打磨 + 节点健康/监控回传。

## 实现顺序
Phase 1 web 向导(核心)→ Phase 2 image + CI → Phase 3 门户。每阶段可独立交付、让真实社区用起来后再进下一阶段。

## 关联
- 设计:`community-node-kit-design.md`(四层架构原稿)
- 发布结构:`release-structure.md`(airaccount-kms / airaccount-node 两发布物)
- KMS-TEE 托管:`dvt-tee-bls-custody-design.md`
- 任务:T10(本程序落地)
