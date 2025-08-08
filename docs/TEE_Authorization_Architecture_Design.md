# AirAccount TEE 授权架构设计

## 摘要

本文档设计了 AirAccount 的四层 TEE 授权架构，解决 eth_wallet 权限控制缺陷，实现生产级安全的 Web3 钱包系统。

## 1. 授权需求对比分析

### 1.1 eth_wallet 现状分析

**现有权限模型**:
```rust
// eth_wallet 的简单权限模型
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    // 直接处理命令，无任何授权检查
    match Command::from(cmd_id) {
        Command::CreateWallet => create_wallet(&input)?,
        Command::SignTransaction => sign_transaction(&input)?,
        // ...
    }
}
```

**安全缺陷**:
- ❌ 无用户身份验证
- ❌ 无会话权限管理
- ❌ 无操作级别授权
- ❌ 无审计日志记录
- ❌ 无防重放攻击机制

### 1.2 AirAccount 授权需求

**业务场景需求**:
1. **Web2 用户体验**: 用户通过生物识别（指纹/面部）快速授权
2. **多钱包管理**: 用户可管理多个钱包，每个钱包有不同权限
3. **分级授权**: 不同操作需要不同级别的授权
4. **审计合规**: 记录所有关键操作，支持合规审计
5. **防攻击**: 防重放、防暴力破解、防权限提升

**技术需求**:
- 集成 WebAuthn/Passkey 技术
- 支持多因素认证 (MFA)
- 实现会话管理和超时控制
- 提供细粒度的操作权限控制
- 支持分布式授权（未来多节点场景）

## 2. 四层授权架构设计

### 2.1 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                    AirAccount 四层授权架构                      │
└─────────────────────────────────────────────────────────────┘

┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   第4层: 操作授权   │    │   第3层: 用户认证   │    │   第2层: 会话管理   │
│                 │    │                 │    │                 │
│ • 权限矩阵检查     │    │ • WebAuthn 验证   │    │ • 会话令牌管理     │
│ • 操作风险评估     │    │ • 生物识别认证     │    │ • 超时控制       │
│ • 动态策略执行     │    │ • MFA 验证       │    │ • 并发会话限制     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
          │                        │                        │
          └──────────────────────────┼────────────────────────┘
                                     │
                          ┌─────────────────┐
                          │   第1层: TA访问控制 │
                          │                 │
                          │ • TA 证书验证    │
                          │ • CA 身份认证    │
                          │ • 安全信道建立   │
                          └─────────────────┘
```

### 2.2 第1层: TA 访问控制

**职责**: 确保只有授权的客户端应用可以访问 TA

**实现机制**:

```rust
// TA 访问控制结构
#[derive(Debug, Clone)]
pub struct TAAccessControl {
    authorized_cas: HashSet<CAIdentity>,
    certificate_store: CertificateStore,
    secure_channel: SecureChannel,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CAIdentity {
    pub app_id: String,        // 应用标识符
    pub cert_hash: [u8; 32],   // 证书哈希
    pub public_key: [u8; 64],  // 公钥
}

impl TAAccessControl {
    // CA 身份验证
    pub fn authenticate_ca(&self, ca_cert: &Certificate) -> Result<CAIdentity> {
        // 1. 验证证书签名
        self.certificate_store.verify_certificate(ca_cert)?;
        
        // 2. 检查证书是否在授权列表中
        let ca_identity = CAIdentity::from_certificate(ca_cert)?;
        if !self.authorized_cas.contains(&ca_identity) {
            return Err(AuthError::UnauthorizedCA);
        }
        
        // 3. 建立安全信道
        self.secure_channel.establish(&ca_identity)?;
        
        Ok(ca_identity)
    }
    
