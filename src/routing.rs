use std::collections::HashMap;
use std::sync::Arc;
use http::{Request, StatusCode, Method};
use bytes::Bytes;
use serde_json::Value;
use tracing::{info, debug};

use crate::error::{AppError, AppResult};
use crate::database::DatabaseService;
use crate::currency::CurrencyHelper;
use crate::auth::AuthenticatedUser;

/// HTTP request context containing parsed information
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub method: Method,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<Bytes>,
    pub user: Option<AuthenticatedUser>,
    pub request_id: String,
}

impl RequestContext {
    /// Create a new request context from HTTP request
    pub fn from_request(req: &Request<()>, body: Option<Bytes>) -> Self {
        let mut query_params = HashMap::new();
        let mut headers = HashMap::new();

        // Parse query parameters
        if let Some(query) = req.uri().query() {
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    query_params.insert(
                        urlencoding::decode(key).unwrap_or_default().to_string(),
                        urlencoding::decode(value).unwrap_or_default().to_string(),
                    );
                }
            }
        }

        // Extract headers
        for (name, value) in req.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // Generate request ID
        let request_id = uuid::Uuid::new_v4().to_string();

        Self {
            method: req.method().clone(),
            path: req.uri().path().to_string(),
            query_params,
            headers,
            body,
            user: None,
            request_id,
        }
    }

    /// Get query parameter by name
    pub fn query_param(&self, name: &str) -> Option<&String> {
        self.query_params.get(name)
    }

    /// Get header by name
    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    /// Get user ID if authenticated
    pub fn user_id(&self) -> Option<&str> {
        self.user.as_ref().map(|u| u.user_id.as_str())
    }
}

