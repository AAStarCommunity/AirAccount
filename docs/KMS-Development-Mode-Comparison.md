# KMS 开发模式对比

**最后更新**: 2025-10-02 04:00

---

## 📊 两种开发模式对比

| 特性 | 自动化模式 | 三终端手动模式 |
|------|-----------|---------------|
| **适用场景** | 日常开发、快速测试 | 调试问题、监控日志 |
| **步骤数量** | 4 步 | 9 步 |
| **终端数量** | 1 个 | 3 个 |
| **启动时间** | ~60 秒 | ~60 秒 |
| **TA 日志** | ❌ 不可见 | ✅ Terminal 3 实时显示 |
| **CA 日志** | ❌ 不可见 | ✅ Terminal 2 实时显示 |
| **系统日志** | ❌ 不可见 | ✅ Terminal 1 实时显示 |
| **交互能力** | ❌ 无 | ✅ Terminal 2 可交互 |
| **自动化程度** | ✅ 完全自动 | ⚠️ 部分自动（步骤 8） |
| **学习曲线** | ✅ 简单 | ⚠️ 需要理解三层架构 |
| **推荐用途** | 功能开发、快速迭代 | 问题排查、性能分析 |

---

## 🚀 模式 1: 自动化模式

### 适用场景

✅ **使用此模式，当你**:
- 开发新功能
- 快速测试 API
- 不需要查看日志
- 想要最快的迭代速度

❌ **不使用此模式，当你**:
- 遇到 bug 需要调试
- 需要查看 TA 内部日志
- 想要监控签名过程
- 需要进入 QEMU 执行命令

### 完整流程

```bash
# 1. 清理
./scripts/kms-cleanup.sh

# 2. 部署
./scripts/kms-deploy.sh clean

# 3. 启动（只需 Terminal 2）
./scripts/terminal2-guest-vm.sh

# 4. 测试（等待 60 秒后）
curl http://localhost:3000/health | jq .
```

### 优点

- ⚡ **快速**: 只需 4 个命令
- 🎯 **简单**: 无需管理多个终端
- 🤖 **自动**: 一切自动完成
- 📦 **轻量**: 最小化资源占用

### 缺点

- 🔍 **无日志**: 看不到 TA/CA 实时输出
- 🚫 **无交互**: 不能进入 QEMU shell
- 🐛 **难调试**: 问题排查依赖日志文件

### 参考文档

📖 `docs/KMS-Quick-Start.md`

---

## 🔧 模式 2: 三终端手动模式

### 适用场景

✅ **使用此模式，当你**:
- 调试 TA 或 CA 代码
- 需要查看签名过程日志
- 排查 API 启动失败
- 监控 OP-TEE 行为
- 需要在 QEMU 内执行命令

❌ **不使用此模式，当你**:
- 只是快速测试功能
- 不需要深入调试
- 想要节省终端窗口

### 完整流程

```bash
# 1-2. Docker 和 Cloudflare（按需）
docker restart teaclave_dev_env  # 可选
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &  # 可选

# 3. 清理
./scripts/kms-cleanup.sh

# 4. 部署
./scripts/kms-deploy.sh clean

# 5-7. 启动三个终端
# Terminal 3
./scripts/terminal3-secure-log.sh

# Terminal 2
./scripts/terminal2-guest-vm.sh

# Terminal 1
./scripts/terminal1-qemu.sh

# 8. Terminal 2 自动执行挂载和启动
# （自动完成，但可手动验证/重启）

# 9. 测试
curl http://localhost:3000/health | jq .
```

### 优点

- 🔍 **完整日志**: 三层日志全部可见
  - Terminal 3: Secure World (TA)
  - Terminal 2: Normal World (CA)
  - Terminal 1: QEMU 系统
- 🎮 **交互式**: Terminal 2 提供 QEMU shell
- 🐛 **易调试**: 实时查看错误和警告
- 📊 **可监控**: 观察每个 API 调用的执行过程

### 缺点

- ⏱️ **复杂**: 需要管理 3 个终端
- 🪟 **占用**: 需要更多屏幕空间
- 📚 **学习**: 需要理解三层架构

