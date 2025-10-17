# KMS 系统重启完整指南

## 🚀 快速启动（推荐）

**前提条件**: Docker 容器 `teaclave_dev_env` 正在运行

### 一键启动脚本（最简单）

```bash
# 停止旧的 QEMU（如果有）
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 || true

# 停止并重启 expect 监听脚本
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell || true
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"

# 等待监听器启动
sleep 3

# 启动 QEMU（带 3000 端口转发）
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"

# 等待 QEMU 和 API Server 自动启动（45 秒）
echo "⏳ 等待 QEMU 启动和 API Server 自动启动（45 秒）..."
sleep 45

# 验证
echo "✅ 验证端口转发配置..."
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*"

echo "✅ 测试 Mac 本地访问..."
curl http://localhost:3000/health

echo "✅ 测试公网访问（需要 cloudflared 运行）..."
curl https://kms.aastar.io/health
```

### 工作原理

1. **expect 脚本自动化**：
   - 监听 QEMU 的 54320 端口
   - 自动登录（root）
   - 自动挂载共享目录到 `/root/shared`
   - 自动绑定 TA 目录到 `/lib/optee_armtz`
   - **自动启动 kms-api-server**

2. **端口转发**：
   - QEMU 内 `0.0.0.0:3000` → Docker 内 `3000`
   - Docker 端口映射 `3000:3000` → Mac `localhost:3000`

3. **无需手动操作**：
   - 不需要手动复制 TA 文件
   - 不需要手动启动 API Server
   - 一切都是自动的！

---

## 📋 分步启动流程（了解细节）

### 步骤 1: 启动 expect 监听脚本

```bash
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
```

这会在后台启动，等待 QEMU 连接到 54320 端口。

---

### 步骤 2: 启动 QEMU（带 3000 端口转发）

```bash
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"
```

**重要**：QEMU 配置了正确的端口转发：
- `hostfwd=:127.0.0.1:54433-:4433` (HTTPS)
- `hostfwd=tcp:0.0.0.0:3000-:3000` (KMS API) ✅

---

### 步骤 3: 等待自动化完成

expect 脚本会自动完成以下操作：

1. 等待 QEMU 启动（约 30 秒）
2. 自动登录 root 用户
3. 挂载共享目录到 `/root/shared`
4. 绑定 TA 目录到 `/lib/optee_armtz`
5. **启动 kms-api-server**

**等待时间**: 约 45 秒

---

### 步骤 4: 验证系统状态

```bash
# 1. 检查 QEMU 端口转发配置
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*"
# 应该看到: hostfwd=:127.0.0.1:54433-:4433,hostfwd=tcp:0.0.0.0:3000-:3000

# 2. 测试 Mac 本地访问
curl http://localhost:3000/health
# 应该返回 JSON 响应

# 3. （可选）测试公网访问
curl https://kms.aastar.io/health
```

---

## 🔍 验证端口转发配置

检查 QEMU 是否有正确的端口转发：

```bash
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*"
```

**应该看到**：
```
hostfwd=:127.0.0.1:54433-:4433,hostfwd=tcp:0.0.0.0:3000-:3000
```

如果只有 `hostfwd=:127.0.0.1:54433-:4433`，说明 QEMU 使用了错误的配置，需要重新启动。

---

## 🐛 常见问题排查

### 问题 1: `curl http://localhost:3000/health` 返回 "Connection reset"

**原因**：QEMU 端口转发配置缺少 3000 端口。

**解决**：
1. 停止 QEMU：在 QEMU 内执行 `poweroff`
2. 重新执行步骤 2

---

### 问题 2: 公网返回 502 或 1033 错误

**原因**：cloudflared 无法连接到 localhost:3000。

**检查**：
```bash
# 测试 Mac 本地
curl http://localhost:3000/health

# 如果失败，说明端口转发有问题，回到问题 1
```

---

### 问题 3: QEMU 无法启动

**检查**：
```bash
docker exec teaclave_dev_env cat /tmp/qemu.log
```

如果看到 "Permission denied" 或其他错误，可能是旧的脚本问题。

