# KMS 系统启动和使用指南

最后更新: 2025-10-01 17:41

## 🚀 快速启动

### Mac 重启或首次启动后：

```bash
# 1. 启动 Docker 容器
docker start teaclave_dev_env

# 2. 启动 Cloudflare 隧道（仅需运行一次）
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &

# 3. 启动 KMS 服务（选择下面的方式 A 或 B）
```

### 方式 A: 一键自动启动 ⭐ 推荐

```bash
./scripts/kms-auto-start.sh
```

**优点**：
- 一条命令完成所有启动
- 自动等待并验证服务
- 45 秒后自动测试 API

**查看日志**：
```bash
./scripts/kms-monitor.sh
```

### 方式 B: 手动三终端启动（用于调试）

**终端 1**（Secure World 日志 - TA）：
```bash
./scripts/terminal3-secure-log.sh
```

**终端 2**（Guest VM Shell - CA）：
```bash
./scripts/terminal2-guest-vm.sh
```

**终端 3**（QEMU + API Server）：
```bash
./scripts/terminal1-qemu.sh
```

**优点**：
- 实时查看所有日志
- 适合开发和调试

## 📡 访问地址

- **本地**: http://localhost:3000
- **公网**: https://kms.aastar.io
- **健康检查**: `curl http://localhost:3000/health`

## 🔧 常用命令

### 查看系统状态
```bash
./scripts/kms-startup-guide.sh
```

### 部署新代码
```bash
./scripts/kms-deploy.sh
```

### 重启 API Server
```bash
./scripts/kms-restart-api.sh
```

### 监控日志
```bash
./scripts/kms-monitor.sh
```
选择：
1. Secure World 日志 (TA)
2. Guest VM Shell (CA)
3. QEMU 日志
4. API Server 日志
5. Cloudflared 日志

## 📝 API 使用示例

所有 POST API 都需要 AWS KMS 兼容的 HTTP 头：

### 健康检查
```bash
curl http://localhost:3000/health
```

### 创建密钥
```bash
curl -X POST http://localhost:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{
    "Description": "My test key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'
```

### 列出密钥
```bash
curl -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}'
```

### 查询密钥详情
```bash
curl -X POST http://localhost:3000/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d '{"KeyId": "your-key-id"}'
```

### 推导地址
```bash
curl -X POST http://localhost:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DeriveAddress" \
  -d '{
    "KeyId": "your-key-id",
    "AddressIndex": 0
  }'
```

### 签名
```bash
curl -X POST http://localhost:3000/Sign \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.Sign" \
  -d '{
    "KeyId": "your-key-id",
    "Message": "SGVsbG8gV29ybGQ=",
    "MessageType": "RAW",
    "SigningAlgorithm": "ECDSA_SHA_256"
  }'
```

### 获取公钥
```bash
curl -X POST http://localhost:3000/GetPublicKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.GetPublicKey" \
  -d '{"KeyId": "your-key-id"}'
```

### 删除密钥
```bash
curl -X POST http://localhost:3000/DeleteKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ScheduleKeyDeletion" \
  -d '{
    "KeyId": "your-key-id",
    "PendingWindowInDays": 7
  }'
```

## 🔍 故障排查

### 1. API 返回 Connection refused
**原因**: QEMU 或 API Server 未运行
**解决**: 运行 `./scripts/kms-auto-start.sh`

### 2. Cloudflared 错误
**检查日志**: `tail -f /tmp/cloudflared.log`
**重启**:
```bash
pkill cloudflared
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &
```

### 3. 端口被占用
脚本会自动清理端口，如果仍有问题：
```bash
docker exec teaclave_dev_env pkill -f qemu-system-aarch64
docker exec teaclave_dev_env pkill -f socat
```

### 4. 查看 QEMU 日志
```bash
docker exec teaclave_dev_env cat /tmp/qemu.log
```

## 📚 相关文档

- **完整变更日志**: `docs/Changes.md`
- **KMS 详细说明**: `kms/README.md`
- **部署指南**: `docs/Deploy.md`

## ⚙️ 开发工作流

1. **修改代码** → 编辑 `kms/host/` 或 `kms/ta/`
2. **部署** → `./scripts/kms-deploy.sh`
3. **重启 API** → `./scripts/kms-restart-api.sh`（如果部署脚本自动重启失败）
4. **测试** → `curl http://localhost:3000/...`
5. **查看日志** → `./scripts/kms-monitor.sh`

## 🎯 系统架构

```
Mac (localhost:3000)
    ↓
Docker (127.0.0.1:3000)
    ↓
QEMU Guest (0.0.0.0:3000)
    ↓
KMS API Server (Rust + Warp)
    ↓
OP-TEE TA (Secure World)
    ↓
Secure Storage
```

```
Internet (https://kms.aastar.io)
    ↓
Cloudflare Tunnel
    ↓
Mac (127.0.0.1:3000)
```
