<!-- Created: 2026-07-18 -->
# 社区节点镜像 + CI 打包设计草案（#21 / Phase 2）

> 目标：CI 一次构建，产出社区**开箱刷机 + 一键上线**所需的全部产物，并支撑 portal 提供的
> 「几种刷机方案」。承接 community-node-kit-design.md 四层架构的 Layer 1（AAstar 预构建 & 发布）。
> 关联：#19 模型 A 一键注册（board 侧 register-node.mjs 的 SDK 打包缺口在此闭合）。
> 状态：设计草案，待评审。

---

## 0. 为什么需要它（现状缺口）

| 已有 ✅ | 缺 ❌（本设计解决） |
|---|---|
| KMS TA(RSA-4096 签名)+CA(aarch64) 交叉编译（mx93-build.sh / Docker） | **CI release 流水线**（现只有 optee-build.yml，不出 release 产物） |
| DVT bare-node bundle（含 node） | **register-node.mjs 的 @aastar/operator+core+viem 打包**（#19 缺口：bundle 里没有 @aastar 包） |
| 向导 setup-server.py + register-node.mjs + selfinit | **整盘 flashable 镜像**（.wic），现只有 NXP 原厂 wic |
| 部署脚本（deploy-dvt / mx93-deploy） | **刷机方案 × 产物**的组装 + 校验 |

---

## 1. CI 产出物（每个版本 tag）

```
airaccount-node-<ver>/                      # ① 增量 bundle（网络 provision 用）
  ├── kms/            签名 TA(RSA-4096) + CA(aarch64) + CLI
  ├── dvt/            bare-node bundle（dist + node_modules + 内置 node，arm64）
  ├── node-setup/     setup-server.py + register-node.mjs + setup.html
  │     └── node_modules/   ← ★ #19 缺口:@aastar/operator+@aastar/core+viem(含 dist)
  ├── selfinit/       aastar-kms-selfinit.{sh,service} + finalize-helper.sh + sudoers
  └── install/        aastar-node-installer.sh（解包 + 装 systemd unit）

airaccount-node-<ver>.wic.zst               # ② 整盘镜像(balenaEtcher 拖拽刷)
airaccount-node-<ver>.manifest.json         # ③ 产物哈希 + 版本 + measurement
```

### ★ register-node 的 SDK 打包（#19 闭合，已实现 = esbuild 单文件）
`@aastar/operator`+`@aastar/core`+`viem` **不在 DVT bare-node bundle 里**（实测确认）。
**方案定为 esbuild 单文件 bundle**（试过 pnpm deploy：symlink 拷不动 + 拉进 vitest/vite 达 230M，弃）：
- `kms/node-setup/build-register-bundle.sh` → esbuild 把 register-node.mjs + operator+core+viem
  bundle 成 **`register-node.bundle.mjs`（~2MB,tree-shaken,纯 JS 无 native → 跨 arch）**。
- 板侧只需 `node register-node.bundle.mjs`（DVT bundle 自带 node），**零 node_modules / 零解析问题**。
- 向导 `attempt_onchain_register` 优先用 bundle，无则回落源文件（dev）。
- **已测**：脚本产出的 bundle 从空目录跑通 Sepolia dryRun（@aastar/operator@0.43.0）。
> 版本钉死：banner 写入 operator 版本；CI 把版本进 manifest，避免漂移。
> 构建产物 `register-node.bundle.mjs` gitignore（不入库，CI/build 时生成）。

---

## 2. 整盘镜像内容（.wic）

```
基底：NXP FRDM-IMX93 原厂 OpenSTLinux/Yocto（LF_v6.6.36…）—— OP-TEE 4.8 已在
  + 预置 airaccount-node bundle → /opt/aastar/
  + systemd units enabled: kms-api / aastar-kms-selfinit(或 aastar-node-setup) / dvt(key-less)
  + 出厂 kms.env: KMS_BLS_PROVISIONING=1（首启 provision，selfinit/向导后自动关）
  + cloudflared / frp 客户端（隧道，社区二选一）
  + 首启膨胀分区(resize rootfs to SD/eMMC)
不烤进镜像（首启/向导时生成，绝不进 git/镜像）：
  ✗ BLS 私钥(TEE 内生成) ✗ keystore 密码 ✗ operator 私钥 ✗ API key ✗ 社区 rpId/域名
```

---

## 3. 「几种刷机方案」× 傻瓜度（portal 提供）

