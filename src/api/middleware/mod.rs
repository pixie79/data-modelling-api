// Middleware module - contains observability and other middleware

pub mod cors;
pub mod observability;
pub mod rate_limit;

// Re-export for convenience
#[allow(unused_imports)]
pub use cors::{create_cors_layer, create_cors_layer_from_env, create_custom_cors_layer};
// Rate limit exports are kept for potential future use
#[allow(unused_imports)]
pub use rate_limit::{SharedRateLimiter, create_rate_limit_layer, rate_limit_middleware};
