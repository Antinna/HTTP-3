use bytes::Bytes;
use http::{Response, StatusCode};
use quinn::{Endpoint, ServerConfig, Connection as QuinnConnection};
use rustls::{ServerConfig as TlsServerConfig, pki_types::PrivateKeyDer};
use std::net::SocketAddr;

use std::sync::Arc;
use tracing::{info, warn, error, debug};

use crate::config::AppConfig;
use crate::database::DatabaseService;
use crate::error::{AppError, AppResult};
use crate::currency::CurrencyHelper;
use crate::routing::{Router, RequestContext, ResponseBuilder, AppServices, LoggingMiddleware, AuthMiddleware, CorsMiddleware, ValidationMiddleware};
use crate::handlers;

/// HTTP/3 Server with QUIC protocol support
pub struct Http3Server {
    endpoint: Endpoint,
    router: Arc<Router>,
    services: AppServices,
    config: AppConfig,
}

impl Http3Server {
    /// Create a new HTTP/3 server
    pub async fn new(
        config: AppConfig,
        database: Arc<DatabaseService>,
        currency_helper: Arc<CurrencyHelper>,
    ) -> AppResult<Self> {
        // Install the default crypto provider for rustls
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .map_err(|_| AppError::Internal("Failed to install crypto provider".to_string()))?;

        // Generate TLS configuration
        let tls_config = Self::create_tls_config().await?;
        
        // Create Quinn server configuration
        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
                .map_err(|e| AppError::Internal(format!("Failed to create QUIC config: {}", e)))?
        ));

        // Bind the endpoint
        let bind_addr: SocketAddr = config.server_address()
            .parse()
            .map_err(|e| AppError::Internal(format!("Invalid server address: {}", e)))?;
            
        let endpoint = Endpoint::server(server_config, bind_addr)
            .map_err(|e| AppError::Internal(format!("Failed to create endpoint: {}", e)))?;

        // Create services container
        let services = AppServices {
            database: database.clone(),
            currency_helper: currency_helper.clone(),
        };

        // Set up router with routes and middleware
        let router = Self::setup_router();

        info!("HTTP/3 server initialized on {}", config.server_address());

        Ok(Self {
            endpoint,
            router: Arc::new(router),
            services,
            config,
        })
    }

    /// Set up the router with all routes and middleware
    fn setup_router() -> Router {
        use http::Method;
        
        let mut router = Router::new();

        // Add middleware
        router.add_middleware("logging", Arc::new(LoggingMiddleware));
        router.add_middleware("auth", Arc::new(AuthMiddleware));
        router.add_middleware("cors", Arc::new(CorsMiddleware));
        router.add_middleware("validation", Arc::new(ValidationMiddleware));

        // Add routes
        router.add_route(Method::GET, "/", Box::new(|ctx, services| {
            Box::pin(handlers::root_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/health", Box::new(|ctx, services| {
            Box::pin(handlers::health_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/api/docs", Box::new(|ctx, services| {
            Box::pin(handlers::api_docs_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/api/currency", Box::new(|ctx, services| {
            Box::pin(handlers::currency_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/api/users/profile", Box::new(|ctx, services| {
            Box::pin(handlers::user_profile_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/api/menu", Box::new(|ctx, services| {
            Box::pin(handlers::menu_handler(ctx, services))
        }));

        router.add_route(Method::GET, "/api/orders", Box::new(|ctx, services| {
            Box::pin(handlers::orders_handler(ctx, services))
        }));

        router.add_route(Method::OPTIONS, "/*", Box::new(|ctx, services| {
            Box::pin(handlers::cors_preflight_handler(ctx, services))
        }));

        router
    }

    /// Create TLS configuration with ALPN protocol negotiation
    async fn create_tls_config() -> AppResult<TlsServerConfig> {
        // Generate self-signed certificate
        let cert_chain_and_key = Self::generate_self_signed_cert()?;

        // Build TLS configuration
        let mut tls_config = TlsServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                cert_chain_and_key.cert_chain,
                cert_chain_and_key.private_key,
            )
            .map_err(|e| AppError::Internal(format!("Failed to create TLS config: {}", e)))?;

        // Set ALPN protocols with fallback support
        // h3 = HTTP/3, h2 = HTTP/2, http/1.1 = HTTP/1.1
        tls_config.alpn_protocols = vec![
            b"h3".to_vec(),           // HTTP/3 (primary)
            b"h2".to_vec(),           // HTTP/2 (fallback)
            b"http/1.1".to_vec(),     // HTTP/1.1 (fallback)
        ];

        Ok(tls_config)
    }

    /// Generate self-signed certificate for development
    fn generate_self_signed_cert() -> AppResult<CertificateChain> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .map_err(|e| AppError::Internal(format!("Failed to generate certificate: {}", e)))?;

        let private_key = PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());
        let cert_chain = vec![cert.cert.der().clone()];

        Ok(CertificateChain {
            cert_chain,
            private_key,
        })
    }

    /// Start the HTTP/3 server
    pub async fn start(self) -> AppResult<()> {
        info!("Starting HTTP/3 server on {}", self.config.server_address());

        // Main server loop
        while let Some(conn) = self.endpoint.accept().await {
            let conn = conn.await
                .map_err(|e| AppError::Internal(format!("Connection failed: {}", e)))?;

            // Clone router and services for this connection
            let router = Arc::clone(&self.router);
            let services = self.services.clone();

            // Spawn connection handler
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(conn, router, services).await {
                    error!("Connection handling failed: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Handle individual QUIC connection
    async fn handle_connection(
        conn: QuinnConnection,
        router: Arc<Router>,
        services: AppServices,
    ) -> AppResult<()> {
        // Create H3 connection from Quinn connection
        let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create H3 connection: {}", e)))?;

        debug!("New HTTP/3 connection established");

        // Handle requests on this connection
        loop {
            match h3_conn.accept().await {
                Ok(Some(req_resolver)) => {
                    // Clone router and services for this request
                    let router_clone = Arc::clone(&router);
                    let services_clone = services.clone();

                    // Spawn request handler
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_request(
                            req_resolver,
                            router_clone,
                            services_clone,
                        ).await {
                            error!("Request handling failed: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    debug!("Connection closed by client");
                    break;
                }
                Err(e) => {
                    warn!("Error accepting request: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle individual HTTP request
    async fn handle_request(
        req_resolver: h3::server::RequestResolver<h3_quinn::Connection, bytes::Bytes>,
        router: Arc<Router>,
        services: AppServices,
    ) -> AppResult<()> {
        // Resolve the request
        let (req, mut stream) = req_resolver.resolve_request().await
            .map_err(|e| AppError::Internal(format!("Failed to resolve request: {}", e)))?;

        info!(
            "Request: {} {} (HTTP/{:?})",
            req.method(),
            req.uri().path(),
            req.version()
        );

        // Create request context
        let ctx = RequestContext::from_request(&req, None);

        // Route the request using the new router
        let response_builder = match router.route(ctx, services).await {
            Ok(builder) => builder,
            Err(e) => {
                error!("Routing error: {}", e);
                // Create error response
                let error_response = serde_json::json!({
                    "error": "Internal Server Error",
                    "message": e.to_string(),
                    "timestamp": chrono::Utc::now()
                });
                ResponseBuilder::new()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .json(&error_response)
            }
        };

        // Build the response
        let (response_body, content_type, status_code) = response_builder.build();

        // Build HTTP response
        let response = Response::builder()
            .status(status_code)
            .header("content-type", content_type)
            .header("server", "hotel-booking-http3/1.0")
            .header("access-control-allow-origin", "*")
            .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS")
            .header("access-control-allow-headers", "Content-Type, Authorization")
            .body(())
            .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))?;

        // Send response
        stream.send_response(response).await
            .map_err(|e| AppError::Internal(format!("Failed to send response: {}", e)))?;

        stream.send_data(Bytes::from(response_body)).await
            .map_err(|e| AppError::Internal(format!("Failed to send data: {}", e)))?;

        stream.finish().await
            .map_err(|e| AppError::Internal(format!("Failed to finish stream: {}", e)))?;

        Ok(())
    }


}

/// Certificate chain structure
struct CertificateChain {
    cert_chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    private_key: PrivateKeyDer<'static>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::database::DatabaseService;
    use crate::currency::CurrencyHelper;

    #[tokio::test]
    async fn test_tls_config_creation() {
        // Install crypto provider for the test
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        
        let result = Http3Server::create_tls_config().await;
        assert!(result.is_ok(), "TLS config creation should succeed");
        
        let tls_config = result.unwrap();
        assert!(!tls_config.alpn_protocols.is_empty(), "ALPN protocols should be configured");
        assert!(tls_config.alpn_protocols.contains(&b"h3".to_vec()), "Should support HTTP/3");
    }

    #[test]
    fn test_certificate_generation() {
        let result = Http3Server::generate_self_signed_cert();
        assert!(result.is_ok(), "Certificate generation should succeed");
        
        let cert_chain = result.unwrap();
        assert!(!cert_chain.cert_chain.is_empty(), "Certificate chain should not be empty");
    }

    #[tokio::test]
    async fn test_server_creation() {
        // Skip if no database connection available
        let config = match AppConfig::from_env() {
            Ok(config) => config,
            Err(_) => {
                println!("Skipping server creation test: No config available");
                return;
            }
        };

        let database = match DatabaseService::new(&config.database_url).await {
            Ok(db) => Arc::new(db),
            Err(_) => {
                println!("Skipping server creation test: No database connection");
                return;
            }
        };

        let currency_helper = match CurrencyHelper::from_env() {
            Ok(helper) => Arc::new(helper),
            Err(_) => {
                println!("Skipping server creation test: No currency helper");
                return;
            }
        };

        let result = Http3Server::new(config, database, currency_helper).await;
        assert!(result.is_ok(), "Server creation should succeed");
    }
}