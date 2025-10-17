# KMS 开发文档导航

**最后更新**: 2025-10-02 04:01

---

## 🚀 快速开始

**我是新手，想快速开始**:
👉 [`KMS-Quick-Start.md`](./KMS-Quick-Start.md) - 4 步开始开发

**我需要查看日志调试**:
👉 [`KMS-Development-Guide-Manual.md`](./KMS-Development-Guide-Manual.md) - 三终端完整监控

**我想了解两种模式的区别**:
👉 [`KMS-Development-Mode-Comparison.md`](./KMS-Development-Mode-Comparison.md) - 模式对比

---

## 📚 文档列表

### 核心开发文档

| 文档 | 用途 | 推荐阅读顺序 |
|------|------|------------|
| [`KMS-Quick-Start.md`](./KMS-Quick-Start.md) | 快速开始（自动化模式） | ⭐ 第一次阅读 |
| [`KMS-Development-Mode-Comparison.md`](./KMS-Development-Mode-Comparison.md) | 两种开发模式对比 | 🔍 遇到问题时 |
| [`KMS-Development-Guide-Manual.md`](./KMS-Development-Guide-Manual.md) | 三终端手动模式（调试） | 🐛 调试时阅读 |
| [`KMS-Development-Workflow.md`](./KMS-Development-Workflow.md) | 完整开发流程和经验总结 | 📖 深入理解 |

### 设计和变更文档

| 文档 | 用途 |
|------|------|
| [`KMS-Wallet-Address-Management-Design.md`](./KMS-Wallet-Address-Management-Design.md) | 钱包地址管理系统设计文档 |
| [`Changes.md`](./Changes.md) | 项目变更日志 |

---

## 🎯 我应该看哪个文档？

### 场景 1: 第一次开发 KMS

```
1. 阅读: KMS-Quick-Start.md
2. 执行 4 个命令
3. 成功运行后继续开发
```

### 场景 2: API 不响应，需要调试

```
1. 阅读: KMS-Development-Mode-Comparison.md (了解两种模式)
2. 阅读: KMS-Development-Guide-Manual.md (学习手动模式)
3. 切换到三终端模式
4. 查看日志排查问题
```

### 场景 3: 想要深入理解架构

```
1. 阅读: KMS-Wallet-Address-Management-Design.md (设计)
2. 阅读: KMS-Development-Workflow.md (完整流程)
3. 阅读: Changes.md (了解历史演进)
```

### 场景 4: 快速查询命令

**自动化模式**:
```bash
./scripts/kms-cleanup.sh && \
./scripts/kms-deploy.sh clean && \
./scripts/terminal2-guest-vm.sh
```

**手动模式**:
```bash
./scripts/kms-cleanup.sh
./scripts/kms-deploy.sh clean
./scripts/terminal3-secure-log.sh  # 终端 1
./scripts/terminal2-guest-vm.sh    # 终端 2
./scripts/terminal1-qemu.sh        # 终端 3
```

---

## 📊 开发模式速览

| 特性 | 自动化模式 | 手动模式 |
|------|----------|---------|
| 步骤 | 4 步 | 9 步 |
| 终端 | 1 个 | 3 个 |
| 日志 | ❌ | ✅ 完整 |
| 交互 | ❌ | ✅ Shell |
| 速度 | ⚡ 快 | 🐢 慢 |
| 适用 | 日常开发 | 调试 |

**选择建议**: 默认用自动化，遇到问题切手动。

---

## 🛠️ 核心脚本

| 脚本 | 用途 | 何时使用 |
|------|------|---------|
| `kms-cleanup.sh` | 清理进程 | 每次重启前 |
| `kms-deploy.sh` | 编译部署 | 修改代码后 |
| `kms-deploy.sh clean` | 完全重建 | 修改代码后（推荐） |
| `terminal3-secure-log.sh` | TA 日志 | 手动模式 |
| `terminal2-guest-vm.sh` | CA 日志 + Shell | 两种模式都需要 |
| `terminal1-qemu.sh` | QEMU 日志 | 手动模式 |

---

## 🎓 学习路径

### 新手路径（推荐）

```
Day 1: 快速开始
  ├─ 阅读 KMS-Quick-Start.md
  ├─ 运行 4 步流程
  └─ 测试基本 API

Day 2: 理解架构
  ├─ 阅读 KMS-Development-Workflow.md
  ├─ 了解成功经验和常见问题
  └─ 尝试创建钱包和签名

Day 3: 深入调试
  ├─ 阅读 KMS-Development-Mode-Comparison.md
  ├─ 阅读 KMS-Development-Guide-Manual.md
  ├─ 尝试三终端模式
  └─ 观察日志理解执行流程

Day 4+: 高级开发
  └─ 阅读 KMS-Wallet-Address-Management-Design.md
```

