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
