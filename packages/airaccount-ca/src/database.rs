/**
 * 数据库服务 - 内存存储版本
 * 遵循与Node.js CA相同的数据结构和逻辑
 * 
 * 重要架构原则：
 * - 节点可能跑路，用户凭证（Passkey + Email）必须由用户自己存储
 * - 此数据库只存储临时会话数据和非关键信息
 * - 用户的Passkey凭证应存储在客户端（浏览器、移动设备）
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use base64::Engine;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionData {
    pub session_id: String,
    pub user_id: String,
    pub email: String,
    pub is_authenticated: bool,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeRecord {
    pub challenge: String,
    pub user_id: String,
    pub challenge_type: String, // 'registration' | 'authentication'
    pub created_at: i64,
    pub expires_at: i64,
    pub used: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DbUserAccount {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthenticatorDevice {
    pub id: Option<i64>,
    pub user_id: String,
    pub credential_id: Vec<u8>,
    pub credential_public_key: Vec<u8>,
    pub counter: i64,
    pub transports: Vec<String>, // JSON array of transport methods
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct Database {
    // 内存存储，遵循相同的表结构
    sessions: HashMap<String, SessionData>,
    challenges: HashMap<String, ChallengeRecord>,
    user_accounts: HashMap<String, DbUserAccount>,
    authenticator_devices: HashMap<i64, AuthenticatorDevice>,
    device_counter: i64,
}

impl Database {
    pub fn new(_db_path: Option<&str>) -> Result<Self> {
        println!("📦 初始化内存数据库（与Node.js CA相同的数据结构）");
        
        Ok(Database {
            sessions: HashMap::new(),
            challenges: HashMap::new(),
            user_accounts: HashMap::new(),
            authenticator_devices: HashMap::new(),
            device_counter: 0,
        })
    }
    
    // WebAuthn相关方法 - 与Node.js CA逻辑一致
    pub fn store_challenge(&mut self, challenge: &str, user_id: &str, challenge_type: &str) -> Result<()> {
        let now = self.current_timestamp();
        let expires_at = now + 300; // 5分钟过期，与Node.js CA一致
        
        let record = ChallengeRecord {
            challenge: challenge.to_string(),
            user_id: user_id.to_string(),
            challenge_type: challenge_type.to_string(),
            created_at: now,
            expires_at,
            used: false,
        };
        
        self.challenges.insert(challenge.to_string(), record);
        
        println!("📋 存储challenge: {} for user: {} (类型: {})", 
                &challenge[..16.min(challenge.len())], user_id, challenge_type);
        
        Ok(())
    }
    
    pub fn verify_and_use_challenge(&mut self, challenge: &str, user_id: &str) -> Result<bool> {
        let now = self.current_timestamp();
        
        match self.challenges.get_mut(challenge) {
            Some(record) => {
                if record.user_id != user_id {
                    return Ok(false); // 用户不匹配
                }
                if record.used {
                    return Ok(false); // 已使用
                }
                if now > record.expires_at {
                    return Ok(false); // 已过期
                }
                
                // 标记为已使用
                record.used = true;
                
                println!("✅ Challenge验证成功: {} for user: {}", 
                        &challenge[..16.min(challenge.len())], user_id);
                
                Ok(true)
            }
            None => Ok(false), // 不存在
        }
    }
    
    pub fn create_or_update_user(&mut self, user_id: &str, username: &str, display_name: &str) -> Result<()> {
        let now = self.current_timestamp();
        
        let user = DbUserAccount {
            user_id: user_id.to_string(),
            username: username.to_string(),
            display_name: display_name.to_string(),
            created_at: now,
            updated_at: now,
        };
        
        self.user_accounts.insert(user_id.to_string(), user);
        
        println!("👤 创建/更新用户: {} ({})", username, display_name);
        
        Ok(())
    }
    
    pub fn get_user(&self, user_id: &str) -> Result<Option<DbUserAccount>> {
        Ok(self.user_accounts.get(user_id).cloned())
    }
    
    pub fn store_device(&mut self, device: &AuthenticatorDevice) -> Result<i64> {
        self.device_counter += 1;
        let device_id = self.device_counter;
        
        let mut device_with_id = device.clone();
        device_with_id.id = Some(device_id);
        
        self.authenticator_devices.insert(device_id, device_with_id);
        
        println!("🔐 存储设备: ID={} for user: {} (凭证ID: {})", 
                device_id, device.user_id, 
                base64::prelude::BASE64_STANDARD.encode(&device.credential_id[..16.min(device.credential_id.len())]));
        
        Ok(device_id)
    }
    
    pub fn get_user_devices(&self, user_id: &str) -> Result<Vec<AuthenticatorDevice>> {
        let devices: Vec<AuthenticatorDevice> = self.authenticator_devices
            .values()
            .filter(|device| device.user_id == user_id)
            .cloned()
            .collect();
        
        Ok(devices)
    }
    
    pub fn update_device_counter(&mut self, credential_id: &[u8], new_counter: i64) -> Result<()> {
        let now = self.current_timestamp();
        
        for device in self.authenticator_devices.values_mut() {
            if device.credential_id == credential_id {
                device.counter = new_counter;
                device.updated_at = now;
                
                println!("🔄 更新设备计数器: {} -> {}", device.counter, new_counter);
                return Ok(());
            }
        }
        
        Err(anyhow!("Device not found with credential ID"))
    }
    
    pub fn list_users(&self) -> Result<Vec<String>> {
        let mut users: Vec<String> = self.user_accounts.keys().cloned().collect();
        users.sort();
        Ok(users)
    }
    
    pub fn get_user_info(&self, user_id: &str) -> Result<Option<String>> {
        match self.get_user(user_id)? {
            Some(user) => {
                let devices = self.get_user_devices(user_id)?;
                let info = format!(
                    "用户ID: {}\n用户名: {}\n显示名称: {}\n设备数量: {}\n创建时间: {}",
                    user.user_id,
                    user.username,
                    user.display_name,
                    devices.len(),
                    user.created_at
                );
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }
    
    // 会话管理 - 与Node.js CA一致
    pub fn create_session(&mut self, user_id: &str, email: &str, duration_secs: Option<i64>) -> Result<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = self.current_timestamp();
        let expires_at = now + duration_secs.unwrap_or(3600); // 默认1小时
        
        let session = SessionData {
            session_id: session_id.clone(),
            user_id: user_id.to_string(),
            email: email.to_string(),
            is_authenticated: false,
            created_at: now,
            expires_at,
            last_activity: now,
        };
        
        self.sessions.insert(session_id.clone(), session);
        
        println!("🎫 创建会话: {} for user: {} (过期: {}秒)", 
                session_id, user_id, duration_secs.unwrap_or(3600));
        
        Ok(session_id)
    }
    
    pub fn authenticate_session(&mut self, session_id: &str) -> Result<bool> {
        let now = self.current_timestamp();
        
        match self.sessions.get_mut(session_id) {
            Some(session) => {
                if now > session.expires_at {
                    return Ok(false); // 已过期
                }
                
                session.is_authenticated = true;
                session.last_activity = now;
                
                println!("✅ 会话认证成功: {}", session_id);
                Ok(true)
            }
            None => Ok(false), // 不存在
        }
    }
    
    // 清理过期数据 - 与Node.js CA一致
    pub fn cleanup_expired(&mut self) -> Result<()> {
        let now = self.current_timestamp();
        
        // 清理过期会话
        let expired_sessions: Vec<String> = self.sessions
            .iter()
            .filter(|(_, session)| session.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();
        
        for session_id in expired_sessions {
            self.sessions.remove(&session_id);
        }
        
        // 清理过期挑战
        let expired_challenges: Vec<String> = self.challenges
            .iter()
            .filter(|(_, challenge)| challenge.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();
        
        for challenge_id in expired_challenges {
            self.challenges.remove(&challenge_id);
        }
        
        println!("🧹 清理过期数据完成");
        
        Ok(())
    }
    
    fn current_timestamp(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}