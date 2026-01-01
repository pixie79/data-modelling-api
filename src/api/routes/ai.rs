//! AI service routes.

use axum::{Router, extract::State, http::StatusCode, response::Json, routing::post};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::tables::AppState;
use crate::services::ai_service::{AIErrorResolution, AIService};
use tracing::warn;

#[derive(Deserialize, ToSchema)]
struct ResolveErrorsRequest {
    sql_content: Option<String>,
    yaml_content: Option<String>,
    error_message: Option<String>,
    errors: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
struct ResolveErrorsResponse {
    resolutions: Vec<AIErrorResolution>,
}

/// Create the AI service router
pub fn ai_router() -> Router<AppState> {
    Router::new().route("/resolve-errors", post(resolve_errors))
}

/// POST /ai/resolve-errors - Use AI to resolve import errors
#[utoipa::path(
    post,
    path = "/ai/resolve-errors",
    tag = "AI",
    request_body = ResolveErrorsRequest,
    responses(
        (status = 200, description = "AI resolutions generated successfully", body = ResolveErrorsResponse),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
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
