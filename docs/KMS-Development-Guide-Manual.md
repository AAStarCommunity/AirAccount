# KMS 开发指南 - 三终端手动模式

> **适用场景**: 需要查看实时日志、调试问题、监控 TA/CA 输出

**最后更新**: 2025-10-02 03:58

---

## 📋 完整开发流程（增强版）

### 步骤 1: Docker 重启（按需）

**何时需要**:
- Docker 内有僵尸进程
- 容器状态异常
- 长时间开发后

**方式 1: 命令行**
```bash
docker stop teaclave_dev_env
docker start teaclave_dev_env

# 验证容器状态
docker ps | grep teaclave_dev_env
```

**方式 2: Docker Desktop**
- 打开 Docker Desktop
- 找到 teaclave_dev_env 容器
- 点击重启按钮

---

### 步骤 2: 启动 Cloudflare Tunnel（可选）

**用途**: 公网访问（https://kms.aastar.io）

```bash
# 启动 tunnel（后台运行）
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &

# 验证运行
ps aux | grep cloudflared
tail -f /tmp/cloudflared.log

# 停止（如果需要）
killall cloudflared
```

**何时需要**:
- 需要从外部测试 API
- 需要通过公网域名访问
- 否则只用 localhost:3000 即可跳过

---

### 步骤 3: 清理所有旧进程 ⭐

```bash
./scripts/kms-cleanup.sh
```

**清理内容**:
- ✅ QEMU 进程
- ✅ socat 监听器（端口 54320, 54321）
- ✅ expect 脚本
- ✅ 显示僵尸进程数量

**输出示例**:
```
🧹 清理 KMS 相关进程...

  停止 QEMU... ✓
  停止 socat... ✓
  停止监听器... ✓
  清理僵尸进程... ✓

✅ 清理完成！
```

---

### 步骤 4: 部署（编译 + 复制）⭐

**增量构建**（快速，适合小改动）:
```bash
./scripts/kms-deploy.sh
```

**完全重新构建**（推荐，修改代码后）:
```bash
./scripts/kms-deploy.sh clean
```

**自动执行**:
1. 同步 `kms/` → SDK (`rsync -av --delete`)
2. 检查 STD 依赖（rust/libc）
3. 编译 TA (aarch64-unknown-optee)
4. 编译 Host (aarch64-unknown-linux-gnu)
5. 复制到 `/opt/teaclave/shared/` 和 `/opt/teaclave/shared/ta/`

**输出验证**:
```bash
# 部署脚本最后会显示文件列表
-rw-r--r-- 1 root root    595K Oct  2 03:46 4319f351-...-80ee4f824cdd.ta
-rwxr-xr-x 1 root root    707K Oct  2 03:46 kms
-rwxr-xr-x 1 root root    2.9M Oct  2 03:46 kms-api-server
```

**时间参考**:
- 增量构建: ~30 秒
- 完全重建: ~2-3 分钟

---

### 步骤 5: Terminal 3 - Secure World 日志 🔒

**作用**: 查看 TA（Trusted Application）的实时输出

```bash
./scripts/terminal3-secure-log.sh
```

**会看到什么**:
- OP-TEE 安全世界的启动日志
- TA 加载信息
- TA 内部的 debug 输出
- 签名、密钥生成等安全操作日志

**输出示例**:
```
🔒 Starting Secure World Log listener...
    ✅ 端口 54321 已释放
    Listening on port 54321
    Waiting for QEMU to connect...

# QEMU 连接后会显示：
D/TC:? 0 tee_ta_init_pseudo_ta_session:297 Lookup pseudo TA 4319f351-0b24-4097-b659-80ee4f824cdd
D/TC:? 0 ldelf_load_ldelf:107 ldelf load address 0x40006000
D/TA:  [+] TA: create_entry_point
D/TA:  [+] TA: open_session_entry_point
```

**保持此终端打开**以持续监控安全世界日志。

---

### 步骤 6: Terminal 2 - Guest VM Shell 🖥️

