# KMS API 完整测试指南

*最后修改时间: Mon Sep 29 11:19:40 +07 2025*

## 🚀 标准测试流程

### 第一步：启动Docker环境
```bash
# 1. 启动Docker Desktop
open -a Docker

# 2. 等待Docker启动（约30秒）
sleep 30 && docker ps

# 3. 启动KMS Docker容器
docker run -d --name kms-docker-new -p 8080:8080 \
  -v /Volumes/UltraDisk/Dev2/aastar/AirAccount:/opt/kms \
  teaclave-optee-nostd tail -f /dev/null

# 4. 在容器中启动KMS API服务器
docker exec -d kms-docker-new bash -c \
  'cd /opt/kms/kms/kms-api && \
   export LD_LIBRARY_PATH=/opt/teaclave/optee/optee_client/export_arm64/usr/lib && \
   source ~/.cargo/env && \
   cargo run --release > /tmp/kms.log 2>&1'
```

### 第二步：验证本地API
```bash
# 等待服务器启动（约15秒）
sleep 15

# 测试健康检查
curl -s localhost:8080/health

# 运行完整API测试
docker cp scripts/test-kms-curl.sh kms-docker-new:/tmp/
docker exec kms-docker-new /tmp/test-kms-curl.sh http://localhost:8080
```

### 第三步：隧道配置和测试

#### ⚠️ 重要提醒：临时隧道404问题修复
**每次创建新的临时隧道时都会遇到此问题，请务必使用以下配置：**

临时隧道默认无ingress配置会返回404错误。修复方法：
```bash
# 创建正确的临时隧道配置文件
cat > /tmp/temp-tunnel-fixed-config.yaml << EOF
ingress:
  - service: http://localhost:8080
    originRequest:
      noTLSVerify: true
      httpHostHeader: localhost
EOF

# 使用配置启动临时隧道
cloudflared tunnel --config /tmp/temp-tunnel-fixed-config.yaml --url http://localhost:8080
```

#### 方案A：使用正式隧道 (推荐)
```bash
# 1. 创建隧道配置
cat > /tmp/kms-tunnel-config.yaml << EOF
tunnel: e0c33444-b793-498b-9b90-83a7fea4e856
credentials-file: /Users/nicolasshuaishuai/.cloudflared/e0c33444-b793-498b-9b90-83a7fea4e856.json

ingress:
  - hostname: kms.aastar.io
    service: http://localhost:8080
    originRequest:
      httpHostHeader: kms.aastar.io
      noTLSVerify: true
  - service: http_status:404
EOF

# 2. 启动隧道
cloudflared tunnel --config /tmp/kms-tunnel-config.yaml run kms-api-public &

# 3. 测试
sleep 10
curl -s https://kms.aastar.io/health
```

#### 方案B：使用临时隧道 + 配置文件
```bash
# 1. 创建临时隧道配置
cat > /tmp/temp-tunnel-config.yaml << EOF
ingress:
  - service: http://localhost:8080
EOF

# 2. 启动配置化的临时隧道
cloudflared tunnel --config /tmp/temp-tunnel-config.yaml --url http://localhost:8080 &

# 3. 获取URL并测试
sleep 10
# 从日志中获取URL: https://xxx.trycloudflare.com
curl -s https://[临时URL]/health
```

#### 方案C：使用wrangler创建新应用
```bash
# 1. 创建新的Pages项目
wrangler pages project create kms-api-test --production-branch=main

# 2. 创建新隧道
cloudflared tunnel create kms-api-test

# 3. 配置DNS记录（手动在Cloudflare控制台）
```

## 🔧 问题诊断清单

### 本地服务检查
- [ ] Docker运行正常: `docker ps`
- [ ] KMS容器运行: `docker ps | grep kms`
- [ ] 本地API响应: `curl localhost:8080/health`
- [ ] 完整API测试通过

### 隧道连接检查
- [ ] 隧道进程运行: `ps aux | grep cloudflared`
- [ ] 隧道连接建立: 检查日志中"Registered tunnel connection"
- [ ] 配置文件正确: 检查credentials-file路径和tunnel ID

### 公共访问检查
- [ ] DNS解析正确: `nslookup kms.aastar.io`
- [ ] TLS握手成功: `curl -v https://kms.aastar.io/health`
- [ ] 隧道接收请求: 检查日志中HTTP请求记录
- [ ] 无404错误: 检查originService不是http_status:404

## 🚨 常见问题解决

### Error 1033
- **原因**: DNS配置问题或隧道路由失败
- **解决**:
  1. 检查DNS CNAME记录指向正确的.cfargotunnel.com
  2. 重启隧道服务
  3. 清理DNS缓存: `sudo dscacheutil -flushcache`

### 临时隧道404错误
- **原因**: 没有ingress配置
- **解决**: 使用配置文件而不是简单的--url参数

### 隧道连接但无HTTP日志
- **原因**: 请求未到达隧道
- **解决**:
  1. 检查防火墙设置
  2. 重新创建隧道
  3. 使用不同的隧道域名

## 📝 成功测试记录

### 本地API测试结果
```
✅ 健康检查: GET /health
✅ 创建密钥: TrentService.CreateKey
✅ 获取公钥: TrentService.GetPublicKey
✅ 签名消息: TrentService.Sign (需要SigningAlgorithm字段)
✅ 列出密钥: GET /keys
```

### 隧道配置信息
- 隧道ID: `e0c33444-b793-498b-9b90-83a7fea4e856`
- 域名: `kms.aastar.io`
- DNS记录: CNAME -> `e0c33444-b793-498b-9b90-83a7fea4e856.cfargotunnel.com`
- 账户: jhfnetboy@gmail.com

## 🔄 下次测试清单

1. [ ] 启动Docker Desktop
2. [ ] 运行KMS容器
3. [ ] 启动KMS API服务器
4. [ ] 验证本地API全部通过
5. [ ] 选择隧道方案并配置
6. [ ] 测试公共访问
7. [ ] 记录结果到本文档

## 📞 紧急方案

如果所有隧道方案都失败：
1. 使用ngrok作为备选: `ngrok http 8080`
2. 直接使用Docker端口映射测试
3. 在其他网络环境测试连接问题