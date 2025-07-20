use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Request context for logging and tracing
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub user_id: Option<i64>,
    pub user_type: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub started_at: Instant,
    pub timestamp: DateTime<Utc>,
}

impl RequestContext {
    /// Create new request context
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            user_id: None,
            user_type: None,
            ip_address: None,
            user_agent: None,
            started_at: Instant::now(),
            timestamp: Utc::now(),
        }
    }
    
    /// Set user information
    pub fn with_user(mut self, user_id: i64, user_type: String) -> Self {
        self.user_id = Some(user_id);
        self.user_type = Some(user_type);
        self
    }
    
    /// Set client information
    pub fn with_client_info(mut self, ip_address: Option<String>, user_agent: Option<String>) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }
    
    /// Get elapsed time since request started
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP request log entry
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestLog {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub query_params: Option<String>,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub user_id: Option<i64>,
    pub user_type: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub error_message: Option<String>,
}

impl RequestLog {
    /// Create request log from context and response info
    pub fn new(
        ctx: &RequestContext,
        method: String,
        path: String,
        query_params: Option<String>,
        status_code: u16,
        error_message: Option<String>,
    ) -> Self {
        Self {
            request_id: ctx.request_id.clone(),
            method,
            path,
            query_params,
            status_code,
            response_time_ms: ctx.elapsed().as_millis() as u64,
            user_id: ctx.user_id,
            user_type: ctx.user_type.clone(),
            ip_address: ctx.ip_address.clone(),
            user_agent: ctx.user_agent.clone(),
            timestamp: ctx.timestamp,
            error_message,
        }
    }
    
    /// Log the request using appropriate log level
    pub fn log(&self) {
        let log_data = serde_json::to_string(self).unwrap_or_else(|_| "Failed to serialize log".to_string());
        
        match self.status_code {
            200..=299 => info!("HTTP Request: {}", log_data),
            300..=399 => info!("HTTP Request (Redirect): {}", log_data),
            400..=499 => warn!("HTTP Request (Client Error): {}", log_data),
            500..=599 => error!("HTTP Request (Server Error): {}", log_data),
            _ => debug!("HTTP Request: {}", log_data),
        }
    }
}

/// Application event log entry
#[derive(Debug, Serialize, Deserialize)]
pub struct EventLog {
    pub event_id: String,
    pub event_type: String,
    pub event_name: String,
    pub user_id: Option<i64>,
    pub user_type: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
}

impl EventLog {
    /// Create new event log
    pub fn new(event_type: String, event_name: String) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            event_type,
            event_name,
            user_id: None,
            user_type: None,
            resource_type: None,
            resource_id: None,
            metadata: None,
            timestamp: Utc::now(),
            request_id: None,
        }
    }
    
    /// Set user context
    pub fn with_user(mut self, user_id: i64, user_type: String) -> Self {
        self.user_id = Some(user_id);
        self.user_type = Some(user_type);
        self
    }
    
    /// Set resource context
    pub fn with_resource(mut self, resource_type: String, resource_id: String) -> Self {
        self.resource_type = Some(resource_type);
        self.resource_id = Some(resource_id);
        self
    }
    
    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
    
    /// Set request context
    pub fn with_request(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
    
    /// Log the event
    pub fn log(&self) {
        let log_data = serde_json::to_string(self).unwrap_or_else(|_| "Failed to serialize event".to_string());
        info!("Application Event: {}", log_data);
    }
}

/// Performance metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub duration_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
}

impl PerformanceMetrics {
    /// Create performance metrics
    pub fn new(operation: String, duration: std::time::Duration, success: bool) -> Self {
        Self {
            operation,
            duration_ms: duration.as_millis() as u64,
            success,
            error_message: None,
            metadata: None,
            timestamp: Utc::now(),
            request_id: None,
        }
    }
    
    /// Set error information
    pub fn with_error(mut self, error_message: String) -> Self {
        self.error_message = Some(error_message);
        self.success = false;
        self
    }
    
    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
    
    /// Set request context
    pub fn with_request(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
    
    /// Log the metrics
    pub fn log(&self) {
        let log_data = serde_json::to_string(self).unwrap_or_else(|_| "Failed to serialize metrics".to_string());
        
        if self.success {
            info!("Performance Metrics: {}", log_data);
        } else {
            warn!("Performance Metrics (Failed): {}", log_data);
        }
    }
}

/// Initialize logging system
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .init();
    
    info!("Logging system initialized");
    Ok(())
}

/// Macro for logging database operations
#[macro_export]
macro_rules! log_db_operation {
    ($operation:expr, $result:expr, $request_id:expr) => {
        match &$result {
            Ok(_) => {
                let metrics = PerformanceMetrics::new(
                    format!("db_{}", $operation),
                    std::time::Duration::from_millis(0), // Would need actual timing
                    true,
                ).with_request($request_id.clone());
                metrics.log();
            }
            Err(e) => {
                let metrics = PerformanceMetrics::new(
                    format!("db_{}", $operation),
                    std::time::Duration::from_millis(0), // Would need actual timing
                    false,
                ).with_error(e.to_string())
                .with_request($request_id.clone());
                metrics.log();
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_request_context_creation() {
        let ctx = RequestContext::new();
        assert!(!ctx.request_id.is_empty());
        assert!(ctx.user_id.is_none());
        assert!(ctx.user_type.is_none());
    }
    
    #[test]
    fn test_request_context_with_user() {
        let ctx = RequestContext::new().with_user(123, "customer".to_string());
        assert_eq!(ctx.user_id, Some(123));
        assert_eq!(ctx.user_type, Some("customer".to_string()));
    }
    
    #[test]
    fn test_event_log_creation() {
        let event = EventLog::new("user".to_string(), "login".to_string())
            .with_user(123, "customer".to_string())
            .with_resource("session".to_string(), "sess_123".to_string());
        
        assert_eq!(event.event_type, "user");
        assert_eq!(event.event_name, "login");
        assert_eq!(event.user_id, Some(123));
        assert_eq!(event.resource_type, Some("session".to_string()));
    }
    
    #[test]
    fn test_performance_metrics() {
        let duration = std::time::Duration::from_millis(150);
        let metrics = PerformanceMetrics::new("database_query".to_string(), duration, true);
        
        assert_eq!(metrics.operation, "database_query");
        assert_eq!(metrics.duration_ms, 150);
        assert!(metrics.success);
    }
}