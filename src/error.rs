use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Application error types
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Anyhow(#[from] anyhow::Error),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Payment error: {0}")]
    Payment(String),

    #[error("External service error ({0}): {1}")]
    ExternalService(String, String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl AppError {
    /// Get HTTP status code for the error
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Authentication(_) => StatusCode::UNAUTHORIZED,
            AppError::Authorization(_) => StatusCode::FORBIDDEN,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Payment(_) => StatusCode::PAYMENT_REQUIRED,
            AppError::ExternalService(_, _) => StatusCode::BAD_GATEWAY,
            AppError::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RateLimit(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code for client identification
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Authentication(_) => "AUTHENTICATION_ERROR",
            AppError::Authorization(_) => "AUTHORIZATION_ERROR",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::Payment(_) => "PAYMENT_ERROR",
            AppError::ExternalService(_, _) => "EXTERNAL_SERVICE_ERROR",
            AppError::Configuration(_) => "CONFIGURATION_ERROR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::RateLimit(_) => "RATE_LIMIT_EXCEEDED",
            AppError::Internal(_) => "INTERNAL_ERROR",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            AppError::Anyhow(_) => "INTERNAL_ERROR",
        }
    }

    /// Check if error should be logged as warning vs error
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            AppError::Authentication(_)
                | AppError::Authorization(_)
                | AppError::Validation(_)
                | AppError::NotFound(_)
                | AppError::BadRequest(_)
                | AppError::Conflict(_)
                | AppError::RateLimit(_)
        )
    }
}

/// Error response format for API clients
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: String,
    pub status: u16,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    /// Create error response from AppError
    pub fn from_app_error(error: AppError, request_id: Option<String>) -> Self {
        let status_code = error.status_code();

        Self {
            error: error.error_code().to_string(),
            message: error.to_string(),
            code: error.error_code().to_string(),
            status: status_code.as_u16(),
            timestamp: Utc::now(),
            request_id,
            details: None,
        }
    }

    /// Create error response with additional details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Create validation error with field details
    pub fn validation_error(message: String, field_errors: Vec<FieldError>) -> Self {
        let details = serde_json::json!({
            "field_errors": field_errors
        });

        Self {
            error: "VALIDATION_ERROR".to_string(),
            message,
            code: "VALIDATION_ERROR".to_string(),
            status: StatusCode::BAD_REQUEST.as_u16(),
            timestamp: Utc::now(),
            request_id: None,
            details: Some(details),
        }
    }
}

/// Field-specific validation error
#[derive(Debug, Serialize, Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub code: String,
}

impl FieldError {
    pub fn new(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            code: code.into(),
        }
    }
}

/// Result type alias for application operations
pub type AppResult<T> = Result<T, AppError>;

/// Helper functions for creating common errors
impl AppError {
    pub fn not_found(resource: &str, id: impl fmt::Display) -> Self {
        Self::NotFound(format!("{} with id {} not found", resource, id))
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    pub fn authorization(message: impl Into<String>) -> Self {
        Self::Authorization(message.into())
    }

    pub fn payment(message: impl Into<String>) -> Self {
        Self::Payment(message.into())
    }

    pub fn external_service(service: &str, message: impl Into<String>) -> Self {
        Self::ExternalService(service.to_string(), message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            AppError::NotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Authentication("test".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Authorization("test".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::Validation("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AppError::NotFound("test".to_string()).error_code(),
            "NOT_FOUND"
        );
        assert_eq!(
            AppError::Authentication("test".to_string()).error_code(),
            "AUTHENTICATION_ERROR"
        );
        assert_eq!(
            AppError::Validation("test".to_string()).error_code(),
            "VALIDATION_ERROR"
        );
    }

    #[test]
    fn test_client_error_classification() {
        assert!(AppError::NotFound("test".to_string()).is_client_error());
        assert!(AppError::Validation("test".to_string()).is_client_error());
        assert!(!AppError::Database(sqlx::Error::RowNotFound).is_client_error());
    }

    #[test]
    fn test_error_response_creation() {
        let error = AppError::NotFound("User".to_string());
        let response = ErrorResponse::from_app_error(error, Some("req-123".to_string()));

        assert_eq!(response.error, "NOT_FOUND");
        assert_eq!(response.status, 404);
        assert_eq!(response.request_id, Some("req-123".to_string()));
    }
}
