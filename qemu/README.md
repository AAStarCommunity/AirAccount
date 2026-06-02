# AirAccount QEMU 开发工具链

本目录是 AirAccount KMS 的**完整本地开发与测试环境**，基于 QEMU 模拟 OP-TEE TrustZone。

所有新功能在真实硬件（i.MX 95）部署前，**必须先在此环境通过 regression 测试**。

---

## 快速开始

```bash
# 1. 首次初始化（约 15-30 分钟，主要是拉 Docker 镜像）
make -C qemu setup

# 2. 构建 TA + CA
make -C qemu build

# 3. 启动 QEMU（新开 tmux 窗口或在同一终端）
make -C qemu start

# 4. 部署到 QEMU guest（QEMU 启动后等待 buildroot login 出现再执行）
make -C qemu deploy

# 5. 验证
curl http://localhost:3000/health

# 6. 运行完整测试
make -C qemu test
```

---

## 环境架构

```
macOS Apple Silicon (arm64)
  └── OrbStack (Docker)
        └── x86_64 容器 [--platform linux/amd64, Rosetta]
              └── QEMU qemu-system-aarch64
                    ├── Normal World: Linux (aarch64, cortex-a57)
                    │     └── kms-api-server  [CA]
                    │           ↕ port 3000
                    │     mac:3000 ←──────────────── Cloudflare Tunnel
                    └── Secure World: OP-TEE 4.5.0
                          └── AirAccount TA (UUID: 4319f351-...)  [TA]
```

**注意（Apple Silicon）**：QEMU 在 Rosetta 模拟的 x86_64 容器内运行，速度比原生 QEMU 慢约 2-3 倍。这是开发环境的限制，不影响功能验证。生产部署使用 i.MX 95 真实硬件。

---

## 目录说明

```
qemu/
├── Makefile      # 顶层入口（make -C qemu <目标>）
├── README.md     # 本文档
├── setup.sh      # 一次性初始化
├── build.sh      # 构建 TA + CA
├── start.sh      # 启动 QEMU（tmux）
├── deploy.sh     # 部署到 QEMU guest
├── test.sh       # 集成测试套件
├── stop.sh       # 停止 QEMU
├── status.sh     # 显示当前状态
└── lib/
    └── log.sh    # 日志工具函数
```

---

## 目录结构（运行时）

| 路径 | 说明 |
|------|------|
| `/opt/teaclave/shared/` | 9p 共享目录（Mac ↔ QEMU guest） |
| `/opt/teaclave/shared/ta/` | TA 二进制（.ta 文件） |
| `/opt/teaclave/shared/kms-api-server` | CA 二进制 |
| `/opt/teaclave/images/x86_64-optee-qemuv8-*/` | QEMU 镜像（bl1.bin, Image, rootfs） |
| `[guest] /data/kms/kms.db` | SQLite 数据库（必须在 guest 本地磁盘） |
| `[guest] /lib/optee_armtz/` | TA 加载路径（tee-supplicant 监听） |

---

## 测试阶段

| 阶段 | 内容 | 命令 |
|------|------|------|
| P0 | 健康检查（/health, /version） | `make -C qemu test p0` |
| P1 | 密钥生命周期（CreateKey/Sign/Delete） | `make -C qemu test p1` |
| P2 | WebAuthn 流程（需要 FIDO2 设备） | `make -C qemu test p2` |
| P3 | v0.19.0 新功能回归 | `make -C qemu test p3` |
| P4 | 安全负向测试（无 auth 拒绝等） | `make -C qemu test p4` |
| 快速回归 | P0+P1+P3+P4 | `make -C qemu regression` |
| 全套 | P0-P4 | `make -C qemu test` |

---

## 常见问题

### SQLite 不能放在 9p 挂载目录
```bash
# ❌ 错误
KMS_DB_PATH=/root/shared/kms.db ./kms-api-server

# ✅ 正确（guest 本地磁盘）
KMS_DB_PATH=/data/kms/kms.db ./kms-api-server
```

### 端口 54320/54321 被占用
```bash
docker exec teaclave_dev_env bash -c "kill -9 \$(lsof -ti:54320) 2>/dev/null; true"
docker exec teaclave_dev_env bash -c "kill -9 \$(lsof -ti:54321) 2>/dev/null; true"
```

### TA 更新后未生效
```bash
make -C qemu deploy-ta   # 重新加载 TA + 重启 tee-supplicant
```

### 查看 OP-TEE 日志
在 QEMU 运行时，Secure World 日志输出到 tmux 右下窗格（port 54321）。
也可以用：`socat TCP:localhost:54321 -`

---

## 与 i.MX 95 部署的关系

QEMU 验证通过后，部署到真实硬件：
```bash
# 构建产物完全相同（aarch64），直接 scp
scp /opt/teaclave/shared/ta/4319f351-*.ta root@imx95:/lib/optee_armtz/
scp /opt/teaclave/shared/kms-api-server   root@imx95:/usr/bin/
ssh root@imx95 "systemctl restart tee-supplicant kms-api-server"
```

详见：`docs/hardware-imx95-fetmx9596c.md`

---

## 开发工作流

```
代码修改
  ↓
make -C qemu build        # 构建
  ↓
make -C qemu deploy       # 部署到 QEMU
  ↓
make -C qemu regression   # 快速回归测试
  ↓（通过后）
git commit && git push    # 提交
  ↓（PR merge 后）
make -C qemu test         # 完整测试
  ↓（通过后）
scp → i.MX 95 硬件        # 生产部署
```