**解决**：手动启动 QEMU（见下面的"手动启动方法"）。

---

## 🛠️ 手动启动 QEMU 方法（备用）

如果脚本不工作，使用这个命令手动启动 QEMU：

```bash
docker exec -it teaclave_dev_env bash -c "
cd /opt/teaclave/images/x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory && \
IMG_DIRECTORY=/opt/teaclave/images \
IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory \
QEMU_HOST_SHARE_DIR=/opt/teaclave/shared \
./qemu-system-aarch64 \
  -nodefaults \
  -nographic \
  -serial tcp:localhost:54320 \
  -serial tcp:localhost:54321 \
  -smp 2 \
  -s -machine virt,secure=on,acpi=off,gic-version=3 \
  -cpu cortex-a57 \
  -d unimp -semihosting-config enable=on,target=native \
  -m 1057 \
  -bios bl1.bin \
  -initrd rootfs.cpio.gz \
  -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' \
  -kernel Image \
  -fsdev local,id=fsdev0,path=/opt/teaclave/shared,security_model=none \
  -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
  -netdev user,id=vmnic,hostfwd=:127.0.0.1:54433-:4433,hostfwd=tcp:0.0.0.0:3000-:3000 \
  -device virtio-net-device,netdev=vmnic
"
```

---

## 📊 快速状态检查

```bash
./scripts/kms-status.sh
```

会显示：
- Docker 容器状态
- QEMU 进程状态
- 端口转发配置
- API Server 状态（Docker 内、Mac 本地、公网）
- cloudflared 状态

---

## ⚡ 快速重启命令（全部一起）

```bash
# 停止所有服务
docker exec teaclave_dev_env bash -c "poweroff" 2>/dev/null &
pkill cloudflared
sleep 5

# 重新启动
./scripts/terminal2-guest-vm.sh &
sleep 3
./scripts/kms-qemu-terminal1.sh
# 在 QEMU 内: cp *.ta /lib/optee_armtz/ && ./kms-api-server > kms-api.log 2>&1 &
# Ctrl+C 退出

# 启动 cloudflared
cloudflared tunnel run kms-tunnel &

# 测试
curl https://kms.aastar.io/health
```

---

## 🔑 关键知识点

1. **Terminal2 不会启动 QEMU**：它只是监听 54320 端口，等待 QEMU 连接
2. **Terminal1 启动 QEMU**：使用修复后的脚本，包含 3000 端口转发
3. **端口转发必须是 0.0.0.0:3000**：不能是 127.0.0.1:3000
4. **TA 文件必须复制到 /lib/optee_armtz/**：否则 API 会返回 0xffff0008 错误
5. **cloudflared 在 Mac 上运行**：连接到 Mac 的 localhost:3000

---

## ⚡ 创建一键启动脚本

为了方便使用，可以创建一个脚本：

**文件**: `scripts/kms-auto-start.sh`

```bash
#!/bin/bash
# KMS 完全自动启动脚本

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}🔄 停止旧的 QEMU...${NC}"
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 || true
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell || true
sleep 2

echo -e "${GREEN}🚀 启动 expect 监听脚本...${NC}"
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
sleep 3

echo -e "${GREEN}🖥️  启动 QEMU（带 3000 端口转发）...${NC}"
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"

echo -e "${YELLOW}⏳ 等待 45 秒让 QEMU 和 API Server 启动...${NC}"
sleep 45

echo -e "${GREEN}✅ 验证端口转发配置...${NC}"
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*"

echo ""
echo -e "${GREEN}✅ 测试 Mac 本地访问...${NC}"
curl -s http://localhost:3000/health | jq .

echo ""
echo -e "${GREEN}✅ 所有服务已启动！${NC}"
echo -e "   Mac 本地: ${GREEN}http://localhost:3000${NC}"
echo -e "   公网访问: ${GREEN}https://kms.aastar.io${NC}"
```

**使用方法**：
```bash
chmod +x scripts/kms-auto-start.sh
./scripts/kms-auto-start.sh
```

---

*最后更新: 2025-10-01 17:30*
