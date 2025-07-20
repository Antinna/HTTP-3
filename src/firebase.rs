use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::error::{AppError, AppResult};

/// Firebase configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct FirebaseConfig {
    pub project_id: String,
    pub api_key: String,
    pub service_account_email: Option<String>,
    pub private_key: Option<String>,
    pub auth_domain: String,
    pub database_url: Option<String>,
}

impl FirebaseConfig {
    /// Load Firebase configuration from environment variables
    pub fn from_env() -> AppResult<Self> {
        let project_id = env::var("FIREBASE_PROJECT_ID")
            .map_err(|_| AppError::Configuration("FIREBASE_PROJECT_ID environment variable is required".to_string()))?;
        
        let api_key = env::var("FIREBASE_API_KEY")
            .map_err(|_| AppError::Configuration("FIREBASE_API_KEY environment variable is required".to_string()))?;
        
        let service_account_email = env::var("FIREBASE_SERVICE_ACCOUNT_EMAIL").ok();
        let private_key = env::var("FIREBASE_PRIVATE_KEY").ok();
        
        let auth_domain = env::var("FIREBASE_AUTH_DOMAIN")
            .unwrap_or_else(|_| format!("{}.firebaseapp.com", project_id));
        
        let database_url = env::var("FIREBASE_DATABASE_URL").ok();
        
        Ok(Self {
            project_id,
            api_key,
            service_account_email,
            private_key,
            auth_domain,
            database_url,
        })
    }
    
    /// Check if service account configuration is available
    pub fn has_service_account(&self) -> bool {
        self.service_account_email.is_some() && self.private_key.is_some()
    }
}

/// Firebase JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseTokenClaims {
    pub iss: String,
    pub aud: String,
    pub auth_time: u64,
    pub user_id: String,
    pub sub: String,
    pub iat: u64,
    pub exp: u64,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub firebase: FirebaseAuthContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseAuthContext {
    pub identities: HashMap<String, Vec<String>>,
    pub sign_in_provider: String,
}

/// Phone authentication request
#[derive(Debug, Serialize)]
pub struct PhoneAuthRequest {
    #[serde(rename = "phoneNumber")]
    pub phone_number: String,
    #[serde(rename = "recaptchaToken")]
    pub recaptcha_token: Option<String>,
}

/// Phone authentication response
#[derive(Debug, Deserialize)]
pub struct PhoneAuthResponse {
    #[serde(rename = "sessionInfo")]
    pub session_info: String,
}

/// OTP verification request
#[derive(Debug, Serialize)]
pub struct OtpVerificationRequest {
    #[serde(rename = "sessionInfo")]
    pub session_info: String,
    pub code: String,
}

/// OTP verification response
#[derive(Debug, Deserialize)]
pub struct OtpVerificationResponse {
    #[serde(rename = "idToken")]
    pub id_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: String,
    #[serde(rename = "localId")]
    pub local_id: String,
}

/// Token refresh request
#[derive(Debug, Serialize)]
pub struct TokenRefreshRequest {
    #[serde(rename = "grant_type")]
    pub grant_type: String,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
}

/// Token refresh response
#[derive(Debug, Deserialize)]
pub struct TokenRefreshResponse {
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "expires_in")]
    pub expires_in: String,
    #[serde(rename = "token_type")]
    pub token_type: String,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    #[serde(rename = "id_token")]
    pub id_token: String,
    #[serde(rename = "user_id")]
    pub user_id: String,
    #[serde(rename = "project_id")]
    pub project_id: String,
}

/// Firebase authentication service
#[derive(Debug, Clone)]
pub struct FirebaseAuth {
    config: FirebaseConfig,
    client: Client,
    public_keys: Option<HashMap<String, String>>,
}

impl FirebaseAuth {
    /// Create new Firebase authentication service
    pub fn new(config: FirebaseConfig) -> Self {
        let client = Client::new();
        
        Self {
            config,
            client,
            public_keys: None,
        }
    }
    
    /// Create Firebase auth service from environment
    pub fn from_env() -> AppResult<Self> {
        let config = FirebaseConfig::from_env()?;
        info!("Firebase authentication service initialized for project: {}", config.project_id);
        Ok(Self::new(config))
    }
    
    /// Send OTP to phone number
    pub async fn send_otp(&self, phone_number: &str, recaptcha_token: Option<String>) -> AppResult<String> {
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:sendVerificationCode?key={}",
            self.config.api_key
        );
        
        let request = PhoneAuthRequest {
            phone_number: phone_number.to_string(),
            recaptcha_token,
        };
        