    // 安全信道加密
    pub fn encrypt_response(&self, data: &[u8], ca_id: &CAIdentity) -> Result<Vec<u8>> {
        self.secure_channel.encrypt(data, ca_id)
    }
}
```

**安全特性**:
- ✅ 基于公钥证书的 CA 身份验证
- ✅ 授权 CA 白名单机制
- ✅ 端到端加密通信信道
- ✅ 防中间人攻击

### 2.3 第2层: 会话管理

**职责**: 管理客户端会话，防止会话劫持和重放攻击

**实现机制**:

```rust
// 会话管理器
#[derive(Debug)]
pub struct SessionManager {
    active_sessions: HashMap<SessionId, SessionInfo>,
    session_config: SessionConfig,
    rng: SecureRng,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub ca_identity: CAIdentity,
    pub user_id: Option<UserId>,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    pub permissions: SessionPermissions,
    pub nonce_counter: u64,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub session_timeout: Duration,      // 会话超时时间
    pub max_concurrent_sessions: u32,   // 最大并发会话数
    pub require_heartbeat: bool,        // 是否需要心跳
    pub anti_replay_window: Duration,   // 防重放时间窗口
}

impl SessionManager {
    // 创建新会话
    pub fn create_session(&mut self, ca_identity: CAIdentity) -> Result<SessionToken> {
        // 1. 检查并发会话限制
        self.check_session_limit(&ca_identity)?;
        
        // 2. 生成安全的会话ID和令牌
        let session_id = self.generate_session_id();
        let session_token = self.generate_session_token();
        
        // 3. 创建会话信息
        let session_info = SessionInfo {
            session_id: session_id.clone(),
            ca_identity,
            user_id: None, // 用户认证后填充
            created_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            permissions: SessionPermissions::default(),
            nonce_counter: 0,
        };
        
        // 4. 存储会话
        self.active_sessions.insert(session_id, session_info);
        
        Ok(session_token)
    }
    
    // 验证会话
    pub fn validate_session(&mut self, token: &SessionToken, nonce: u64) -> Result<&mut SessionInfo> {
        let session_id = self.extract_session_id(token)?;
        let session = self.active_sessions.get_mut(&session_id)
            .ok_or(AuthError::InvalidSession)?;
        
        // 1. 检查会话超时
        if session.last_activity.elapsed()? > self.session_config.session_timeout {
            self.active_sessions.remove(&session_id);
            return Err(AuthError::SessionExpired);
        }
        
        // 2. 防重放攻击检查
        if nonce <= session.nonce_counter {
            return Err(AuthError::ReplayAttack);
        }
        
        // 3. 更新会话状态
        session.last_activity = SystemTime::now();
        session.nonce_counter = nonce;
        
        Ok(session)
    }
    
    // 清理过期会话
    pub fn cleanup_expired_sessions(&mut self) {
        let now = SystemTime::now();
        self.active_sessions.retain(|_, session| {
            session.last_activity.elapsed().unwrap_or_default() <= self.session_config.session_timeout
        });
    }
}
```

**安全特性**:
- ✅ 基于加密令牌的会话标识
- ✅ 会话超时自动清理机制
- ✅ 防重放攻击 nonce 验证
- ✅ 并发会话数量限制
- ✅ 会话状态实时监控

### 2.4 第3层: 用户认证

**职责**: 验证用户身份，支持多种认证方式

**实现机制**:

```rust
// 用户认证管理器
#[derive(Debug)]
pub struct UserAuthManager {
    auth_providers: HashMap<AuthType, Box<dyn AuthProvider>>,
    user_store: UserStore,
    auth_policies: AuthPolicyStore,
}

#[derive(Debug, Clone)]
pub enum AuthType {
    WebAuthn,
    Biometric,
    PIN,
    Hardware,    // 硬件密钥
}

// WebAuthn 认证提供者
#[derive(Debug)]
pub struct WebAuthnProvider {
    rp_id: String,
    origin: String,
    credential_store: CredentialStore,
}

impl AuthProvider for WebAuthnProvider {
    // 开始认证流程
    fn begin_authentication(&self, user_id: &UserId) -> Result<AuthChallenge> {
        // 1. 获取用户凭据列表
        let credentials = self.credential_store.get_user_credentials(user_id)?;
        if credentials.is_empty() {
            return Err(AuthError::NoCredentials);
        }
        
        // 2. 生成认证挑战
        let challenge = self.generate_challenge();
        let auth_request = AuthenticatorAssertionRequest {
            challenge: challenge.clone(),
            rp_id: self.rp_id.clone(),
            allowed_credentials: credentials,
            user_verification: UserVerificationRequirement::Required,
        };
        
        Ok(AuthChallenge {
            challenge_id: challenge.id,
            auth_request,
            expires_at: SystemTime::now() + Duration::from_secs(300), // 5分钟过期
        })
    }
    
