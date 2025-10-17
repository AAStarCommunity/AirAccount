# 🚀 Teaclave TrustZone QEMU 启动指南

## ⚠️ 重要: 正确的启动顺序

QEMU需要连接到监听器端口,所以必须**先启动监听器,再启动QEMU**。

---

## 📋 三步启动流程

### 第1步: 启动Guest VM监听器 (Terminal 2)

打开**第1个终端窗口**,执行:

```bash
./scripts/terminal2-guest-vm.sh
```

**预期输出**:
```
🖥️  Starting Guest VM Shell listener...
    Listening on port 54320
    Waiting for QEMU to connect...

Listening on TCP port 54320 for guest vm output...
```

✅ 看到 "Listening on TCP port 54320" 后,保持此终端运行,继续下一步。

---

### 第2步: 启动Secure World监听器 (Terminal 3)

打开**第2个终端窗口**,执行:

```bash
./scripts/terminal3-secure-log.sh
```

**预期输出**:
```
🔒 Starting Secure World Log listener...
    Listening on port 54321
    Waiting for QEMU to connect...

Listening on TCP port 54321 for TA output...
```

✅ 看到 "Listening on TCP port 54321" 后,保持此终端运行,继续下一步。

---

### 第3步: 启动QEMU (Terminal 1)

打开**第3个终端窗口**,执行:

```bash
./scripts/terminal1-qemu.sh
```

脚本会提示确认Terminal 2和3已启动,按**回车**继续。

**预期输出**:
```
🚀 Starting QEMU...
+ ./qemu-system-aarch64 -nodefaults -nographic ...
```

✅ QEMU启动后,Terminal 2会开始显示Linux启动日志。

---

## 🎯 第4步: 运行Hello World

等待Terminal 2显示登录提示:

```
Welcome to Buildroot, type root or test to login
buildroot login:
```

输入用户名并挂载共享目录:

```bash
root
mkdir -p shared && mount -t 9p -o trans=virtio host shared && cd shared
mount --bind ta/ /lib/optee_armtz
./host/hello_world-rs
```

**预期输出**:
```
original value is 29
inc value is 129
dec value is 29
Success
```

---

## ✅ 验证成功标志

| 终端 | 成功标志 |
|------|----------|
| **Terminal 1 (QEMU)** | 进程保持运行,无报错 |
| **Terminal 2 (Guest VM)** | 显示 "Success" |
| **Terminal 3 (Secure World)** | 显示 TA生命周期日志 |

---

## 🔧 故障排查

### 问题: "Connection refused" 错误

**原因**: Terminal 2和3未启动,端口无监听器

**解决**:
1. 停止QEMU (Ctrl+C)
2. 先启动Terminal 2
3. 再启动Terminal 3
4. 最后启动Terminal 1

### 问题: 终端卡住无输出

**原因**: QEMU未连接到监听器

**解决**:
1. 确认3个终端都在运行
2. 检查端口是否被占用: `lsof -i :54320`
3. 重启所有终端

---

## 📝 快速参考

```bash
# 完整流程
./scripts/trustzone-dev-env.sh all        # 步骤1-4: 自动化构建

# 然后按顺序执行:
./scripts/terminal2-guest-vm.sh           # 先启动
./scripts/terminal3-secure-log.sh         # 再启动
./scripts/terminal1-qemu.sh               # 最后启动
```

---

*最后更新: 2025-09-30*