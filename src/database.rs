use crate::error::{AppError, AppResult};
use anyhow::Context;
use sqlx::{ConnectOptions, MySql, MySqlPool, Transaction};
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, warn, error};

/// Database service for managing MySQL connections and transactions
#[derive(Debug, Clone)]
pub struct DatabaseService {
    pool: MySqlPool,
}

impl DatabaseService {
    /// Create new database service with connection pool
    pub async fn new(database_url: &str) -> AppResult<Self> {
        info!("Initializing database connection pool");
        
        let pool = Self::create_pool_with_retry(database_url, 3).await?;
        
        info!("Database connection pool initialized successfully");
        
        Ok(Self { pool })
    }
    
    /// Create database connection pool with retry logic
    async fn create_pool_with_retry(database_url: &str, max_retries: u32) -> AppResult<MySqlPool> {
        let mut retry_count = 0;
        let mut last_error = None;
        
        while retry_count < max_retries {
            match MySqlPool::connect_with(
                sqlx::mysql::MySqlConnectOptions::from_str(database_url)
                    .context("Invalid database URL")?
                    .disable_statement_logging()
            )
            .await
            {
                Ok(pool) => {
                    // Test the connection
                    match sqlx::query("SELECT 1").execute(&pool).await {
                        Ok(_) => {
                            info!("Database connection established successfully");
                            return Ok(pool);
                        }
                        Err(e) => {
                            warn!("Database connection test failed on attempt {}: {}", retry_count + 1, e);
                            last_error = Some(e.into());
                        }
                    }
                }
                Err(e) => {
                    warn!("Database connection failed on attempt {}: {}", retry_count + 1, e);
                    last_error = Some(e.into());
                }
            }
            
            retry_count += 1;
            if retry_count < max_retries {
                let delay = Duration::from_secs(2_u64.pow(retry_count)); // Exponential backoff
                info!("Retrying database connection in {:?}...", delay);
                tokio::time::sleep(delay).await;
            }
        }
        
        Err(AppError::Database(
            last_error.unwrap_or_else(|| {
                sqlx::Error::Configuration("Failed to connect to database after retries".into())
            })
        ))
    }
    
    /// Create database service with custom pool configuration
    pub async fn with_config(database_url: &str, max_connections: u32, connect_timeout: Duration) -> AppResult<Self> {
        info!("Initializing database connection pool with custom configuration");
        
        let pool = MySqlPool::connect_with(
            sqlx::mysql::MySqlConnectOptions::from_str(database_url)
                .context("Invalid database URL")?
                .disable_statement_logging()
        )
        .await
        .context("Failed to create database connection pool")?;
        
        // Test the connection
        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .context("Failed to test database connection")?;
        
        info!("Database connection pool initialized with {} max connections", max_connections);
        
        Ok(Self { pool })
    }
    
