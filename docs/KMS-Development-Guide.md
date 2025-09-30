# KMS 开发、部署和测试指南

## 网络架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│ Mac Host (Your Development Machine)                                 │
│                                                                      │
│  1. 编辑代码: /AirAccount/kms/                                       │
│     ├── host/src/api_server.rs                                      │
│     ├── host/src/ta_client.rs                                       │
│     └── ta/src/lib.rs                                               │
│                                                                      │
│  2. Docker Container: teaclave_dev_env (-p 3000:3000)               │
│     ┌──────────────────────────────────────────────────────────┐   │
│     │ Volume Mount: /root/teaclave_sdk_src                      │   │
│     │ (指向 Mac 的 third_party/teaclave-trustzone-sdk)          │   │
│     │                                                            │   │
│     │ 3. 编译环境                                                │   │
│     │    make (在 /root/teaclave_sdk_src/projects/web3/kms)    │   │
│     │    ↓                                                       │   │
│     │    产物存放: /opt/teaclave/shared/                        │   │
│     │              ├── kms-api-server (CA binary)               │   │
│     │              └── *.ta (TA binary)                          │   │
│     │                                                            │   │
│     │ 4. QEMU (ARM64 + OP-TEE)                                  │   │
│     │    ┌────────────────────────────────────────────────┐    │   │
│     │    │ Guest VM (Ubuntu 24.04 ARM64)                  │    │   │
│     │    │                                                 │    │   │
│     │    │ 挂载点: /root/shared (9p virtio)               │    │   │
│     │    │         ↑                                       │    │   │
│     │    │         └─ 映射到 /opt/teaclave/shared         │    │   │
│     │    │                                                 │    │   │
│     │    │ 5. TA 部署                                      │    │   │
│     │    │    cp /root/shared/*.ta /lib/optee_armtz/      │    │   │
│     │    │                                                 │    │   │
│     │    │ 6. KMS API Server (监听 0.0.0.0:3000)          │    │   │
│     │    │    ┌─────────────────────────────────────┐     │    │   │
│     │    │    │ Normal World (CA)                   │     │    │   │
│     │    │    │  - HTTP Server (Actix-web)          │     │    │   │
│     │    │    │  - ta_client.rs (TEEC API)          │     │    │   │
│     │    │    │         ↓                            │     │    │   │
│     │    │    │    OP-TEE Client API                │     │    │   │
│     │    │    └──────────────┬──────────────────────┘     │    │   │
│     │    │                   ↓                             │    │   │
│     │    │    ┌─────────────────────────────────────┐     │    │   │
│     │    │    │ Secure World (TA)                   │     │    │   │
│     │    │    │  - eth_wallet TA                    │     │    │   │
│     │    │    │  - UUID: 4319f351-...               │     │    │   │
│     │    │    │  - Key Management + Signing         │     │    │   │
│     │    │    └─────────────────────────────────────┘     │    │   │
│     │    │                                                 │    │   │
│     │    │ 端口转发: Guest:3000 → Docker:3000             │    │   │
│     │    └────────────────────────────────────────────────┘    │   │
│     │                                                            │   │
│     │ 7. Cloudflared (运行在 Docker 内)                         │   │
│     │    连接: localhost:3000 → kms.aastar.io                   │   │
│     └──────────────────────────────────────────────────────────┘   │
│                                                                      │
│  Docker 端口映射: 容器3000 → Mac 3000 (目前 Mac 端无法访问，         │
│                   因为 Docker for Mac 限制)                         │
└─────────────────────────────────────────────────────────────────────┘
                              ↓
                    Cloudflare Tunnel
                              ↓
                   🌐 https://kms.aastar.io
```

## 开发流程

### 1. 启动开发环境

```bash
# 首次启动或重启容器
./scripts/kms-dev-env.sh start

# 查看容器状态
./scripts/kms-dev-env.sh status

# 进入容器调试（如需要）
./scripts/kms-dev-env.sh shell
```

**注意**：容器使用 volume mount，Mac 上修改代码会立即反映到容器内。

### 2. 修改代码

在 Mac 上直接编辑：

```bash
# API Server 代码
vim kms/host/src/api_server.rs

# TA Client 代码
vim kms/host/src/ta_client.rs

# TA 代码（慎重修改！）
vim kms/ta/src/lib.rs
```

**重要提醒**：
- ✅ 优先修改 `api_server.rs` 和 `ta_client.rs`
- ⚠️ 尽量不修改 `ta/src/lib.rs`（TA 代码）
- 📝 保持 CA-TA 交互机制不变

### 3. 编译项目

```bash
# 方案 A: 在 Mac 上执行（推荐）
./scripts/kms-dev-env.sh build

# 方案 B: 进入容器手动编译
./scripts/kms-dev-env.sh shell
# 进入后：
cd projects/web3/kms
make clean && make
```

编译产物自动生成到：
- **Host binary**: `host/target/aarch64-unknown-linux-gnu/release/kms-api-server`
- **TA binary**: `ta/target/aarch64-unknown-optee/release/*.ta`

### 4. 同步到 QEMU 共享目录

```bash
./scripts/kms-dev-env.sh sync
```

这会将编译产物复制到 `/opt/teaclave/shared/`，QEMU Guest VM 可以访问。

## 部署流程

### 方案 A: 本地测试（不发布到公网）

```bash
# 1. 确保容器和 QEMU 运行
./scripts/kms-dev-env.sh status

# 2. 启动 QEMU（如果未运行）
docker exec -d teaclave_dev_env bash -l -c \
  "cd /opt/teaclave/images/x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory && \
   ./qemu-system-aarch64 -nodefaults -nographic \
   -serial tcp:localhost:54320,server,nowait \
   -serial tcp:localhost:54321,server,nowait \
   -smp 2 -s -machine virt,secure=on,acpi=off,gic-version=3 \
   -cpu cortex-a57 -d unimp -semihosting-config enable=on,target=native \
   -m 1057 -bios bl1.bin -initrd rootfs.cpio.gz \
   -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' \
   -kernel Image \
   -fsdev local,id=fsdev0,path=/opt/teaclave/shared,security_model=none \
   -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
   -netdev user,id=vmnic,hostfwd=:127.0.0.1:54433-:4433,hostfwd=tcp:127.0.0.1:3000-:3000 \
   -device virtio-net-device,netdev=vmnic > /tmp/qemu.log 2>&1"

# 3. 等待 QEMU 启动
sleep 10

# 4. 在 QEMU 中部署并启动 KMS
docker exec teaclave_dev_env bash -l -c "
(
echo 'root'
sleep 2
echo ''
sleep 2
echo 'mkdir -p /root/shared'
sleep 1
echo 'mount -t 9p -o trans=virtio host /root/shared'
sleep 2
echo 'cp /root/shared/*.ta /lib/optee_armtz/'
sleep 1
echo 'cd /root/shared'
sleep 1
echo 'killall kms-api-server 2>/dev/null || true'
sleep 1
echo './kms-api-server > /tmp/kms.log 2>&1 &'
sleep 3
) | socat - TCP:localhost:54320
"

# 5. 测试（Docker 内部访问）
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health
```

### 方案 B: 完整发布到公网（推荐，已封装）

```bash
# 一键发布脚本（尚未完成，需要手动步骤）
# ./scripts/publish-kms-complete.sh

# 当前需要手动执行：
# 1. 启动容器和 QEMU（参考方案 A 的步骤 1-4）

# 2. 启动 cloudflared
docker exec teaclave_dev_env bash -c \
  "pkill cloudflared; \
   cloudflared tunnel --config /root/.cloudflared/config-docker.yml run kms-tunnel \
   > /tmp/cloudflared.log 2>&1 &"

# 3. 验证公网访问
sleep 5
curl -s https://kms.aastar.io/health | jq .
```

## 测试流程

### 1. Docker 内部测试（最可靠）

```bash
# Health check
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health | jq .

# 创建钱包
docker exec teaclave_dev_env curl -s -X POST http://127.0.0.1:3000/CreateKey \
  -H "Content-Type: application/json" \
  -d '{"Description":"test-wallet"}' | jq .

# 派生地址
docker exec teaclave_dev_env curl -s -X POST http://127.0.0.1:3000/DeriveAddress \
  -H "Content-Type: application/json" \
  -d '{"KeyId":"<wallet-uuid>","DerivationPath":"m/44'"'"'/60'"'"'/0'"'"'/0/0"}' | jq .
```

### 2. 公网测试

```bash
# Health check
curl -s https://kms.aastar.io/health | jq .

# 创建钱包
curl -s -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -d '{"Description":"production-wallet"}' | jq .

# 列出所有钱包
curl -s -X POST https://kms.aastar.io/ListKeys \
  -H "Content-Type: application/json" \
  -d '{}' | jq .
```

### 3. Mac 本地测试（当前不可用）

**注意**：由于 Docker for Mac 的限制，Mac 无法直接访问 `localhost:3000`。
- ❌ `curl http://localhost:3000/health` → 失败
- ✅ 使用 Docker 内部测试或公网测试

## 开发中的常见场景

### 场景 1: 修改 API 端点逻辑

```bash
# 1. 编辑代码
vim kms/host/src/api_server.rs

# 2. 重新编译
./scripts/kms-dev-env.sh build

# 3. 同步到 QEMU
./scripts/kms-dev-env.sh sync

# 4. 重启 KMS API Server
docker exec teaclave_dev_env bash -l -c "
(
echo 'killall kms-api-server'
sleep 1
echo 'cd /root/shared && ./kms-api-server > /tmp/kms.log 2>&1 &'
sleep 2
) | socat - TCP:localhost:54320
"

# 5. 测试
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health
```

### 场景 2: 修改 TA 交互逻辑

```bash
# 1. 编辑 ta_client.rs（不是 TA 代码！）
vim kms/host/src/ta_client.rs

# 2-5. 同场景 1
```

### 场景 3: 修改 TA 代码（慎重！）

```bash
# 1. 编辑 TA
vim kms/ta/src/lib.rs

# 2. 编译（TA + Host 都需要重新编译）
./scripts/kms-dev-env.sh build

# 3. 同步
./scripts/kms-dev-env.sh sync

# 4. 重新部署 TA 和重启服务
docker exec teaclave_dev_env bash -l -c "
(
echo 'cp /root/shared/*.ta /lib/optee_armtz/'
sleep 1
echo 'killall kms-api-server'
sleep 1
echo 'cd /root/shared && ./kms-api-server > /tmp/kms.log 2>&1 &'
sleep 2
) | socat - TCP:localhost:54320
"

# 5. 测试
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health
```

### 场景 4: 调试问题

```bash
# 查看 KMS API 日志（在 QEMU Guest 内）
docker exec teaclave_dev_env bash -l -c "
(
echo 'cat /tmp/kms.log | tail -50'
sleep 2
) | socat - TCP:localhost:54320
"

# 查看 QEMU 日志
docker exec teaclave_dev_env tail -50 /tmp/qemu.log

# 查看 cloudflared 日志
docker exec teaclave_dev_env tail -50 /tmp/cloudflared.log

# 进入 QEMU Guest VM（交互式调试）
# 开启新终端执行：
docker exec -it teaclave_dev_env bash
# 进入后：
socat - TCP:localhost:54320
# 现在你在 QEMU 的 shell 中，可以手动执行命令
```

## 网络连接说明

### 1. Volume Mount (代码同步)

```
Mac: /AirAccount/third_party/teaclave-trustzone-sdk
  ↕ (双向同步)
Docker: /root/teaclave_sdk_src
```

**特点**：
- ✅ Mac 修改代码，Docker 立即可见
- ✅ Docker 编译产物，Mac 立即可见
- 📝 无需手动复制文件

### 2. Docker 共享目录 (编译产物传递)

```
Docker: /opt/teaclave/shared/
  ↕ (9p virtio 文件共享)
QEMU Guest: /root/shared/
```

**特点**：
- ✅ Docker 中的 `/opt/teaclave/shared/` 映射到 QEMU 的 `/root/shared/`
- ✅ 需要在 QEMU 中 mount: `mount -t 9p -o trans=virtio host /root/shared`
- 📝 通过 virtio-9p-device 实现

### 3. 端口转发链

```
QEMU Guest:3000 (KMS API Server)
  ↓ (QEMU hostfwd)
Docker:3000
  ↓ (Docker -p 3000:3000，但 Mac 无法访问)
(Mac:3000) ❌ 不可用

Docker 内 cloudflared → localhost:3000 ✅ 可用
  ↓
Internet: https://kms.aastar.io ✅ 可用
```

**为什么 Mac 无法访问 localhost:3000？**
- Docker for Mac 使用虚拟机（HyperKit/QEMU）
- 端口映射 `-p 3000:3000` 在 Linux 上有效，但在 Mac 上有限制
- Rosetta 2 翻译层也可能影响端口转发

**解决方案**：
- ✅ 在 Docker 内测试：`docker exec teaclave_dev_env curl http://127.0.0.1:3000/health`
- ✅ 通过公网测试：`curl https://kms.aastar.io/health`
- ✅ cloudflared 运行在 Docker 内，访问 localhost:3000 正常

### 4. Serial 连接 (QEMU 控制)

```
QEMU Serial Console (Guest VM Shell)
  ↓ (TCP socket)
Docker:54320
  ↓
通过 socat 发送命令
```

**用法**：
```bash
# 自动化发送命令
echo 'ls -la' | socat - TCP:localhost:54320

# 交互式连接（开新终端）
docker exec -it teaclave_dev_env bash
socat - TCP:localhost:54320
```

## 快速参考

### 常用命令

```bash
# 容器管理
./scripts/kms-dev-env.sh start    # 启动容器
./scripts/kms-dev-env.sh stop     # 停止容器
./scripts/kms-dev-env.sh status   # 查看状态
./scripts/kms-dev-env.sh shell    # 进入容器

# 开发流程
./scripts/kms-dev-env.sh build    # 编译
./scripts/kms-dev-env.sh sync     # 同步产物

# 测试
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health
curl -s https://kms.aastar.io/health

# 调试
docker exec teaclave_dev_env tail -f /tmp/qemu.log
docker exec teaclave_dev_env tail -f /tmp/cloudflared.log
```

### 端口列表

| 端口  | 作用                          | 访问方式                |
|-------|-------------------------------|-------------------------|
| 3000  | KMS API (QEMU Guest)          | Docker 内: localhost:3000 |
| 54320 | QEMU Serial Console (Guest)   | socat - TCP:localhost:54320 |
| 54321 | QEMU Secure Console           | socat - TCP:localhost:54321 |
| 54433 | QEMU HTTPS (保留)             | Docker 内: localhost:54433 |

### 目录结构

```
AirAccount/
├── kms/
│   ├── host/src/
│   │   ├── api_server.rs       ← 主要开发文件
│   │   ├── ta_client.rs        ← 主要开发文件
│   │   └── main.rs
│   └── ta/src/
│       └── lib.rs              ← 慎重修改！
├── third_party/teaclave-trustzone-sdk/  ← Volume mount 到 Docker
└── scripts/
    ├── kms-dev-env.sh          ← 容器管理
    └── publish-kms-complete.sh ← 发布脚本（待完善）
```

## 故障排查

### 问题 1: Mac 无法访问 localhost:3000

**症状**: `curl http://localhost:3000/health` 超时或连接被拒绝

**原因**: Docker for Mac 限制

**解决**:
```bash
# 使用 Docker 内测试
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health

# 或使用公网
curl https://kms.aastar.io/health
```

### 问题 2: QEMU 中找不到 kms-api-server

**症状**: `/root/shared/` 目录为空

**解决**:
```bash
# 1. 检查 Docker 共享目录
docker exec teaclave_dev_env ls -la /opt/teaclave/shared/

# 2. 如果为空，重新同步
./scripts/kms-dev-env.sh sync

# 3. 在 QEMU 中重新 mount
docker exec teaclave_dev_env bash -l -c "
(
echo 'umount /root/shared 2>/dev/null || true'
sleep 1
echo 'mount -t 9p -o trans=virtio host /root/shared'
sleep 2
echo 'ls -la /root/shared/'
sleep 1
) | socat - TCP:localhost:54320
"
```

### 问题 3: 修改代码后没有生效

**症状**: API 行为没有变化

**检查清单**:
```bash
# 1. 确认代码已保存
# 2. 重新编译
./scripts/kms-dev-env.sh build

# 3. 同步产物
./scripts/kms-dev-env.sh sync

# 4. 重启 KMS Server
docker exec teaclave_dev_env bash -l -c "
(
echo 'killall kms-api-server'
sleep 1
echo 'cd /root/shared && ./kms-api-server > /tmp/kms.log 2>&1 &'
sleep 2
echo 'ps aux | grep kms-api-server'
) | socat - TCP:localhost:54320
"

# 5. 查看日志确认新版本
docker exec teaclave_dev_env bash -l -c "
(
echo 'cat /tmp/kms.log | tail -20'
sleep 2
) | socat - TCP:localhost:54320
"
```

### 问题 4: cloudflared 502 错误

**症状**: `curl https://kms.aastar.io/health` 返回 502

**解决**:
```bash
# 1. 检查 KMS 是否运行
docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health

# 2. 如果 KMS 正常，重启 cloudflared
docker exec teaclave_dev_env bash -c \
  "pkill cloudflared; \
   cloudflared tunnel --config /root/.cloudflared/config-docker.yml run kms-tunnel \
   > /tmp/cloudflared.log 2>&1 &"

# 3. 查看 cloudflared 日志
sleep 3
docker exec teaclave_dev_env tail -30 /tmp/cloudflared.log
```

## 最佳实践

1. **开发时**：
   - ✅ 优先修改 `api_server.rs` 和 `ta_client.rs`
   - ✅ 每次修改后执行完整的 build → sync → restart 流程
   - ✅ 使用 Docker 内测试，避免 Mac 网络限制

2. **部署时**：
   - ✅ 测试通过后再发布到公网
   - ✅ 保持 cloudflared 运行在 Docker 内（避免 Mac 端口问题）
   - ✅ 监控日志：QEMU、KMS、cloudflared

3. **测试时**：
   - ✅ 先测 Docker 内部（`docker exec ... curl`）
   - ✅ 再测公网（`curl https://kms.aastar.io`）
   - ✅ 使用 `jq` 格式化 JSON 输出

4. **调试时**：
   - ✅ 查看三层日志：QEMU、KMS、cloudflared
   - ✅ 使用 socat 交互式连接 QEMU
   - ✅ 验证每一层网络连接

---

**总结**：你的开发流程基本不变，只是测试时需要通过 Docker exec 或公网访问，而不是 Mac 的 localhost。