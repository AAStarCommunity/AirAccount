/**
 * 完整WebAuthn服务实现
 * 支持完整的Passkey存储、状态管理和错误处理
 */

// use anyhow::{anyhow, Result};
// use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use base64::Engine;

use crate::database::{Database, RegistrationState, AuthenticationState, RegistrationStep, AuthenticationStep};
use crate::webauthn_errors::{WebAuthnError, WebAuthnResult, log_webauthn_error};

pub struct WebAuthnService {
    webauthn: Webauthn,
    database: Arc<Mutex<Database>>,
}

impl WebAuthnService {
    pub fn new(database: Arc<Mutex<Database>>) -> WebAuthnResult<Self> {
        let rp_id = "localhost";
        let rp_origin = Url::parse("http://localhost:3002")
            .map_err(|e| WebAuthnError::ConfigurationError { 
                message: format!("Failed to parse origin URL: {}", e)
            })?;
        
        let builder = WebauthnBuilder::new(rp_id, &rp_origin)
            .map_err(|e| WebAuthnError::ConfigurationError { 
                message: format!("Failed to create WebAuthn builder: {}", e)
            })?;
        
        let webauthn = builder
            .rp_name("AirAccount Rust CA")
            .build()
            .map_err(|e| WebAuthnError::ConfigurationError { 
                message: format!("Failed to build WebAuthn: {}", e)
            })?;
        
        println!("🔐 WebAuthn服务初始化完成");
        println!("   - RP ID: {}", rp_id);
        println!("   - Origin: {}", rp_origin);
        
        Ok(WebAuthnService {
            webauthn,
            database,
        })
    }
    
    /// 开始Passkey注册流程
    pub async fn start_registration(&self, user_id: &str, display_name: &str) -> WebAuthnResult<CreationChallengeResponse> {
        let user_unique_id = Uuid::new_v4();
        
        // 确保用户在数据库中存在
        {
            let mut db = self.database.lock().await;
            db.create_or_update_user(user_id, user_id, display_name)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
        }
        
        // 获取用户现有passkeys（用于排除已注册的设备）
        let existing_passkeys = {
            let db = self.database.lock().await;
            db.get_user_passkeys(user_id)
                .map_err(|e| WebAuthnError::DatabaseError(e))?
        };
        
        let exclude_credentials: Vec<webauthn_rs::prelude::CredentialID> = existing_passkeys
            .iter()
            .map(|passkey| passkey.cred_id().clone())
            .collect();
        
        println!("🔄 开始注册流程: 用户={}, 已有passkey={}", user_id, existing_passkeys.len());
        
        let (ccr, registration_state) = self.webauthn
            .start_passkey_registration(
                user_unique_id,
                user_id,
                display_name,
                Some(exclude_credentials),
            )
            .map_err(|e| {
                log_webauthn_error(&WebAuthnError::RegistrationFailed { 
                    reason: e.to_string() 
                }, "start_registration");
                WebAuthnError::RegistrationFailed { reason: e.to_string() }
            })?;
        
        // 存储registration状态到数据库
        {
            let mut db = self.database.lock().await;
            let challenge_str = base64::prelude::BASE64_STANDARD.encode(ccr.public_key.challenge.as_ref());
            
            // 存储传统challenge记录（兼容性）
            db.store_challenge(&challenge_str, user_id, "registration")
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 存储完整registration状态
            let state_data = serde_json::to_string(&registration_state)
                .map_err(|e| WebAuthnError::StateStorageError { reason: e.to_string() })?;
            
            let reg_state = RegistrationState {
                user_id: user_id.to_string(),
                challenge: challenge_str.clone(),
                state_data,
                created_at: db.current_timestamp(),
                expires_at: db.current_timestamp() + 300, // 5分钟过期
                step: RegistrationStep::ChallengeGenerated,
            };
            
            db.store_registration_state(&challenge_str, reg_state)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
        }
        
        println!("✅ 注册challenge生成成功: 用户={}", user_id);
        Ok(ccr)
    }
    
