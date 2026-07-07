<!-- Created: 2026-07-06 -->
# 生产 mx93 初始化 · 社区部署 · 灾备 runbook

> 配套:[部署 runbook](./kms-dvt-imx93-deploy.md) · [性能基线](./kms-dvt-imx93-baseline.md) ·
> `kms/deploy/`(community.toml.example / deploy-dvt.sh / dvt-unlock.sh)。

## 0. 新板到手的工作流(谁做什么)

**你做(物理 / 一次性)**
1. **刷机**(板子若空白):Mac 直连 USB,`uuu` 刷 LF OP-TEE 镜像(见 memory `hardware_mx93_reflash_procedure`)。预刷好则跳过。
2. **联网**:接 WiFi 或加入 Tailscale(固定 IP,全球可达;推荐)。
3. **决定域名**:本社区 rpId(生产 KMS 默认 aastar.io;别的社区填自己域名)。
4. **部署时手动输 `NODE_KEY_PASSPHRASE`**(BLS keystore 密码,不落盘,设计如此)。

**我做(远程,SSH 可达后)**
- **KMS**:Docker 交叉编译 TA+CA(`scripts/mx93-build.sh all`,生产 profile 不带 dev-rpid)→ 刷 TA + 装 CA + systemd + 重算 measurement + provision API key。
- **DVT**:`deploy-dvt.sh --config community.toml`(装 node → 源码 build → 生成独立 BLS 密钥 → EIP-2335 加密 → tmpfs 密码 → 加固 systemd)。
- **config / 链上注册(`POST /node/register`)/ 双 E2E / health**。

> 简言之:**你刷机+联网+输密码,我远程编译+部署+配置+测试。** 板子预刷 + 上 Tailscale 后,我基本一条命令链完成。

## 1. 社区初始化指南(AAstar 默认 vs 他人覆盖)

**弹性可配字段**(改这些就能部署自己的实例):

| 字段 | AAstar 默认 | 他人必改 | 位置 |
|---|---|---|---|
| 域名 / rpId / origin | `aastar.io` / `kms.aastar.io` | ✅ 自己域名 | `[community].domain` `[kms].rp_id/origin` |
| 链 RPC | 内部 | ✅ 自己的可靠 RPC | `[chain].eth_rpc_url` |
| 合约地址(测/生产) | 见 config | 一般沿用/主网自填 | `[contracts_*]` |
| Cloudflare tunnel | AAstar zone | ✅ 自己 zone + token | `[secrets]` |
| node_name / 端口 | — | 可改 | `[dvt]` |
| **秘密**(密码/key/token) | — | ✅ 自己 provision | `[secrets]`(不入库) |

**他人部署三步**:① `cp community.toml.example community.toml` 改上表字段 → ② provision 秘密(见 §2)→ ③ 跑 KMS 部署 + `deploy-dvt.sh`。
rpId 走 `KMS_RP_ID`/`KMS_ORIGIN` env **运行期生效,不用重编 KMS**。

## 2. 秘密管理(全部不入库,手动 provision)

| 秘密 | 用途 | provision | 保管 |
|---|---|---|---|
| `NODE_KEY_PASSPHRASE` | 解密 BLS keystore | 部署时手动输 → tmpfs `/run/dvt/pass` | 你脑子/密码管理器,**盘上永无** |
| `ETH_PRIVATE_KEY` | operator:链上注册 + gas(须 = validator owner) | dvt.env(600)或 tmpfs | 冷保管 |
| KMS API key | KMS 认证 | `api-key generate`(hash 存 kms.db) | 发给客户端时一次性 |
| `CLOUDFLARE_TUNNEL_TOKEN` | 本社区 tunnel | tunnel config | — |
| `X402_AUTH_SECRET` | (可选)x402 HMAC | dvt.env | — |

**轮换**:BLS 密码轮换 = 用旧密码解、新密码重 `encrypt-node-key.mjs`;API key = generate 新 + revoke 旧。

## 3. 灾备(板子坏了怎么办)

- **DVT node 身份可迁移**:异地保存**加密 keystore(node_state.json)+ 密码分开**。换板重装 `deploy-dvt.sh` 时把 keystore 放回、输密码 → 同一 BLS 身份恢复(链上注册的 slot 不变)。
- **⚠️ KMS TEE 身份不可迁移**:用户私钥在 TEE 安全世界、RPMB 防回滚计数器、dirf.db **都绑当前物理板**,换板 = 换 TEE = 换 attestation key。**板坏 = 该板 TEE 内密钥没了**。
  - 用户资产不丢的保障在**合约层 social recovery**(归 airaccount-contract,不在 KMS)—— 半去中心化模型:换实例=换 rpId=换 passkey=走 social recovery 迁移(见 memory `project_decentralization_model`)。
  - 所以生产板要:UPS 防断电损坏 + 定期确认 `/RollbackCounter` 正常 + 关键用户提前完成合约层 recovery 配置。
- **keystore 备份**:`scp keystore` 到异地,`gpg` 再加一层;密码走另一渠道。

## 4. 时间同步(必须)

TA 的 challenge/attestation TTL、WebAuthn 时效都依赖正确时间(TA 用 `ree_time`)。板子须开 NTP:
```
systemctl enable --now systemd-timesyncd    # 或 chronyd
timedatectl status                          # 确认 synchronized
```
时间偏差 → challenge 过期误判 / attestation evidence 时间戳错。

## 5. 监控告警

- **DVT**:内置 ops-alert(`OPS_ALERT_*` env → Telegram);`/health` capabilities + `/node/info`。
- **KMS**:`/health`(ta_mode=real)+ `/QueueStatus`(队列深度 + 熔断器)+ `/RollbackCounter`。
- 接监控:cron 拉 `/health` + 队列深度,异常告警;熔断器 open / queue 堆积 = 板子出问题。
- 公网只走 Cloudflare Tunnel;两服务绑 127.0.0.1;SSH 仅密钥。

## 6. 部署后验收(生产板)

1. KMS `/version`:板上 `127.0.0.1:3000` **且** 公网都 = 目标版本 + `profile=prod`。
2. DVT `/health` = 1.9.0 + keystore 解密日志;`/node/info` 出独立 BLS 公钥(非仓库测试键)。
3. **KMS 完整链上 E2E**:CreateKey→DeriveAddress→Sign→链上验证(合约地址走 config)。
4. **DVT 完整链上 E2E**:`POST /node/register` 注册 → `realnode-e2e.mjs`(`E2E_ACCOUNT` = config)→ 链上 `validate=0`。
5. fail-closed 抽测:清 `/run/dvt/pass` → DVT 起不来;无 API key → KMS 拒。
6. NTP synchronized;共存 RAM headroom;`free -m` 有余。
