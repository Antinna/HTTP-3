use anyhow::Result;
use std::sync::Arc;
use tracing::info;

mod auth;
mod config;
mod currency;
mod database;
mod error;
mod firebase;
mod handlers;
mod logging;
mod models;
mod routing;
mod server;

use config::AppConfig;
use currency::CurrencyHelper;
use database::DatabaseService;
use firebase::FirebaseAuth;
use server::Http3Server;

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
    database
        .migrate()
        .await
        .expect("Failed to run database migrations");
    info!("Database migrations completed successfully");

    // Perform database health check
    match database.health_check().await {
        Ok(health) => {
            if health.is_healthy {
                info!(
                    "Database health check passed - Response time: {}ms",
                    health.response_time_ms
                );
            } else {
                panic!("Database health check failed: {:?}", health.error_message);
            }
        }
        Err(e) => {
            panic!("Database health check error: {}", e);
        }
    }

    // Initialize currency helper
    let currency_helper = CurrencyHelper::from_env().expect("Failed to initialize currency helper");
    info!(
        "Currency helper initialized successfully with {} ({})",
        currency_helper.name(),
        currency_helper.symbol()
    );

    // Initialize Firebase authentication service
    let firebase_auth =
        FirebaseAuth::from_env().expect("Failed to initialize Firebase authentication service");
    info!(
        "Firebase authentication service initialized successfully for project: {}",
        firebase_auth.config().project_id
    );

    // Create HTTP/3 server with services
    let server = Http3Server::new(config, Arc::new(database), Arc::new(currency_helper))
        .await
        .expect("Failed to create HTTP/3 server");

    // Start the server
    server.start().await.expect("Server failed to start");

    Ok(())
}
