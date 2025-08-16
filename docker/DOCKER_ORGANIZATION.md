# Docker文件组织结构

本目录包含AirAccount项目的所有Docker相关文件，按功能分类组织。

## 📁 目录结构

```
docker/
├── README.md                       # Docker环境使用指南
├── DOCKER_ORGANIZATION.md          # 本文档 - 文件组织说明
├── Dockerfile.optee                # 主OP-TEE开发环境
├── build/                          # 构建相关Docker文件
│   ├── Dockerfile.qemu-tee         # QEMU TEE环境构建
│   └── Dockerfile.ta-ca-build      # TA/CA交叉编译构建
├── integration/                    # 集成测试Docker文件
│   ├── Dockerfile.integration-test # 完整集成测试环境
│   └── Dockerfile.simple-integration # 简化集成测试
├── test/                          # 测试环境Docker文件
│   ├── Dockerfile.basic-test       # 基础功能测试
│   ├── Dockerfile.optee-test       # OP-TEE专项测试
│   ├── Dockerfile.simple-test      # 简单测试环境
│   └── Dockerfile.test             # 通用测试环境
└── scripts/                       # Docker相关脚本
    └── start-tee-service.sh        # TEE服务启动脚本
```

## 🚀 使用方式

### 开发环境
```bash
# 主OP-TEE开发环境
docker build -f docker/Dockerfile.optee -t airaccount-optee .

# TA/CA构建环境
docker build -f docker/build/Dockerfile.ta-ca-build -t airaccount-build .

# QEMU TEE环境
docker build -f docker/build/Dockerfile.qemu-tee -t airaccount-qemu .
```

### 测试环境
```bash
# 基础测试
docker build -f docker/test/Dockerfile.basic-test -t airaccount-basic-test .

# OP-TEE专项测试
docker build -f docker/test/Dockerfile.optee-test -t airaccount-optee-test .

# 完整集成测试
docker build -f docker/integration/Dockerfile.integration-test -t airaccount-integration .
```

### 集成测试
```bash
# 完整集成测试
docker build -f docker/integration/Dockerfile.integration-test -t airaccount-integration .

# 简化集成测试
docker build -f docker/integration/Dockerfile.simple-integration -t airaccount-simple .
```

## 📋 文件功能说明

### 核心开发环境
- **`Dockerfile.optee`**: 完整的OP-TEE开发环境，包含所有必要的工具链和依赖

### 构建环境 (`build/`)
- **`Dockerfile.ta-ca-build`**: 专用于TA和CA的交叉编译环境
- **`Dockerfile.qemu-tee`**: QEMU ARM虚拟化TEE环境

### 集成测试 (`integration/`)  
- **`Dockerfile.integration-test`**: 完整的集成测试环境，包含所有测试依赖
- **`Dockerfile.simple-integration`**: 轻量级集成测试，适用于快速验证

### 单元测试 (`test/`)
- **`Dockerfile.basic-test`**: 基础功能单元测试环境
- **`Dockerfile.optee-test`**: OP-TEE特定功能测试
- **`Dockerfile.simple-test`**: 简单快速测试环境
- **`Dockerfile.test`**: 通用测试环境模板

## 🔧 更新记录

### 2025-01-15: 文件重组
- 将散布在根目录的8个Docker文件整理到docker/目录
- 按功能分类到build/、integration/、test/子目录
- 更新相关脚本中的文件路径引用

### 组织前后对比

**组织前** (根目录散布):
```
Dockerfile.basic-test
Dockerfile.integration-test
Dockerfile.optee-test
Dockerfile.qemu-tee
Dockerfile.simple-integration
Dockerfile.simple-test
Dockerfile.ta-ca-build
Dockerfile.test
```

**组织后** (按功能分类):
```
docker/
├── build/Dockerfile.qemu-tee
├── build/Dockerfile.ta-ca-build
├── integration/Dockerfile.integration-test
├── integration/Dockerfile.simple-integration
├── test/Dockerfile.basic-test
├── test/Dockerfile.optee-test
├── test/Dockerfile.simple-test
└── test/Dockerfile.test
```

## 📚 相关文档

- [Docker环境使用指南](README.md)
- [项目构建指南](../docs/Deploy.md)
- [测试指南](../TESTING_GUIDE.md)
- [开发环境设置](../docs/Quick-Start-Guide.md)

---
*📅 最后更新: 2025-01-15*  
*🏷️ 版本: v1.0*