    // 验证认证响应
    fn verify_authentication(&self, challenge: &AuthChallenge, response: &AuthResponse) -> Result<AuthResult> {
        // 1. 验证挑战是否过期
        if SystemTime::now() > challenge.expires_at {
            return Err(AuthError::ChallengeExpired);
        }
        
        // 2. 解析认证响应
        let assertion = AuthenticatorAssertionResponse::from_bytes(&response.data)?;
        
        // 3. 验证签名
        let credential = self.credential_store.get_credential(&assertion.credential_id)?;
        let verification_data = self.build_verification_data(&challenge, &assertion)?;
        
        if !credential.verify_signature(&verification_data, &assertion.signature)? {
            return Err(AuthError::InvalidSignature);
        }
        
        // 4. 验证用户验证标志
        if assertion.user_verified != true {
            return Err(AuthError::UserVerificationFailed);
        }
        
        Ok(AuthResult {
            user_id: credential.user_id.clone(),
            auth_method: AuthType::WebAuthn,
            auth_level: AuthLevel::High,
            expires_at: SystemTime::now() + Duration::from_secs(3600), // 1小时有效
        })
    }
}

// 生物识别认证提供者
#[derive(Debug)]
pub struct BiometricProvider {
    template_store: BiometricTemplateStore,
    liveness_detector: LivenessDetector,
}

impl AuthProvider for BiometricProvider {
    fn begin_authentication(&self, user_id: &UserId) -> Result<AuthChallenge> {
        // 1. 检查用户是否注册了生物识别
        if !self.template_store.has_template(user_id)? {
            return Err(AuthError::NotRegistered);
        }
        
        // 2. 生成生物识别挑战
        let challenge = BiometricChallenge {
            challenge_id: self.generate_challenge_id(),
            user_id: user_id.clone(),
            required_quality: BiometricQuality::High,
            liveness_required: true,
        };
        
        Ok(AuthChallenge::Biometric(challenge))
    }
    
