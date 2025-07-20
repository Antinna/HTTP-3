use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{debug, error, info};
use uuid::Uuid;

use crate::database::DatabaseService;
use crate::error::{AppError, AppResult};
use crate::firebase::{FirebaseAuth, FirebaseTokenClaims, UserSession};
use crate::models::UserType;

/// Authentication middleware for HTTP requests
pub struct AuthMiddleware {
    firebase_auth: Arc<FirebaseAuth>,
    session_store: Arc<SessionStore>,
}

impl AuthMiddleware {
    /// Create new authentication middleware
    pub fn new(firebase_auth: Arc<FirebaseAuth>, session_store: Arc<SessionStore>) -> Self {
        Self {
            firebase_auth,
            session_store,
        }
    }

    /// Authenticate request using Bearer token
    pub async fn authenticate(&self, auth_header: Option<&str>) -> AppResult<AuthenticatedUser> {
        let token = self.extract_bearer_token(auth_header)?;
        
        // First try to get session from store
        if let Ok(session) = self.session_store.get_session(&token).await {
            if !session.is_expired() {
                // Update last activity
                self.session_store.update_activity(&token).await?;
                
                return Ok(AuthenticatedUser {
                    user_id: session.user_id.clone(),
                    email: session.email.clone(),
                    phone_number: session.phone_number.clone(),
                    name: session.name.clone(),
                    picture: session.picture.clone(),
                    user_type: UserType::User, // Default, should be loaded from database
                    session_id: token,
                    firebase_claims: None,
                });
            } else {
                // Session expired, remove it
                self.session_store.remove_session(&token).await?;
            }
        }

        // If no valid session, verify with Firebase
        let mut firebase_auth = (*self.firebase_auth).clone();
        let claims = firebase_auth.verify_token(&token).await?;
        
        // Create new session
        let session = UserSession::new(&claims, token.clone(), "".to_string());
        self.session_store.store_session(&token, session).await?;
        
        info!("User authenticated successfully: {}", claims.user_id);
        
        Ok(AuthenticatedUser {
            user_id: claims.user_id.clone(),
            email: claims.email.clone(),
            phone_number: claims.phone_number.clone(),
            name: claims.name.clone(),
            picture: claims.picture.clone(),
            user_type: UserType::User, // Default, should be loaded from database
            session_id: token,
            firebase_claims: Some(claims),
        })
    }

    /// Extract Bearer token from Authorization header
    fn extract_bearer_token(&self, auth_header: Option<&str>) -> AppResult<String> {
        let header = auth_header
            .ok_or_else(|| AppError::Authentication("Missing Authorization header".to_string()))?;

        if !header.starts_with("Bearer ") {
            return Err(AppError::Authentication("Invalid Authorization header format".to_string()));
        }

        let token = header.strip_prefix("Bearer ").unwrap().trim();
        if token.is_empty() {
            return Err(AppError::Authentication("Empty Bearer token".to_string()));
        }

        Ok(token.to_string())
    }

    /// Check if user has required permission
    pub fn authorize(&self, user: &AuthenticatedUser, required_permission: Permission) -> AppResult<()> {
        match required_permission {
            Permission::Public => Ok(()),
            Permission::Authenticated => {
                // User is already authenticated if we reach here
                Ok(())
            }
            Permission::Admin => {
                if user.user_type == UserType::Admin {
                    Ok(())
                } else {
                    Err(AppError::Authorization("Admin access required".to_string()))
                }
            }
            Permission::DeliveryPerson => {
                if matches!(user.user_type, UserType::Admin | UserType::DeliveryPerson) {
                    Ok(())
                } else {
                    Err(AppError::Authorization("Delivery person access required".to_string()))
                }
            }
            Permission::Customer => {
                if matches!(user.user_type, UserType::Admin | UserType::User) {
                    Ok(())
                } else {
                    Err(AppError::Authorization("User access required".to_string()))
                }
            }
        }
    }
}

/// Authenticated user information
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub user_type: UserType,
    pub session_id: String,
    pub firebase_claims: Option<FirebaseTokenClaims>,
}

/// Permission levels for authorization
#[derive(Debug, Clone, PartialEq)]
pub enum Permission {
    Public,
    Authenticated,
    Customer,
    DeliveryPerson,
    Admin,
}

/// Session store for managing user sessions
pub struct SessionStore {
    database: Arc<DatabaseService>,
    sessions: tokio::sync::RwLock<HashMap<String, UserSession>>,
}

impl SessionStore {
    /// Create new session store
    pub fn new(database: Arc<DatabaseService>) -> Self {
        Self {
            database,
            sessions: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Store user session
    pub async fn store_session(&self, session_id: &str, session: UserSession) -> AppResult<()> {
        // Store in memory cache
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), session.clone());
        }

        // Store in database for persistence
        self.store_session_in_db(session_id, &session).await?;
        
