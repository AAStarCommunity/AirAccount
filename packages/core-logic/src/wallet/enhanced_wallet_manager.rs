// Licensed to AirAccount under the Apache License, Version 2.0
// Enhanced wallet manager integrating all security improvements

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::security::{
    SecurityManager, KeyDerivationManager, KdfParams, DerivedKey,
    TEEEntropySource, TamperProofAuditLog, AuditEvent, AuditLevel,
    // SecureMemory, SecureBytes, // 保留以备将来使用
    BatchAuditProcessor, BatchAuditConfig,
    SecureMemoryPool, SecureMemoryBlock, SimdMemoryOps
};
use super::{WalletError, WalletResult, WalletCore};

/// 增强的钱包配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedWalletConfig {
    /// 使用增强的密钥派生
    pub use_kdf: bool,
    /// KDF参数配置
    pub kdf_params: KdfParams,
    /// 启用防篡改审计
    pub enable_tamper_proof_audit: bool,
    /// 启用高质量熵源
    pub use_tee_entropy: bool,
    /// 钱包锁定超时（秒）
    pub lock_timeout_seconds: u64,
    /// 最大并发会话数
    pub max_concurrent_sessions: usize,
    /// 启用多重签名支持
    pub enable_multisig: bool,
    /// 启用批量审计处理
    pub enable_batch_audit: bool,
    /// 启用安全内存池
    pub enable_memory_pool: bool,
    /// 启用SIMD优化
    pub enable_simd_ops: bool,
}

impl Default for EnhancedWalletConfig {
    fn default() -> Self {
        Self {
            use_kdf: true,
            kdf_params: KdfParams::default(),
            enable_tamper_proof_audit: true,
            use_tee_entropy: true,
            lock_timeout_seconds: 300, // 5分钟
            max_concurrent_sessions: 5,
            enable_multisig: false,
            enable_batch_audit: true,
            enable_memory_pool: true,
            enable_simd_ops: true,
        }
    }
}

/// 钱包会话信息
#[derive(Debug, Clone)]
pub struct WalletSession {
    pub session_id: Uuid,
    pub wallet_id: Uuid,
    pub created_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
    pub is_authenticated: bool,
    pub permissions: WalletPermissions,
}

/// 钱包权限
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletPermissions {
    pub can_sign: bool,
    pub can_export_mnemonic: bool,
    pub can_derive_keys: bool,
    pub can_change_settings: bool,
    pub max_transaction_value: Option<u64>,
    pub allowed_operations: Vec<String>,
}

impl Default for WalletPermissions {
    fn default() -> Self {
        Self {
            can_sign: true,
            can_export_mnemonic: false, // 默认不允许导出助记词
            can_derive_keys: true,
            can_change_settings: false,
            max_transaction_value: None,
            allowed_operations: vec!["sign_transaction".to_string(), "get_address".to_string()],
        }
    }
}

/// 增强的钱包条目
#[derive(Debug)]
struct EnhancedWalletEntry {
    _wallet: WalletCore,
    derived_key: Option<DerivedKey>,
    access_count: u64,
    last_access: std::time::SystemTime,
    is_locked: bool,
    lock_reason: Option<String>,
}

/// 增强的钱包管理器
pub struct EnhancedWalletManager {
    config: EnhancedWalletConfig,
    security_manager: SecurityManager,
    kdf_manager: Option<KeyDerivationManager>,
    entropy_source: Option<TEEEntropySource>,
    audit_log: Option<TamperProofAuditLog>,
    batch_audit: Option<BatchAuditProcessor>,
    memory_pool: Option<SecureMemoryPool>,
    simd_ops: Option<SimdMemoryOps>,
    wallets: Arc<Mutex<HashMap<Uuid, EnhancedWalletEntry>>>,
    sessions: Arc<Mutex<HashMap<Uuid, WalletSession>>>,
}

