# Teaclave TrustZone SDK 开发环境搭建指南

基于官方文档: https://teaclave.apache.org/trustzone-sdk-docs/emulate-and-dev-in-docker.md

## 环境要求

- Docker Desktop (已安装并运行)
- 子模块已初始化: `third_party/teaclave-trustzone-sdk`

## 快速开始

### 一键执行所有自动化步骤

```bash
./scripts/trustzone-dev-env.sh all
```

这将自动完成:
1. ✅ 拉取Docker镜像
2. ✅ 启动开发容器
3. ✅ 构建Hello World示例
4. ✅ 同步artifacts到模拟器

## 分步执行

### 步骤1: 拉取Docker镜像

```bash
./scripts/trustzone-dev-env.sh 1
# 或
./scripts/trustzone-dev-env.sh pull
```

**说明:** 拉取预构建的开发环境镜像 (linux/amd64,在Apple Silicon上通过Rosetta运行)

### 步骤2: 启动开发容器

```bash
./scripts/trustzone-dev-env.sh 2
# 或
./scripts/trustzone-dev-env.sh start
```

**说明:** 启动容器并挂载SDK源码目录

### 步骤3: 构建Hello World示例

```bash
./scripts/trustzone-dev-env.sh 3
# 或
./scripts/trustzone-dev-env.sh build
```

**说明:** 编译Host应用和Trusted Application (TA)

### 步骤4: 同步artifacts到模拟器

```bash
./scripts/trustzone-dev-env.sh 4
# 或
./scripts/trustzone-dev-env.sh sync
```

**说明:** 将编译好的二进制文件同步到QEMU共享目录

## 交互式运行 (步骤5-6)

运行需要3个终端同时工作:

### Terminal 1: QEMU控制台

```bash
docker exec -it teaclave_dev_env bash -l -c "LISTEN_MODE=ON start_qemuv8"
```

**作用:** 启动QEMU模拟器,模拟ARM TrustZone环境

### Terminal 2: Guest VM Shell

```bash
docker exec -it teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
```

**作用:** 连接到Guest虚拟机的Shell,用于运行应用程序

### Terminal 3: Secure World日志

```bash
docker exec -it teaclave_dev_env bash -l -c "listen_on_secure_world_log"
```

**作用:** 查看Secure World (TEE)的日志输出

### 运行Hello World

在 **Terminal 2 (Guest VM)** 中执行:

```bash
./host/hello_world-rs
```

**预期输出:**
- Terminal 2: 应用程序输出
- Terminal 3: TA内部日志

## 快速命令参考

```bash
# 查看帮助
./scripts/trustzone-dev-env.sh

# 显示交互模式说明
./scripts/trustzone-dev-env.sh interactive

# 清理环境(停止并删除容器)
./scripts/trustzone-dev-env.sh clean
```

## 手动操作(不使用脚本)

如果需要手动操作:

```bash
# 进入容器
docker exec -it teaclave_dev_env bash -l

# 在容器内构建
cd /root/teaclave_sdk_src
make -C examples/hello_world-rs/

# 同步到模拟器
make -C examples/hello_world-rs/ emulate
```

## 架构说明

```
┌─────────────────────────────────────────┐
│       macOS Host (arm64)                │
│  ┌───────────────────────────────────┐  │
│  │  Docker Container (linux/amd64)   │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  QEMU (aarch64)             │  │  │
│  │  │  ┌───────────┬───────────┐  │  │  │
│  │  │  │ Normal    │  Secure   │  │  │  │
│  │  │  │ World     │  World    │  │  │  │
│  │  │  │ (Linux)   │  (OP-TEE) │  │  │  │
│  │  │  │           │           │  │  │  │
│  │  │  │ Host App  │    TA     │  │  │  │
│  │  │  └───────────┴───────────┘  │  │  │
│  │  └─────────────────────────────┘  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

- **Host App:** 普通世界应用 (aarch64)
- **TA (Trusted Application):** 可信应用,运行在Secure World
- **OP-TEE:** Open Portable Trusted Execution Environment

## 注意事项

1. **平台兼容性:** 镜像是linux/amd64,在Apple Silicon上性能会较慢
2. **容器名称:** 默认使用 `teaclave_dev_env`
3. **SDK路径:** 挂载 `third_party/teaclave-trustzone-sdk` 到容器
4. **环境变量:** 容器需要使用login shell (`bash -l`)来加载环境

## 故障排查

### Docker守护进程未运行
```bash
# 启动Docker Desktop
open /Applications/Docker.app
```

### 容器已存在
```bash
./scripts/trustzone-dev-env.sh clean
```

### 构建失败
```bash
# 检查容器状态
docker ps -a | grep teaclave_dev_env

# 查看容器日志
docker logs teaclave_dev_env

# 重新进入容器
docker exec -it teaclave_dev_env bash -l
```

## 下一步

- 探索其他示例: `examples/` 目录
- 开发自定义TA
- 集成到现有项目