**作用**:
- 自动挂载 shared 目录
- 自动启动 kms-api-server
- 提供 QEMU 内的交互式 shell

```bash
./scripts/terminal2-guest-vm.sh
```

**自动执行流程**:
1. ✅ 启动端口 54320 监听器
2. ✅ 等待 QEMU 启动并连接
3. ✅ 自动登录（用户：root）
4. ✅ 执行以下命令：
   ```bash
   mkdir -p shared
   mount -t 9p -o trans=virtio host shared
   cd shared
   mount --bind ta/ /lib/optee_armtz
   mount --bind plugin/ /usr/lib/tee-supplicant/plugins/
   ./kms-api-server > kms-api.log 2>&1 &
   echo 'KMS API Server started'
   ```

**会看到什么**:
```
🖥️  Starting Guest VM Shell listener...
    ✅ 端口 54320 已释放
    Listening on port 54320
    Waiting for QEMU to connect...

# QEMU 连接后：
spawn socat TCP-LISTEN:54320,reuseaddr,fork -,raw,echo=0
Listening on TCP port 54320 for guest vm output...

buildroot login: root
# mkdir -p shared && mount -t 9p -o trans=virtio host shared && cd shared
# mount --bind ta/ /lib/optee_armtz
# mount --bind plugin/ /usr/lib/tee-supplicant/plugins/
# ./kms-api-server > kms-api.log 2>&1 &
# echo 'KMS API Server started'
KMS API Server started
#
```

**此时会进入交互模式**，你可以手动执行命令。

**保持此终端打开**以查看 CA（Client Application）日志。

---

### 步骤 7: Terminal 1 - QEMU 控制 🚀

**作用**: 启动 QEMU 虚拟机

```bash
./scripts/terminal1-qemu.sh
```

**启动前确认**:
```
🚀 Starting QEMU...

⚠️  请确认:
    1. Terminal 2 已启动并显示 'Listening on TCP port 54320'
    2. Terminal 3 已启动并显示 'Listening on TCP port 54321'

按回车继续,或Ctrl+C取消...
```

**按回车后启动 QEMU**，会看到 Linux 内核启动日志。

**QEMU 启动参数**:
- `-machine virt,secure=on` - 启用 TrustZone
- `-cpu cortex-a57` - ARM CPU
- `-m 1057` - 1GB 内存
- `-netdev user,hostfwd=tcp:0.0.0.0:3000-:3000` - 端口转发
- `-fsdev local,path=/opt/teaclave/shared` - 共享目录

**会看到什么**:
```
Starting kernel ...

[    0.000000] Booting Linux on physical CPU 0x0000000000 [0x411fd070]
[    0.000000] Linux version 5.15.0 ...
...
[   10.234567] Starting OP-TEE supplicant...
...

Welcome to Buildroot
buildroot login:
```

**此时 Terminal 2 会自动登录并执行挂载**。

**保持此终端打开**以查看 QEMU 系统日志。

---

### 步骤 8: Terminal 2 交互操作（增强版）⭐

**重要**: expect 脚本会**自动执行大部分操作**，但你可以在 Terminal 2 中手动执行以下命令来验证或重启服务。

#### 8.1 验证挂载（自动完成，可选验证）

```bash
# 查看 shared 目录内容
ls /root/shared/
# 输出: 4319f351-...-80ee4f824cdd.ta  kms  kms-api-server  kms-test-page.html  ta/  plugin/

# 进入 shared 目录
cd /root/shared/

# 查看 ta 子目录
ls ta/
# 输出: 4319f351-0b24-4097-b659-80ee4f824cdd.ta

# 验证 TA 已挂载到 /lib/optee_armtz
ls /lib/optee_armtz/
# 输出应包含: 4319f351-0b24-4097-b659-80ee4f824cdd.ta
```

#### 8.2 检查 API Server 状态