    /// 完成Passkey注册
    pub async fn finish_registration(&self, challenge: &str, credential: &RegisterPublicKeyCredential) -> WebAuthnResult<()> {
        // 获取注册状态
        let reg_state = {
            let db = self.database.lock().await;
            db.get_registration_state(challenge)
                .map_err(|e| WebAuthnError::DatabaseError(e))?
                .ok_or(WebAuthnError::InvalidChallenge)?
        };
        
        // 检查状态是否有效
        if reg_state.step != RegistrationStep::ChallengeGenerated {
            return Err(WebAuthnError::InvalidRegistrationState {
                current_step: format!("{:?}", reg_state.step),
                expected_step: "ChallengeGenerated".to_string(),
            });
        }
        
        // 检查是否过期
        let now = {
            let db = self.database.lock().await;
            db.current_timestamp()
        };
        if now > reg_state.expires_at {
            return Err(WebAuthnError::InvalidChallenge);
        }
        
        // 反序列化registration状态
        let registration_state: PasskeyRegistration = serde_json::from_str(&reg_state.state_data)
            .map_err(|e| WebAuthnError::PasskeyDeserializationError { reason: e.to_string() })?;
        
        println!("🔍 验证注册凭证: 用户={}", reg_state.user_id);
        
        // 完成注册验证
        let passkey = self.webauthn
            .finish_passkey_registration(credential, &registration_state)
            .map_err(|e| {
                log_webauthn_error(&WebAuthnError::RegistrationFailed { 
                    reason: e.to_string() 
                }, "finish_registration");
                WebAuthnError::RegistrationFailed { reason: e.to_string() }
            })?;
        
        // 存储Passkey到数据库
        {
            let mut db = self.database.lock().await;
            db.store_passkey(&reg_state.user_id, &passkey)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 更新注册状态
            db.update_registration_step(challenge, RegistrationStep::Completed)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
        }
        
        println!("✅ 注册完成: 用户={}, 凭证ID={}", 
                reg_state.user_id, hex::encode(&passkey.cred_id().as_ref()[..8]));
        Ok(())
    }
    
    /// 开始Passkey认证流程
    pub async fn start_authentication(&self, user_id: &str) -> WebAuthnResult<RequestChallengeResponse> {
        // 获取用户Passkeys
        let passkeys = {
            let db = self.database.lock().await;
            db.get_user_passkeys(user_id)
                .map_err(|e| WebAuthnError::DatabaseError(e))?
        };
        
        if passkeys.is_empty() {
            return Err(WebAuthnError::NoDevicesRegistered { 
                user_id: user_id.to_string() 
            });
        }
        
        println!("🔓 开始认证流程: 用户={}, passkey数量={}", user_id, passkeys.len());
        
        // 使用完整的Passkey对象进行认证
        let (rcr, authentication_state) = self.webauthn
            .start_passkey_authentication(&passkeys)
            .map_err(|e| {
                log_webauthn_error(&WebAuthnError::AuthenticationFailed { 
                    reason: e.to_string() 
                }, "start_authentication");
                WebAuthnError::AuthenticationFailed { reason: e.to_string() }
            })?;
        
        // 存储authentication状态到数据库
        {
            let mut db = self.database.lock().await;
            let challenge_str = base64::prelude::BASE64_STANDARD.encode(rcr.public_key.challenge.as_ref());
            
            // 存储传统challenge记录（兼容性）
            db.store_challenge(&challenge_str, user_id, "authentication")
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 存储完整authentication状态
            let state_data = serde_json::to_string(&authentication_state)
                .map_err(|e| WebAuthnError::StateStorageError { reason: e.to_string() })?;
            
            let auth_state = AuthenticationState {
                user_id: user_id.to_string(),
                challenge: challenge_str.clone(),
                state_data,
                created_at: db.current_timestamp(),
                expires_at: db.current_timestamp() + 300, // 5分钟过期
                step: AuthenticationStep::ChallengeGenerated,
                session_id: None,
            };
            
            db.store_authentication_state(&challenge_str, auth_state)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
        }
        
        println!("✅ 认证challenge生成成功: 用户={}", user_id);
        Ok(rcr)
    }
    
