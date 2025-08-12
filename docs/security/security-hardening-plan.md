# AirAccount安全加固计划

## 项目背景
AirAccount是基于TEE（可信执行环境）的Web3账户系统，使用OP-TEE在树莓派5上运行。由于项目涉及私钥管理和交易签名，安全性至关重要。

## 加固目标
1. 防范供应链攻击，特别是恶意依赖注入
2. 保护TEE环境中的敏感数据
3. 建立多层防护体系
4. 实现持续安全监控

## 第一阶段：立即执行（24小时内）

### 1.1 依赖审计和清理

```bash
# 安装必要的安全工具
cargo install cargo-audit
cargo install cargo-deny
cargo install cargo-outdated
cargo install cargo-geiger

# 执行初始审计
cargo audit
cargo tree --duplicates
```

### 1.2 创建安全配置文件

**deny.toml**:
```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"
notice = "warn"

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-3-Clause",
]
copyleft = "warn"

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"
skip = []

# 禁止已知的恶意包
deny = [
    { name = "rands" },
    { name = "rustdecimal" },
    { name = "lazystatic" },
    { name = "oncecell" },
    { name = "serd" },
    { name = "envlogger" },
    { name = "postgress" },
    { name = "if-cfg" },
    { name = "xrvrv" },
]

[sources]
unknown-registry = "deny"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

### 1.3 设置CI/CD安全检查

**.github/workflows/security.yml**:
```yaml
name: Security Audit

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]
  schedule:
    - cron: '0 0 * * *'  # 每日检查

jobs:
  security-audit:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    
    - name: Cache cargo registry
      uses: actions/cache@v3
      with:
        path: ~/.cargo/registry
        key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Install security tools
      run: |
        cargo install cargo-audit
        cargo install cargo-deny
        cargo install cargo-geiger
    
    - name: Run cargo audit
      run: cargo audit
    
    - name: Run cargo deny
      run: cargo deny check
    
    - name: Check for unsafe code
      run: cargo geiger
    
    - name: Check for suspicious patterns
      run: |
        ./scripts/security-check.sh
    
    - name: Upload security report
      if: failure()
      uses: actions/upload-artifact@v3
      with:
        name: security-report
        path: |
          cargo-audit.txt
          cargo-deny.txt
```

## 第二阶段：短期措施（1周内）

### 2.1 开发环境隔离

**Docker开发环境**:
```dockerfile
# Dockerfile.secure-dev
FROM rust:1.75

# 安装安全工具
RUN cargo install cargo-audit cargo-deny cargo-geiger cargo-outdated

# 创建非root用户
RUN useradd -m -s /bin/bash developer
USER developer

# 设置工作目录
WORKDIR /workspace

# 限制网络访问（在运行时使用--network=none）
```

### 2.2 Git Hooks配置

**.git/hooks/pre-commit**:
```bash
#!/bin/bash
# 预提交安全检查

echo "运行安全检查..."

# 检查敏感信息
if git diff --cached | grep -E "(SECRET|API_KEY|PRIVATE_KEY|PASSWORD)" ; then
    echo "错误: 检测到可能的敏感信息泄露"
    exit 1
fi

# 运行快速安全扫描
./scripts/security-check.sh

exit $?
```

### 2.3 依赖锁定策略

修改**Cargo.toml**:
```toml
[dependencies]
# 使用精确版本
serde = "=1.0.195"
tokio = "=1.35.1"

# TEE相关依赖 - 特别注意安全
teaclave-sgx-sdk = "=1.1.6"

[patch.crates-io]
# 如果需要修补特定包的安全问题
# vulnerable-crate = { git = "https://github.com/our-fork/fixed-crate" }
```

## 第三阶段：中期措施（1月内）

### 3.1 建立私有Registry

```bash
# 设置Nexus或Artifactory作为私有registry
# 配置~/.cargo/config.toml
cat >> ~/.cargo/config.toml << EOF
[registries]
airaccount = { 
    index = "https://registry.airaccount.internal/git/index" 
}

[source.crates-io]
replace-with = "airaccount-mirror"

[source.airaccount-mirror]
registry = "https://registry.airaccount.internal/api/v1/crates"
EOF
```

### 3.2 TEE特定安全措施

```rust
// 在TEE代码中添加额外的安全检查
use std::panic;

pub fn secure_init() {
    // 设置panic处理
    panic::set_hook(Box::new(|info| {
        // 清理敏感数据
        secure_cleanup();
        eprintln!("Security panic: {:?}", info);
    }));
    
    // 验证TEE环境
    verify_tee_environment();
    
    // 初始化安全监控
    init_security_monitoring();
}