```bash
# 查看进程
ps aux | grep kms-api-server
# 输出: root  123  0.0  kms-api-server

# 查看日志
cat kms-api.log
# 或实时查看
tail -f kms-api.log
```

#### 8.3 重启 API Server（如果需要）

**场景**:
- API 启动失败
- 需要重新加载配置
- 部署了新版本但 expect 脚本没有重启

```bash
# 1. 停止旧进程
ps aux | grep kms-api-server
killall kms-api-server
ps aux | grep kms-api-server  # 确认已停止

# 2. 查看旧日志（排查问题）
cat kms-api.log

# 3. 启动新进程
cd /root/shared
./kms-api-server > kms-api.log 2>&1 &

# 4. 验证启动（等待 2-3 秒）
sleep 3
cat kms-api.log
wget -qO- http://127.0.0.1:3000/health
```

#### 8.4 手动挂载（仅在自动化失败时）

**如果 expect 脚本没有自动挂载**:

```bash
# 1. 创建并挂载 shared 目录
mkdir -p /root/shared
mount -t 9p -o trans=virtio host /root/shared

# 2. 验证挂载
ls /root/shared/
cd /root/shared/

# 3. 挂载 TA 文件
mount --bind ta/ /lib/optee_armtz
ls /lib/optee_armtz/

# 4. 挂载 plugin（如果有）
mount --bind plugin/ /usr/lib/tee-supplicant/plugins/

# 5. 启动 API Server
./kms-api-server > kms-api.log 2>&1 &
sleep 3
wget -qO- http://127.0.0.1:3000/health
```

#### 8.5 调试技巧

```bash
# 查看网络状态
ip addr show

# 测试端口
netstat -tuln | grep 3000

# 查看系统日志
dmesg | tail -20

# 查看 TA 加载状态
ls -la /lib/optee_armtz/

# 测试 API（从 QEMU 内部）
wget -qO- http://127.0.0.1:3000/health | head -20

# 查看完整 API 启动日志
cat kms-api.log | head -50
```

---

### 步骤 9: 测试验证 ✅

#### 9.1 本地测试（Mac 主机）

```bash
# 健康检查
curl -s http://localhost:3000/health | jq .

# 预期输出：
{
  "endpoints": {
    "GET": ["/health"],
    "POST": ["/CreateKey", "/DescribeKey", "/ListKeys",
             "/DeriveAddress", "/Sign", "/SignHash", "/DeleteKey"]
  },
  "service": "kms-api",
  "status": "healthy",
  "ta_mode": "real",
  "version": "0.1.0"
}
```

#### 9.2 公网测试（需要先启动 cloudflared）

```bash
curl -s https://kms.aastar.io/health | jq .
```

#### 9.3 功能测试

**测试 1: 创建第一个钱包和地址**
```bash
curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{
    "Description": "Test Wallet 1",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }' | jq '.KeyMetadata | {KeyId, Address, DerivationPath}'

# 预期输出：
{
  "KeyId": "48c8d60e-0134-4488-926a-5521accb9e14",
  "Address": "0xad365342c8ee4a951251c10fff8f840cbdf1dd4e",
  "DerivationPath": "m/44'/60'/0'/0/0"
}
```

**保存 KeyId 用于后续测试。**

**测试 2: 创建第二个地址（同一钱包）**
```bash
WALLET_ID="48c8d60e-0134-4488-926a-5521accb9e14"  # 替换为上面的 KeyId

curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d "{
    \"KeyId\": \"$WALLET_ID\",
    \"Description\": \"Test Wallet 1 - Address 2\",
    \"KeyUsage\": \"SIGN_VERIFY\",
    \"KeySpec\": \"ECC_SECG_P256K1\",
    \"Origin\": \"AWS_KMS\"
  }" | jq '.KeyMetadata | {KeyId, Address, DerivationPath}'

# 预期输出：
{
  "KeyId": "48c8d60e-0134-4488-926a-5521accb9e14",  # 相同的 KeyId
  "Address": "0xc6ba2ba8537eb5aed7a049d5e51ca7bb08279ff9",
  "DerivationPath": "m/44'/60'/0'/0/1"  # ✅ 自动递增到 /1
}
```