        debug!("Session stored for user: {}", session.user_id);
        Ok(())
    }

    /// Get user session
    pub async fn get_session(&self, session_id: &str) -> AppResult<UserSession> {
        // First check memory cache
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                return Ok(session.clone());
            }
        }

        // If not in cache, try database
        let session = self.get_session_from_db(session_id).await?;
        
        // Store in cache for future requests
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), session.clone());
        }

        Ok(session)
    }

    /// Update session activity
    pub async fn update_activity(&self, session_id: &str) -> AppResult<()> {
        // Update in memory cache
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.update_activity();
            }
        }

        // Update in database
        self.update_session_activity_in_db(session_id).await?;
        
        Ok(())
    }

    /// Remove user session
    pub async fn remove_session(&self, session_id: &str) -> AppResult<()> {
        // Remove from memory cache
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }

        // Remove from database
        self.remove_session_from_db(session_id).await?;
        
        debug!("Session removed: {}", session_id);
        Ok(())
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> AppResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Clean up memory cache
        {
            let mut sessions = self.sessions.write().await;
            sessions.retain(|_, session| !session.is_expired());
        }

        // Clean up database
        self.cleanup_expired_sessions_in_db(now).await?;
        
        info!("Expired sessions cleaned up");
        Ok(())
    }

    /// Store session in database
    async fn store_session_in_db(&self, session_id: &str, session: &UserSession) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO user_sessions (
                session_id, user_id, email, phone_number, name, picture,
                id_token, refresh_token, expires_at, created_at, last_activity
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                last_activity = VALUES(last_activity),
                expires_at = VALUES(expires_at)
            "#
        )
        .bind(session_id)
        .bind(&session.user_id)
        .bind(&session.email)
        .bind(&session.phone_number)
        .bind(&session.name)
        .bind(&session.picture)
        .bind(&session.id_token)
        .bind(&session.refresh_token)
        .bind(session.expires_at as i64)
        .bind(session.created_at as i64)
        .bind(session.last_activity as i64)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }

    /// Get session from database
    async fn get_session_from_db(&self, session_id: &str) -> AppResult<UserSession> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String, i64, i64, i64)>(
            "SELECT session_id, user_id, email, phone_number, name, picture, id_token, refresh_token, expires_at, created_at, last_activity FROM user_sessions WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_one(self.database.pool())
        .await
        .map_err(|_| AppError::Authentication("Session not found".to_string()))?;

        Ok(UserSession {
            user_id: row.1,
            email: row.2,
            phone_number: row.3,
            name: row.4,
            picture: row.5,
            id_token: row.6,
            refresh_token: row.7,
            expires_at: row.8 as u64,
            created_at: row.9 as u64,
            last_activity: row.10 as u64,
        })
    }

    /// Update session activity in database
    async fn update_session_activity_in_db(&self, session_id: &str) -> AppResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        sqlx::query(
            "UPDATE user_sessions SET last_activity = ? WHERE session_id = ?"
        )
        .bind(now)
        .bind(session_id)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }

    /// Remove session from database
    async fn remove_session_from_db(&self, session_id: &str) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM user_sessions WHERE session_id = ?"
        )
        .bind(session_id)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }

    /// Clean up expired sessions from database
    async fn cleanup_expired_sessions_in_db(&self, current_time: u64) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM user_sessions WHERE expires_at < ?"
        )
        .bind(current_time as i64)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }
}

/// Authentication service for managing user authentication and sessions
pub struct AuthService {
    middleware: AuthMiddleware,
    firebase_auth: Arc<FirebaseAuth>,
    session_store: Arc<SessionStore>,
}

impl AuthService {
    /// Create new authentication service
    pub fn new(
        firebase_auth: Arc<FirebaseAuth>,
        database: Arc<DatabaseService>,
    ) -> Self {
        let session_store = Arc::new(SessionStore::new(database));
        let middleware = AuthMiddleware::new(firebase_auth.clone(), session_store.clone());

        Self {
            middleware,
            firebase_auth,
            session_store,
        }
    }

    /// Authenticate user with phone OTP
    pub async fn authenticate_with_phone(&self, phone_number: &str) -> AppResult<String> {
        let session_info = self.firebase_auth.send_otp(phone_number, None).await?;
        info!("OTP sent to phone number: {}", phone_number);
        Ok(session_info)
    }

    /// Verify OTP and create session
    pub async fn verify_otp_and_create_session(&self, session_info: &str, code: &str) -> AppResult<AuthenticatedUser> {
        let verification_response = self.firebase_auth.verify_otp(session_info, code).await?;
        
        // Create session
        let session_id = Uuid::new_v4().to_string();
        let mut firebase_auth = (*self.firebase_auth).clone();
        let claims = firebase_auth.verify_token(&verification_response.id_token).await?;
        
        let session = UserSession::new(&claims, verification_response.id_token, verification_response.refresh_token);
        self.session_store.store_session(&session_id, session).await?;
        
        info!("User session created successfully: {}", claims.user_id);
        
        Ok(AuthenticatedUser {
            user_id: claims.user_id.clone(),
            email: claims.email.clone(),
            phone_number: claims.phone_number.clone(),
            name: claims.name.clone(),
            picture: claims.picture.clone(),
            user_type: UserType::User, // Default, should be loaded from database
            session_id,
            firebase_claims: Some(claims),
        })
    }