        debug!("Sending OTP to phone number: {}", phone_number);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to send OTP: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Firebase OTP send failed: {}", error_text);
            return Err(AppError::ExternalService("Firebase".to_string(), format!("OTP send failed: {}", error_text)));
        }
        
        let auth_response: PhoneAuthResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to parse OTP response: {}", e)))?;
        
        info!("OTP sent successfully to phone number: {}", phone_number);
        Ok(auth_response.session_info)
    }
    
    /// Verify OTP and get ID token
    pub async fn verify_otp(&self, session_info: &str, code: &str) -> AppResult<OtpVerificationResponse> {
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:signInWithPhoneNumber?key={}",
            self.config.api_key
        );
        
        let request = OtpVerificationRequest {
            session_info: session_info.to_string(),
            code: code.to_string(),
        };
        
        debug!("Verifying OTP with session info: {}", session_info);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to verify OTP: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Firebase OTP verification failed: {}", error_text);
            return Err(AppError::Authentication(format!("OTP verification failed: {}", error_text)));
        }
        
        let verification_response: OtpVerificationResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to parse verification response: {}", e)))?;
        
        info!("OTP verified successfully for user: {}", verification_response.local_id);
        Ok(verification_response)
    }
    
    /// Refresh ID token using refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> AppResult<TokenRefreshResponse> {
        let url = format!(
            "https://securetoken.googleapis.com/v1/token?key={}",
            self.config.api_key
        );
        
        let request = TokenRefreshRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: refresh_token.to_string(),
        };
        
        debug!("Refreshing Firebase token");
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to refresh token: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Firebase token refresh failed: {}", error_text);
            return Err(AppError::Authentication(format!("Token refresh failed: {}", error_text)));
        }
        
        let refresh_response: TokenRefreshResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to parse refresh response: {}", e)))?;
        
        info!("Token refreshed successfully for user: {}", refresh_response.user_id);
        Ok(refresh_response)
    }
    
    /// Fetch Firebase public keys for JWT verification
    async fn fetch_public_keys(&mut self) -> AppResult<()> {
        let url = "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";
        
        debug!("Fetching Firebase public keys");
        
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to fetch public keys: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Failed to fetch Firebase public keys: {}", error_text);
            return Err(AppError::ExternalService("Firebase".to_string(), format!("Public key fetch failed: {}", error_text)));
        }
        
        let keys: HashMap<String, String> = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to parse public keys: {}", e)))?;
        
        self.public_keys = Some(keys);
        info!("Firebase public keys fetched successfully");
        Ok(())
    }
    
    /// Verify Firebase ID token and extract claims
    pub async fn verify_token(&mut self, id_token: &str) -> AppResult<FirebaseTokenClaims> {
        // Ensure we have public keys
        if self.public_keys.is_none() {
            self.fetch_public_keys().await?;
        }
        
        // Decode token header to get key ID
        let header = decode_header(id_token)
            .map_err(|e| AppError::Authentication(format!("Invalid token header: {}", e)))?;
        
        let kid = header.kid
            .ok_or_else(|| AppError::Authentication("Token missing key ID".to_string()))?;
        
        // Get the public key for this token
        let public_keys = self.public_keys.as_ref().unwrap();
        let public_key = public_keys.get(&kid)
            .ok_or_else(|| AppError::Authentication("Unknown key ID in token".to_string()))?;
        
        // Set up validation parameters
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.config.project_id]);
        validation.set_issuer(&[&format!("https://securetoken.google.com/{}", self.config.project_id)]);
        
        // Decode and verify the token
        let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
            .map_err(|e| AppError::Authentication(format!("Invalid public key: {}", e)))?;
        
        let token_data = decode::<FirebaseTokenClaims>(id_token, &decoding_key, &validation)
            .map_err(|e| AppError::Authentication(format!("Token verification failed: {}", e)))?;
        
        // Additional validation
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if token_data.claims.exp < now {
            return Err(AppError::Authentication("Token has expired".to_string()));
        }
        
        if token_data.claims.iat > now + 300 {
            return Err(AppError::Authentication("Token issued in the future".to_string()));
        }
        
        debug!("Token verified successfully for user: {}", token_data.claims.user_id);
        Ok(token_data.claims)
    }
    
    /// Get user information from Firebase
    pub async fn get_user_info(&self, id_token: &str) -> AppResult<serde_json::Value> {
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:lookup?key={}",
            self.config.api_key
        );
        
        let request = serde_json::json!({
            "idToken": id_token
        });
        
        debug!("Fetching user information from Firebase");
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to get user info: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Firebase user info fetch failed: {}", error_text);
            return Err(AppError::ExternalService("Firebase".to_string(), format!("User info fetch failed: {}", error_text)));
        }
        
        let user_info: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::ExternalService("Firebase".to_string(), format!("Failed to parse user info: {}", e)))?;
        
        debug!("User information fetched successfully");
        Ok(user_info)
    }
    
    /// Check if token is expired
    pub fn is_token_expired(&self, claims: &FirebaseTokenClaims) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        claims.exp < now
    }
    
    /// Get time until token expires (in seconds)
    pub fn token_expires_in(&self, claims: &FirebaseTokenClaims) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        claims.exp as i64 - now as i64
    }
    
    /// Get Firebase configuration
    pub fn config(&self) -> &FirebaseConfig {
        &self.config
    }
}

