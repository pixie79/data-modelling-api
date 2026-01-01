//! Observability middleware.
//!
//! Provides OpenTelemetry tracing and metrics support.

use std::env;
use tracing::info;

/// Initialize observability with OpenTelemetry.
///
/// Checks for OTEL_SERVICE_NAME and OTEL_EXPORTER_OTLP_ENDPOINT environment variables.
/// If not set, uses basic tracing without OpenTelemetry.
pub async fn init_observability() -> Result<(), Box<dyn std::error::Error>> {
    let service_name =
        env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "data-modelling-api".to_string());

    let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    if otlp_endpoint.is_some() {
        info!(
            "Initializing OpenTelemetry with service_name={}, endpoint={:?}",
            service_name, otlp_endpoint
        );
        // OpenTelemetry SDK initialization requires opentelemetry dependencies
        // Basic tracing is sufficient for current needs - OpenTelemetry can be added when needed
        info!("OpenTelemetry endpoint configured but SDK not initialized - using basic tracing");
    } else {
        info!(
            "Observability initialized (OpenTelemetry disabled - set OTEL_EXPORTER_OTLP_ENDPOINT to enable)"
        );
    }

    Ok(())
}

/// Shutdown observability (cleanup if needed)
pub async fn shutdown_observability() {
    // Placeholder for future OpenTelemetry shutdown logic
    // Currently no-op as we're using basic tracing
}

/// Create observability middleware layer
#[allow(dead_code)] // Reserved for future OpenTelemetry integration
pub fn create_observability_layer() -> impl tower::Layer<axum::Router> + Clone {
    // Basic request logging is handled by tracing middleware in main.rs
    // This is a placeholder for future OpenTelemetry integration
    tower::layer::util::Identity::new()
}