    /// Get reference to the connection pool
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }
    
    /// Execute a function within a database transaction
    pub async fn transaction<F, T>(&self, f: F) -> AppResult<T>
    where
        F: for<'c> FnOnce(&mut Transaction<'c, MySql>) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<T>> + Send + 'c>>,
    {
        let mut tx = self.pool
            .begin()
            .await
            .context("Failed to begin database transaction")?;
        
        match f(&mut tx).await {
            Ok(result) => {
                tx.commit()
                    .await
                    .context("Failed to commit database transaction")?;
                Ok(result)
            }
            Err(e) => {
                if let Err(rollback_err) = tx.rollback().await {
                    error!("Failed to rollback transaction: {}", rollback_err);
                }
                Err(e)
            }
        }
    }
    
    /// Check database health
    pub async fn health_check(&self) -> AppResult<DatabaseHealth> {
        let start = std::time::Instant::now();
        
        // Test basic connectivity
        let connectivity_result = sqlx::query("SELECT 1 as test")
            .fetch_one(&self.pool)
            .await;
        
        let response_time = start.elapsed();
        
        match connectivity_result {
            Ok(_) => {
                // Get pool statistics
                let pool_size = self.pool.size();
                let idle_connections = self.pool.num_idle();
                
                Ok(DatabaseHealth {
                    is_healthy: true,
                    response_time_ms: response_time.as_millis() as u64,
                    pool_size,
                    idle_connections: idle_connections as u32,
                    error_message: None,
                })
            }
            Err(e) => {
                warn!("Database health check failed: {}", e);
                Ok(DatabaseHealth {
                    is_healthy: false,
                    response_time_ms: response_time.as_millis() as u64,
                    pool_size: self.pool.size(),
                    idle_connections: self.pool.num_idle() as u32,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Run database migrations
    pub async fn migrate(&self) -> AppResult<()> {
        info!("Running database migrations");
        
        // Create migrations table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id INT PRIMARY KEY AUTO_INCREMENT,
                version VARCHAR(255) NOT NULL UNIQUE,
                name VARCHAR(255) NOT NULL,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&self.pool)
        .await
        .context("Failed to create migrations table")?;
        
        // Check which migrations have been applied
        let applied_migrations: Vec<String> = sqlx::query_scalar(
            "SELECT version FROM _migrations ORDER BY version"
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch applied migrations")?;
        
        info!("Applied migrations: {:?}", applied_migrations);
        
        // For now, we'll just run the initial schema if not applied
        if !applied_migrations.contains(&"001".to_string()) {
            info!("Applying migration 001_initial_schema");
            
            // Read and execute the migration file
            let migration_sql = include_str!("../migrations/001_initial_schema.sql");
            
            // Split by semicolon and execute each statement
            for statement in migration_sql.split(';') {
                let statement = statement.trim();
                if !statement.is_empty() && !statement.starts_with("--") {
                    sqlx::query(statement)
                        .execute(&self.pool)
                        .await
                        .with_context(|| format!("Failed to execute migration statement: {}", statement))?;
                }
            }
            
            // Record the migration as applied
            sqlx::query(
                "INSERT INTO _migrations (version, name) VALUES (?, ?)"
            )
            .bind("001")
            .bind("initial_schema")
            .execute(&self.pool)
            .await
            .context("Failed to record migration")?;
            
            info!("Migration 001_initial_schema applied successfully");
        }
        
        info!("Database migrations completed successfully");
        Ok(())
    }
    
    /// Close the database connection pool
    pub async fn close(&self) {
        info!("Closing database connection pool");
        self.pool.close().await;
        info!("Database connection pool closed");
    }
}

/// Database health information
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub pool_size: u32,
    pub idle_connections: u32,
    pub error_message: Option<String>,
}

/// Database connection configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl DatabaseConfig {
    /// Create database config from environment
    pub fn from_env() -> AppResult<Self> {
        let url = std::env::var("DATABASE_URL")
            .or_else(|_| {
                // Build URL from components if DATABASE_URL not provided
                let host = std::env::var("DB_HOST")?;
                let port = std::env::var("DB_PORT")?;
                let name = std::env::var("DB_NAME")?;
                let username = std::env::var("DB_USERNAME")?;
                let password = std::env::var("DB_PASSWORD")?;
                
                Ok(format!("mysql://{}:{}@{}:{}/{}", username, password, host, port, name))
            })
            .map_err(|_| AppError::Configuration("Database URL or components not configured".to_string()))?;
        
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .context("Invalid DB_MAX_CONNECTIONS value")?;
        
        let connect_timeout_secs = std::env::var("DB_CONNECT_TIMEOUT")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .context("Invalid DB_CONNECT_TIMEOUT value")?;
        
        let idle_timeout_secs = std::env::var("DB_IDLE_TIMEOUT")
            .unwrap_or_else(|_| "600".to_string())
            .parse()
            .context("Invalid DB_IDLE_TIMEOUT value")?;
        
        let max_lifetime_secs = std::env::var("DB_MAX_LIFETIME")
            .unwrap_or_else(|_| "1800".to_string())
            .parse()
            .context("Invalid DB_MAX_LIFETIME value")?;
        
        Ok(Self {
            url,
            max_connections,
            connect_timeout: Duration::from_secs(connect_timeout_secs),
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            max_lifetime: Duration::from_secs(max_lifetime_secs),
        })
    }
}

/// Macro for executing database queries with error handling and logging
#[macro_export]
macro_rules! db_query {
    ($pool:expr, $query:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $query.execute($pool).await;
        let duration = start.elapsed();
        
        match &result {
            Ok(_) => {
                tracing::debug!("Database operation '{}' completed in {:?}", $operation, duration);
            }
            Err(e) => {
                tracing::error!("Database operation '{}' failed after {:?}: {}", $operation, duration, e);
            }
        }
        
        result.map_err(|e| crate::error::AppError::Database(e))
    }};
}

/// Macro for executing database queries that return data
#[macro_export]
macro_rules! db_fetch {
    ($pool:expr, $query:expr, $operation:expr, $fetch_type:ident) => {{
        let start = std::time::Instant::now();
        let result = $query.$fetch_type($pool).await;
        let duration = start.elapsed();
        
        match &result {
            Ok(_) => {
                tracing::debug!("Database fetch '{}' completed in {:?}", $operation, duration);
            }
            Err(e) => {
                tracing::error!("Database fetch '{}' failed after {:?}: {}", $operation, duration, e);
            }
        }
        
        result.map_err(|e| crate::error::AppError::Database(e))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_database_config_from_components() {
        std::env::set_var("DB_HOST", "localhost");
        std::env::set_var("DB_PORT", "3306");
        std::env::set_var("DB_NAME", "test_db");
        std::env::set_var("DB_USERNAME", "test_user");
        std::env::set_var("DB_PASSWORD", "test_pass");
        
        let config = DatabaseConfig::from_env().expect("Config should load");
        assert!(config.url.contains("localhost:3306"));
        assert!(config.url.contains("test_db"));
        assert_eq!(config.max_connections, 10); // default value
    }
    
    #[test]
    fn test_database_config_with_url() {
        std::env::set_var("DATABASE_URL", "mysql://user:pass@host:3306/db");
        
        let config = DatabaseConfig::from_env().expect("Config should load");
        assert_eq!(config.url, "mysql://user:pass@host:3306/db");
    }
}