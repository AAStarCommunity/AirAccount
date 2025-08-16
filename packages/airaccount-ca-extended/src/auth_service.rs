/**
 * 认证服务 - Email验证和OAuth2集成
 * 
 * 支持：
 * 1. Email验证 - 发送验证码确认用户身份
 * 2. OAuth2集成 - Google、GitHub等第三方登录
 * 3. 用户账户管理 - 绑定第三方账户与钱包
 */

use anyhow::{anyhow, Result};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    Scope, TokenResponse, TokenUrl, BasicClient, RequestTokenError
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, error, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub redirect_url: String,
    pub email_smtp_server: Option<String>,
    pub email_username: Option<String>,
    pub email_password: Option<String>,
}

// OAuth2提供商
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OAuthProvider {
    Google,
    GitHub,
}

// Email验证记录
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EmailVerification {
    pub email: String,
    pub verification_code: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub verified: bool,
}

// OAuth2账户绑定
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OAuthAccount {
    pub id: i64,
    pub user_id: String,
    pub provider: String, // 'google' or 'github'
    pub provider_user_id: String,
    pub provider_email: String,
    pub provider_name: String,
    pub access_token: Option<String>, // 可选存储
    pub created_at: i64,
    pub updated_at: i64,
}

// API响应结构
#[derive(Debug, Serialize)]
pub struct EmailVerificationResponse {
    pub success: bool,
    pub message: String,
    pub verification_id: String,
}

#[derive(Debug, Serialize)]
pub struct OAuthUrlResponse {
    pub success: bool,
    pub auth_url: String,
    pub csrf_token: String,
}

#[derive(Debug, Serialize)]
pub struct OAuthCallbackResponse {
    pub success: bool,
    pub user_info: OAuthUserInfo,
    pub existing_user: bool,
}

#[derive(Debug, Serialize)]
pub struct OAuthUserInfo {
    pub provider: String,
    pub provider_user_id: String,
    pub email: String,
    pub name: String,
    pub user_id: String, // 内部用户ID
}

pub struct AuthService {
    database: Arc<Pool<Sqlite>>,
    config: AuthConfig,
    google_client: Option<BasicClient>,
    github_client: Option<BasicClient>,
    http_client: Client,
}

impl AuthService {
    pub async fn new(config: AuthConfig, database: Arc<Pool<Sqlite>>) -> Result<Self> {
        // 初始化数据库表
        Self::initialize_database(&database).await?;

        // 初始化OAuth2客户端
        let google_client = if let (Some(client_id), Some(client_secret)) = 
            (&config.google_client_id, &config.google_client_secret) {
            Some(
                BasicClient::new(
                    ClientId::new(client_id.clone()),
                    Some(ClientSecret::new(client_secret.clone())),
                    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
                    Some(TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string())?),
                )
                .set_redirect_uri(RedirectUrl::new(format!("{}/oauth/google/callback", config.redirect_url))?),
            )
        } else {
            None
        };

