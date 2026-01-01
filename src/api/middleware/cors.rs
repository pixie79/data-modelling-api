//! CORS middleware configuration.

use tower_http::cors::CorsLayer;

/// Create a CORS layer with permissive settings for development.
///
/// This allows all origins, methods, and headers. For production,
/// you should configure more restrictive CORS settings.
#[allow(dead_code)]
pub fn create_cors_layer() -> CorsLayer {
    CorsLayer::permissive()
}

/// Create a CORS layer based on environment configuration.
///
/// - If `CORS_PERMISSIVE=true` (or `APP_ENV=development`) => permissive.
/// - Otherwise requires `CORS_ALLOWED_ORIGINS` (comma-separated list).
///
/// Optional:
/// - `CORS_ALLOW_CREDENTIALS=true`
#[allow(dead_code)]
pub fn create_cors_layer_from_env() -> CorsLayer {
    use axum::http::{HeaderValue, Method, header};
    use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin};

    // Fail-closed by default: treat missing APP_ENV as production.
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
    let cors_permissive = std::env::var("CORS_PERMISSIVE")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    if cors_permissive || app_env.to_lowercase() == "development" {
        return CorsLayer::permissive();
    }

    let allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect();

    // If misconfigured, fail closed (no origins).
    let allow_origin = if origins.is_empty() {
        AllowOrigin::list(Vec::<HeaderValue>::new())
    } else {
        AllowOrigin::list(origins)
    };

    let allow_credentials = std::env::var("CORS_ALLOW_CREDENTIALS")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    let mut layer = CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
        ]));

    if allow_credentials {
        layer = layer.allow_credentials(true);
    }

    layer
}

/// Create a CORS layer with custom settings.
///
/// # Arguments
///
/// * `allowed_origins` - List of allowed origins (e.g., ["http://localhost:3000"])
/// * `allowed_methods` - List of allowed HTTP methods
/// * `allowed_headers` - List of allowed headers
///
/// # Example
///
/// ```rust,no_run
/// use tower_http::cors::{AllowOrigin, AllowMethods, AllowHeaders};
/// use tower_http::cors::CorsLayer;
/// use data_modelling_api::middleware::create_custom_cors_layer;
///
/// let cors = create_custom_cors_layer(
///     vec!["http://localhost:3000".to_string()],
///     vec!["GET".to_string(), "POST".to_string()],
///     vec!["Content-Type".to_string()],
/// );
/// ```
#[allow(dead_code)]
pub fn create_custom_cors_layer(
    allowed_origins: Vec<String>,
    allowed_methods: Vec<String>,
    allowed_headers: Vec<String>,
) -> CorsLayer {
    use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin};

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            allowed_origins.iter().map(|s| s.parse().unwrap()),
        ))
        .allow_methods(AllowMethods::list(
            allowed_methods.iter().map(|s| s.parse().unwrap()),
        ))
        .allow_headers(AllowHeaders::list(
            allowed_headers.iter().map(|s| s.parse().unwrap()),
        ))
}