    /// 完成Passkey认证
    pub async fn finish_authentication(&self, challenge: &str, credential: &PublicKeyCredential) -> WebAuthnResult<String> {
        // 获取认证状态
        let auth_state = {
            let db = self.database.lock().await;
            db.get_authentication_state(challenge)
                .map_err(|e| WebAuthnError::DatabaseError(e))?
                .ok_or(WebAuthnError::InvalidChallenge)?
        };
        
        // 检查状态是否有效
        if auth_state.step != AuthenticationStep::ChallengeGenerated {
            return Err(WebAuthnError::InvalidAuthenticationState {
                current_step: format!("{:?}", auth_state.step),
                expected_step: "ChallengeGenerated".to_string(),
            });
        }
        
        // 检查是否过期
        let now = {
            let db = self.database.lock().await;
            db.current_timestamp()
        };
        if now > auth_state.expires_at {
            return Err(WebAuthnError::InvalidChallenge);
        }
        
        // 反序列化authentication状态
        let authentication_state: PasskeyAuthentication = serde_json::from_str(&auth_state.state_data)
            .map_err(|e| WebAuthnError::PasskeyDeserializationError { reason: e.to_string() })?;
        
        println!("🔍 验证认证凭证: 用户={}", auth_state.user_id);
        
        // 完成认证验证
        let auth_result = self.webauthn
            .finish_passkey_authentication(credential, &authentication_state)
            .map_err(|e| {
                log_webauthn_error(&WebAuthnError::AuthenticationFailed { 
                    reason: e.to_string() 
                }, "finish_authentication");
                WebAuthnError::AuthenticationFailed { reason: e.to_string() }
            })?;
        
        // 更新Passkey使用时间和创建会话
        let session_id = {
            let mut db = self.database.lock().await;
            
            // 更新Passkey使用时间
            db.update_passkey_usage(&auth_result.cred_id().as_ref())
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 创建会话
            let session_id = db.create_session(&auth_state.user_id, "", Some(3600))
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 认证会话
            db.authenticate_session(&session_id)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            // 更新认证状态
            db.update_authentication_step(challenge, AuthenticationStep::Verified)
                .map_err(|e| WebAuthnError::DatabaseError(e))?;
            
            session_id
        };
        
        println!("✅ 认证完成: 用户={}, 会话={}", auth_state.user_id, session_id);
        Ok(session_id)
    }
    
    /// 获取用户列表
    pub async fn list_users(&self) -> WebAuthnResult<Vec<String>> {
        let db = self.database.lock().await;
        db.list_users().map_err(|e| WebAuthnError::DatabaseError(e))
    }
    
    /// 获取用户信息
    pub async fn get_user_info(&self, user_id: &str) -> WebAuthnResult<String> {
        let db = self.database.lock().await;
        match db.get_user_info(user_id).map_err(|e| WebAuthnError::DatabaseError(e))? {
            Some(info) => Ok(info),
            None => Err(WebAuthnError::UserNotFound { user_id: user_id.to_string() }),
        }
    }
    
    /// 获取WebAuthn统计信息
    pub async fn get_webauthn_stats(&self) -> WebAuthnResult<String> {
        let db = self.database.lock().await;
        db.get_webauthn_stats().map_err(|e| WebAuthnError::DatabaseError(e))
    }
    
    /// 清理过期状态
    pub async fn cleanup_expired(&self) -> WebAuthnResult<()> {
        let mut db = self.database.lock().await;
        db.cleanup_expired().map_err(|e| WebAuthnError::DatabaseError(e))?;
        db.cleanup_expired_states().map_err(|e| WebAuthnError::DatabaseError(e))?;
        println!("🧹 WebAuthn过期状态清理完成");
        Ok(())
    }
}