mod graph;
mod odcl;
mod sql_parser;
mod yaml_processor;

// API module for Rust backend
pub mod api;

// Re-export api modules at crate root for library tests (so routes can use crate::services, crate::models)
pub use api::middleware;
pub use api::models;
pub use api::routes;
pub use api::services;
pub use api::storage;

// DrawIO module for DrawIO XML handling
pub mod drawio;

// Export module for format exporters
pub mod export;

// Unused functions kept for potential future use
// These are intentionally unused but kept for API compatibility
#[allow(dead_code, unused_imports)]
use crate::graph::{detect_cycles, find_cycles, would_create_cycle};
#[allow(dead_code, unused_imports)]
use crate::yaml_processor::validate_odcl;

/// Simple function for testing module initialization
pub fn hello_modelling() -> &'static str {
    "Modelling Rust module initialized"
}
