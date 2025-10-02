# KMS 快速开发指南 ⚡

> **最简流程**: 修改代码 → 4 个命令 → 完成

---

## 🚀 超简流程（4步）

```bash
# 1. 清理
./scripts/kms-cleanup.sh

# 2. 部署（编译 + 复制）
./scripts/kms-deploy.sh clean

# 3. 启动（只需 Terminal 2！）
./scripts/terminal2-guest-vm.sh

# 4. 测试
curl http://localhost:3000/health | jq .
```

---

## 📖 详细说明

### 步骤 1: 清理旧进程
```bash
./scripts/kms-cleanup.sh
```
- 停止 QEMU
- 停止 socat 监听器
- 清理僵尸进程

### 步骤 2: 编译部署
```bash
# 增量构建（快）
./scripts/kms-deploy.sh

# 完全重建（改代码后推荐）
./scripts/kms-deploy.sh clean
```

自动执行：
- ✅ 同步代码到 SDK
- ✅ 编译 TA + Host
- ✅ 部署到 `/opt/teaclave/shared/ta/`

### 步骤 3: 启动服务

**只需运行一个命令**:
```bash
./scripts/terminal2-guest-vm.sh
```

**它会自动**:
1. 启动端口监听器（54320）
2. 等待 QEMU 连接
3. 自动登录 QEMU
4. 挂载 shared 目录
5. 挂载 TA 文件
6. 启动 kms-api-server

**等待时间**: 约 60 秒（观察终端输出）

### 步骤 4: 测试

```bash
# 健康检查
curl http://localhost:3000/health | jq .

# 创建钱包
curl -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{
    "Description": "Test",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }' | jq .
```

---

## 🔧 常见问题

### Q: API 不响应？
```bash
# 等待更长时间（最多 90 秒）
# 或检查进程
docker exec teaclave_dev_env ps aux | grep qemu
docker exec teaclave_dev_env ps aux | grep kms-api
```

### Q: 返回旧版本 API？
```bash
# 重新部署
./scripts/kms-cleanup.sh
./scripts/kms-deploy.sh clean
# 重启 Terminal 2
```

### Q: 端口占用？
```bash
# 先清理，如果还有问题就重启 Docker
./scripts/kms-cleanup.sh
# 或
docker restart teaclave_dev_env
```

---

## 🎯 完整测试流程

```bash
# 1. 创建第一个地址
curl -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{"Description":"Wallet 1","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' \
  | jq '.KeyMetadata | {KeyId, Address, DerivationPath}'

# 保存返回的 KeyId，例如：48c8d60e-0134-4488-926a-5521accb9e14

# 2. 创建第二个地址（同一钱包）
curl -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{"KeyId":"48c8d60e-0134-4488-926a-5521accb9e14","Description":"Wallet 1 Address 2","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' \
  | jq '.KeyMetadata | {KeyId, Address, DerivationPath}'

# 预期：
# 地址 1: m/44'/60'/0'/0/0
# 地址 2: m/44'/60'/0'/0/1  ✅

# 3. 使用 Address 签名（无需 DerivationPath）
curl -X POST http://localhost:3000/Sign \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d '{"Address":"0xad365342c8ee4a951251c10fff8f840cbdf1dd4e","Message":"Hello"}' \
  | jq .
```

---

## 🌐 公网访问（可选）

```bash
# 启动 Cloudflare tunnel
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &

# 测试公网
curl https://kms.aastar.io/health | jq .
```

---

## ⚡ 一键脚本（终极简化）

创建 `scripts/dev.sh`:
```bash
#!/bin/bash
./scripts/kms-cleanup.sh
./scripts/kms-deploy.sh clean
./scripts/terminal2-guest-vm.sh &
sleep 60
curl http://localhost:3000/health | jq .
```

使用：
```bash
./scripts/dev.sh
```

---

## 📝 修改代码后

```bash
# 修改任何代码后，重新运行 4 步流程
./scripts/kms-cleanup.sh && \
./scripts/kms-deploy.sh clean && \
./scripts/terminal2-guest-vm.sh
```

---

## 🎯 关键点总结

✅ **只需要 Terminal 2** - 其他都自动化
✅ **步骤 8 已废弃** - expect 脚本自动挂载和启动
✅ **等待 60 秒** - QEMU 启动需要时间
✅ **重新部署用 clean** - 修改代码后必须完全重建

---

**下次开发直接运行这 4 个命令即可！** 🎉