fn verify_tee_environment() {
    // 检查是否在TEE中运行
    #[cfg(feature = "sgx")]
    {
        use sgx_types::*;
        let mut report = sgx_report_t::default();
        unsafe {
            sgx_create_report(std::ptr::null(), std::ptr::null(), &mut report);
        }
    }
}
```

### 3.3 监控和告警系统

```rust
// monitoring.rs
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SecurityMonitor {
    alerts: Arc<RwLock<Vec<SecurityAlert>>>,
}

impl SecurityMonitor {
    pub async fn check_dependency_changes(&self) {
        // 监控Cargo.lock变化
        let lock_hash = hash_file("Cargo.lock");
        if lock_hash != self.last_known_hash {
            self.raise_alert(SecurityAlert::DependencyChanged);
        }
    }
    
    pub async fn monitor_network_activity(&self) {
        // 监控异常网络活动
        // 特别关注TEE组件的网络行为
    }
}
```

## 第四阶段：长期措施（3月内）

### 4.1 零信任架构

1. **最小权限原则**
   - 每个模块只能访问必要的资源
   - TEE组件完全隔离

2. **持续验证**
   - 运行时依赖完整性检查
   - 定期重新验证所有组件

### 4.2 供应链安全自动化

```yaml
# supply-chain-security.yml
name: Supply Chain Security

on:
  schedule:
    - cron: '0 */6 * * *'  # 每6小时

jobs:
  sbom-generation:
    runs-on: ubuntu-latest
    steps:
      - name: Generate SBOM
        run: |
          cargo tree --format json > sbom.json
          cargo audit --json > audit.json
      
      - name: Upload to security database
        run: |
          curl -X POST https://security.airaccount.io/sbom \
            -H "Content-Type: application/json" \
            -d @sbom.json
```

### 4.3 安全培训和流程

1. **团队培训计划**
   - 供应链攻击识别培训
   - TEE安全最佳实践
   - 应急响应演练

2. **安全评审流程**
   - 所有新依赖必须经过安全团队评审
   - 定期的代码安全审计
   - 渗透测试

## 监控指标

### 关键安全指标（KSI）

| 指标 | 目标 | 当前状态 | 检查频率 |
|-----|------|---------|---------|
| 已知漏洞数量 | 0 | 待检查 | 每日 |
| 过期依赖比例 | <10% | 待检查 | 每周 |
| 安全审计通过率 | 100% | 待检查 | 每次提交 |
| TEE完整性验证 | 100% | 待检查 | 运行时 |
| 供应链风险评分 | <3/10 | 待检查 | 每月 |

## 应急响应计划

### 发现安全事件时的处理流程

1. **立即响应**（15分钟内）
   ```bash
   # 停止所有服务
   systemctl stop airaccount
   
   # 隔离受影响系统
   iptables -I INPUT -j DROP
   iptables -I OUTPUT -j DROP
   
   # 保存证据
   tar -czf evidence-$(date +%Y%m%d-%H%M%S).tar.gz \
     /var/log/ \
     ~/.cargo/ \
     ./target/
   ```

2. **调查分析**（2小时内）
   - 分析日志确定攻击向量
   - 识别受影响的组件
   - 评估数据泄露风险

3. **恢复和加固**（24小时内）
   - 从安全备份恢复
   - 修补已知漏洞
   - 更新安全策略

4. **事后总结**（1周内）
   - 编写事件报告
   - 更新安全流程
   - 分享经验教训

## 成功标准

- [ ] 零安全事件发生
- [ ] 所有依赖都经过审计
- [ ] TEE环境完全隔离
- [ ] 自动化安全检查覆盖率100%
- [ ] 团队安全意识显著提升

## 资源需求

1. **工具许可**
   - 商业安全扫描工具
   - 私有registry服务

2. **人力投入**
   - 专职安全工程师：1人
   - 开发团队安全培训：每月4小时

3. **基础设施**
   - 隔离的测试环境
   - 安全监控系统

## 时间表

| 阶段 | 时间 | 主要任务 | 负责人 |
|-----|------|---------|--------|
| 第一阶段 | 24小时 | 基础安全工具部署 | DevOps |
| 第二阶段 | 1周 | 环境隔离和Git hooks | 开发团队 |
| 第三阶段 | 1月 | TEE安全强化 | 安全团队 |
| 第四阶段 | 3月 | 零信任架构实施 | 架构团队 |

## 下一步行动

1. 立即运行security-check.sh脚本
2. 安装并配置cargo-deny
3. 设置GitHub Actions安全工作流
4. 审查所有现有依赖
5. 建立安全事件响应小组

---

**文档版本**: 1.0
**创建日期**: 2025-08-12
**下次审查**: 2025-08-19
**状态**: 待执行