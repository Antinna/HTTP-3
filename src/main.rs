use anyhow::Result;
use bytes::Bytes;
use quinn::{Endpoint, ServerConfig};
use rustls::{ServerConfig as TlsServerConfig, pki_types::PrivateKeyDer};
use std::sync::Arc;
use tracing::info;

mod config;
mod currency;
mod database;
mod error;
mod firebase;
mod logging;
mod models;

use config::AppConfig;
use currency::CurrencyHelper;
use database::DatabaseService;
use firebase::FirebaseAuth;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging system
    logging::init_logging().expect("Failed to initialize logging");

    // Load application configuration
    let config = AppConfig::from_env().expect("Failed to load configuration");
    info!("Application configuration loaded successfully");
    info!("Server will bind to: {}", config.server_address());

    // Initialize database service
    let database = DatabaseService::new(&config.database_url)
        .await
        .expect("Failed to initialize database service");
    info!("Database service initialized successfully");

    // Run database migrations
    database.migrate()
        .await
        .expect("Failed to run database migrations");
    info!("Database migrations completed successfully");

    // Perform database health check
    match database.health_check().await {
        Ok(health) => {
            if health.is_healthy {
                info!("Database health check passed - Response time: {}ms", health.response_time_ms);
            } else {
                panic!("Database health check failed: {:?}", health.error_message);
            }
        }
        Err(e) => {
            panic!("Database health check error: {}", e);
        }
    }

    // Clone database service for use in request handlers
    let db_service = Arc::new(database);

    // Install the default crypto provider for rustls.
    // This is necessary for rustls to function correctly, especially with AWS-LC-RS.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap(); // Panics if installation fails, which is acceptable for a startup step.

    // Generate a self-signed certificate and private key for the server.
    let cert_chain_and_key = generate_self_signed_cert()?;

    // Build the TLS server configuration using the generated certificate and key.
    // TlsServerConfig::builder() is used to construct the rustls server configuration.
    let mut tls_config = TlsServerConfig::builder()
        .with_no_client_auth() // No client authentication required for this server
        .with_single_cert(
            cert_chain_and_key.cert_chain, // Corrected field name from `cert.cert_chain` to `cert_chain_and_key.cert_chain`
            cert_chain_and_key.private_key,
        )?;

    // Set the ALPN (Application-Layer Protocol Negotiation) protocols.
    // "h3" is the ALPN for HTTP/3.
    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    // Create the Quinn server configuration from the rustls TLS configuration.
    // Quinn requires a `quinn::crypto::rustls::QuicServerConfig` for its crypto setup.
    let server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)?,
    ));

    // Bind the Quinn endpoint to the specified address.
    let bind_addr = config.server_address().parse()?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    info!("HTTP/3 server listening on {}", config.server_address());

    // Main server loop: accept incoming connections.
    while let Some(conn) = endpoint.accept().await {
        // Await the connection to be established.
        let conn = conn.await?;

        // Clone database service for this connection
        let db_service_clone = Arc::clone(&db_service);

        // Spawn a new task to handle each incoming QUIC connection.
        tokio::spawn(async move {
            // Create an h3 server connection from the Quinn connection.
            let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
                .await
                .unwrap(); // Panics if h3 connection setup fails

            // Loop to accept and handle HTTP/3 requests on this connection.
            loop {
                match h3_conn.accept().await {
                    // If a request resolver is received, spawn a task to handle the request.
                    Ok(Some(req_resolver)) => {
                        let db_service_task = Arc::clone(&db_service_clone);
                        tokio::spawn(async move {
                            // Resolve the request to get the HTTP request and the stream.
                            let (req, mut stream) = req_resolver.resolve_request().await.unwrap(); // Panics on error

                            info!(
                                "Got request for path: {}, protocol: {:?}",
                                req.uri().path(),
                                req.version()
                            );

                            // Determine the response body and content type based on the request path.
                            let (response_body, content_type) = match req.uri().path() {
                                "/" => ("hello from http3".to_string(), "text/plain"),
                                "/test" => ("hello from http3 test endpoint".to_string(), "text/plain"),
                                "/health" => {
                                    // Perform database health check
                                    match db_service_task.health_check().await {
                                        Ok(health) => {
                                            let health_json = serde_json::to_string(&health)
                                                .unwrap_or_else(|_| r#"{"error":"Failed to serialize health check"}"#.to_string());
                                            (health_json, "application/json")
                                        }
                                        Err(e) => {
                                            let error_json = format!(r#"{{"error":"Database health check failed","message":"{}"}}"#, e);
                                            (error_json, "application/json")
                                        }
                                    }
                                },
                                "/db/health" => {
                                    // Detailed database health endpoint
                                    match db_service_task.health_check().await {
                                        Ok(health) => {
                                            let detailed_health = serde_json::json!({
                                                "database": health,
                                                "timestamp": chrono::Utc::now(),
                                                "service": "hotel-restaurant-system"
                                            });
                                            (detailed_health.to_string(), "application/json")
                                        }
                                        Err(e) => {
                                            let error_response = serde_json::json!({
                                                "error": "Database health check failed",
                                                "message": e.to_string(),
                                                "timestamp": chrono::Utc::now(),
                                                "service": "hotel-restaurant-system"
                                            });
                                            (error_response.to_string(), "application/json")
                                        }
                                    }
                                },
                                _ => ("hello from http3 - unknown endpoint".to_string(), "text/plain"),
                            };

                            // Build the HTTP response.
                            let response = http::Response::builder()
                                .status(200)
                                .header("content-type", content_type)
                                .header("server", "hotel-restaurant-http3")
                                .body(()) // Body is empty for the header part
                                .unwrap(); // Panics if response building fails

                            // Send the response headers.
                            stream.send_response(response).await.unwrap(); // Panics on error
                            // Send the response data (body).
                            stream.send_data(Bytes::from(response_body)).await.unwrap(); // Panics on error
                            // Finish the stream, indicating no more data will be sent.
                            stream.finish().await.unwrap(); // Panics on error
                        });
                    }
                    // If no more requests are available on this connection, break the loop.
                    Ok(None) => break,
                    // If an error occurs while accepting a request, break the loop.
                    Err(_) => break,
                }
            }
        });
    }
    Ok(()) // Indicate successful execution of the main function
}

// Struct to hold the certificate chain and private key.
struct CertificateChain {
    cert_chain: Vec<rustls::pki_types::CertificateDer<'static>>, // Corrected field name to `cert_chain`
    private_key: PrivateKeyDer<'static>,
}

// Function to generate a simple self-signed certificate for localhost.
fn generate_self_signed_cert() -> Result<CertificateChain> {
    // Generate a simple self-signed certificate for "localhost".
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;

    // Extract the private key in PKCS8 DER format.
    // `cert.signing_key.serialize_der()` is used to get the DER-encoded private key.
    let private_key = PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());

    // Extract the certificate chain in DER format.
    // `cert.cert.der().clone()` is used to get the DER-encoded certificate.
    let cert_chain = vec![cert.cert.der().clone()];

    // Return the CertificateChain struct.
    Ok(CertificateChain {
        cert_chain, // Uses the corrected field name
        private_key,
    })
}
