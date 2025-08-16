/**
 * 简化的WebAuthn Challenge-Response实现
 * 基于用户要求：使用Simple WebAuthn npm包，CA只提供challenge验证
 */

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use tracing::{info, error};

// Challenge存储结构
#[derive(Debug, Clone)]
pub struct Challenge {
    pub challenge: String,
    pub user_id: String,
    pub created_at: u64,
    pub expires_at: u64,
}

// WebAuthn注册请求
#[derive(Debug, Deserialize)]
pub struct RegistrationOptions {
    pub user_id: String,
    pub user_name: String,
    pub user_display_name: String,
    pub rp_name: String,
    pub rp_id: String,
}

// WebAuthn注册挑战响应
#[derive(Debug, Serialize)]
pub struct RegistrationChallenge {
    pub challenge: String,
    pub rp: RelyingParty,
    pub user: User,
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    pub timeout: u32,
    pub attestation: String,
}

#[derive(Debug, Serialize)]
pub struct RelyingParty {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct PubKeyCredParam {
    pub r#type: String,
    pub alg: i32,
}

// WebAuthn认证挑战
#[derive(Debug, Serialize)]
pub struct AuthenticationChallenge {
    pub challenge: String,
    pub timeout: u32,
    pub rp_id: String,
    pub allow_credentials: Vec<AllowedCredential>,
}

#[derive(Debug, Serialize)]
pub struct AllowedCredential {
    pub r#type: String,
    pub id: String,
}

// 认证响应验证请求
#[derive(Debug, Deserialize)]
pub struct AuthenticationResponse {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
    pub challenge: String,
}

// 简化的WebAuthn管理器
pub struct SimpleWebAuthnManager {
    challenges: Arc<Mutex<HashMap<String, Challenge>>>,
    rp_id: String,
    rp_name: String,
}

impl SimpleWebAuthnManager {
    pub fn new(rp_id: String, rp_name: String) -> Self {
        Self {
            challenges: Arc::new(Mutex::new(HashMap::new())),
            rp_id,
            rp_name,
        }
    }

    /// 生成注册挑战
    pub fn generate_registration_challenge(&self, options: RegistrationOptions) -> Result<RegistrationChallenge> {
        let challenge_id = Uuid::new_v4().to_string();
        let challenge_bytes = self.generate_random_challenge()?;
        let challenge_b64 = base64::encode(&challenge_bytes);

        // 存储challenge
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let challenge = Challenge {
            challenge: challenge_b64.clone(),
            user_id: options.user_id.clone(),
            created_at: now,
            expires_at: now + 300, // 5分钟过期
        };

        {
            let mut challenges = self.challenges.lock().unwrap();
            challenges.insert(challenge_id.clone(), challenge);
        }

        info!("Generated registration challenge for user: {}", options.user_id);

        Ok(RegistrationChallenge {
            challenge: challenge_b64,
            rp: RelyingParty {
                id: self.rp_id.clone(),
                name: self.rp_name.clone(),
            },
            user: User {
                id: options.user_id,
                name: options.user_name,
                display_name: options.user_display_name,
            },
            pub_key_cred_params: vec![
                PubKeyCredParam {
                    r#type: "public-key".to_string(),
                    alg: -7, // ES256
                },
                PubKeyCredParam {
                    r#type: "public-key".to_string(),
                    alg: -257, // RS256
                },
            ],
            timeout: 300000, // 5分钟
            attestation: "none".to_string(),
        })
    }

    /// 生成认证挑战
    pub fn generate_authentication_challenge(&self, user_id: &str) -> Result<AuthenticationChallenge> {
        let challenge_bytes = self.generate_random_challenge()?;
        let challenge_b64 = base64::encode(&challenge_bytes);

        // 存储challenge
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let challenge = Challenge {
            challenge: challenge_b64.clone(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + 300,
        };

        {
            let mut challenges = self.challenges.lock().unwrap();
            challenges.insert(challenge_b64.clone(), challenge);
        }

        info!("Generated authentication challenge for user: {}", user_id);

        // 这里应该从数据库获取用户的凭证
        // 暂时返回空的允许凭证列表
        Ok(AuthenticationChallenge {
            challenge: challenge_b64,
            timeout: 300000,
            rp_id: self.rp_id.clone(),
            allow_credentials: vec![], // 实际应用中需要从数据库获取
        })
    }

    /// 验证认证响应（简化版本）
    pub fn verify_authentication(&self, response: AuthenticationResponse) -> Result<bool> {
        // 检查challenge是否存在且未过期
        let challenge = {
            let challenges = self.challenges.lock().unwrap();
            challenges.get(&response.challenge).cloned()
        };

        let challenge = match challenge {
            Some(c) => c,
            None => {
                error!("Challenge not found: {}", response.challenge);
                return Ok(false);
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        if now > challenge.expires_at {
            error!("Challenge expired for user: {}", challenge.user_id);
            return Ok(false);
        }

        // 简化验证：在真实实现中，这里需要：
        // 1. 验证client_data_json中的challenge
        // 2. 验证authenticator_data
        // 3. 验证signature
        // 4. 检查credential_id是否属于该用户

        info!("Authentication verified for user: {}", challenge.user_id);

        // 清理使用过的challenge
        {
            let mut challenges = self.challenges.lock().unwrap();
            challenges.remove(&response.challenge);
        }

        Ok(true)
    }

    /// 清理过期的challenge
    pub fn cleanup_expired_challenges(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut challenges = self.challenges.lock().unwrap();
        challenges.retain(|_, challenge| challenge.expires_at > now);
    }

    // 生成随机challenge
    fn generate_random_challenge(&self) -> Result<Vec<u8>> {
        // 在真实实现中，这里应该使用TEE硬件随机数生成器
        // 暂时使用系统随机数
        let mut challenge = vec![0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut challenge);
        Ok(challenge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_generation() {
        let manager = SimpleWebAuthnManager::new(
            "localhost".to_string(),
            "AirAccount Test".to_string(),
        );

        let options = RegistrationOptions {
            user_id: "user123".to_string(),
            user_name: "test@example.com".to_string(),
            user_display_name: "Test User".to_string(),
            rp_name: "AirAccount".to_string(),
            rp_id: "localhost".to_string(),
        };

        let challenge = manager.generate_registration_challenge(options).unwrap();
        assert!(!challenge.challenge.is_empty());
        assert_eq!(challenge.rp.id, "localhost");
    }

    #[test]
    fn test_authentication_challenge() {
        let manager = SimpleWebAuthnManager::new(
            "localhost".to_string(),
            "AirAccount Test".to_string(),
        );

        let challenge = manager.generate_authentication_challenge("user123").unwrap();
        assert!(!challenge.challenge.is_empty());
        assert_eq!(challenge.rp_id, "localhost");
    }
}