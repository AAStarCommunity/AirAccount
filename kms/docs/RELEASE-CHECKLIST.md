<!-- Created: 2026-06-15 -->
# AirAccount KMS — 发版清单模板（RELEASE CHECKLIST）

> 每次发版**逐项过**。漏掉任何一处版本号 = README 徽章/Swagger 标题对不上、对外显示版本与运行时不一致。
> 用法：发版前 copy 本清单到 PR 描述，逐项打勾。占位 `vX.Y.Z` = 新版本，`BetaN` = 里程碑名。

---

## 0. 决定版本号（双轨）
- [ ] **CA(host)**：语义化版本（feature → minor，fix → patch）。这是对外/运行时主版本。
- [ ] **TA**：仅当 TA 代码变更才 bump（攒到下次 TA 改动）。
- [ ] **proto**：仅当 proto 线格式（命令/结构）变更才 bump。
- [ ] ⚠️ proto bincode 非自描述 → **proto 变 = host + TA 必须同版本一起部署**。

## 1. 版本号必改位置（逐个 grep 确认无旧版本残留）
| # | 文件 | 位置 / 模式 | 内容 |
|---|---|---|---|
| 1 | `kms/host/Cargo.toml` | `^version =` | CA 版本 |
| 2 | `kms/ta/Cargo.toml` | `^version =` | TA 版本 |
| 3 | `kms/proto/Cargo.toml` | `^version =` | proto 版本 |
| 4 | `kms/host/src/api_server.rs` | `const KMS_VERSION` | **运行时 `/version` + `/health` 上报**（最易漏，必改） |
| 5 | `kms/docs/api/openapi.yaml` | `version:`（约 line 8）+ 头注释（line 2）**+ ⚠️ 本版新增/改动的端点必须补进 `paths:`（不是只 bump 版本号——v0.22.0 曾漏掉 `/attestation` + `/.well-known/*`，事后补）** | OpenAPI |
| 6 | `kms/docs/api/index.html` | `<title>`（line 6）+ `<h1>` 里 `BETAN · vX.Y.Z`（line 30） | Swagger UI 页 |
| 7 | `kms/docs/API-TEST-MATRIX.md` | 头部 `> <日期> · vX.Y.Z (BetaN)` | 测试矩阵 |
| 8 | **`README.md`** | **version 徽章（line 7）`version-vX.Y.Z%20BetaN`**（`%20`=空格 URL 编码） | **仓库首页徽章（最易漏，必改）** |
| 9 | `CLAUDE.md` | `**版本**: vX.Y.Z`（约 line 20） | 项目指令 |
| 10 | `kms/docs/RELEASE-PLAN.md` | 头部 `当前：**BetaN (vX.Y.Z) 已发布**` + 日期 | 发版状态 |
| 11 | `kms/host/attestation-measurements.json` | `version` 字段 + `ta_measurement` | ⚠️ **仅当 TA 重新构建/部署** → 必须用 `scripts/ta-measurement.sh` **重算 measurement**（不是只换 label，否则 manifest 撒谎） |

> 验证命令（应只剩 CHANGELOG 历史 + 本清单 + 计划文档里的旧号）：
> `grep -rniE "v?0\.(上一版本)\.[0-9]" --include="*.md" --include="*.toml" --include="*.yaml" --include="*.html" --include="*.rs" . | grep -vE "third_party|node_modules|/\.git/|CHANGELOG|Cargo.lock|backup|\.claude/"`

## 2. CHANGELOG + 文档
- [ ] `kms/CHANGELOG.md`：新增 `## vX.Y.Z (<日期>) — BetaN — <主题>` 段（新增/安全/文档/测试/版本 分节）+ 更新顶部 `> Updated:` 日期。
- [ ] **feature 变动文档**：行为/接口变了就更新对应设计文档 + `README.md` 正文相关段（端点数、测试计数、特性列表）。
- [ ] `docs/design/vX.Y+1.0-plan.md`：滚动出下一版计划（归拢未做项 + 主题）。

## 3. 构建 + 部署 + 验证
- [ ] 交叉编译：`./scripts/mx93-build.sh ca`（仅 host 变）/ `all`（TA 也变）—— aarch64 green。
- [ ] 部署（板子走 WiFi/DHCP，IP 会变 → 自己扫 `192.168.2.0/24:22`，别假设 IP）：
  - CA-only：备份 → scp → 原子替换 → `systemctl restart kms-api.service` → `/RollbackCounter` 烟测。
  - TA 也变：走 `mx93-deploy.sh`（重启 tee-supplicant）+ **重算 measurement manifest（位置 #11）**。
- [ ] **验证 `/version` 上报新版本**：板上 `curl 127.0.0.1:3000/version` **且** 公网 `curl https://kms.aastar.io/version` 都 = `vX.Y.Z`。（这步能抓到漏改 `KMS_VERSION`。）
- [ ] E2E 冒烟：关键端点 + 本次新增/改动的功能。

## 4. 合并 + 打标 + 发布（顺序）
- [ ] 版本 bump + CHANGELOG + 计划文档 走一个 **release PR**（main 有分支保护，需 review approve）。
- [ ] **理想顺序：badge/版本号在 PR 里改好 → 合并 → 再打 tag**（让 tag 快照含正确徽章；本次因 badge 漏改走了 follow-up，下次别这样）。
- [ ] merge 后：`git tag -a vX.Y.Z <merge-commit> -m "..."` + `git push origin vX.Y.Z`。
- [ ] `gh release create vX.Y.Z`（技术版 notes：核心变更 + 双轨版本 + 已知边界，链 CHANGELOG + 计划文档）。
- [ ] 确认 Release 非 draft（`gh release view vX.Y.Z`）。

## 5. 收尾
- [ ] 关闭/更新本版相关 issue；标记 deferred 项。
- [ ] 如硬件/部署方式有变 → 更新 memory（`hardware_mx93.md` 等）。

---
*本清单源于 v0.22.0 复盘：当时漏改 README version 徽章（#8）+ 多处 Swagger/doc 版本标签，事后补 sync。固化于此，杜绝再漏。*
