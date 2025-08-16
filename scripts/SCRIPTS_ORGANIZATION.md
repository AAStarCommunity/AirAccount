# Scripts目录组织结构

本目录包含AirAccount项目的所有脚本文件，按功能分类组织。

## 📁 目录结构

```
scripts/
├── SCRIPTS_ORGANIZATION.md     # 本文档 - 脚本组织说明
├── README.md                   # 脚本使用指南
├── lib/                        # 共享脚本库
│   └── common.sh               # 通用功能函数
├── test/                       # 🧪 测试相关脚本
│   ├── *.sh                    # Shell测试脚本
│   ├── *.js                    # JavaScript测试文件
│   └── validate-test-scripts.sh # 测试脚本验证工具
├── build_*.sh                  # 🔨 构建脚本
├── setup_*.sh                  # ⚙️  环境设置脚本
├── install_*.sh                # 📦 安装脚本
├── verify_*.sh                 # ✅ 验证脚本
├── start_qemu_tee_service.sh   # 🚀 服务启动脚本
└── fly.sh                      # 🎯 快捷启动脚本
```

## 🚀 核心脚本功能

### 构建脚本 (`build_*.sh`)
- **`build_all.sh`**: 完整项目构建，包含TA和CA
- **`build_ca.sh`**: 仅构建Client Application
- **`build_tee.sh`**: 构建TEE环境组件
- **`build_real_tee.sh`**: 构建真实硬件TEE环境
- **`build_toolchains.sh`**: 构建交叉编译工具链

### 环境设置脚本 (`setup_*.sh`)
- **`setup_optee_env.sh`**: 设置OP-TEE开发环境
- **`setup_rust.sh`**: 设置Rust工具链和目标
- **`setup_teaclave_sdk.sh`**: 设置Teaclave TrustZone SDK

### 安装脚本 (`install_*.sh`)
- **`install_dependencies.sh`**: 安装系统依赖包
- **`install_dev_tools.sh`**: 安装开发工具

### 验证脚本 (`verify_*.sh`)
- **`verify_build.sh`**: 验证构建结果
- **`verify_optee_setup.sh`**: 验证OP-TEE环境配置

### 系统检查脚本
- **`check_dev_tools.sh`**: 检查开发工具安装状态
- **`check_system.sh`**: 检查系统要求和配置

### 服务和工具脚本
- **`start_qemu_tee_service.sh`**: 启动QEMU OP-TEE服务
- **`fly.sh`**: Claude命令快捷启动
- **`update-claude.sh`**: 更新Claude配置
- **`security-check.sh`**: 安全检查脚本

### 优化和维护脚本
- **`cleanup_rust_cache.sh`**: 清理Rust缓存
- **`optimize_build_performance.sh`**: 优化构建性能
- **`compile_ca_simple.sh`**: 简化CA编译流程

### 测试和报告脚本
- **`run_final_validation.sh`**: 最终验证测试
- **`create_test_summary.sh`**: 创建测试总结报告

## 🧪 测试脚本目录 (`test/`)

### 集成测试脚本
- **`run-complete-test.sh`**: 完整的集成测试流程
- **`test-complete-integration.sh`**: 完整集成测试
- **`quick-test-sdk-ca.sh`**: SDK-CA快速连接测试

### 组件测试脚本
- **`test_ca_simple.sh`**: CA组件简单测试
- **`test_ta_ca_communication.sh`**: TA-CA通信测试
- **`test_hello_world*.sh`**: Hello World示例测试
- **`test_basic_hello.sh`**: 基础Hello测试

### 环境和健康检查
- **`tee-health-check.sh`**: TEE环境健康检查
- **`test-docker-tee.sh`**: Docker TEE环境测试

### 测试工具和验证
- **`test_framework.sh`**: 测试框架脚本
- **`validate-test-scripts.sh`**: 测试脚本验证工具
- **`test_all.sh`**: 运行所有测试

### JavaScript测试文件
- **`test_sdk_integration.js`**: SDK集成测试
- **`test-webauthn-complete-flow.js`**: WebAuthn完整流程测试

## 🔧 使用方式

### 从项目根目录运行
```bash
# 构建项目
./scripts/build_all.sh

# 设置开发环境
./scripts/setup_optee_env.sh

# 运行完整测试
./scripts/test/run-complete-test.sh

# 启动TEE服务
./scripts/start_qemu_tee_service.sh
```

### 从scripts目录运行
```bash
cd scripts

# 构建相关
./build_all.sh
./build_ca.sh

# 测试相关
cd test
./run-complete-test.sh
./quick-test-sdk-ca.sh
```

## 📋 脚本依赖关系

### 基础依赖流程
```
install_dependencies.sh → setup_optee_env.sh → build_toolchains.sh → build_all.sh
```

### 测试流程
```
build_all.sh → start_qemu_tee_service.sh → test/run-complete-test.sh
```

### 验证流程
```
verify_optee_setup.sh → verify_build.sh → run_final_validation.sh
```

## 🔍 脚本维护指南

### 添加新脚本时
1. 确定脚本功能类型（构建/测试/设置等）
2. 放置到对应目录或按命名约定放在scripts根目录
3. 添加可执行权限：`chmod +x script_name.sh`
4. 更新本文档说明

### 修改脚本路径引用时
1. 使用相对路径：`$(dirname "$0")/../target_dir`
2. 避免硬编码绝对路径
3. 运行 `test/validate-test-scripts.sh` 验证修改

### 脚本规范
- 文件名使用小写加下划线：`build_all.sh`
- 测试脚本统一放在 `test/` 目录
- 使用统一的错误处理和日志格式
- 包含脚本功能说明注释

## 📚 相关文档

- [Docker组织文档](../docker/DOCKER_ORGANIZATION.md)
- [测试指南](../TESTING_GUIDE.md)
- [开发环境设置](../docs/Deploy.md)
- [项目架构文档](../docs/Plan.md)

---
*📅 最后更新: 2025-01-15*  
*🏷️ 版本: v1.0*  
*📊 脚本总数: 35+*