    fn verify_authentication(&self, challenge: &AuthChallenge, response: &AuthResponse) -> Result<AuthResult> {
        let biometric_response = response.as_biometric()?;
        
        // 1. 活体检测
        if challenge.liveness_required && !self.liveness_detector.verify(&biometric_response.image)? {
            return Err(AuthError::LivenessCheckFailed);
        }
        
        // 2. 模板匹配
        let stored_template = self.template_store.get_template(&challenge.user_id)?;
        let match_score = self.match_templates(&stored_template, &biometric_response.template)?;
        
        if match_score < BiometricThreshold::AUTHENTICATION {
            return Err(AuthError::BiometricMismatch);
        }
        
        Ok(AuthResult {
            user_id: challenge.user_id.clone(),
            auth_method: AuthType::Biometric,
            auth_level: AuthLevel::High,
            expires_at: SystemTime::now() + Duration::from_secs(1800), // 30分钟有效
        })
    }
}

// 多因素认证管理
impl UserAuthManager {
    // 执行用户认证
    pub fn authenticate_user(&mut self, session: &mut SessionInfo, auth_request: &AuthRequest) -> Result<AuthResult> {
        // 1. 获取用户的认证策略
        let auth_policy = self.auth_policies.get_policy(&auth_request.user_id)?;
        
        // 2. 根据策略选择认证方法
        let required_methods = self.determine_required_methods(&auth_policy, &auth_request.operation)?;
        
        // 3. 执行多因素认证
        let mut auth_results = Vec::new();
        for auth_type in required_methods {
            let provider = self.auth_providers.get(&auth_type)
                .ok_or(AuthError::UnsupportedAuthMethod)?;
            
            let result = provider.authenticate(&auth_request)?;
            auth_results.push(result);
        }
        
        // 4. 合并认证结果
        let combined_result = self.combine_auth_results(auth_results)?;
        
        // 5. 更新会话用户信息
        session.user_id = Some(combined_result.user_id.clone());
        session.permissions.auth_level = combined_result.auth_level;
        
        Ok(combined_result)
    }
}
```

**安全特性**:
- ✅ 多种认证方式支持（WebAuthn、生物识别、PIN、硬件密钥）
- ✅ 多因素认证 (MFA) 支持
- ✅ 活体检测防欺骗
- ✅ 认证结果有效期控制
- ✅ 认证策略可配置

### 2.5 第4层: 操作授权

**职责**: 基于用户权限和操作风险进行细粒度授权控制

**实现机制**:

```rust
// 操作授权管理器
#[derive(Debug)]
pub struct OperationAuthManager {
    permission_matrix: PermissionMatrix,
    risk_evaluator: RiskEvaluator,
    audit_logger: AuditLogger,
}

#[derive(Debug, Clone)]
pub struct PermissionMatrix {
    user_permissions: HashMap<UserId, UserPermissions>,
    role_permissions: HashMap<Role, RolePermissions>,
    operation_requirements: HashMap<Operation, OperationRequirement>,
}

#[derive(Debug, Clone)]
pub struct UserPermissions {
    pub user_id: UserId,
    pub roles: HashSet<Role>,
    pub wallet_permissions: HashMap<WalletId, WalletPermissions>,
    pub global_permissions: GlobalPermissions,
}

#[derive(Debug, Clone)]
pub struct WalletPermissions {
    pub wallet_id: WalletId,
    pub can_view: bool,
    pub can_derive_address: bool,
    pub can_sign_transaction: bool,
    pub can_manage: bool,
    pub transaction_limits: TransactionLimits,
}

#[derive(Debug, Clone)]
pub struct TransactionLimits {
    pub max_amount_per_tx: Option<u128>,      // 单笔交易限额
    pub max_amount_per_day: Option<u128>,     // 每日限额
    pub allowed_recipients: Option<HashSet<Address>>, // 允许的收款地址
    pub require_confirmation: bool,           // 是否需要二次确认
}

#[derive(Debug, Clone)]
pub enum Operation {
    CreateWallet,
    RemoveWallet,
    DeriveAddress { wallet_id: WalletId, hd_path: String },
    SignTransaction { wallet_id: WalletId, transaction: Transaction },
    ExportPrivateKey { wallet_id: WalletId },
    BackupWallet { wallet_id: WalletId },
}

impl OperationAuthManager {
    // 授权检查
    pub fn authorize_operation(
        &self,
        session: &SessionInfo,
        operation: &Operation,
    ) -> Result<AuthorizationResult> {
        // 1. 基础权限检查
        let user_id = session.user_id.as_ref()
            .ok_or(AuthError::UserNotAuthenticated)?;
        
        let user_permissions = self.permission_matrix.user_permissions.get(user_id)
            .ok_or(AuthError::UserNotFound)?;
        
        // 2. 操作特定权限检查
        match operation {
            Operation::SignTransaction { wallet_id, transaction } => {
                self.authorize_transaction(user_permissions, wallet_id, transaction)?;
            },
            Operation::ExportPrivateKey { wallet_id } => {
                self.authorize_sensitive_operation(user_permissions, wallet_id, session)?;
            },
            _ => {
                self.authorize_basic_operation(user_permissions, operation)?;
            }
        }
        
        // 3. 风险评估
        let risk_score = self.risk_evaluator.evaluate_operation(session, operation)?;
        if risk_score > RiskThreshold::HIGH {
            return Ok(AuthorizationResult::RequireAdditionalAuth(
                self.determine_additional_auth(risk_score)?
            ));
        }
        
        // 4. 记录授权日志
        self.audit_logger.log_authorization_event(AuditEvent {
            session_id: session.session_id.clone(),
            user_id: user_id.clone(),
            operation: operation.clone(),
            result: AuthorizationResult::Approved,
            risk_score,
            timestamp: SystemTime::now(),
        })?;
        
        Ok(AuthorizationResult::Approved)
    }
    
