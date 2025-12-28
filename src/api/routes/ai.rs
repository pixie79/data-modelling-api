//! AI service routes.

use axum::{extract::State, http::StatusCode, response::Json, routing::post, Router};
use serde::{Deserialize, Serialize};

use super::tables::AppState;
use crate::services::ai_service::{AIErrorResolution, AIService};
use tracing::warn;

#[derive(Deserialize)]
struct ResolveErrorsRequest {
    sql_content: Option<String>,
    yaml_content: Option<String>,
    error_message: Option<String>,
    errors: Option<Vec<String>>,
}

#[derive(Serialize)]
struct ResolveErrorsResponse {
    resolutions: Vec<AIErrorResolution>,
}

/// Create the AI service router
pub fn ai_router() -> Router<AppState> {
    Router::new().route("/resolve-errors", post(resolve_errors))
}

/// POST /ai/resolve-errors - Use AI to resolve import errors
async fn resolve_errors(
    State(_state): State<AppState>,
    Json(request): Json<ResolveErrorsRequest>,
) -> Result<Json<ResolveErrorsResponse>, StatusCode> {
    let ai_service = AIService::new();

    let mut resolutions = Vec::new();

    // Resolve SQL errors if provided
    if let (Some(sql_content), Some(error_message)) = (request.sql_content, request.error_message) {
        match ai_service
            .resolve_sql_errors(&sql_content, &error_message)
            .await
        {
            Ok(mut sql_resolutions) => resolutions.append(&mut sql_resolutions),
            Err(e) => {
                warn!("AI service error: {}", e);
            }
        }
    }

    // Resolve ODCL errors if provided
    if let (Some(yaml_content), Some(errors)) = (request.yaml_content, request.errors) {
        match ai_service.resolve_odcl_errors(&yaml_content, &errors).await {
            Ok(mut odcl_resolutions) => resolutions.append(&mut odcl_resolutions),
            Err(e) => {
                warn!("AI service error: {}", e);
            }
        }
    }

    Ok(Json(ResolveErrorsResponse { resolutions }))
}
