# 开发-测试-发布临时通道测试步骤

*创建时间: 2025-09-29*

## 分阶段测试策略

### 阶段1: API设计验证 (当前阶段)

#### 1.1 目标
验证KMS API映射设计的正确性，无需完整的TA实现。

#### 1.2 测试组件
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   AWS KMS API   │───▶│  Protocol       │───▶│   Mock TA       │
│   (HTTP/JSON)   │    │  Converter      │    │ (Fixed Keys)    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

#### 1.3 实现计划
- [x] 完成KMS API设计文档
- [ ] 创建Mock TA模拟器
- [ ] 实现协议转换层
- [ ] 编写API兼容性测试
- [ ] 验证AWS KMS客户端兼容性

#### 1.4 测试用例

##### CreateKey API测试
```bash
curl -X POST http://localhost:8080/ \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -H "Content-Type: application/json" \
  -d '{
    "Description": "Test ETH Key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }'

# 期望响应:
{
  "KeyMetadata": {
    "KeyId": "uuid-v4",
    "Description": "Test ETH Key",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "KeyState": "Enabled"
  }
}
```

##### GetPublicKey API测试
```bash
curl -X POST http://localhost:8080/ \
  -H "X-Amz-Target: TrentService.GetPublicKey" \
  -H "Content-Type: application/json" \
  -d '{
    "KeyId": "uuid-from-create"
  }'

# 期望响应:
{
  "KeyId": "uuid-from-create",
  "PublicKey": "base64-encoded-public-key",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1"
}
```

##### Sign API测试
```bash
curl -X POST http://localhost:8080/ \
  -H "X-Amz-Target: TrentService.Sign" \
  -H "Content-Type: application/json" \
  -d '{
    "KeyId": "uuid-from-create",
    "Message": "SGVsbG8gV29ybGQ=",
    "MessageType": "RAW",
    "SigningAlgorithm": "ECDSA_SHA_256"
  }'

# 期望响应:
{
  "KeyId": "uuid-from-create",
  "Signature": "base64-encoded-signature",
  "SigningAlgorithm": "ECDSA_SHA_256"
}
```

### 阶段2: 集成测试 (中期)

#### 2.1 目标
验证完整的端到端流程，包括真实的crypto操作。

#### 2.2 测试组件
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   KMS Client    │───▶│     Host        │───▶│   Real TA       │
│   (AWS SDK)     │    │  Application    │    │ (OP-TEE)        │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

#### 2.3 实现计划
- [ ] 设置完整OP-TEE环境
- [ ] 构建真实的eth_wallet TA
- [ ] 实现Host应用程序
- [ ] QEMU环境集成测试
- [ ] 性能基准测试

### 阶段3: 生产就绪测试 (后期)

#### 3.1 目标
验证在真实硬件上的部署和运行。

#### 3.2 测试组件
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Production     │───▶│  Raspberry Pi   │───▶│   Hardware      │
│  KMS Service    │    │  OP-TEE Host    │    │     TEE         │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 当前实施计划

### 第1周: Mock实现
```bash
# 1. 创建Mock TA服务器
cd kms/
cargo new --bin mock-kms-server
cd mock-kms-server/

# 2. 实现基础HTTP服务器
# - 支持AWS KMS API格式
# - 使用固定测试密钥
# - 实现所有5个核心API

# 3. 测试脚本
./scripts/test-kms-apis.sh
```

### 第2周: 协议转换
```bash
# 1. 实现完整的协议转换层
# 2. 添加错误处理和边界情况
# 3. AWS SDK兼容性测试
# 4. 文档完善
```

### 第3周: 集成准备
```bash
# 1. OP-TEE环境搭建
# 2. TA构建流程调试
# 3. Host应用程序实现
# 4. QEMU测试环境
```

## 测试自动化

### CI/CD管道
```yaml
name: KMS API Tests
on: [push, pull_request]

jobs:
  api-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
      - name: Run Mock KMS Server
        run: cargo run --bin mock-kms-server &
      - name: Test APIs
        run: ./scripts/test-kms-apis-full.sh
      - name: Validate AWS Compatibility
        run: python tests/aws-sdk-compatibility-test.py
```

### 测试覆盖目标
- [ ] API格式兼容性: 100%
- [ ] 错误处理: 95%
- [ ] 性能基准: 符合预期
- [ ] 安全测试: 通过所有检查

## 发布策略

### 版本规划
- **v0.1-mock**: Mock实现，API验证
- **v0.2-qemu**: QEMU集成测试版本
- **v0.3-hardware**: 硬件部署测试版本
- **v1.0**: 生产就绪版本

### 发布检查清单
- [ ] 所有自动化测试通过
- [ ] 安全审计完成
- [ ] 性能基准达标
- [ ] 文档完善
- [ ] 向后兼容性验证

---

## 即时行动项

### 优先级1 (本周)
1. 实现Mock KMS服务器
2. 创建基础API测试套件
3. 验证AWS KMS客户端兼容性

### 优先级2 (下周)
1. 完善协议转换层
2. 添加全面的错误处理
3. 性能基准测试

### 优先级3 (后续)
1. OP-TEE环境搭建
2. 真实TA实现
3. 硬件部署测试

这个分阶段策略可以让我们在不被复杂的TA构建问题阻塞的情况下，快速验证API设计并取得可见进展。