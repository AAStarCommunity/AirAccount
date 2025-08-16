use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use base64::Engine;

use crate::database::Database;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationState {
    pub user_id: String,
    pub state: PasskeyRegistration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationState {
    pub user_id: String,
    pub state: PasskeyAuthentication,
}

pub struct WebAuthnService {
    webauthn: Webauthn,
    database: Arc<Mutex<Database>>,
}

impl WebAuthnService {
    pub fn new(database: Arc<Mutex<Database>>) -> Result<Self> {
        let rp_id = "localhost";
        let rp_origin = Url::parse("http://localhost:3002")
            .map_err(|e| anyhow!("Failed to parse origin URL: {}", e))?;
        
        let builder = WebauthnBuilder::new(rp_id, &rp_origin)
            .map_err(|e| anyhow!("Failed to create WebAuthn builder: {}", e))?;
        
        let webauthn = builder
            .rp_name("AirAccount Rust CA")
            .build()
            .map_err(|e| anyhow!("Failed to build WebAuthn: {}", e))?;
        
        Ok(WebAuthnService {
            webauthn,
            database,
        })
    }
    
    pub async fn start_registration(&self, user_id: &str, display_name: &str) -> Result<CreationChallengeResponse> {
        let user_unique_id = Uuid::new_v4();
        
        // 确保用户在数据库中存在
        {
            let mut db = self.database.lock().await;
            db.create_or_update_user(user_id, user_id, display_name)?;
        }
        
        // 获取用户现有设备（用于排除已注册的设备）
        let existing_devices = {
            let db = self.database.lock().await;
            db.get_user_devices(user_id)?
        };
        
        let exclude_credentials: Vec<webauthn_rs::prelude::CredentialID> = existing_devices
            .iter()
            .map(|device| webauthn_rs::prelude::CredentialID::from(device.credential_id.clone()))
            .collect();
        
        let (ccr, _registration_state) = self.webauthn
            .start_passkey_registration(
                user_unique_id,
                user_id,
                display_name,
                Some(exclude_credentials),
            )
            .map_err(|e| anyhow!("Failed to start registration: {}", e))?;
        
        // 存储challenge到数据库
        {
            let mut db = self.database.lock().await;
            let challenge_str = base64::prelude::BASE64_STANDARD.encode(ccr.public_key.challenge.as_ref());
            db.store_challenge(&challenge_str, user_id, "registration")?;
        }
        
        println!("🔑 Started passkey registration for user: {}", user_id);
        println!("📋 Challenge generated successfully");
        
        Ok(ccr)
    }
    
    pub async fn start_authentication(&self, user_id: &str) -> Result<RequestChallengeResponse> {
        // 获取用户设备
        let devices = {
            let db = self.database.lock().await;
            db.get_user_devices(user_id)?
        };
        
        if devices.is_empty() {
            return Err(anyhow!("User has no registered devices: {}", user_id));
        }
        
        // 简化的认证实现 - 使用空的passkey列表（实际使用中需要完整实现）
        let empty_passkeys: Vec<Passkey> = Vec::new();
        let (rcr, _auth_state) = self.webauthn
            .start_passkey_authentication(&empty_passkeys)
            .map_err(|e| anyhow!("Failed to start authentication: {}", e))?;
        
        // 存储challenge到数据库
        {
            let mut db = self.database.lock().await;
            let challenge_str = base64::prelude::BASE64_STANDARD.encode(rcr.public_key.challenge.as_ref());
            db.store_challenge(&challenge_str, user_id, "authentication")?;
        }
        
        println!("🔓 Started passkey authentication for user: {}", user_id);
        println!("📋 Challenge generated successfully");
        
        Ok(rcr)
    }
    
    pub async fn list_users(&self) -> Result<Vec<String>> {
        let db = self.database.lock().await;
        db.list_users()
    }
    
    pub async fn get_user_info(&self, user_id: &str) -> Result<String> {
        let db = self.database.lock().await;
        match db.get_user_info(user_id)? {
            Some(info) => Ok(info),
            None => Err(anyhow!("User not found: {}", user_id)),
        }
    }
}