### 三终端职责

#### Terminal 3: Secure World (TA) 🔒

**端口**: 54321

**查看内容**:
- OP-TEE 启动日志
- TA 加载和初始化
- 钱包创建 (`D/TA: [+] Create wallet`)
- 地址派生 (`D/TA: [+] Derive address`)
- 签名操作 (`D/TA: [+] Sign transaction`)
- **安全错误** (`E/TA:`)

**何时关注**:
- TA 加载失败
- 签名错误
- 密钥生成问题

#### Terminal 2: Normal World (CA) 🖥️

**端口**: 54320

**查看内容**:
- QEMU 登录过程
- shared 目录挂载
- TA 文件绑定挂载
- kms-api-server 启动日志
- API 请求日志
- **交互式 shell**

**何时关注**:
- API 启动失败
- 挂载问题
- 网络问题
- 需要手动操作

#### Terminal 1: QEMU 系统 🚀

**查看内容**:
- Linux 内核启动
- 文件系统挂载
- 网络初始化
- OP-TEE supplicant 启动

**何时关注**:
- QEMU 启动失败
- 系统级错误
- 一般不需要持续监控

### 步骤 8 详解

虽然 expect 脚本会**自动执行**以下操作，但保留手动命令用于：
- ✅ 验证自动化结果
- ✅ 重启 API Server
- ✅ 调试挂载问题

**自动执行的操作**:
```bash
mkdir -p shared
mount -t 9p -o trans=virtio host shared
cd shared
mount --bind ta/ /lib/optee_armtz
mount --bind plugin/ /usr/lib/tee-supplicant/plugins/
./kms-api-server > kms-api.log 2>&1 &
```

**手动重启 API**:
```bash
cd /root/shared
killall kms-api-server
./kms-api-server > kms-api.log 2>&1 &
sleep 3
wget -qO- http://127.0.0.1:3000/health
```

### 参考文档

📖 `docs/KMS-Development-Guide-Manual.md`

---

## 🎯 决策流程图

```
开始开发 KMS
    ↓
需要查看日志? ────No───→ 【自动化模式】
    │                      4 步快速开发
    │                      docs/KMS-Quick-Start.md
    Yes
    ↓
需要进入 QEMU? ───No───→ 可以用自动化模式
    │                     （日志在文件中）
    │
    Yes
    ↓
【三终端手动模式】
9 步完整监控
docs/KMS-Development-Guide-Manual.md
```

---

## 🔄 模式切换

### 从自动化模式切换到手动模式

**场景**: 自动化模式遇到问题，需要查看日志

```bash
# 1. 停止自动化模式（Ctrl+C 关闭 Terminal 2）

# 2. 清理
./scripts/kms-cleanup.sh

# 3. 启动三终端模式
./scripts/terminal3-secure-log.sh  # 新终端 3
./scripts/terminal2-guest-vm.sh    # 新终端 2
./scripts/terminal1-qemu.sh        # 新终端 1
```

### 从手动模式切换到自动化模式

**场景**: 调试完成，继续快速开发

```bash
# 1. 关闭所有三个终端（Ctrl+C）

# 2. 清理
./scripts/kms-cleanup.sh

# 3. 只启动 Terminal 2
./scripts/terminal2-guest-vm.sh
```

---

## 📋 推荐工作流

### 日常开发循环

```
开发 → 自动化模式测试 → 发现 bug → 手动模式调试 → 修复 → 自动化模式验证
  ↑                                                                    ↓
  └────────────────────────────────────────────────────────────────────┘
```

### 具体步骤

1. **功能开发** - 使用自动化模式
   ```bash
   vim kms/host/src/api_server.rs
   ./scripts/kms-cleanup.sh && ./scripts/kms-deploy.sh clean
   ./scripts/terminal2-guest-vm.sh
   ```

2. **快速测试** - 自动化模式
   ```bash
   curl http://localhost:3000/CreateKey ...
   ```