impl EnhancedWalletManager {
    /// 创建新的增强钱包管理器
    pub fn new(config: EnhancedWalletConfig) -> WalletResult<Self> {
        let security_manager = SecurityManager::new(Default::default());
        
        // 初始化KDF管理器
        let kdf_manager = if config.use_kdf {
            Some(KeyDerivationManager::new(config.kdf_params.clone())
                .map_err(|e| WalletError::SecurityError(format!("Failed to create KDF manager: {}", e)))?)
        } else {
            None
        };
        
        // 初始化TEE熵源
        let entropy_source = if config.use_tee_entropy {
            Some(TEEEntropySource::new()
                .map_err(|e| WalletError::SecurityError(format!("Failed to create entropy source: {}", e)))?)
        } else {
            None
        };
        
        // 初始化防篡改审计日志
        let audit_log = if config.enable_tamper_proof_audit {
            Some(TamperProofAuditLog::new()
                .map_err(|e| WalletError::SecurityError(format!("Failed to create audit log: {}", e)))?)
        } else {
            None
        };
        
        // 初始化批量审计处理器
        let batch_audit = if config.enable_batch_audit {
            Some(BatchAuditProcessor::new(BatchAuditConfig::default()))
        } else {
            None
        };
        
        // 初始化安全内存池
        let memory_pool = if config.enable_memory_pool {
            Some(SecureMemoryPool::new()
                .map_err(|e| WalletError::SecurityError(format!("Failed to create memory pool: {}", e)))?)
        } else {
            None
        };
        
        // 初始化SIMD操作器
        let simd_ops = if config.enable_simd_ops {
            Some(SimdMemoryOps::new())
        } else {
            None
        };
        
        Ok(Self {
            config,
            security_manager,
            kdf_manager,
            entropy_source,
            audit_log,
            batch_audit,
            memory_pool,
            simd_ops,
            wallets: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// 使用默认配置创建
    pub fn with_defaults() -> WalletResult<Self> {
        Self::new(EnhancedWalletConfig::default())
    }
    
    /// 创建增强的钱包
    pub fn create_enhanced_wallet(&mut self, password: Option<&str>) -> WalletResult<Uuid> {
        let start_time = std::time::Instant::now();
        
        self.audit_operation("create_enhanced_wallet", "Starting enhanced wallet creation")?;
        
        // 使用增强的熵源生成更好的随机性
        if let Some(entropy_source) = &mut self.entropy_source {
            let mut entropy_buffer = vec![0u8; 64];
            entropy_source.gather_entropy(&mut entropy_buffer)
                .map_err(|e| WalletError::SecurityError(format!("Failed to gather entropy: {}", e)))?;
            
            self.audit_operation("entropy_collection", 
                &format!("Collected {} bytes of high-quality entropy", entropy_buffer.len()))?;
        }
        
        // 创建基础钱包
        let wallet = WalletCore::new(&self.security_manager)
            .map_err(|e| WalletError::SecurityError(format!("Failed to create wallet core: {}", e)))?;
        
        let wallet_id = wallet.get_id();
        
        // 如果提供了密码，使用KDF派生密钥
        let derived_key = if let (Some(password), Some(kdf_manager)) = (password, &mut self.kdf_manager) {
            let key = kdf_manager.derive_key(password.as_bytes())
                .map_err(|e| WalletError::CryptographicError(format!("KDF failed: {}", e)))?;
            
            self.audit_operation("kdf_derivation", "Password-based key derivation completed")?;
            Some(key)
        } else {
            None
        };
        
        // 创建增强的钱包条目
        let enhanced_entry = EnhancedWalletEntry {
            _wallet: wallet,
            derived_key,
            access_count: 0,
            last_access: std::time::SystemTime::now(),
            is_locked: false,
            lock_reason: None,
        };
        
        // 存储钱包
        {
            let mut wallets = self.wallets.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
            wallets.insert(wallet_id, enhanced_entry);
        }
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.audit_operation("wallet_created", 
            &format!("Enhanced wallet created successfully in {}ms", duration_ms))?;
        
        Ok(wallet_id)
    }
    
    /// 创建会话
    pub fn create_session(&mut self, wallet_id: Uuid, permissions: WalletPermissions) -> WalletResult<Uuid> {
        // 检查并发会话限制
        {
            let sessions = self.sessions.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
            
            let active_sessions = sessions.values()
                .filter(|s| s.wallet_id == wallet_id && self.is_session_active(s))
                .count();
            
            if active_sessions >= self.config.max_concurrent_sessions {
                return Err(WalletError::SecurityError("Too many concurrent sessions".to_string()));
            }
        }
        
        // 验证钱包存在
        {
            let wallets = self.wallets.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
            
            if !wallets.contains_key(&wallet_id) {
                return Err(WalletError::WalletNotFound(wallet_id));
            }
        }
        
        let session_id = Uuid::new_v4();
        let session = WalletSession {
            session_id,
            wallet_id,
            created_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            is_authenticated: false, // 需要单独的认证步骤
            permissions,
        };
        
        {
            let mut sessions = self.sessions.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
            sessions.insert(session_id, session);
        }
        
        self.audit_operation("session_created", 
            &format!("Session created for wallet {}", wallet_id))?;
        
        Ok(session_id)
    }
    
    /// 认证会话
    pub fn authenticate_session(&mut self, session_id: Uuid, password: Option<&str>) -> WalletResult<bool> {
        let mut sessions = self.sessions.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
        
        let session = sessions.get_mut(&session_id)
            .ok_or(WalletError::SecurityError("Session not found".to_string()))?;
        
        // 检查会话是否过期
        if !self.is_session_active(session) {
            self.audit_operation("session_expired", 
                &format!("Authentication failed - session {} expired", session_id))?;
            return Ok(false);
        }
        
        // 如果钱包有派生密钥，需要验证密码
        if let Some(password) = password {
            let wallets = self.wallets.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
            
            if let Some(wallet_entry) = wallets.get(&session.wallet_id) {
                if let Some(derived_key) = &wallet_entry.derived_key {
                    if let Some(kdf_manager) = &mut self.kdf_manager {
                        let is_valid = kdf_manager.verify_key(password.as_bytes(), derived_key)
                            .map_err(|e| WalletError::CryptographicError(format!("Password verification failed: {}", e)))?;
                        
                        if !is_valid {
                            self.audit_operation("authentication_failed", 
                                &format!("Invalid password for session {}", session_id))?;
                            return Ok(false);
                        }
                    }
                }
            }
        }
        
        // 认证成功
        session.is_authenticated = true;
        session.last_activity = std::time::SystemTime::now();
        
        self.audit_operation("authentication_success", 
            &format!("Session {} authenticated successfully", session_id))?;
        
        Ok(true)
    }
    
    /// 使用会话签名交易
    pub fn sign_transaction_with_session(
        &mut self, 
        session_id: Uuid, 
        hd_path: &str, 
        transaction_data: &[u8]
    ) -> WalletResult<Vec<u8>> {
        // 验证会话
        let wallet_id = {
            let mut sessions = self.sessions.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
            
            let session = sessions.get_mut(&session_id)
                .ok_or(WalletError::SecurityError("Session not found".to_string()))?;
            
            if !session.is_authenticated || !self.is_session_active(session) {
                return Err(WalletError::InsufficientPermissions);
            }
            
            if !session.permissions.can_sign {
                return Err(WalletError::InsufficientPermissions);
            }
            
            session.last_activity = std::time::SystemTime::now();
            session.wallet_id
        };
        
        // 获取钱包并执行签名
        {
            let mut wallets = self.wallets.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
            
            let wallet_entry = wallets.get_mut(&wallet_id)
                .ok_or(WalletError::WalletNotFound(wallet_id))?;
            
            if wallet_entry.is_locked {
                return Err(WalletError::SecurityError(
                    format!("Wallet is locked: {}", 
                           wallet_entry.lock_reason.as_deref().unwrap_or("Unknown reason"))
                ));
            }
            
            // 更新访问统计
            wallet_entry.access_count += 1;
            wallet_entry.last_access = std::time::SystemTime::now();
            
            // 执行签名（这里简化为返回哈希）
            use sha3::{Digest, Sha3_256};
            let mut hasher = Sha3_256::new();
            hasher.update(transaction_data);
            hasher.update(hd_path.as_bytes());
            let signature = hasher.finalize().to_vec();
            
            self.audit_operation("transaction_signed", 
                &format!("Transaction signed for wallet {} at path {}", wallet_id, hd_path))?;
            
            Ok(signature)
        }
    }
    
    /// 锁定钱包
    pub fn lock_wallet(&mut self, wallet_id: Uuid, reason: &str) -> WalletResult<()> {
        let mut wallets = self.wallets.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
        
        let wallet_entry = wallets.get_mut(&wallet_id)
            .ok_or(WalletError::WalletNotFound(wallet_id))?;
        
        wallet_entry.is_locked = true;
        wallet_entry.lock_reason = Some(reason.to_string());
        
        self.audit_operation("wallet_locked", 
            &format!("Wallet {} locked: {}", wallet_id, reason))?;
        
        Ok(())
    }
    
    /// 解锁钱包
    pub fn unlock_wallet(&mut self, wallet_id: Uuid) -> WalletResult<()> {
        let mut wallets = self.wallets.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
        
        let wallet_entry = wallets.get_mut(&wallet_id)
            .ok_or(WalletError::WalletNotFound(wallet_id))?;
        
        wallet_entry.is_locked = false;
        wallet_entry.lock_reason = None;
        
        self.audit_operation("wallet_unlocked", 
            &format!("Wallet {} unlocked", wallet_id))?;
        
        Ok(())
    }
    
    /// 批量创建钱包（性能优化版本）
    pub fn create_wallets_batch(&mut self, count: usize, password: Option<&str>) -> WalletResult<Vec<Uuid>> {
        let start_time = std::time::Instant::now();
        let mut wallet_ids = Vec::with_capacity(count);
        
        // 预分配内存以提高性能
        if let Some(memory_pool) = &self.memory_pool {
            // 使用内存池预分配一些内存块
            for _ in 0..count.min(10) {
                let _ = memory_pool.allocate(1024); // 预分配1KB块
            }
        }
        
        for i in 0..count {
            let wallet_id = self.create_enhanced_wallet(password)
                .map_err(|e| WalletError::SecurityError(format!("Failed to create wallet {}: {}", i, e)))?;
            wallet_ids.push(wallet_id);
        }
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.audit_operation("batch_wallet_creation", 
            &format!("Created {} wallets in {}ms ({:.2}ms/wallet)", 
                   count, duration_ms, duration_ms as f64 / count as f64))?;
        
        Ok(wallet_ids)
    }
    
    /// 高性能安全内存分配
    pub fn allocate_secure_memory(&self, size: usize) -> WalletResult<SecureMemoryBlock> {
        if let Some(memory_pool) = &self.memory_pool {
            memory_pool.allocate(size)
                .map_err(|e| WalletError::SecurityError(format!("Memory allocation failed: {}", e)))
        } else {
            Err(WalletError::SecurityError("Memory pool not enabled".to_string()))
        }
    }
    
    /// SIMD优化的安全内存清零
    pub fn secure_zero_memory(&self, data: &mut [u8]) -> WalletResult<()> {
        if let Some(simd_ops) = &self.simd_ops {
            simd_ops.secure_zero(data);
            Ok(())
        } else {
            // 回退到标准清零
            use zeroize::Zeroize;
            data.zeroize();
            Ok(())
        }
    }
    
    /// SIMD优化的安全内存比较
    pub fn secure_compare_memory(&self, a: &[u8], b: &[u8]) -> WalletResult<bool> {
        if let Some(simd_ops) = &self.simd_ops {
            Ok(simd_ops.secure_compare(a, b))
        } else {
            // 回退到常时间比较
            if a.len() != b.len() {
                return Ok(false);
            }
            let mut result = 0u8;
            for i in 0..a.len() {
                result |= a[i] ^ b[i];
            }
            Ok(result == 0)
        }
    }
    
    /// 批量签名操作（高性能版本）
    pub fn batch_sign_transactions(
        &mut self, 
        session_id: Uuid,
        transactions: Vec<(&str, &[u8])> // (hd_path, transaction_data)
    ) -> WalletResult<Vec<Vec<u8>>> {
        let start_time = std::time::Instant::now();
        
        // 验证会话一次
        let wallet_id = {
            let mut sessions = self.sessions.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
            
            let session = sessions.get_mut(&session_id)
                .ok_or(WalletError::SecurityError("Session not found".to_string()))?;
            
            if !session.is_authenticated || !self.is_session_active(session) {
                return Err(WalletError::InsufficientPermissions);
            }
            
            if !session.permissions.can_sign {
                return Err(WalletError::InsufficientPermissions);
            }
            
            session.last_activity = std::time::SystemTime::now();
            session.wallet_id
        };
        
        let mut signatures = Vec::with_capacity(transactions.len());
        
        // 批量处理签名
        {
            let mut wallets = self.wallets.lock()
                .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
            
            let wallet_entry = wallets.get_mut(&wallet_id)
                .ok_or(WalletError::WalletNotFound(wallet_id))?;
            
            if wallet_entry.is_locked {
                return Err(WalletError::SecurityError(
                    format!("Wallet is locked: {}", 
                           wallet_entry.lock_reason.as_deref().unwrap_or("Unknown reason"))
                ));
            }
            
            // 批量签名处理
            for (hd_path, transaction_data) in transactions {
                use sha3::{Digest, Sha3_256};
                let mut hasher = Sha3_256::new();
                hasher.update(transaction_data);
                hasher.update(hd_path.as_bytes());
                let signature = hasher.finalize().to_vec();
                signatures.push(signature);
            }
            
            // 更新访问统计
            wallet_entry.access_count += signatures.len() as u64;
            wallet_entry.last_access = std::time::SystemTime::now();
        }
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.audit_operation("batch_transaction_signing", 
            &format!("Signed {} transactions in {}ms ({:.2}ms/tx)", 
                   signatures.len(), duration_ms, duration_ms as f64 / signatures.len() as f64))?;
        
        Ok(signatures)
    }
    
    /// 获取性能统计信息
    pub fn get_performance_stats(&self) -> WalletResult<PerformanceStats> {
        let mut stats = PerformanceStats::default();
        
        // 获取内存池统计
        if let Some(memory_pool) = &self.memory_pool {
            let pool_stats = memory_pool.get_comprehensive_stats()
                .map_err(|e| WalletError::SecurityError(format!("Failed to get pool stats: {}", e)))?;
            
            stats.memory_pool_stats = Some(MemoryPoolSummary {
                total_allocated: pool_stats.total_allocated,
                peak_usage: pool_stats.allocation_stats.peak_memory_usage,
                current_usage: pool_stats.allocation_stats.current_memory_usage,
                pool_hit_rate: if pool_stats.allocation_stats.total_allocations > 0 {
                    (pool_stats.allocation_stats.pool_hits as f64 / 
                     pool_stats.allocation_stats.total_allocations as f64) * 100.0
                } else {
                    0.0
                },
            });
        }
        
        // 获取SIMD能力信息
        if let Some(simd_ops) = &self.simd_ops {
            let capabilities = simd_ops.get_capabilities();
            stats.simd_capabilities = Some(SimdCapabilitiesSummary {
                has_avx2: capabilities.has_avx2,
                has_avx512: capabilities.has_avx512,
                has_neon: capabilities.has_neon,
                has_sse4_2: capabilities.has_sse4_2,
            });
        }
        
        // 获取批量审计统计
        if let Some(batch_audit) = &self.batch_audit {
            if let Ok(audit_stats) = batch_audit.get_stats() {
                stats.batch_audit_stats = Some(BatchAuditSummary {
                    total_entries: audit_stats.total_entries,
                    batches_processed: audit_stats.batches_processed,
                    queue_size: audit_stats.queue_size,
                    dropped_entries: audit_stats.dropped_entries,
                });
            }
        }
        
        // 获取钱包和会话统计
        let wallets_count = self.wallets.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?
            .len();
            
        let sessions_count = self.sessions.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?
            .len();
        
        stats.total_wallets = wallets_count;
        stats.total_sessions = sessions_count;
        
        Ok(stats)
    }

    /// 获取钱包统计信息
    pub fn get_wallet_stats(&self, wallet_id: Uuid) -> WalletResult<WalletStats> {
        let wallets = self.wallets.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock wallets".to_string()))?;
        
        let wallet_entry = wallets.get(&wallet_id)
            .ok_or(WalletError::WalletNotFound(wallet_id))?;
        
        Ok(WalletStats {
            wallet_id,
            access_count: wallet_entry.access_count,
            last_access: wallet_entry.last_access,
            is_locked: wallet_entry.is_locked,
            has_derived_key: wallet_entry.derived_key.is_some(),
        })
    }
    
    /// 清理过期会话
    pub fn cleanup_expired_sessions(&mut self) -> WalletResult<usize> {
        let mut sessions = self.sessions.lock()
            .map_err(|_| WalletError::SecurityError("Failed to lock sessions".to_string()))?;
        
        let initial_count = sessions.len();
        sessions.retain(|_, session| self.is_session_active(session));
        let removed_count = initial_count - sessions.len();
        
        if removed_count > 0 {
            self.audit_operation("sessions_cleanup", 
                &format!("Cleaned up {} expired sessions", removed_count))?;
        }
        
        Ok(removed_count)
    }
    
    /// 检查会话是否活跃
    fn is_session_active(&self, session: &WalletSession) -> bool {
        if let Ok(elapsed) = session.last_activity.elapsed() {
            elapsed.as_secs() < self.config.lock_timeout_seconds
        } else {
            false
        }
    }
    
    /// 审计操作
    fn audit_operation(&self, operation: &str, details: &str) -> WalletResult<()> {
        if let Some(audit_log) = &self.audit_log {
            let event = AuditEvent::SecurityOperation {
                operation: operation.to_string(),
                details: details.to_string(),
                success: true,
                risk_level: "MEDIUM".to_string(),
            };
            
            audit_log.log_event(AuditLevel::Security, event, "enhanced_wallet_manager")
                .map_err(|e| WalletError::SecurityError(format!("Audit failed: {}", e)))?;
        }
        Ok(())
    }
}

/// 钱包统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStats {
    pub wallet_id: Uuid,
    pub access_count: u64,
    pub last_access: std::time::SystemTime,
    pub is_locked: bool,
    pub has_derived_key: bool,
}

/// 性能统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub memory_pool_stats: Option<MemoryPoolSummary>,
    pub simd_capabilities: Option<SimdCapabilitiesSummary>,
    pub batch_audit_stats: Option<BatchAuditSummary>,
    pub total_wallets: usize,
    pub total_sessions: usize,
}

/// 内存池摘要统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolSummary {
    pub total_allocated: usize,
    pub peak_usage: usize,
    pub current_usage: usize,
    pub pool_hit_rate: f64,
}

