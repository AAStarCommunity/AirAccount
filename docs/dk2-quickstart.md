# DK2 快速部署手册（Mac Mini 操作）

> 版本：v0.19.3 | 板子：STM32MP157F-DK2 | 连接：USB Ethernet 192.168.7.2

---

## 前置检查

```bash
# 确认板子已通电并连接 USB
ping -c 2 192.168.7.2

# 确认 SSH 免密
ssh root@192.168.7.2 whoami    # 应输出: root
# 如果要求密码，先执行:
# ssh-copy-id root@192.168.7.2
```

---

## Step 0：获取代码

```bash
# 如果 Mac Mini 上还没有仓库
git clone https://github.com/AAStarCommunity/AirAccount.git
cd AirAccount

# 如果已有仓库，拉最新
cd AirAccount
git checkout main && git pull origin main
```

---

## Step 1：获取 TA Dev Kit

TA Dev Kit 必须和板子上的 OP-TEE 版本匹配，从板子上提取最安全。

```bash
# 在板子上找 export-ta_arm32 路径
ssh root@192.168.7.2 "find / -name ta_dev_kit.mk 2>/dev/null"
# 示例输出: /usr/include/optee/export-ta_arm32/mk/ta_dev_kit.mk

# 创建目录并修正权限（避免 scp Permission denied）
sudo mkdir -p /opt/dk2-ta-dev-kit
sudo chown "$(whoami)" /opt/dk2-ta-dev-kit

# 拷回 Mac Mini（替换成你找到的路径）
scp -r root@192.168.7.2:/usr/include/optee/export-ta_arm32 /opt/dk2-ta-dev-kit/

# 验证
ls /opt/dk2-ta-dev-kit/export-ta_arm32/mk/ta_dev_kit.mk
```

---

## Step 2：启动 Docker

```bash
open -a Docker   # 启动 Docker Desktop
# 等待状态栏 Docker 图标变实心（约 30 秒）
docker info      # 不报错说明已就绪
```

---

## Step 3：编译

```bash
cd AirAccount

export DK2_TA_DEV_KIT_DIR=/opt/dk2-ta-dev-kit/export-ta_arm32

./scripts/dk2-build.sh
```

**首次运行约 10–15 分钟**（构建 Docker 镜像 + 交叉编译）。完成后看到：

```
[ok] Build complete. Artifacts in build/dk2/:
  4319f351-0b24-4097-b659-80ee4f824cdd.ta
  kms-api-server
```

---

## Step 4：部署

### 首次部署（安装 systemd 服务）

```bash
DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh --first-run
```

### 后续更新

```bash
DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh
```

成功输出：

```
[ok] Deploy successful!
  Health: {"status":"healthy","service":"kms-api","version":"0.19.0",...}
  Logs:   ssh root@192.168.7.2 'journalctl -u kms-api-server -f'
```

---

## Step 5：验证

```bash
# 健康检查（主要验证手段）
curl http://192.168.7.2:3000/health
# 期望: {"status":"healthy","service":"kms-api","version":"0.19.0",...}

# 查看服务日志
ssh root@192.168.7.2 'journalctl -u kms-api-server -n 50 --no-pager'

# 查看 TEE 调用次数（确认 TA 在工作）
ssh root@192.168.7.2 'cat /sys/kernel/debug/optee/call_count'
```

> **完整 API 测试**（CreateKey 等接口需要先完成 WebAuthn 注册，有 PasskeyPublicKey 才能调用）：
> ```bash
> # 在板子上运行完整测试脚本
> ssh root@192.168.7.2
> cd /path/to/AirAccount && bash scripts/test-kms-api-simple.sh
> ```
> 详见 `docs/dk2-deployment-guide.md`。

---

## 故障排查

| 问题 | 排查命令 |
|------|---------|
| SSH 连不上 | `ping 192.168.7.2`，检查 USB 线 |
| TA Dev Kit 找不到 | `ssh root@192.168.7.2 "find / -name ta_dev_kit.mk 2>/dev/null"` |
| Docker 构建失败 | `./scripts/dk2-build.sh clean` 后重试 |
| 健康检查失败 | `ssh root@192.168.7.2 'journalctl -u kms-api-server -n 30 --no-pager'` |
| tee-supplicant 未运行 | `ssh root@192.168.7.2 'systemctl status tee-supplicant'` |

---

## 完成后

部署成功即完成 v0.19.3 的 DK2 硬件验证。下一步等 i.MX93 到货，流程相同但目标架构改为 `aarch64`，参考 `docs/migration-to-MX95.md`。