| 方案 | 载体 | 社区操作 | 傻瓜度 | 产物 |
|---|---|---|---|---|
| **A 预刷板** | AAstar 刷好邮寄 | 通电→联网→浏览器填表 | ★★★ | AAstar 内部用 .wic |
| **B balenaEtcher** | 社区下 .wic 自刷 SD/eMMC | GUI 拖拽刷→开机→填表 | ★★ | `.wic.zst` + 校验 |
| **C uuu 恢复** | USB 串口下载模式 | CLI 跑 uuu 脚本 | ★（有时序坑，仅高级/救砖） | uuu 脚本 + bootloader |
| **D 网络 provision** | 已有基础 OP-TEE 镜像 | installer 下增量 bundle | ★ | `airaccount-node-<ver>/` + installer |

> portal 主推 **A/B**；C/D 作为救砖 / 高级路径。

---

## 4. 刷机方案测试矩阵（= 验收计划）

每个方案在**真板首启**后必须过下面这套 E2E（可脚本化）：

| # | 检查 | 期望 | 覆盖方案 |
|---|---|---|---|
| T1 | 刷入 + 首启起来 | 串口见 login，systemd 无 failed | A/B/C/D |
| T2 | 首启膨胀分区 | rootfs 占满介质 | B/C/D |
| T3 | kms-api /health | 200 | 全部 |
| T4 | selfinit/向导 provision BLS | `/gen-key` 出 key_id+pubkey，写 kms.env | 全部 |
| T5 | 向导 fresh-board E2E | test_setup_server.py 8 例过（板上跑一遍） | A/B（web 向导路径） |
| T6 | register-node.mjs 解析 SDK | `--dry-run` 对 Sepolia 出 minStake=30 计划 | 全部（验 §1★ 打包） |
| T7 | KMS /pop | 出 pop tuple（真 TEE） | 全部（这条只有真板能测） |
| T8 | 一键注册（预充值 operator） | registerWithProof tx 上链，isRegistered | A（模型 A 完整闭环） |
| T9 | finalize 关 gate | prov.conf 删除，重启后 /gen-key 拒 | 全部 |
| T10 | 加电自启/断电恢复 | 重启后服务自恢复（密码 tmpfs 需重输的除外） | 全部 |

> T6/T7/T8 正是 #19 现在**本地测不了、要真板**的部分——刷机测试正好一并收。

---

## 5. CI 流水线（GitHub Actions）

```
on: tag v*   (或 release)
jobs:
  build-kms:     交叉编译 TA(签名)+CA        （复用 mx93-build.sh / Docker，self-hosted 或 QEMU）
  build-dvt:     打 bare-node bundle          （build-bare-node.sh）
  build-sdk-pack: pnpm build+deploy @aastar/operator+core+viem → node-setup/node_modules  ★#19
  assemble-bundle: 组 airaccount-node-<ver>/ + installer + manifest(哈希)
  build-image:   基底 wic + 注入 bundle + enable units + 首启膨胀 → .wic.zst   （需 root/loop 或 self-hosted）
  publish:       GH Release 挂 bundle + .wic.zst + manifest
```

难点/待定：
- **build-image 需 loop 挂载 / root**：GitHub hosted runner 受限 → 可能需 self-hosted 或 `guestfish`/`libguestfs`。
- **TA 签名密钥**：RSA-4096 私钥在 CI 需安全注入（GH Secrets / OIDC → KMS），不落仓库。
- **measurement 一致性**：镜像内 CA/TA 的 measurement 写进 manifest（对齐 attestation-measurements）。

---

## 6. 落地顺序（增量，各自可交付）
1. **build-sdk-pack + assemble-bundle**（先补 #19 打包缺口 + 出 bundle Release）——最小可用，解锁模型 A 板侧。
2. **build-image（.wic）+ balenaEtcher 文档**（方案 B）。
3. **CI 全流水线 + publish + manifest**。
4. **portal 接下载**（→ #22）。

---

## 7. 开放问题
- build-image 用 GH self-hosted runner 还是 libguestfs 在 hosted 上跑？
- TA 签名私钥 CI 注入方式（Secrets vs 远程签名服务）？
- 基底镜像体积（NXP full image 数百 MB）→ Release 挂 `.wic.zst`，可接受？
- 方案 C(uuu) 是否值得投入（时序坑多、非傻瓜）——还是只留 A/B/D？