**测试 3: 使用 Address 签名（新功能）**
```bash
ADDRESS="0xad365342c8ee4a951251c10fff8f840cbdf1dd4e"  # 替换为第一个地址

curl -s -X POST 'http://localhost:3000/Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d "{
    \"Address\": \"$ADDRESS\",
    \"Message\": \"Hello World\"
  }" | jq .

# 预期输出：
{
  "Signature": "d45e403b1df0e4e3807dd4361425905ddb43619b...",
  "TransactionHash": "[TX_HASH_OR_MESSAGE_HASH]"
}
```

**测试 4: 传统方式签名（向后兼容）**
```bash
curl -s -X POST 'http://localhost:3000/Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d "{
    \"KeyId\": \"$WALLET_ID\",
    \"DerivationPath\": \"m/44'/60'/0'/0/0\",
    \"Message\": \"Hello World\"
  }" | jq .
```

---

## 🔍 三终端日志监控指南

### Terminal 3 - 查看什么？

**关键日志**:
- `D/TA: [+] TA: create_entry_point` - TA 创建
- `D/TA: [+] TA: open_session_entry_point` - 会话打开
- `D/TA: [+] Create wallet` - 创建钱包
- `D/TA: [+] Derive address` - 派生地址
- `D/TA: [+] Sign transaction` - 签名交易
- `E/TA:` - **错误日志**（重点关注）

**调试技巧**:
```bash
# 在 Terminal 3 中，可以 grep 特定内容
# （需要提前 tee 到文件）
docker exec teaclave_dev_env tail -f /tmp/secure_world.log | grep "ERROR\|WARN\|Create wallet"
```

### Terminal 2 - 查看什么？

**关键输出**:
- `🚀 KMS API Server starting on http://0.0.0.0:3000`
- `📚 Supported APIs: ...`
- `🔐 TA Mode: ✅ Real TA`
- `📝 KMS API called: CreateKey`
- `📝 KMS Sign API called with Address: 0x...`
- API 请求和响应日志

**实时查看 API 日志**:
```bash
# 在 Terminal 2 中
tail -f /root/shared/kms-api.log
```

### Terminal 1 - 查看什么？

**关键信息**:
- 内核启动日志
- OP-TEE supplicant 启动
- 文件系统挂载
- 网络初始化
- QEMU 系统错误

**一般无需持续监控**，除非出现系统级问题。

---

## 🛠️ 常见问题排查

### 问题 1: API 不响应

**症状**: `curl http://localhost:3000/health` 超时

**排查步骤**:

1. **检查 Terminal 2**:
   ```bash
   # 看到 "KMS API Server started" 了吗？
   # 如果没有，查看错误信息
   cat /root/shared/kms-api.log
   ```

2. **检查进程**:
   ```bash
   # 在 Terminal 2 中
   ps aux | grep kms-api-server
   ```

3. **检查端口**:
   ```bash
   # 在 Terminal 2 中
   netstat -tuln | grep 3000
   ```

4. **手动重启**:
   ```bash
   # 参考步骤 8.3
   cd /root/shared
   killall kms-api-server
   ./kms-api-server > kms-api.log 2>&1 &
   sleep 3
   wget -qO- http://127.0.0.1:3000/health
   ```

### 问题 2: TA 加载失败

**症状**: Terminal 3 显示 "TA not found" 或类似错误

**排查步骤**:

1. **检查 TA 文件是否存在**:
   ```bash
   # 在 Terminal 2 中
   ls -la /lib/optee_armtz/
   # 应该看到: 4319f351-0b24-4097-b659-80ee4f824cdd.ta
   ```

2. **检查 shared 目录挂载**:
   ```bash
   ls -la /root/shared/ta/
   ```

3. **重新挂载 TA**:
   ```bash
   mount --bind /root/shared/ta/ /lib/optee_armtz
   ls /lib/optee_armtz/
   ```