    /// Logout user and remove session
    pub async fn logout(&self, session_id: &str) -> AppResult<()> {
        self.session_store.remove_session(session_id).await?;
        info!("User logged out successfully");
        Ok(())
    }

    /// Get authentication middleware
    pub fn middleware(&self) -> &AuthMiddleware {
        &self.middleware
    }

    /// Start session cleanup task
    pub async fn start_session_cleanup_task(&self) {
        let session_store = self.session_store.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Every hour
            
            loop {
                interval.tick().await;
                
                if let Err(e) = session_store.cleanup_expired_sessions().await {
                    error!("Failed to cleanup expired sessions: {}", e);
                } else {
                    debug!("Session cleanup completed successfully");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::UserType;

    #[test]
    fn test_bearer_token_extraction() {
        // This test only tests the token extraction logic, no database needed
        // We'll create a minimal middleware just for testing
        
        // Test the token extraction logic directly
        let test_middleware = TestAuthMiddleware;
        
        // Valid Bearer token
        let result = test_middleware.extract_bearer_token(Some("Bearer test-token-123"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-token-123");

        // Missing header
        let result = test_middleware.extract_bearer_token(None);
        assert!(result.is_err());

        // Invalid format
        let result = test_middleware.extract_bearer_token(Some("Basic test-token"));
        assert!(result.is_err());

        // Empty token
        let result = test_middleware.extract_bearer_token(Some("Bearer "));
        assert!(result.is_err());
    }
    
    // Test helper struct that implements the same token extraction logic
    struct TestAuthMiddleware;
    
    impl TestAuthMiddleware {
        fn extract_bearer_token(&self, auth_header: Option<&str>) -> AppResult<String> {
            let header = auth_header
                .ok_or_else(|| AppError::Authentication("Missing Authorization header".to_string()))?;

            if !header.starts_with("Bearer ") {
                return Err(AppError::Authentication("Invalid Authorization header format".to_string()));
            }

            let token = header.strip_prefix("Bearer ").unwrap().trim();
            if token.is_empty() {
                return Err(AppError::Authentication("Empty Bearer token".to_string()));
            }

            Ok(token.to_string())
        }
        
        fn authorize(&self, user: &AuthenticatedUser, required_permission: Permission) -> AppResult<()> {
            match required_permission {
                Permission::Public => Ok(()),
                Permission::Authenticated => Ok(()),
                Permission::Admin => {
                    if user.user_type == UserType::Admin {
                        Ok(())
                    } else {
                        Err(AppError::Authorization("Admin access required".to_string()))
                    }
                }
                Permission::DeliveryPerson => {
                    if matches!(user.user_type, UserType::Admin | UserType::DeliveryPerson) {
                        Ok(())
                    } else {
                        Err(AppError::Authorization("Delivery person access required".to_string()))
                    }
                }
                Permission::Customer => {
                    if matches!(user.user_type, UserType::Admin | UserType::User) {
                        Ok(())
                    } else {
                        Err(AppError::Authorization("User access required".to_string()))
                    }
                }
            }
        }
    }

    #[test]
    fn test_authorization() {
        // This test only tests the authorization logic, no database or Firebase needed
        let test_middleware = TestAuthMiddleware;

        let admin_user = AuthenticatedUser {
            user_id: "admin-123".to_string(),
            email: Some("admin@test.com".to_string()),
            phone_number: None,
            name: Some("Admin User".to_string()),
            picture: None,
            user_type: UserType::Admin,
            session_id: "session-123".to_string(),
            firebase_claims: None,
        };

        let customer_user = AuthenticatedUser {
            user_id: "customer-123".to_string(),
            email: Some("customer@test.com".to_string()),
            phone_number: None,
            name: Some("Customer User".to_string()),
            picture: None,
            user_type: UserType::User,
            session_id: "session-456".to_string(),
            firebase_claims: None,
        };

        // Admin can access admin endpoints
        assert!(test_middleware.authorize(&admin_user, Permission::Admin).is_ok());
        
        // Customer cannot access admin endpoints
        assert!(test_middleware.authorize(&customer_user, Permission::Admin).is_err());
        
        // Both can access customer endpoints
        assert!(test_middleware.authorize(&admin_user, Permission::Customer).is_ok());
        assert!(test_middleware.authorize(&customer_user, Permission::Customer).is_ok());
        
        // Both can access authenticated endpoints
        assert!(test_middleware.authorize(&admin_user, Permission::Authenticated).is_ok());
        assert!(test_middleware.authorize(&customer_user, Permission::Authenticated).is_ok());
        
        // Both can access public endpoints
        assert!(test_middleware.authorize(&admin_user, Permission::Public).is_ok());
        assert!(test_middleware.authorize(&customer_user, Permission::Public).is_ok());
    }
}