    // 交易授权检查
    fn authorize_transaction(
        &self,
        user_permissions: &UserPermissions,
        wallet_id: &WalletId,
        transaction: &Transaction,
    ) -> Result<()> {
        let wallet_perms = user_permissions.wallet_permissions.get(wallet_id)
            .ok_or(AuthError::WalletAccessDenied)?;
        
        // 1. 基本签名权限检查
        if !wallet_perms.can_sign_transaction {
            return Err(AuthError::InsufficientPermissions);
        }
        
        // 2. 交易金额限制检查
        let limits = &wallet_perms.transaction_limits;
        if let Some(max_amount) = limits.max_amount_per_tx {
            if transaction.value > max_amount {
                return Err(AuthError::TransactionLimitExceeded);
            }
        }
        
        // 3. 每日限额检查
        if let Some(daily_limit) = limits.max_amount_per_day {
            let today_total = self.get_daily_transaction_total(wallet_id)?;
            if today_total + transaction.value > daily_limit {
                return Err(AuthError::DailyLimitExceeded);
            }
        }
        
        // 4. 收款地址白名单检查
        if let Some(allowed_recipients) = &limits.allowed_recipients {
            if let Some(recipient) = transaction.to {
                if !allowed_recipients.contains(&recipient) {
                    return Err(AuthError::RecipientNotAllowed);
                }
            }
        }
        
        Ok(())
    }
    
    // 敏感操作授权检查
    fn authorize_sensitive_operation(
        &self,
        user_permissions: &UserPermissions,
        wallet_id: &WalletId,
        session: &SessionInfo,
    ) -> Result<()> {
        // 1. 检查是否有管理权限
        let wallet_perms = user_permissions.wallet_permissions.get(wallet_id)
            .ok_or(AuthError::WalletAccessDenied)?;
        
        if !wallet_perms.can_manage {
            return Err(AuthError::InsufficientPermissions);
        }
        
        // 2. 检查认证级别
        if session.permissions.auth_level < AuthLevel::High {
            return Err(AuthError::AuthLevelTooLow);
        }
        
        // 3. 检查最近认证时间
        let auth_age = session.last_activity.elapsed().unwrap_or_default();
        if auth_age > Duration::from_secs(300) { // 5分钟内的认证
            return Err(AuthError::RecentAuthRequired);
        }
        
        Ok(())
    }
}

// 风险评估器
#[derive(Debug)]
pub struct RiskEvaluator {
    risk_models: HashMap<RiskFactor, RiskModel>,
}

#[derive(Debug, Clone)]
pub enum RiskFactor {
    TransactionAmount,
    RecipientAddress,
    GeographicLocation,
    DeviceFingerprint,
    TimePattern,
    VelocityPattern,
}

impl RiskEvaluator {
    pub fn evaluate_operation(&self, session: &SessionInfo, operation: &Operation) -> Result<RiskScore> {
        let mut total_score = RiskScore::LOW;
        
        // 1. 交易金额风险评估
        if let Operation::SignTransaction { transaction, .. } = operation {
            let amount_risk = self.evaluate_transaction_amount(transaction.value)?;
            total_score = total_score.max(amount_risk);
        }
        
        // 2. 设备指纹风险评估
        let device_risk = self.evaluate_device_fingerprint(session)?;
        total_score = total_score.max(device_risk);
        
        // 3. 行为模式风险评估
        let behavior_risk = self.evaluate_behavior_pattern(session, operation)?;
        total_score = total_score.max(behavior_risk);
        
        Ok(total_score)
    }
}
```

**安全特性**:
- ✅ 细粒度的操作权限控制
- ✅ 基于角色的访问控制 (RBAC)
- ✅ 交易限额和白名单机制
- ✅ 实时风险评估和适应性授权
- ✅ 完整的审计日志记录

## 3. 集成实现方案

### 3.1 增强的 TA 入口点

```rust
// 增强的命令处理器
#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> optee_utee::Result<()> {
    let mut authorization_context = AuthorizationContext::new();
    