### 有经验开发者

```
1. 快速浏览 KMS-Quick-Start.md (10分钟)
2. 运行自动化模式测试 (5分钟)
3. 需要时参考 KMS-Development-Guide-Manual.md
```

---

## 🔍 快速查询

### 我想...

**...快速测试 API**
→ `KMS-Quick-Start.md`

**...查看 TA 内部日志**
→ `KMS-Development-Guide-Manual.md` → Terminal 3

**...进入 QEMU 执行命令**
→ `KMS-Development-Guide-Manual.md` → Terminal 2 交互模式

**...了解地址自动递增原理**
→ `KMS-Wallet-Address-Management-Design.md`

**...查看历史修改记录**
→ `Changes.md`

**...排查部署失败问题**
→ `KMS-Development-Workflow.md` → 常见问题排查

**...重启 API Server**
→ `KMS-Development-Guide-Manual.md` → 步骤 8.3

**...验证新功能是否部署**
→ `curl http://localhost:3000/health | jq .endpoints`

---

## ✅ 检查清单

### 第一次使用

- [ ] 已阅读 `KMS-Quick-Start.md`
- [ ] Docker 已启动
- [ ] 成功运行 `./scripts/kms-cleanup.sh`
- [ ] 成功运行 `./scripts/kms-deploy.sh clean`
- [ ] 成功启动 Terminal 2
- [ ] API 响应正常（`curl http://localhost:3000/health`）

### 日常开发

- [ ] 修改代码前 git commit
- [ ] 运行 `kms-cleanup.sh`
- [ ] 运行 `kms-deploy.sh clean`
- [ ] 启动服务（自动化或手动模式）
- [ ] 测试功能
- [ ] 更新文档（如有 API 变化）

### 遇到问题

- [ ] 检查 Docker 是否运行
- [ ] 查看 `Changes.md` 是否有类似问题
- [ ] 切换到手动模式查看日志
- [ ] 参考 `KMS-Development-Workflow.md` 的问题排查部分
- [ ] 检查 `/opt/teaclave/shared/ta/` 文件时间戳

---

## 📞 获取帮助

### 按优先级

1. **查看文档**: 本目录下的 `KMS-*.md` 文件
2. **查看日志**:
   - 自动化模式: `docker exec teaclave_dev_env cat /opt/teaclave/shared/kms-api.log`
   - 手动模式: 直接看 Terminal 2 和 3
3. **查看历史**: `Changes.md` 查看类似问题的解决方案
4. **重现问题**: 使用手动模式重现并记录完整日志

---

## 🎯 快速决策树

```
需要开发 KMS
    ↓
有经验? ──Yes──→ 用自动化模式（KMS-Quick-Start.md）
    │
    No
    ↓
第一次使用? ──Yes──→ 从 KMS-Quick-Start.md 开始
    │
    No
    ↓
遇到问题? ──Yes──→ 看 KMS-Development-Mode-Comparison.md
    │                 然后切换到手动模式
    No
    ↓
想深入理解? ──Yes──→ 阅读 KMS-Development-Workflow.md
                      和 KMS-Wallet-Address-Management-Design.md
```

---

## 🔗 文档关系图

```
KMS-README.md (本文档)
    │
    ├─→ KMS-Quick-Start.md (自动化模式，新手入口)
    │       │
    │       └─→ 遇到问题 → KMS-Development-Mode-Comparison.md
    │
    ├─→ KMS-Development-Mode-Comparison.md (模式对比)
    │       │
    │       ├─→ 选择自动化 → KMS-Quick-Start.md
    │       └─→ 选择手动 → KMS-Development-Guide-Manual.md
    │
    ├─→ KMS-Development-Guide-Manual.md (三终端手动模式)
    │       │
    │       └─→ 深入理解 → KMS-Development-Workflow.md
    │
    ├─→ KMS-Development-Workflow.md (完整流程 + 经验总结)
    │
    ├─→ KMS-Wallet-Address-Management-Design.md (设计文档)
    │
    └─→ Changes.md (变更日志)
```

---

## 📝 贡献指南

修改文档时请：
1. 更新对应文档的"最后更新"时间
2. 更新本 README 的对应部分
3. 在 `Changes.md` 添加变更记录

---

**最后更新**: 2025-10-02 04:01

**从这里开始你的 KMS 开发之旅！** 🚀