/// SIMD能力摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdCapabilitiesSummary {
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_neon: bool,
    pub has_sse4_2: bool,
}

/// 批量审计摘要统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAuditSummary {
    pub total_entries: u64,
    pub batches_processed: u64,
    pub queue_size: usize,
    pub dropped_entries: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_enhanced_wallet_creation() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        
        let wallet_id = manager.create_enhanced_wallet(Some("test_password")).unwrap();
        assert!(!wallet_id.is_nil());
        
        let stats = manager.get_wallet_stats(wallet_id).unwrap();
        assert_eq!(stats.wallet_id, wallet_id);
        assert!(stats.has_derived_key);
        assert!(!stats.is_locked);
    }
    
    #[test]
    fn test_session_management() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        let wallet_id = manager.create_enhanced_wallet(Some("test_password")).unwrap();
        
        let session_id = manager.create_session(wallet_id, WalletPermissions::default()).unwrap();
        assert!(!session_id.is_nil());
        
        // 认证会话
        let auth_result = manager.authenticate_session(session_id, Some("test_password")).unwrap();
        assert!(auth_result);
        
        // 错误密码认证
        let wrong_auth = manager.authenticate_session(session_id, Some("wrong_password")).unwrap();
        assert!(!wrong_auth);
    }
    
    #[test]
    fn test_transaction_signing() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        let wallet_id = manager.create_enhanced_wallet(Some("test_password")).unwrap();
        
        let session_id = manager.create_session(wallet_id, WalletPermissions::default()).unwrap();
        manager.authenticate_session(session_id, Some("test_password")).unwrap();
        
        let transaction_data = b"test transaction data";
        let signature = manager.sign_transaction_with_session(
            session_id, 
            "m/44'/60'/0'/0/0", 
            transaction_data
        ).unwrap();
        
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 32); // SHA3-256 output size
    }
    
    #[test]
    fn test_wallet_locking() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        let wallet_id = manager.create_enhanced_wallet(None).unwrap();
        
        manager.lock_wallet(wallet_id, "Security test").unwrap();
        
        let stats = manager.get_wallet_stats(wallet_id).unwrap();
        assert!(stats.is_locked);
        
        manager.unlock_wallet(wallet_id).unwrap();
        
        let stats = manager.get_wallet_stats(wallet_id).unwrap();
        assert!(!stats.is_locked);
    }
    
    #[test]
    fn test_batch_wallet_creation() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        
        let start_time = std::time::Instant::now();
        let wallet_ids = manager.create_wallets_batch(5, Some("test_password")).unwrap();
        let duration = start_time.elapsed();
        
        assert_eq!(wallet_ids.len(), 5);
        println!("Batch created 5 wallets in {:?}", duration);
        
        // 验证所有钱包都可以获取统计信息
        for wallet_id in wallet_ids {
            let stats = manager.get_wallet_stats(wallet_id).unwrap();
            assert!(stats.has_derived_key);
        }
    }
    
    #[test]
    fn test_batch_signing() {
        let mut manager = EnhancedWalletManager::with_defaults().unwrap();
        let wallet_id = manager.create_enhanced_wallet(Some("test_password")).unwrap();
        
        let session_id = manager.create_session(wallet_id, WalletPermissions::default()).unwrap();
        manager.authenticate_session(session_id, Some("test_password")).unwrap();
        
        let transactions = vec![
            ("m/44'/60'/0'/0/0", b"tx1" as &[u8]),
            ("m/44'/60'/0'/0/1", b"tx2" as &[u8]),
            ("m/44'/60'/0'/0/2", b"tx3" as &[u8]),
        ];
        
        let start_time = std::time::Instant::now();
        let signatures = manager.batch_sign_transactions(session_id, transactions).unwrap();
        let duration = start_time.elapsed();
        
        assert_eq!(signatures.len(), 3);
        println!("Batch signed 3 transactions in {:?}", duration);
        
        // 验证每个签名都是32字节（SHA3-256）
        for signature in signatures {
            assert_eq!(signature.len(), 32);
        }
    }
    
    #[test]
    fn test_simd_memory_operations() {
        let manager = EnhancedWalletManager::with_defaults().unwrap();
        
        // 测试安全清零
        let mut data = vec![0xFF; 1024];
        manager.secure_zero_memory(&mut data).unwrap();
        
        for &byte in &data {
            assert_eq!(byte, 0);
        }
        
        // 测试安全比较
        let data1 = vec![0xAA; 256];
        let data2 = vec![0xAA; 256];
        let data3 = vec![0xBB; 256];
        
        assert!(manager.secure_compare_memory(&data1, &data2).unwrap());
        assert!(!manager.secure_compare_memory(&data1, &data3).unwrap());
    }
    
    #[test]
    fn test_performance_stats() {
        let manager = EnhancedWalletManager::with_defaults().unwrap();
        
        let stats = manager.get_performance_stats().unwrap();
        
        // 验证性能统计结构
        assert!(stats.simd_capabilities.is_some());
        assert!(stats.memory_pool_stats.is_some());
        
        if let Some(simd_caps) = &stats.simd_capabilities {
            println!("SIMD Capabilities: AVX2={}, AVX512={}, NEON={}, SSE4.2={}", 
                   simd_caps.has_avx2, simd_caps.has_avx512, 
                   simd_caps.has_neon, simd_caps.has_sse4_2);
        }
        
        if let Some(pool_stats) = &stats.memory_pool_stats {
            println!("Memory Pool: allocated={}, peak={}, current={}, hit_rate={:.1}%",
                   pool_stats.total_allocated, pool_stats.peak_usage,
                   pool_stats.current_usage, pool_stats.pool_hit_rate);
        }
    }
    
    #[test]
    fn test_secure_memory_allocation() {
        let manager = EnhancedWalletManager::with_defaults().unwrap();
        
        // 测试不同大小的内存分配
        for size in [32, 64, 128, 256, 512, 1024, 2048] {
            let block = manager.allocate_secure_memory(size).unwrap();
            assert_eq!(block.len(), size);
            assert!(block.capacity() >= size);
        }
        
        let perf_stats = manager.get_performance_stats().unwrap();
        if let Some(pool_stats) = &perf_stats.memory_pool_stats {
            assert!(pool_stats.total_allocated > 0);
            println!("Total allocated: {} bytes", pool_stats.total_allocated);
        }
    }
}