    // 解析输入参数
    let mut p0 = unsafe { params.0.as_memref()? };
    let mut p1 = unsafe { params.1.as_memref()? };
    let mut p2 = unsafe { params.2.as_value()? };
    
    let input_data = p0.buffer();
    let command = Command::from(cmd_id);
    
    // 执行四层授权检查
    let auth_result = match authorization_context.full_authorization_check(command, input_data) {
        Ok(result) => result,
        Err(e) => {
            let error_response = AuthErrorResponse {
                error: e,
                timestamp: SystemTime::now(),
                session_id: None,
            };
            
            let serialized_error = bincode::serialize(&error_response)?;
            p1.buffer().write(&serialized_error)?;
            p2.set_a(serialized_error.len() as u32);
            
            return Err(Error::new(ErrorKind::AccessDenied));
        }
    };
    
    // 执行经过授权的操作
    let output = match handle_authorized_invoke(command, input_data, &auth_result) {
        Ok(output) => output,
        Err(e) => {
            // 记录操作失败审计日志
            authorization_context.audit_logger.log_operation_failure(&auth_result, &e)?;
            return Err(e.into());
        }
    };
    
    // 记录操作成功审计日志
    authorization_context.audit_logger.log_operation_success(&auth_result, &output)?;
    
    // 返回结果
    p1.buffer().write(&output)?;
    p2.set_a(output.len() as u32);
    
    Ok(())
}
```

### 3.2 WebAuthn 集成方案

```rust
// WebAuthn 集成示例
pub struct WebAuthnIntegration {
    client: WebAuthnClient,
    credential_store: TEECredentialStore,
}

impl WebAuthnIntegration {
    // 注册新的 WebAuthn 凭据
    pub fn register_credential(&mut self, user_id: &UserId, registration_response: &RegisterResponse) -> Result<()> {
        // 1. 验证注册响应
        let credential = self.client.finish_registration(registration_response)?;
        
        // 2. 存储凭据到 TEE 安全存储
        self.credential_store.store_credential(user_id, &credential)?;
        
        // 3. 记录注册事件
        self.audit_logger.log_credential_registration(user_id, &credential.id)?;
        
        Ok(())
    }
    
    // 认证用户
    pub fn authenticate(&self, auth_response: &AuthResponse) -> Result<UserId> {
        // 1. 从 TEE 安全存储加载凭据
        let credential = self.credential_store.get_credential(&auth_response.credential_id)?;
        
        // 2. 验证认证响应
        let user_id = self.client.finish_authentication(&credential, auth_response)?;
        
        // 3. 记录认证事件
        self.audit_logger.log_authentication_success(&user_id)?;
        
        Ok(user_id)
    }
}
```

## 4. 安全测试和验证

### 4.1 安全测试用例

```rust
#[cfg(test)]
mod authorization_tests {
    use super::*;
    
    // 测试未授权访问
    #[test]
    fn test_unauthorized_access() {
        let mut auth_context = AuthorizationContext::new();
        
        // 未经认证的用户尝试签名交易
        let result = auth_context.authorize_operation(
            &create_unauthenticated_session(),
            &Operation::SignTransaction {
                wallet_id: WalletId::new(),
                transaction: create_test_transaction(),
            }
        );
        
        assert_eq!(result.unwrap_err(), AuthError::UserNotAuthenticated);
    }
    
