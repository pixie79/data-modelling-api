// Middleware module - contains observability and other middleware

pub mod cors;
pub mod observability;

// Re-export for convenience
#[allow(unused_imports)]
pub use cors::{create_cors_layer, create_custom_cors_layer};
