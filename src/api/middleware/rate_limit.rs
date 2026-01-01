//! Rate limiting middleware.
//!
//! Provides rate limiting for API endpoints using the governor crate.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use tower::Layer;

/// Rate limiter state
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub type RateLimiterState = Arc<
    RateLimiter<
        governor::state::direct::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
>;

/// Shared rate limiter (alias for compatibility)
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub type SharedRateLimiter = RateLimiterState;

/// Create a rate limiter with default settings (100 requests per minute)
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub fn create_rate_limiter() -> RateLimiterState {
    let quota = Quota::per_minute(NonZeroU32::new(100).unwrap());
    Arc::new(RateLimiter::direct(quota))
}

/// Create a rate limiter with custom quota
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub fn create_rate_limiter_with_quota(requests_per_minute: u32) -> RateLimiterState {
    let quota = Quota::per_minute(
        NonZeroU32::new(requests_per_minute).unwrap_or(NonZeroU32::new(100).unwrap()),
    );
    Arc::new(RateLimiter::direct(quota))
}

/// Rate limiting middleware
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiterState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match limiter.check() {
        Ok(_) => Ok(next.run(request).await),
        Err(_) => {
            tracing::warn!("Rate limit exceeded for request: {}", request.uri());
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

/// Create a rate limit layer for Axum
/// Note: This requires the router to have RateLimiterState in its state
#[allow(dead_code)] // Reserved for future rate limiting implementation
pub fn create_rate_limit_layer() -> impl Layer<axum::Router> + Clone {
    // Return identity layer - rate limiting should be applied via middleware in main.rs
    // This is a placeholder for future implementation
    tower::layer::util::Identity::new()
}