    // 测试权限提升攻击
    #[test]
    fn test_privilege_escalation() {
        let mut auth_context = AuthorizationContext::new();
        let session = create_low_privilege_session();
        
        // 低权限用户尝试导出私钥
        let result = auth_context.authorize_operation(
            &session,
            &Operation::ExportPrivateKey {
                wallet_id: session.user_permissions.wallet_ids[0].clone(),
            }
        );
        
        assert_eq!(result.unwrap_err(), AuthError::InsufficientPermissions);
    }
    
    // 测试重放攻击防护
    #[test]
    fn test_replay_attack_protection() {
        let mut session_mgr = SessionManager::new();
        let session_token = session_mgr.create_session(create_test_ca_identity()).unwrap();
        
        // 第一次请求成功
        let nonce1 = 100;
        assert!(session_mgr.validate_session(&session_token, nonce1).is_ok());
        
        // 重放相同 nonce 失败
        let result = session_mgr.validate_session(&session_token, nonce1);
        assert_eq!(result.unwrap_err(), AuthError::ReplayAttack);
        
        // 较小的 nonce 失败
        let nonce2 = 99;
        let result = session_mgr.validate_session(&session_token, nonce2);
        assert_eq!(result.unwrap_err(), AuthError::ReplayAttack);
    }
}
```

### 4.2 性能基准测试

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn benchmark_authorization_check() {
        let mut auth_context = AuthorizationContext::new();
        let session = create_authorized_session();
        let operation = Operation::DeriveAddress {
            wallet_id: WalletId::new(),
            hd_path: "m/44'/60'/0'/0/0".to_string(),
        };
        
        let iterations = 1000;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = auth_context.authorize_operation(&session, &operation).unwrap();
        }
        
        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;
        
        println!("平均授权检查时间: {:?}", avg_time);
        
        // 断言授权检查应该在 1ms 以内完成
        assert!(avg_time < Duration::from_millis(1));
    }
}
```

## 5. 部署和运维

### 5.1 配置管理

```rust
// 授权配置
#[derive(Debug, Deserialize)]
pub struct AuthorizationConfig {
    pub session_config: SessionConfig,
    pub auth_policies: AuthPolicyConfig,
    pub risk_thresholds: RiskThresholdConfig,
    pub audit_config: AuditConfig,
}

// 运维接口
impl AuthorizationContext {
    // 动态更新授权策略
    pub fn update_auth_policy(&mut self, user_id: &UserId, policy: AuthPolicy) -> Result<()> {
        self.permission_matrix.update_user_policy(user_id, policy)?;
        self.audit_logger.log_policy_update(user_id)?;
        Ok(())
    }
    
    // 获取授权统计信息
    pub fn get_auth_statistics(&self) -> AuthStatistics {
        AuthStatistics {
            active_sessions: self.session_manager.active_session_count(),
            auth_success_rate: self.get_success_rate(),
            high_risk_operations: self.get_high_risk_count(),
            blocked_attempts: self.get_blocked_count(),
        }
    }
}
```

## 6. 总结

### 6.1 架构优势

1. **分层防护**: 四层授权提供深度防御
2. **灵活配置**: 支持动态策略调整
3. **标准兼容**: 兼容 WebAuthn/FIDO2 标准
4. **性能优化**: 缓存和批处理优化性能
5. **审计合规**: 完整的操作审计记录

### 6.2 安全保障

- ✅ 防权限提升攻击
- ✅ 防会话劫持
- ✅ 防重放攻击
- ✅ 防暴力破解
- ✅ 支持零信任架构

### 6.3 下一步工作

1. **原型实现**: 基于设计实现完整的授权系统原型
2. **安全测试**: 进行全面的渗透测试和安全评估
3. **性能调优**: 优化授权检查性能，确保用户体验
4. **集成测试**: 与 eth_wallet 集成，验证兼容性

---

**设计完成时间**: 2025-01-08  
**架构师**: Claude AI Assistant  
**版本**: v1.0