4. **检查部署**:
   ```bash
   # 在 Mac 主机
   docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/ta/
   # 确认时间戳是最新的
   ```

### 问题 3: 端口被占用

**症状**: `bind(): Address already in use`

**解决方案**:
```bash
# 在 Mac 主机
./scripts/kms-cleanup.sh

# 如果还有问题，重启 Docker
docker restart teaclave_dev_env

# 然后重新执行步骤 3-7
```

### 问题 4: expect 脚本没有自动执行

**症状**: Terminal 2 停在 "buildroot login:" 不自动登录

**解决方案**:
1. 手动输入 `root` 并回车
2. 手动执行步骤 8.4 的命令
3. 或者重启：Ctrl+C 关闭 Terminal 2，重新运行 `./scripts/terminal2-guest-vm.sh`

### 问题 5: 编译错误

**常见错误和修复**:

```bash
# 错误 1: "use of undeclared crate or module"
# 检查 lib.rs 是否正确导出模块

# 错误 2: "expected String, found &String"
# 使用 .to_string() 而不是 .clone()

# 错误 3: "variable does not need to be mutable"
# 移除不必要的 mut 关键字

# 如果持续失败，完全清理重建
cd kms
docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make clean"
./scripts/kms-deploy.sh clean
```

---

## 📝 开发流程检查清单

使用此清单确保每个步骤都正确执行：

- [ ] **步骤 1**: Docker 已重启（按需）
- [ ] **步骤 2**: Cloudflare tunnel 已启动（按需）
- [ ] **步骤 3**: 清理完成，无报错
- [ ] **步骤 4**: 部署完成，看到最新时间戳的文件
- [ ] **步骤 5**: Terminal 3 显示 "Listening on port 54321"
- [ ] **步骤 6**: Terminal 2 显示 "Listening on port 54320"
- [ ] **步骤 7**: Terminal 1 按回车后 QEMU 启动
- [ ] **步骤 8**: Terminal 2 显示 "KMS API Server started"
- [ ] **步骤 9**: `curl http://localhost:3000/health` 返回正常

**如果某个步骤失败**，参考对应的问题排查部分。

---

## 🎯 快速参考卡片

```
┌─────────────────────────────────────────────────────────────┐
│                KMS 开发流程速查表                              │
├─────────────────────────────────────────────────────────────┤
│ 1. 清理:     ./scripts/kms-cleanup.sh                       │
│ 2. 部署:     ./scripts/kms-deploy.sh clean                  │
│ 3. Terminal3: ./scripts/terminal3-secure-log.sh   (54321)   │
│ 4. Terminal2: ./scripts/terminal2-guest-vm.sh     (54320)   │
│ 5. Terminal1: ./scripts/terminal1-qemu.sh                   │
│ 6. 等待:     60 秒（观察 Terminal 2 输出）                    │
│ 7. 测试:     curl http://localhost:3000/health              │
├─────────────────────────────────────────────────────────────┤
│ Terminal 2 自动操作（无需手动）:                              │
│  ✅ 挂载 /root/shared                                        │
│  ✅ 绑定挂载 TA 到 /lib/optee_armtz                          │
│  ✅ 启动 kms-api-server                                      │
├─────────────────────────────────────────────────────────────┤
│ 如需手动重启 API:                                            │
│  cd /root/shared                                            │
│  killall kms-api-server                                     │
│  ./kms-api-server > kms-api.log 2>&1 &                      │
└─────────────────────────────────────────────────────────────┘
```

---

## 📚 相关文档

- **快速入门**: `docs/KMS-Quick-Start.md`（自动化模式）
- **完整工作流**: `docs/KMS-Development-Workflow.md`
- **设计文档**: `docs/KMS-Wallet-Address-Management-Design.md`
- **更新日志**: `docs/Changes.md`

---

**最后更新**: 2025-10-02 03:58

**下次开发时直接参考此文档！** 📖