3. **发现 bug** - 切换到手动模式
   ```bash
   # Ctrl+C 停止 Terminal 2
   ./scripts/kms-cleanup.sh
   # 启动三终端
   ./scripts/terminal3-secure-log.sh
   ./scripts/terminal2-guest-vm.sh
   ./scripts/terminal1-qemu.sh
   ```

4. **查看日志调试**
   - Terminal 3: 查看 TA 日志找错误
   - Terminal 2: 查看 API 日志
   - Terminal 2: 手动重启 API 测试修复

5. **修复并验证** - 切回自动化模式
   ```bash
   vim kms/ta/src/main.rs
   ./scripts/kms-cleanup.sh && ./scripts/kms-deploy.sh clean
   ./scripts/terminal2-guest-vm.sh
   curl http://localhost:3000/CreateKey ...
   ```

---

## 🛠️ 工具和脚本

### 共用脚本

| 脚本 | 用途 | 两种模式都需要 |
|------|------|---------------|
| `kms-cleanup.sh` | 清理进程 | ✅ |
| `kms-deploy.sh` | 编译部署 | ✅ |

### 自动化模式专用

只需 `terminal2-guest-vm.sh`

### 手动模式专用

需要所有三个:
- `terminal3-secure-log.sh`
- `terminal2-guest-vm.sh`
- `terminal1-qemu.sh`

---

## 💡 最佳实践

### 自动化模式

```bash
# 修改代码后的标准流程
vim kms/host/src/api_server.rs
./scripts/kms-cleanup.sh && \
./scripts/kms-deploy.sh clean && \
./scripts/terminal2-guest-vm.sh

# 等待 60 秒后测试
sleep 60 && curl http://localhost:3000/health | jq .
```

### 手动模式

```bash
# 调试时的标准流程
./scripts/kms-cleanup.sh
./scripts/kms-deploy.sh clean

# 在三个不同的终端窗口中
./scripts/terminal3-secure-log.sh  # 终端 1
./scripts/terminal2-guest-vm.sh    # 终端 2
./scripts/terminal1-qemu.sh        # 终端 3 (按回车启动)

# 在 Terminal 2 中观察自动启动
# 如果需要重启 API:
cd /root/shared && killall kms-api-server && ./kms-api-server > kms-api.log 2>&1 &
```

---

## 📚 相关文档

1. **自动化模式**: `docs/KMS-Quick-Start.md`
2. **手动模式**: `docs/KMS-Development-Guide-Manual.md`
3. **完整工作流**: `docs/KMS-Development-Workflow.md`
4. **更新日志**: `docs/Changes.md`

---

## ❓ 常见问题

### Q1: 我应该用哪个模式？

**A**: 默认使用自动化模式。只在遇到问题需要调试时切换到手动模式。

### Q2: 手动模式的步骤 8 还需要手动操作吗？

**A**: 不需要。expect 脚本会自动执行。但文档保留了手动命令用于：
- 验证自动化结果
- 重启 API Server
- 调试挂载问题

### Q3: 可以混合使用吗？

**A**: 可以。例如：
- 启动时用手动模式查看日志
- 确认无误后关闭 Terminal 1 和 3，只保留 Terminal 2 继续开发

### Q4: 三终端模式下，Terminal 1 必须保持打开吗？

**A**: 不必须。Terminal 1 主要用于查看 QEMU 启动日志。启动成功后可以关闭，只保留 Terminal 2 和 3。

### Q5: 如何保存日志？

**手动模式**:
```bash
# Terminal 2 的日志保存在 QEMU 内
# 在 Terminal 2 中：
cat /root/shared/kms-api.log > /root/shared/api-$(date +%Y%m%d-%H%M%S).log

# 从 Mac 主机访问
docker exec teaclave_dev_env cat /opt/teaclave/shared/api-*.log
```

**自动化模式**:
```bash
# 日志在 QEMU 内，需要手动提取
docker exec teaclave_dev_env cat /opt/teaclave/shared/kms-api.log
```

---

**最后更新**: 2025-10-02 04:00

**选择你的模式，开始开发！** 🚀
