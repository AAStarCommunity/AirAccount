// Licensed to AirAccount under the Apache License, Version 2.0
// Wallet management system for user-wallet bindings

use crate::security::{SecurityManager, AuditEvent};
use super::{WalletError, WalletResult, AirAccountWallet, WalletCore};
use uuid::Uuid;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 用户钱包绑定关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWalletBinding {
    pub user_id: u64,
    pub wallet_id: Uuid,
    pub address: [u8; 20],
    pub alias: Option<String>,
    pub is_primary: bool,
    pub permissions: WalletPermissions,
}

/// 钱包权限配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletPermissions {
    pub can_read: bool,
    pub can_sign: bool,
    pub can_delete: bool,
    pub daily_limit: Option<u128>,
    pub requires_biometric: bool,
}

impl WalletPermissions {
    pub fn full_permissions() -> Self {
        Self {
            can_read: true,
            can_sign: true,
            can_delete: true,
            daily_limit: None,
            requires_biometric: false,
        }
    }
    
    pub fn read_only() -> Self {
        Self {
            can_read: true,
            can_sign: false,
            can_delete: false,
            daily_limit: Some(0),
            requires_biometric: false,
        }
    }
}

/// 钱包管理器
pub struct WalletManager {
    security_manager: SecurityManager,
    wallet_storage: HashMap<Uuid, WalletCore>,
    user_bindings: HashMap<u64, Vec<UserWalletBinding>>,
}

impl WalletManager {
    pub fn new(security_manager: &SecurityManager) -> WalletResult<Self> {
        Ok(Self {
            security_manager: security_manager.clone(),
            wallet_storage: HashMap::new(),
            user_bindings: HashMap::new(),
        })
    }
    
    /// 存储钱包绑定关系
    pub async fn store_wallet_binding(&mut self, binding: UserWalletBinding) -> WalletResult<()> {
        self.security_manager.audit_info(
            AuditEvent::TEEOperation {
                operation: "store_wallet_binding".to_string(),
                duration_ms: 0,
                success: true,
            },
            "wallet_manager",
        );
        
        self.user_bindings
            .entry(binding.user_id)
            .or_insert_with(Vec::new)
            .push(binding);
            
        Ok(())
    }
    
    /// 加载钱包
    pub async fn load_wallet(&self, wallet_id: &Uuid) -> WalletResult<AirAccountWallet> {
        let core = self.wallet_storage
            .get(wallet_id)
            .ok_or_else(|| WalletError::WalletNotFound(*wallet_id))?;
            
        Ok(AirAccountWallet::from_core(core.clone(), self.security_manager.clone()))
    }
}