        let github_client = if let (Some(client_id), Some(client_secret)) = 
            (&config.github_client_id, &config.github_client_secret) {
            Some(
                BasicClient::new(
                    ClientId::new(client_id.clone()),
                    Some(ClientSecret::new(client_secret.clone())),
                    AuthUrl::new("https://github.com/login/oauth/authorize".to_string())?,
                    Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string())?),
                )
                .set_redirect_uri(RedirectUrl::new(format!("{}/oauth/github/callback", config.redirect_url))?),
            )
        } else {
            None
        };

        Ok(Self {
            database,
            config,
            google_client,
            github_client,
            http_client: Client::new(),
        })
    }

    async fn initialize_database(pool: &Pool<Sqlite>) -> Result<()> {
        // Email验证表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS email_verifications (
                email TEXT PRIMARY KEY,
                verification_code TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                verified BOOLEAN DEFAULT FALSE
            )
            "#
        )
        .execute(pool)
        .await?;

        // OAuth账户绑定表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS oauth_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                provider_user_id TEXT NOT NULL,
                provider_email TEXT NOT NULL,
                provider_name TEXT NOT NULL,
                access_token TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(provider, provider_user_id),
                FOREIGN KEY (user_id) REFERENCES user_accounts (user_id) ON DELETE CASCADE
            )
            "#
        )
        .execute(pool)
        .await?;

        // 创建索引
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_oauth_accounts_user_id ON oauth_accounts (user_id)")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_oauth_accounts_provider ON oauth_accounts (provider, provider_user_id)")
            .execute(pool)
            .await?;

        info!("✅ Auth service database tables initialized");
        Ok(())
    }

    /// 发送Email验证码
    pub async fn send_email_verification(&self, email: &str) -> Result<EmailVerificationResponse> {
        info!("📧 Sending email verification to: {}", email);

        // 生成6位数验证码
        let verification_code = format!("{:06}", rand::random::<u32>() % 1000000);
        let verification_id = Uuid::new_v4().to_string();
        
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 600; // 10分钟过期

        // 存储验证码
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO email_verifications 
            (email, verification_code, created_at, expires_at, verified)
            VALUES (?, ?, ?, ?, FALSE)
            "#
        )
        .bind(email)
        .bind(&verification_code)
        .bind(now)
        .bind(expires_at)
        .execute(&*self.database)
        .await?;

        // 实际应用中这里应该发送真实的邮件
        // 这里简化处理，记录到日志
        info!("📧 Email verification code for {}: {}", email, verification_code);
        warn!("🚧 Email sending not implemented - verification code logged above");

        Ok(EmailVerificationResponse {
            success: true,
            message: "验证码已发送到您的邮箱，请查收".to_string(),
            verification_id,
        })
    }

    /// 验证Email验证码
    pub async fn verify_email_code(&self, email: &str, code: &str) -> Result<bool> {
        let now = chrono::Utc::now().timestamp();
        
        let verification = sqlx::query_as::<_, EmailVerification>(
            "SELECT * FROM email_verifications WHERE email = ? AND expires_at > ?"
        )
        .bind(email)
        .bind(now)
        .fetch_optional(&*self.database)
        .await?;

        if let Some(verification) = verification {
            if verification.verification_code == code {
                // 标记为已验证
                sqlx::query(
                    "UPDATE email_verifications SET verified = TRUE WHERE email = ?"
                )
                .bind(email)
                .execute(&*self.database)
                .await?;

                info!("✅ Email verification successful for: {}", email);
                return Ok(true);
            }
        }

        warn!("❌ Email verification failed for: {}", email);
        Ok(false)
    }

    /// 生成OAuth2授权URL
    pub async fn get_oauth_auth_url(&self, provider: OAuthProvider) -> Result<OAuthUrlResponse> {
        let client = match provider {
            OAuthProvider::Google => self.google_client.as_ref()
                .ok_or_else(|| anyhow!("Google OAuth not configured"))?,
            OAuthProvider::GitHub => self.github_client.as_ref()
                .ok_or_else(|| anyhow!("GitHub OAuth not configured"))?,
        };

        let scopes = match provider {
            OAuthProvider::Google => vec![Scope::new("email".to_string()), Scope::new("profile".to_string())],
            OAuthProvider::GitHub => vec![Scope::new("user:email".to_string())],
        };

        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(scopes)
            .url();

        info!("🔗 Generated OAuth auth URL for {:?}", provider);

        Ok(OAuthUrlResponse {
            success: true,
            auth_url: auth_url.to_string(),
            csrf_token: csrf_token.secret().clone(),
        })
    }

    /// 处理OAuth2回调
    pub async fn handle_oauth_callback(
        &self,
        provider: OAuthProvider,
        code: &str,
        state: &str,
    ) -> Result<OAuthCallbackResponse> {
        info!("🔄 Processing OAuth callback for {:?}", provider);

        let client = match provider {
            OAuthProvider::Google => self.google_client.as_ref()
                .ok_or_else(|| anyhow!("Google OAuth not configured"))?,
            OAuthProvider::GitHub => self.github_client.as_ref()
                .ok_or_else(|| anyhow!("GitHub OAuth not configured"))?,
        };

        // 交换授权码获取访问令牌
        let token_result = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(&self.http_client)
            .await?;

        let access_token = token_result.access_token().secret();

        // 获取用户信息
        let user_info = match provider {
            OAuthProvider::Google => self.get_google_user_info(access_token).await?,
            OAuthProvider::GitHub => self.get_github_user_info(access_token).await?,
        };

        // 检查是否已存在用户
        let existing_oauth = self.get_oauth_account(&provider, &user_info.id).await;
        let existing_user = existing_oauth.is_ok();

        // 生成或获取用户ID
        let user_id = if let Ok(oauth_account) = existing_oauth {
            oauth_account.user_id
        } else {
            // 生成新的用户ID
            Uuid::new_v4().to_string()
        };

        // 保存或更新OAuth账户信息
        self.save_oauth_account(&user_id, &provider, &user_info, access_token).await?;

        info!("✅ OAuth callback processed for {:?}: {}", provider, user_info.email);

        Ok(OAuthCallbackResponse {
            success: true,
            user_info: OAuthUserInfo {
                provider: format!("{:?}", provider).to_lowercase(),
                provider_user_id: user_info.id,
                email: user_info.email,
                name: user_info.name,
                user_id,
            },
            existing_user,
        })
    }

    async fn get_google_user_info(&self, access_token: &str) -> Result<GoogleUserInfo> {
        let response = self.http_client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await?;

        let user_info: GoogleUserInfo = response.json().await?;
        Ok(user_info)
    }

    async fn get_github_user_info(&self, access_token: &str) -> Result<GitHubUserInfo> {
        let response = self.http_client
            .get("https://api.github.com/user")
            .bearer_auth(access_token)
            .header("User-Agent", "AirAccount-CA")
            .send()
            .await?;

        let user_info: GitHubUserInfo = response.json().await?;
        Ok(user_info)
    }

    async fn get_oauth_account(&self, provider: &OAuthProvider, provider_user_id: &str) -> Result<OAuthAccount> {
        let provider_str = format!("{:?}", provider).to_lowercase();
        
        let account = sqlx::query_as::<_, OAuthAccount>(
            "SELECT * FROM oauth_accounts WHERE provider = ? AND provider_user_id = ?"
        )
        .bind(provider_str)
        .bind(provider_user_id)
        .fetch_one(&*self.database)
        .await?;

        Ok(account)
    }

    async fn save_oauth_account(
        &self,
        user_id: &str,
        provider: &OAuthProvider,
        user_info: &dyn OAuthUserInfoTrait,
        access_token: &str,
    ) -> Result<()> {
        let provider_str = format!("{:?}", provider).to_lowercase();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO oauth_accounts 
            (user_id, provider, provider_user_id, provider_email, provider_name, access_token, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, 
                COALESCE((SELECT created_at FROM oauth_accounts WHERE provider = ? AND provider_user_id = ?), ?),
                ?
            )
            "#
        )
        .bind(user_id)
        .bind(&provider_str)
        .bind(user_info.get_id())
        .bind(user_info.get_email())
        .bind(user_info.get_name())
        .bind(access_token)
        .bind(&provider_str)
        .bind(user_info.get_id())
        .bind(now)
        .bind(now)
        .execute(&*self.database)
        .await?;

        Ok(())
    }

    /// 获取用户的OAuth账户
    pub async fn get_user_oauth_accounts(&self, user_id: &str) -> Result<Vec<OAuthAccount>> {
        let accounts = sqlx::query_as::<_, OAuthAccount>(
            "SELECT * FROM oauth_accounts WHERE user_id = ? ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&*self.database)
        .await?;

        Ok(accounts)
    }
}

// OAuth用户信息特征
trait OAuthUserInfoTrait {
    fn get_id(&self) -> &str;
    fn get_email(&self) -> &str;
    fn get_name(&self) -> &str;
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
    name: String,
}

impl OAuthUserInfoTrait for GoogleUserInfo {
    fn get_id(&self) -> &str { &self.id }
    fn get_email(&self) -> &str { &self.email }
    fn get_name(&self) -> &str { &self.name }
}

#[derive(Debug, Deserialize)]
struct GitHubUserInfo {
    id: u64,
    email: Option<String>,
    name: Option<String>,
    login: String,
}

impl OAuthUserInfoTrait for GitHubUserInfo {
    fn get_id(&self) -> &str { &self.id.to_string() }
    fn get_email(&self) -> &str { 
        self.email.as_deref().unwrap_or(&self.login)
    }
    fn get_name(&self) -> &str { 
        self.name.as_deref().unwrap_or(&self.login)
    }
}