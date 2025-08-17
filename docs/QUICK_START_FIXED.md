# AirAccount 快速启动指南 (修复版)

**创建时间**: 2025-08-17 11:35:00 +07  
**最后更新**: 2025-08-17 11:35:00 +07

## 🎯 问题修复总结

此文档解决了以下问题：
1. ✅ **QEMU多进程问题** - 确保只运行一个QEMU实例
2. ✅ **共享目录挂载问题** - 提供手动挂载解决方案
3. ✅ **TA构建环境问题** - 修复缺失的环境变量
4. ✅ **测试流程优化** - 清晰的五步测试法

## 🚀 快速启动步骤

### 步骤1: 环境修复和设置

```bash
# 1. 清理环境
cd /Volumes/UltraDisk/Dev2/aastar/AirAccount
./scripts/fix-test-environment.sh

# 2. 设置OP-TEE环境
./scripts/setup-env.sh
source ~/.airaccount_env
```

### 步骤2: 启动QEMU环境

```bash
# 清理旧进程
pkill -f qemu-system-aarch64

# 启动QEMU (新终端)
cd third_party/incubator-teaclave-trustzone-sdk/tests/
./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04
```

### 步骤3: 在QEMU中修复共享目录

```bash
# 在QEMU控制台中 (登录: root)
mkdir -p /shared
mount -t 9p -o trans=virtio,version=9p2000.L host /shared
ls -la /shared/
```

### 步骤4: 构建和测试TA

```bash
# 在主机上构建TA
cd packages/airaccount-ta-simple
make clean && make

# 在QEMU中测试
cp /shared/11223344-5566-7788-99aa-bbccddeeff01.ta /lib/optee_armtz/
chmod 444 /lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta
/shared/airaccount-ca test
```

### 步骤5: 启动CA服务和Demo

```bash
# 启动Node.js CA (新终端)
cd packages/airaccount-ca-nodejs
npm run dev

# 启动Demo前端 (新终端)
cd demo-real
npm run dev
```

## 📋 核心修复内容

### 1. 环境变量修复

**正确的环境变量设置：**
```bash
export OPTEE_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee"
export TA_DEV_KIT_DIR="$OPTEE_DIR/optee_os/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="$OPTEE_DIR/optee_client/export_arm64"
```

### 2. QEMU共享目录挂载修复

**问题**: `/shared/` 目录未自动挂载  
**解决方案**: 手动挂载9p文件系统
```bash
mkdir -p /shared
mount -t 9p -o trans=virtio,version=9p2000.L host /shared
```

### 3. QEMU多进程问题修复

**问题**: 多个QEMU进程同时运行导致冲突  
**解决方案**: 启动前清理所有QEMU进程
```bash
pkill -f qemu-system-aarch64
```

## 🔧 创建的修复文件

1. **`scripts/fix-test-environment.sh`** - 环境修复脚本
2. **`scripts/setup-env.sh`** - OP-TEE环境设置脚本
3. **`docs/MANUAL_TESTING_GUIDE_FIXED.md`** - 修复版测试指南
4. **`docs/QUICK_START_FIXED.md`** - 此快速启动指南

## 🎯 验证成功标准

### 环境验证
- [ ] 只有一个QEMU进程运行
- [ ] `/shared/` 目录可访问，包含TA和CA文件
- [ ] TA构建环境变量正确设置

### 功能验证
- [ ] `/shared/airaccount-ca test` 返回 5/5 通过
- [ ] Node.js CA健康检查返回healthy
- [ ] Demo前端可访问 http://localhost:5174

## 🚨 常见问题解决

### Q: QEMU启动后没有/shared目录？
A: 在QEMU中运行：
```bash
mkdir -p /shared
mount -t 9p -o trans=virtio,version=9p2000.L host /shared
```

### Q: TA构建失败，提示环境变量错误？
A: 运行环境设置脚本：
```bash
./scripts/setup-env.sh
source ~/.airaccount_env
```

### Q: 有多个QEMU进程运行？
A: 清理所有QEMU进程：
```bash
pkill -f qemu-system-aarch64
```

### Q: CA服务端口被占用？
A: 清理端口占用：
```bash
lsof -ti:3002 | xargs kill -9
```

## 📖 详细测试指南

完整的五步测试法请参考：`docs/MANUAL_TESTING_GUIDE_V3.md`

## 🎉 修复完成

现在您可以按照修复后的流程进行完整的五步测试，所有已知问题都已解决！