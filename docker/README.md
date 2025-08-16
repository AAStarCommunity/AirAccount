# Docker 配置目录

此目录包含AirAccount项目的所有Docker相关文件。

## 文件说明

### 开发环境
- `Dockerfile.dev` - 开发环境镜像
- `docker-compose.dev.yml` - 开发环境编排
- `Dockerfile.optee` - OP-TEE开发环境

### 生产环境  
- `Dockerfile.prod` - 生产环境镜像
- `docker-compose.prod.yml` - 生产环境编排

### CA服务
- `Dockerfile.ca-node` - Node.js CA服务
- `Dockerfile.ca-rust` - Rust CA服务

### 测试环境
- `Dockerfile.test` - 测试环境镜像
- `docker-compose.test.yml` - 测试环境编排

## 使用说明

```bash
# 开发环境
docker-compose -f docker/docker-compose.dev.yml up

# 生产环境
docker-compose -f docker/docker-compose.prod.yml up

# 测试环境
docker-compose -f docker/docker-compose.test.yml up
```