/// HTTP response builder
#[derive(Debug)]
pub struct ResponseBuilder {
    status: StatusCode,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl ResponseBuilder {
    /// Create a new response builder
    pub fn new() -> Self {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("server".to_string(), "hotel-booking-http3/1.0".to_string());
        headers.insert("access-control-allow-origin".to_string(), "*".to_string());
        headers.insert("access-control-allow-methods".to_string(), "GET, POST, PUT, DELETE, OPTIONS".to_string());
        headers.insert("access-control-allow-headers".to_string(), "Content-Type, Authorization".to_string());

        Self {
            status: StatusCode::OK,
            headers,
            body: None,
        }
    }

    /// Set response status
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Set response header
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    /// Set response body as JSON
    pub fn json(mut self, value: &Value) -> Self {
        self.body = Some(value.to_string());
        self.headers.insert("content-type".to_string(), "application/json".to_string());
        self
    }

    /// Set response body as text
    pub fn text(mut self, text: &str) -> Self {
        self.body = Some(text.to_string());
        self.headers.insert("content-type".to_string(), "text/plain".to_string());
        self
    }

    /// Build the response
    pub fn build(self) -> (String, String, StatusCode) {
        let content_type = self.headers.get("content-type").cloned().unwrap_or_else(|| "text/plain".to_string());
        let body = self.body.unwrap_or_else(|| String::new());
        (body, content_type, self.status)
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Route handler function type
pub type RouteHandler = Box<dyn Fn(RequestContext, AppServices) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<ResponseBuilder>> + Send>> + Send + Sync>;

/// Application services container
#[derive(Clone)]
pub struct AppServices {
    pub database: Arc<DatabaseService>,
    pub currency_helper: Arc<CurrencyHelper>,
}

/// Route definition
#[derive(Clone)]
pub struct Route {
    pub method: Method,
    pub path: String,
    pub handler: Arc<RouteHandler>,
    pub middleware: Vec<String>,
}

/// HTTP router
pub struct Router {
    routes: Vec<Route>,
    middleware: HashMap<String, Arc<dyn Middleware>>,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            middleware: HashMap::new(),
        }
    }

    /// Add a route
    pub fn add_route(&mut self, method: Method, path: &str, handler: RouteHandler) {
        self.routes.push(Route {
            method,
            path: path.to_string(),
            handler: Arc::new(handler),
            middleware: Vec::new(),
        });
    }

    /// Add middleware
    pub fn add_middleware(&mut self, name: &str, middleware: Arc<dyn Middleware>) {
        self.middleware.insert(name.to_string(), middleware);
    }

    /// Route a request to the appropriate handler
    pub async fn route(&self, mut ctx: RequestContext, services: AppServices) -> AppResult<ResponseBuilder> {
        debug!("Routing request: {} {}", ctx.method, ctx.path);

        // Find matching route
        let route = self.routes.iter()
            .find(|r| r.method == ctx.method && self.path_matches(&r.path, &ctx.path))
            .ok_or_else(|| AppError::NotFound(format!("Route {} {} not found", ctx.method, ctx.path)))?;

        // Apply middleware
        for middleware_name in &route.middleware {
            if let Some(middleware) = self.middleware.get(middleware_name) {
                ctx = middleware.process(ctx, &services).await?;
            }
        }

        // Call the handler
        let handler = &route.handler;
        handler(ctx, services).await
    }

    /// Check if path matches route pattern
    fn path_matches(&self, pattern: &str, path: &str) -> bool {
        // Simple exact match for now
        // TODO: Implement path parameters and wildcards
        pattern == path
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware trait
#[async_trait::async_trait]
pub trait Middleware: Send + Sync {
    async fn process(&self, ctx: RequestContext, services: &AppServices) -> AppResult<RequestContext>;
}

/// Logging middleware
pub struct LoggingMiddleware;

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    async fn process(&self, ctx: RequestContext, _services: &AppServices) -> AppResult<RequestContext> {
        info!(
            "Request {} {} from {} - Request ID: {}",
            ctx.method,
            ctx.path,
            ctx.header("x-forwarded-for").or_else(|| ctx.header("x-real-ip")).unwrap_or(&"unknown".to_string()),
            ctx.request_id
        );
        Ok(ctx)
    }
}

/// Authentication middleware
pub struct AuthMiddleware;

#[async_trait::async_trait]
impl Middleware for AuthMiddleware {
    async fn process(&self, mut ctx: RequestContext, _services: &AppServices) -> AppResult<RequestContext> {
        // Extract authorization header
        if let Some(auth_header) = ctx.header("authorization") {
            if auth_header.starts_with("Bearer ") {
                let token = &auth_header[7..];
                // TODO: Validate JWT token with Firebase
                debug!("Found bearer token: {}", &token[..std::cmp::min(token.len(), 20)]);
                
                // For now, create a mock authenticated user
                // In a real implementation, this would validate the token
                if !token.is_empty() {
                    ctx.user = Some(AuthenticatedUser {
                        user_id: "mock_user_id".to_string(),
                        email: Some("mock@example.com".to_string()),
                        phone_number: None,
                        name: Some("Mock User".to_string()),
                        picture: None,
                        user_type: crate::models::UserType::User,
                        session_id: "mock_session".to_string(),
                        firebase_claims: None,
                    });
                }
            }
        }
        Ok(ctx)
    }
}

/// CORS middleware
pub struct CorsMiddleware;

#[async_trait::async_trait]
impl Middleware for CorsMiddleware {
    async fn process(&self, ctx: RequestContext, _services: &AppServices) -> AppResult<RequestContext> {
        // CORS headers are added in ResponseBuilder by default
        Ok(ctx)
    }
}

/// Validation middleware
pub struct ValidationMiddleware;

#[async_trait::async_trait]
impl Middleware for ValidationMiddleware {
    async fn process(&self, ctx: RequestContext, _services: &AppServices) -> AppResult<RequestContext> {
        // Basic request validation
        if ctx.path.len() > 1000 {
            return Err(AppError::BadRequest("Request path too long".to_string()));
        }

        if let Some(body) = &ctx.body {
            if body.len() > 10_000_000 { // 10MB limit
                return Err(AppError::BadRequest("Request body too large".to_string()));
            }
        }

        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;

    #[test]
    fn test_response_builder() {
        let response = ResponseBuilder::new()
            .status(StatusCode::CREATED)
            .header("custom-header", "custom-value")
            .json(&serde_json::json!({"message": "test"}))
            .build();

        assert_eq!(response.2, StatusCode::CREATED);
        assert!(response.0.contains("test"));
        assert_eq!(response.1, "application/json");
    }

    #[test]
    fn test_request_context_query_params() {
        let req = Request::builder()
            .uri("http://example.com/test?param1=value1&param2=value%202")
            .body(())
            .unwrap();

        let ctx = RequestContext::from_request(&req, None);
        
        assert_eq!(ctx.query_param("param1"), Some(&"value1".to_string()));
        assert_eq!(ctx.query_param("param2"), Some(&"value 2".to_string()));
        assert_eq!(ctx.query_param("nonexistent"), None);
    }

    #[tokio::test]
    async fn test_router_basic_routing() {
        let mut router = Router::new();
        
        router.add_route(Method::GET, "/test", Box::new(|_ctx, _services| {
            Box::pin(async move {
                Ok(ResponseBuilder::new().text("test response"))
            })
        }));

        let ctx = RequestContext {
            method: Method::GET,
            path: "/test".to_string(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
            body: None,
            user: None,
            request_id: "test".to_string(),
        };

        let services = AppServices {
            database: Arc::new(DatabaseService::new("mock://").await.unwrap()),
            currency_helper: Arc::new(CurrencyHelper::from_env().unwrap()),
        };

        let result = router.route(ctx, services).await;
        assert!(result.is_ok());
    }
}