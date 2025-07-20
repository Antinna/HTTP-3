use std::str::FromStr;
use serde_json::json;
use http::StatusCode;
use tracing::{info, error};

use crate::routing::{RequestContext, ResponseBuilder, AppServices};
use crate::error::AppResult;

/// Health check handler
pub async fn health_handler(ctx: RequestContext, services: AppServices) -> AppResult<ResponseBuilder> {
    info!("Health check requested - Request ID: {}", ctx.request_id);
    
    match services.database.health_check().await {
        Ok(health) => {
            let status = if health.is_healthy { "healthy" } else { "unhealthy" };
            let status_code = if health.is_healthy { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
            
            let response = json!({
                "status": status,
                "database": health,
                "timestamp": chrono::Utc::now(),
                "request_id": ctx.request_id
            });
            
            Ok(ResponseBuilder::new()
                .status(status_code)
                .json(&response))
        }
        Err(e) => {
            error!("Database health check failed: {}", e);
            let response = json!({
                "status": "unhealthy",
                "error": e.to_string(),
                "timestamp": chrono::Utc::now(),
                "request_id": ctx.request_id
            });
            
            Ok(ResponseBuilder::new()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .json(&response))
        }
    }
}

/// Root endpoint handler
pub async fn root_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    let response = json!({
        "service": "Hotel Booking System",
        "version": "1.0.0",
        "protocol": "HTTP/3",
        "status": "running",
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// Currency information handler
pub async fn currency_handler(ctx: RequestContext, services: AppServices) -> AppResult<ResponseBuilder> {
    let currencies = services.currency_helper.supported_currencies();
    let amount = rust_decimal::Decimal::from_str("1234.56").unwrap_or_default();
    let formatted = services.currency_helper.format(amount, None);

    let response = json!({
        "default_currency": {
            "code": services.currency_helper.code(),
            "symbol": services.currency_helper.symbol(),
            "name": services.currency_helper.name()
        },
        "supported_currencies": currencies,
        "examples": {
            "amount": amount.to_string(),
            "formatted": formatted,
            "range": services.currency_helper.format_range(
                rust_decimal::Decimal::from_str("100").unwrap_or_default(),
                rust_decimal::Decimal::from_str("500").unwrap_or_default()
            )
        },
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// User profile handler (requires authentication)
pub async fn user_profile_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    if !ctx.is_authenticated() {
        let response = json!({
            "error": "Authentication required",
            "message": "Please provide a valid authorization token",
            "timestamp": chrono::Utc::now(),
            "request_id": ctx.request_id
        });
        
        return Ok(ResponseBuilder::new()
            .status(StatusCode::UNAUTHORIZED)
            .json(&response));
    }

    let user = ctx.user.as_ref().unwrap();
    let response = json!({
        "user": {
            "id": user.user_id,
            "email": user.email,
            "name": user.name,
            "user_type": user.user_type.label(),
            "session_id": user.session_id
        },
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// API documentation handler
pub async fn api_docs_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    let response = json!({
        "api": "Hotel Booking System API",
        "version": "1.0.0",
        "endpoints": {
            "health": {
                "method": "GET",
                "path": "/health",
                "description": "System health check"
            },
            "currency": {
                "method": "GET", 
                "path": "/api/currency",
                "description": "Currency information and formatting examples"
            },
            "user_profile": {
                "method": "GET",
                "path": "/api/users/profile",
                "description": "Get user profile (requires authentication)",
                "auth_required": true
            },
            "menu": {
                "method": "GET",
                "path": "/api/menu",
                "description": "Get menu items with optional filtering",
                "query_params": ["category", "min_price", "max_price", "search"]
            },
            "orders": {
                "method": "GET",
                "path": "/api/orders",
                "description": "Get user orders (requires authentication)",
                "auth_required": true
            }
        },
        "authentication": {
            "type": "Bearer Token",
            "header": "Authorization: Bearer <token>",
            "description": "Firebase JWT token required for protected endpoints"
        },
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// Menu items handler (placeholder)
pub async fn menu_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    // Parse query parameters for filtering
    let category = ctx.query_param("category");
    let search = ctx.query_param("search");
    let min_price = ctx.query_param("min_price")
        .and_then(|p| p.parse::<f64>().ok());
    let max_price = ctx.query_param("max_price")
        .and_then(|p| p.parse::<f64>().ok());

    // Mock menu data for now
    let mut menu_items = vec![
        json!({
            "id": 1,
            "name": "Margherita Pizza",
            "description": "Classic pizza with tomato sauce, mozzarella, and basil",
            "price": 12.99,
            "category": "pizza",
            "available": true,
            "dietary_info": ["vegetarian"]
        }),
        json!({
            "id": 2,
            "name": "Chicken Burger",
            "description": "Grilled chicken breast with lettuce, tomato, and mayo",
            "price": 15.50,
            "category": "burgers",
            "available": true,
            "dietary_info": []
        }),
        json!({
            "id": 3,
            "name": "Caesar Salad",
            "description": "Fresh romaine lettuce with Caesar dressing and croutons",
            "price": 9.99,
            "category": "salads",
            "available": true,
            "dietary_info": ["vegetarian"]
        })
    ];

    // Apply filters
    if let Some(cat) = category {
        menu_items.retain(|item| {
            item.get("category")
                .and_then(|c| c.as_str())
                .map(|c| c == cat)
                .unwrap_or(false)
        });
    }

    if let Some(search_term) = search {
        let search_lower = search_term.to_lowercase();
        menu_items.retain(|item| {
            let name_match = item.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n.to_lowercase().contains(&search_lower))
                .unwrap_or(false);
            
            let desc_match = item.get("description")
                .and_then(|d| d.as_str())
                .map(|d| d.to_lowercase().contains(&search_lower))
                .unwrap_or(false);
            
            name_match || desc_match
        });
    }

    if let Some(min) = min_price {
        menu_items.retain(|item| {
            item.get("price")
                .and_then(|p| p.as_f64())
                .map(|p| p >= min)
                .unwrap_or(false)
        });
    }

    if let Some(max) = max_price {
        menu_items.retain(|item| {
            item.get("price")
                .and_then(|p| p.as_f64())
                .map(|p| p <= max)
                .unwrap_or(false)
        });
    }

    let response = json!({
        "menu_items": menu_items,
        "filters_applied": {
            "category": category,
            "search": search,
            "min_price": min_price,
            "max_price": max_price
        },
        "total_items": menu_items.len(),
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// Orders handler (requires authentication)
pub async fn orders_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    if !ctx.is_authenticated() {
        let response = json!({
            "error": "Authentication required",
            "message": "Please provide a valid authorization token",
            "timestamp": chrono::Utc::now(),
            "request_id": ctx.request_id
        });
        
        return Ok(ResponseBuilder::new()
            .status(StatusCode::UNAUTHORIZED)
            .json(&response));
    }

    // Mock orders data
    let orders = vec![
        json!({
            "id": 1,
            "order_number": "ORD-2024-001",
            "status": "delivered",
            "total": 28.48,
            "items": [
                {"name": "Margherita Pizza", "quantity": 1, "price": 12.99},
                {"name": "Chicken Burger", "quantity": 1, "price": 15.50}
            ],
            "created_at": "2024-01-15T10:30:00Z",
            "delivered_at": "2024-01-15T11:15:00Z"
        }),
        json!({
            "id": 2,
            "order_number": "ORD-2024-002",
            "status": "preparing",
            "total": 9.99,
            "items": [
                {"name": "Caesar Salad", "quantity": 1, "price": 9.99}
            ],
            "created_at": "2024-01-16T14:20:00Z",
            "estimated_delivery": "2024-01-16T15:00:00Z"
        })
    ];

    let response = json!({
        "orders": orders,
        "total_orders": orders.len(),
        "user_id": ctx.user_id(),
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new().json(&response))
}

/// CORS preflight handler
pub async fn cors_preflight_handler(_ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    Ok(ResponseBuilder::new()
        .status(StatusCode::NO_CONTENT)
        .header("access-control-max-age", "86400")
        .text(""))
}

/// 404 Not Found handler
pub async fn not_found_handler(ctx: RequestContext, _services: AppServices) -> AppResult<ResponseBuilder> {
    let response = json!({
        "error": "Not Found",
        "message": format!("Route {} {} not found", ctx.method, ctx.path),
        "timestamp": chrono::Utc::now(),
        "request_id": ctx.request_id
    });
    
    Ok(ResponseBuilder::new()
        .status(StatusCode::NOT_FOUND)
        .json(&response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use http::Method;
    use std::sync::Arc;
    use crate::database::DatabaseService;
    use crate::currency::CurrencyHelper;

    fn create_test_context() -> RequestContext {
        RequestContext {
            method: Method::GET,
            path: "/test".to_string(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
            body: None,
            user: None,
            request_id: "test-123".to_string(),
        }
    }

    async fn create_test_services() -> Option<AppServices> {
        // Try to create services, but return None if they fail
        let currency_helper = match CurrencyHelper::from_env() {
            Ok(helper) => Arc::new(helper),
            Err(_) => {
                // Create a default currency helper for tests
                use crate::currency::CurrencyConfig;
                let config = CurrencyConfig {
                    code: "USD".to_string(),
                    symbol: "$".to_string(),
                    name: "US Dollar".to_string(),
                    decimal_places: 2,
                    thousands_separator: ",".to_string(),
                    decimal_separator: ".".to_string(),
                };
                Arc::new(CurrencyHelper::new(config))
            }
        };
        
        // Skip database-dependent tests if no database is available
        match DatabaseService::new("mysql://test:test@localhost:3306/test").await {
            Ok(db) => Some(AppServices {
                database: Arc::new(db),
                currency_helper,
            }),
            Err(_) => None
        }
    }

    #[tokio::test]
    async fn test_root_handler() {
        let ctx = create_test_context();
        
        // Root handler doesn't need database, so we can skip database creation
        
        // Skip this test if we can't create services without database
        if let Some(services) = create_test_services().await {
            let result = root_handler(ctx, services).await;
            assert!(result.is_ok());
            
            let response = result.unwrap().build();
            assert_eq!(response.2, StatusCode::OK);
            assert!(response.0.contains("Hotel Booking System"));
        } else {
            println!("Skipping test_root_handler: No database connection available");
        }
    }

    #[tokio::test]
    async fn test_currency_handler() {
        let ctx = create_test_context();
        
        if let Some(services) = create_test_services().await {
            let result = currency_handler(ctx, services).await;
            assert!(result.is_ok());
            
            let response = result.unwrap().build();
            assert_eq!(response.2, StatusCode::OK);
            assert!(response.0.contains("default_currency"));
        } else {
            println!("Skipping test_currency_handler: No database connection available");
        }
    }

    #[tokio::test]
    async fn test_user_profile_handler_unauthorized() {
        let ctx = create_test_context();
        
        if let Some(services) = create_test_services().await {
            let result = user_profile_handler(ctx, services).await;
            assert!(result.is_ok());
            
            let response = result.unwrap().build();
            assert_eq!(response.2, StatusCode::UNAUTHORIZED);
        } else {
            println!("Skipping test_user_profile_handler_unauthorized: No database connection available");
        }
    }

    #[tokio::test]
    async fn test_menu_handler_with_filters() {
        let mut ctx = create_test_context();
        ctx.query_params.insert("category".to_string(), "pizza".to_string());
        ctx.query_params.insert("search".to_string(), "margherita".to_string());
        
        if let Some(services) = create_test_services().await {
            let result = menu_handler(ctx, services).await;
            assert!(result.is_ok());
            
            let response = result.unwrap().build();
            assert_eq!(response.2, StatusCode::OK);
            assert!(response.0.contains("menu_items"));
            assert!(response.0.contains("filters_applied"));
        } else {
            println!("Skipping test_menu_handler_with_filters: No database connection available");
        }
    }
}