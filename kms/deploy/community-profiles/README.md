# 社区节点刷机方案 · 3 种组合 profile

外部社区加入时，一块刷好的 MX93 板可按需跑三种角色之一。每个 profile = 一份 config +
镜像该 enable 哪些 systemd unit + bundle 带哪些组件。

| 组合 | profile | 跑什么 | bundle 组件 | 用途 |
|---|---|---|---|---|
| **1 独立 KMS** | `profile-kms-only.toml` | 只 kms-api | kms-ta/ca + node-setup | 只提供密钥服务 |
| **2 独立 DVT** | `profile-dvt-only.toml` | 只 dvt(本地 keystore) | dvt-bare-node + node-setup + register-sdk | 只当验证节点 |
| **3 联合** | `profile-combined.toml` | kms + dvt | 全部 | 单板全功能(板A/B 同款,最傻瓜) |

三者差异 = `[services].enable` + `[bundle].components` + `[dvt].signer_mode`(tee/local/none)。
同一套镜像 + 首启按 profile 选择启用，即可分化成三种角色。

---

## ⚠️ 用板 B 实测刷机的两个硬前提

**1. 真正可刷的镜像还不存在（需要 #21 build，未做）。**
上面是 config profile；把它变成**能刷进板子的 `.wic` 镜像**，要 CI 交叉编译 TA(签名)+CA、
打 DVT bundle、把 @aastar SDK 打进 register-node 的 node_modules、组整盘镜像 —— 这套
`community-node-image-ci-design.md`(#21) 的流水线**还没建**，本地此刻造不出真镜像。

**2. 板 B 是活的生产节点，刷机=毁掉它（除非备份+还原）。**
板 B = kms1 + dvt3，dvt3 链上注册了、质押 30 GToken。它**从 SD 卡启动**(mmcblk0 29G)。
- ✅ 关键密钥/配置已备份 → `~/board-b-backup-2026-07-18/board-b-secrets.tgz`
  (含 `/etc/airaccount` + dvt3 keystore `node_state.json` + KMS 安全存储 `dirf.db`)。
- dirf.db 绑本机 HUK：还原到**同一块板**能解密 → KMS 密钥可救回。
- 但 secrets tar **不含 OS/bootloader**：裸机刷回需**整卡镜像**(见下)。

---

## 安全实测路径（推荐顺序）

1. **整卡镜像**（裸机还原底线）：关机 → 拔 SD 卡 → 插 Mac 读卡器 → `dd if=/dev/diskN of=board-b-full.img`
   （或 `.img.gz`）。有它 = 刷坏了随时还原成今天的板 B。
2. **备用 SD 卡最安全**：拿**另一张 SD 卡**刷测试镜像 → 插板 B 开机测 → 测完换回原卡，
   **板 B 生产节点全程不受影响**。← 强烈推荐，只要你有空白 SD 卡。
3. 若无备用卡：在原卡上测，测完 `dd` 还原整卡镜像 + 还原 secrets.tgz。

每个刷机方案首启后跑 `community-node-image-ci-design.md` 的 **T1–T10 验收矩阵**
（含向导 E2E test_setup_server.py、register-node --dry-run、KMS /pop 真机——正好补上本地测不了的部分）。
