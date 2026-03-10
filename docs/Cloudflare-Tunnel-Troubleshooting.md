# Cloudflare 隧道故障排除指南

*创建时间: Mon Sep 29 11:19:40 +07 2025*

## 🔧 常见问题和解决方案

### 问题1: 临时隧道返回 404 错误

**症状:**
- 使用 `cloudflared tunnel --url http://localhost:8080` 创建的临时隧道
- 访问隧道URL时返回 HTTP 404 错误
- 本地服务 `http://localhost:8080` 正常工作

**原因:**
临时隧道默认没有 ingress 配置，无法正确路由请求到本地服务。

**解决方案:**
```bash
# 1. 创建正确的临时隧道配置文件
cat > /tmp/temp-tunnel-fixed-config.yaml << EOF
ingress:
  - service: http://localhost:8080
    originRequest:
      noTLSVerify: true
      httpHostHeader: localhost
EOF

# 2. 使用配置启动临时隧道
cloudflared tunnel --config /tmp/temp-tunnel-fixed-config.yaml --url http://localhost:8080
```

**验证方法:**
```bash
# 获取隧道URL（从输出中复制）
TEMP_URL="https://[random-words].trycloudflare.com"

# 测试KMS API
curl -s "$TEMP_URL/" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -H "Content-Type: application/json" \
  -d '{"Description":"Test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
```

### 问题2: 为什么所有隧道都显示 4 个连接

**现象:**
无论创建什么隧道，都会看到 "4个连接已建立" 的消息。

**解释:**
这是 **正常行为**！Cloudflare 隧道默认创建 4 个连接到不同的边缘服务器，用于：
- 高可用性
- 负载均衡
- 故障转移

**日志示例:**
```
INF Registered tunnel connection connIndex=0 location=sin15
INF Registered tunnel connection connIndex=1 location=bkk02
INF Registered tunnel connection connIndex=2 location=bkk02
INF Registered tunnel connection connIndex=3 location=sin11
```

这不是错误，不需要修复！

### 问题3: DNS 记录配置错误 (Error 1033)

**症状:**
- 正式域名返回 HTTP 530 错误
- 错误代码: 1033
- DNS 记录已正确配置为 CNAME

**常见原因:**
1. DNS 传播时间（最多48小时，通常几分钟）
2. Cloudflare 缓存问题
3. 隧道ID与域名不匹配

**解决步骤:**
```bash
# 1. 验证DNS记录
dig kms2.aastar.io CNAME

# 2. 检查隧道状态
cloudflared tunnel list

# 3. 验证隧道配置
cat /tmp/kms-final-tunnel-config.yaml

# 4. 等待DNS传播（监控脚本）
while true; do
  HTTP_CODE=$(curl -s -w "%{http_code}" "https://kms2.aastar.io/" -o /dev/null)
  echo "$(date): HTTP $HTTP_CODE"
  [ "$HTTP_CODE" = "200" ] && break
  sleep 300
done
```

## 📝 最佳实践

### 1. 临时隧道配置模板
保存以下配置文件以备重用：
```yaml
# /tmp/temp-tunnel-standard-config.yaml
ingress:
  - service: http://localhost:8080
    originRequest:
      noTLSVerify: true
      httpHostHeader: localhost
```

### 2. 正式隧道配置模板
```yaml
# 正式隧道配置模板
tunnel: [TUNNEL_ID]
credentials-file: /Users/[USERNAME]/.cloudflared/[TUNNEL_ID].json

ingress:
  - hostname: [YOUR_DOMAIN]
    service: http://localhost:8080
    originRequest:
      httpHostHeader: [YOUR_DOMAIN]
      noTLSVerify: true
  - service: http_status:404
```

### 3. 快速故障排除清单

1. ✅ 本地服务是否运行？ `curl http://localhost:8080/health`
2. ✅ 隧道是否启动？ 检查进程和日志
3. ✅ 临时隧道有配置吗？ 使用上述配置文件
4. ✅ DNS记录正确吗？ `dig [domain] CNAME`
5. ✅ 等待DNS传播了吗？ 通常5-15分钟

## 🔍 调试命令

```bash
# 检查本地服务
curl -v http://localhost:8080/health

# 检查隧道进程
ps aux | grep cloudflared

# 检查隧道连接
cloudflared tunnel list

# 测试DNS解析
nslookup [domain]
dig [domain] CNAME

# 查看隧道日志
tail -f /tmp/tunnel.log
```

记住：遇到404错误时，首先检查是否使用了正确的 ingress 配置！