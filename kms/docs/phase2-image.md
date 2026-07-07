<!-- Created: 2026-07-07 -->
# Phase 2 — 可下载镜像 / 社区自刷板

> 社区上手程序 Phase 2(见 [`community-onboarding-program.md`](community-onboarding-program.md))。
> 社区体验:下镜像 → balenaEtcher/uuu 刷 → 开机 → Phase 1 的 web 向导。

## 两条路径(按工程量)

### 路径 A(本阶段交付):基础 OP-TEE 镜像 + installer 覆盖安装
社区刷 **NXP 官方 FRDM-IMX93 OP-TEE 镜像**(`LF_v6.6.36-2.1.0_images_FRDM`,已含 OP-TEE + tee-supplicant),然后跑**一条命令** `aastar-node-installer.sh` 从 GH release 装 airaccount-node → 起 KMS+DVT+向导。

- **优**:不需要重建整盘镜像/Yocto;基础镜像用 NXP 官方的(已验证、含 OP-TEE);installer 幂等、可升级。
- **缺**:社区多跑一条命令(非纯拖拽即用)。
- **工件**:`kms/deploy/aastar-node-installer.sh` + 刷基础镜像图文(uuu / balenaEtcher)。

### 路径 B(后续):全烤 `.wic` 整盘镜像(纯拖拽)
把 airaccount-node 烤进 NXP 镜像 → 出一个 `.wic`,balenaEtcher 拖拽即刷、开机即向导。

- **优**:社区最丝滑(零命令)。
- **缺**:需 **NXP Yocto BSP 构建环境**(bitbake + meta-layers,GB 级下载 + 小时级构建)→ 只能在 CI/专用构建机做。
- **做法**:加一个 meta-airaccount Yocto layer(装 TA/CA/DVT/node-setup 的 recipe + 首启 provisioning)→ CI 出 `.wic` → 发 GH release。**待 Yocto 环境就绪再落地。**

## 本阶段决定:先做路径 A
路径 A 现在就能交付一个真实的「下载 + 刷基础镜像 + 一条命令 → 向导」自助流程,不卡 Yocto。路径 B 作为后续把最后一条命令也省掉。

## 刷基础镜像(社区图文,待补全)
- **uuu**(USB 下载,推荐):`uuu -b emmc_all flash.bin LF...rootfs.wic.zst`(mac_arm uuu 1.5.243,见 `docs/mx93-reflash-*.md`)。
- **balenaEtcher**(SD/eMMC via reader):拖拽 `.wic` 刷卡。
- 刷完开机联网 → 跑 installer(见下)。

## installer(路径 A)
```bash
# 刷好基础 OP-TEE 镜像的板子上,一条命令:
curl -fsSL https://raw.githubusercontent.com/AAStarCommunity/AirAccount/main/kms/deploy/aastar-node-installer.sh | sudo bash
#   或指定版本: ... | sudo VERSION=v0.28.0 bash
```
它做:下 airaccount-node release → 装 TA/CA/manifest + DVT(v1.10.0)+ node-setup → 装 systemd → 置首启 provisioning + enable 向导 → 起服务。详见脚本头注释。

## 待办
- 路径 A:installer 在**全新基础镜像板**上端到端验证(现有开发板已装满,需干净板验证)。
- 刷基础镜像图文补全(截图 / 分步)。
- 路径 B:meta-airaccount Yocto layer + CI(待 Yocto 环境)。