/// User session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub user_id: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub id_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub created_at: u64,
    pub last_activity: u64,
}

impl UserSession {
    /// Create new user session from Firebase token claims and tokens
    pub fn new(
        claims: &FirebaseTokenClaims,
        id_token: String,
        refresh_token: String,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            user_id: claims.user_id.clone(),
            email: claims.email.clone(),
            phone_number: claims.phone_number.clone(),
            name: claims.name.clone(),
            picture: claims.picture.clone(),
            id_token,
            refresh_token,
            expires_at: claims.exp,
            created_at: now,
            last_activity: now,
        }
    }
    
    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        self.expires_at < now
    }
    
    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    /// Get session age in seconds
    pub fn age(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.created_at
    }
    
    /// Get time since last activity in seconds
    pub fn idle_time(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.last_activity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_firebase_config_creation() {
        // Test with minimal required environment variables
        unsafe {
            std::env::set_var("FIREBASE_PROJECT_ID", "test-project");
            std::env::set_var("FIREBASE_API_KEY", "test-api-key");
        }
        
        let config = FirebaseConfig::from_env().unwrap();
        
        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.auth_domain, "test-project.firebaseapp.com");
        assert!(!config.has_service_account());
        
        // Clean up
        unsafe {
            std::env::remove_var("FIREBASE_PROJECT_ID");
            std::env::remove_var("FIREBASE_API_KEY");
        }
    }
    
    #[test]
    fn test_firebase_config_with_service_account() {
        // Test with service account configuration
        unsafe {
            std::env::set_var("FIREBASE_PROJECT_ID", "test-project");
            std::env::set_var("FIREBASE_API_KEY", "test-api-key");
            std::env::set_var("FIREBASE_SERVICE_ACCOUNT_EMAIL", "test@test-project.iam.gserviceaccount.com");
            std::env::set_var("FIREBASE_PRIVATE_KEY", "-----BEGIN PRIVATE KEY-----\ntest-key\n-----END PRIVATE KEY-----");
        }
        
        let config = FirebaseConfig::from_env().unwrap();
        
        assert!(config.has_service_account());
        assert_eq!(config.service_account_email.unwrap(), "test@test-project.iam.gserviceaccount.com");
        
        // Clean up
        unsafe {
            std::env::remove_var("FIREBASE_PROJECT_ID");
            std::env::remove_var("FIREBASE_API_KEY");
            std::env::remove_var("FIREBASE_SERVICE_ACCOUNT_EMAIL");
            std::env::remove_var("FIREBASE_PRIVATE_KEY");
        }
    }
    
    #[test]
    fn test_user_session_creation() {
        let claims = FirebaseTokenClaims {
            iss: "https://securetoken.google.com/test-project".to_string(),
            aud: "test-project".to_string(),
            auth_time: 1234567890,
            user_id: "test-user-id".to_string(),
            sub: "test-user-id".to_string(),
            iat: 1234567890,
            exp: 1234567890 + 3600, // 1 hour from now
            email: Some("test@example.com".to_string()),
            email_verified: Some(true),
            phone_number: Some("+1234567890".to_string()),
            name: Some("Test User".to_string()),
            picture: None,
            firebase: FirebaseAuthContext {
                identities: HashMap::new(),
                sign_in_provider: "phone".to_string(),
            },
        };
        
        let session = UserSession::new(
            &claims,
            "test-id-token".to_string(),
            "test-refresh-token".to_string(),
        );
        
        assert_eq!(session.user_id, "test-user-id");
        assert_eq!(session.email.unwrap(), "test@example.com");
        assert_eq!(session.phone_number.unwrap(), "+1234567890");
        assert_eq!(session.name.unwrap(), "Test User");
        assert_eq!(session.id_token, "test-id-token");
        assert_eq!(session.refresh_token, "test-refresh-token");
    }
    
    #[test]
    fn test_session_expiration() {
        let mut claims = FirebaseTokenClaims {
            iss: "https://securetoken.google.com/test-project".to_string(),
            aud: "test-project".to_string(),
            auth_time: 1234567890,
            user_id: "test-user-id".to_string(),
            sub: "test-user-id".to_string(),
            iat: 1234567890,
            exp: 1234567890, // Already expired
            email: None,
            email_verified: None,
            phone_number: None,
            name: None,
            picture: None,
            firebase: FirebaseAuthContext {
                identities: HashMap::new(),
                sign_in_provider: "phone".to_string(),
            },
        };
        
        let session = UserSession::new(
            &claims,
            "test-id-token".to_string(),
            "test-refresh-token".to_string(),
        );
        
        assert!(session.is_expired());
        
        // Test non-expired session
        claims.exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + 3600; // 1 hour from now
        
        let session = UserSession::new(
            &claims,
            "test-id-token".to_string(),
            "test-refresh-token".to_string(),
        );
        
        assert!(!session.is_expired());
    }
}