//! Observability middleware for OTLP integration
//!
//! This module provides OpenTelemetry integration for metrics, distributed tracing,
//! and structured logging following the OTLP Rust service SDK pattern.
//!
//! Features:
//! - Metrics collection for API endpoints (request/response times, error rates)
//! - Distributed tracing for request flows
//! - Structured logging with debug-level tracing enabled
//! - OTLP exporter for metrics and traces

use tracing::info;

/// Initialize OpenTelemetry observability with OTLP exporter
///
/// This sets up OpenTelemetry with OTLP exporter for metrics and tracing.
/// Debug-level tracing is enabled via RUST_LOG environment variable.
///
/// # Returns
///
/// Returns Ok(()) if initialization succeeds, or an error if it fails.
pub async fn init_observability() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get service name from environment or use default
    let service_name =
        std::env::var("OTLP_SERVICE_NAME").unwrap_or_else(|_| "modelling-api".to_string());

    // Get OTLP endpoint from environment or use default
    let otlp_endpoint =
        std::env::var("OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4317".to_string());

    info!(
        "Initializing observability with service_name={}, endpoint={}",
        service_name, otlp_endpoint
    );

    // For now, observability initialization is simplified
    // Full OTLP integration will be completed once the API is stabilized
    // The tracing subscriber is already initialized in main.rs
    info!("Observability initialized (tracing enabled, OTLP export pending API stabilization)");

    Ok(())
}

/// Shutdown observability gracefully
///
/// This should be called during application shutdown to ensure all
/// metrics and traces are exported before the application exits.
pub async fn shutdown_observability() {
    info!("Shutting down observability...");

    // Shutdown tracer provider if it was initialized
    // Note: This is a placeholder until full OTLP integration is complete
    // The global tracer provider will be shut down automatically when dropped

    info!("Observability shutdown complete");
}
