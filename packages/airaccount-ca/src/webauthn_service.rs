use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use webauthn_rs::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserAccount {
    pub user_id: String,
    pub display_name: String,
    pub credentials: Vec<Passkey>,
}

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
    users: Arc<Mutex<HashMap<String, UserAccount>>>,
    registration_states: Arc<Mutex<HashMap<String, RegistrationState>>>,
    auth_states: Arc<Mutex<HashMap<String, AuthenticationState>>>,
}

impl WebAuthnService {
    pub fn new() -> Result<Self> {
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
            users: Arc::new(Mutex::new(HashMap::new())),
            registration_states: Arc::new(Mutex::new(HashMap::new())),
            auth_states: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub async fn start_registration(&self, user_id: &str, display_name: &str) -> Result<CreationChallengeResponse> {
        let user_unique_id = Uuid::new_v4();
        
        let (ccr, registration_state) = self.webauthn
            .start_passkey_registration(
                user_unique_id,
                user_id,
                display_name,
                None, // No existing credentials to exclude
            )
            .map_err(|e| anyhow!("Failed to start registration: {}", e))?;
        
        // Store registration state
        let session_id = Uuid::new_v4().to_string();
        let mut states = self.registration_states.lock().await;
        states.insert(session_id.clone(), RegistrationState {
            user_id: user_id.to_string(),
            state: registration_state,
        });
        
        println!("🔑 Started passkey registration for user: {}", user_id);
        println!("📋 Session ID: {}", session_id);
        
        Ok(ccr)
    }
    
    pub async fn finish_registration(
        &self,
        session_id: &str,
        reg_response: &RegisterPublicKeyCredential,
    ) -> Result<String> {
        let mut states = self.registration_states.lock().await;
        let reg_state = states.remove(session_id)
            .ok_or_else(|| anyhow!("Registration session not found or expired"))?;
        
        let passkey = self.webauthn
            .finish_passkey_registration(reg_response, &reg_state.state)
            .map_err(|e| anyhow!("Failed to finish registration: {}", e))?;
        
        // Store user account with new passkey
        let mut users = self.users.lock().await;
        let user_account = users.entry(reg_state.user_id.clone()).or_insert(UserAccount {
            user_id: reg_state.user_id.clone(),
            display_name: "User".to_string(),
            credentials: Vec::new(),
        });
        
        user_account.credentials.push(passkey);
        
        println!("✅ Passkey registration completed for user: {}", reg_state.user_id);
        println!("🔐 Credential ID: {}", hex::encode(&user_account.credentials.last().unwrap().cred_id()));
        
        Ok(format!("Registration successful for user: {}", reg_state.user_id))
    }
    
    pub async fn start_authentication(&self, user_id: &str) -> Result<RequestChallengeResponse> {
        let users = self.users.lock().await;
        let user_account = users.get(user_id)
            .ok_or_else(|| anyhow!("User not found: {}", user_id))?;
        
        let (rcr, auth_state) = self.webauthn
            .start_passkey_authentication(&user_account.credentials)
            .map_err(|e| anyhow!("Failed to start authentication: {}", e))?;
        
        // Store authentication state
        let session_id = Uuid::new_v4().to_string();
        let mut states = self.auth_states.lock().await;
        states.insert(session_id.clone(), AuthenticationState {
            user_id: user_id.to_string(),
            state: auth_state,
        });
        
        println!("🔓 Started passkey authentication for user: {}", user_id);
        println!("📋 Session ID: {}", session_id);
        
        Ok(rcr)
    }
    
    pub async fn finish_authentication(
        &self,
        session_id: &str,
        auth_response: &PublicKeyCredential,
    ) -> Result<String> {
        let mut states = self.auth_states.lock().await;
        let auth_state = states.remove(session_id)
            .ok_or_else(|| anyhow!("Authentication session not found or expired"))?;
        
        let auth_result = self.webauthn
            .finish_passkey_authentication(auth_response, &auth_state.state)
            .map_err(|e| anyhow!("Failed to finish authentication: {}", e))?;
        
        // Update user's credential counter
        let mut users = self.users.lock().await;
        if let Some(user_account) = users.get_mut(&auth_state.user_id) {
            for cred in &mut user_account.credentials {
                cred.update_credential(&auth_result);
            }
        }
        
        println!("✅ Passkey authentication completed for user: {}", auth_state.user_id);
        println!("🔐 Counter updated for credential");
        
        Ok(format!("Authentication successful for user: {}", auth_state.user_id))
    }
    
    pub async fn list_users(&self) -> Result<Vec<String>> {
        let users = self.users.lock().await;
        Ok(users.keys().cloned().collect())
    }
    
    pub async fn get_user_info(&self, user_id: &str) -> Result<String> {
        let users = self.users.lock().await;
        let user = users.get(user_id)
            .ok_or_else(|| anyhow!("User not found: {}", user_id))?;
        
        Ok(format!(
            "User: {}\nCredentials: {}\nDisplay: {}",
            user.user_id,
            user.credentials.len(),
            user.display_name
        ))
    }
}