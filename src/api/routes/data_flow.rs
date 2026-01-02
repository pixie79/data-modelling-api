//! Data-flow diagram routes.
//!
//! Provides domain-scoped CRUD endpoints for data-flow diagrams.

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{delete, get, put},
};
use serde::Deserialize;
use serde_json::Value;
use tracing::{error, info};
use uuid::Uuid;

use super::app_state::AppState;
use super::auth_context::AuthContext;
use super::workspace::DomainPath;
use crate::models::data_flow_diagram::{
    CreateDataFlowDiagramRequest, DataFlowDiagram, UpdateDataFlowDiagramRequest,
};

/// Create the data-flow diagram router
pub fn data_flow_router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(list_data_flow_diagrams).post(create_data_flow_diagram),
        )
        .route("/{diagram_id}", get(get_data_flow_diagram))
        .route("/{diagram_id}", put(update_data_flow_diagram))
        .route("/{diagram_id}", delete(delete_data_flow_diagram))
}

/// Path parameters for data-flow diagram routes
#[derive(Deserialize)]
struct DataFlowDiagramPath {
    diagram_id: String,
}

/// GET /workspace/domains/{domain}/data-flow-diagrams - List all data-flow diagrams for a domain
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/data-flow-diagrams",
    tag = "Data Flow Diagrams",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Data-flow diagrams retrieved successfully", body = Vec<DataFlowDiagram>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Domain not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn list_data_flow_diagrams(
    State(state): State<AppState>,
    Path(domain_path): Path<DomainPath>,
    headers: HeaderMap,
    _auth: AuthContext,
) -> Result<Json<Vec<DataFlowDiagram>>, StatusCode> {
    // Ensure domain is loaded
    let ctx = super::workspace::ensure_domain_loaded(&state, &headers, &domain_path.domain).await?;

    // Try PostgreSQL storage first
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_data_flow_diagrams(ctx.domain_info.id).await {
            Ok(diagrams) => {
                info!(
                    "Retrieved {} data-flow diagrams from PostgreSQL",
                    diagrams.len()
                );
                return Ok(Json(diagrams));
            }
            Err(e) => {
                error!("Failed to get data-flow diagrams from PostgreSQL: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback - return empty for now
    // Actual file-based implementation would read from data-flow.yaml
    Ok(Json(vec![]))
}

/// POST /workspace/domains/{domain}/data-flow-diagrams - Create a new data-flow diagram
#[utoipa::path(
    post,
    path = "/workspace/domains/{domain}/data-flow-diagrams",
    tag = "Data Flow Diagrams",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    request_body = CreateDataFlowDiagramRequest,
    responses(
        (status = 200, description = "Data-flow diagram created successfully", body = DataFlowDiagram),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Domain not found"),
        (status = 409, description = "Conflict - diagram with this name already exists"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn create_data_flow_diagram(
    State(state): State<AppState>,
    Path(domain_path): Path<DomainPath>,
    headers: HeaderMap,
    _auth: AuthContext,
    Json(request): Json<CreateDataFlowDiagramRequest>,
) -> Result<Json<DataFlowDiagram>, StatusCode> {
    // Ensure domain is loaded
    let ctx = super::workspace::ensure_domain_loaded(&state, &headers, &domain_path.domain).await?;

    // Validate request
    if request.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Try PostgreSQL storage first
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .create_data_flow_diagram(
                ctx.domain_info.id,
                request.name,
                request.description,
                request.diagram_data,
                &ctx.user_context,
            )
            .await
        {
            Ok(diagram) => {
                info!("Created data-flow diagram '{}' in PostgreSQL", diagram.name);
                return Ok(Json(diagram));
            }
            Err(crate::storage::StorageError::Other(msg)) if msg.contains("already exists") => {
                return Err(StatusCode::CONFLICT);
            }
            Err(e) => {
                error!("Failed to create data-flow diagram in PostgreSQL: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback - not implemented yet
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// GET /workspace/domains/{domain}/data-flow-diagrams/{diagram_id} - Get a data-flow diagram by ID
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/data-flow-diagrams/{diagram_id}",
    tag = "Data Flow Diagrams",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("diagram_id" = String, Path, description = "Diagram UUID")
    ),
    responses(
        (status = 200, description = "Data-flow diagram retrieved successfully", body = DataFlowDiagram),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Diagram not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn get_data_flow_diagram(
    State(state): State<AppState>,
    Path((domain_path, diagram_path)): Path<(DomainPath, DataFlowDiagramPath)>,
    headers: HeaderMap,
    _auth: AuthContext,
) -> Result<Json<DataFlowDiagram>, StatusCode> {
    // Ensure domain is loaded
    let ctx = super::workspace::ensure_domain_loaded(&state, &headers, &domain_path.domain).await?;

    let diagram_id =
        Uuid::parse_str(&diagram_path.diagram_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try PostgreSQL storage first
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .get_data_flow_diagram(ctx.domain_info.id, diagram_id)
            .await
        {
            Ok(Some(diagram)) => {
                return Ok(Json(diagram));
            }
            Ok(None) => {
                return Err(StatusCode::NOT_FOUND);
            }
            Err(e) => {
                error!("Failed to get data-flow diagram from PostgreSQL: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback - not implemented yet
    Err(StatusCode::NOT_FOUND)
}

/// PUT /workspace/domains/{domain}/data-flow-diagrams/{diagram_id} - Update a data-flow diagram
#[utoipa::path(
    put,
    path = "/workspace/domains/{domain}/data-flow-diagrams/{diagram_id}",
    tag = "Data Flow Diagrams",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("diagram_id" = String, Path, description = "Diagram UUID")
    ),
    request_body = UpdateDataFlowDiagramRequest,
    responses(
        (status = 200, description = "Data-flow diagram updated successfully", body = DataFlowDiagram),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Diagram not found"),
        (status = 409, description = "Conflict - version mismatch"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn update_data_flow_diagram(
    State(state): State<AppState>,
    Path((domain_path, diagram_path)): Path<(DomainPath, DataFlowDiagramPath)>,
    headers: HeaderMap,
    _auth: AuthContext,
    Json(request): Json<UpdateDataFlowDiagramRequest>,
) -> Result<Json<DataFlowDiagram>, StatusCode> {
    // Ensure domain is loaded
    let ctx = super::workspace::ensure_domain_loaded(&state, &headers, &domain_path.domain).await?;

    let diagram_id =
        Uuid::parse_str(&diagram_path.diagram_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try PostgreSQL storage first
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .update_data_flow_diagram(
                diagram_id,
                ctx.domain_info.id,
                request.name,
                request.description,
                request.diagram_data,
                request.expected_version,
                &ctx.user_context,
            )
            .await
        {
            Ok(diagram) => {
                info!("Updated data-flow diagram '{}' in PostgreSQL", diagram.name);
                return Ok(Json(diagram));
            }
            Err(crate::storage::StorageError::NotFound { .. }) => {
                return Err(StatusCode::NOT_FOUND);
            }
            Err(crate::storage::StorageError::VersionConflict { .. }) => {
                return Err(StatusCode::CONFLICT);
            }
            Err(e) => {
                error!("Failed to update data-flow diagram in PostgreSQL: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback - not implemented yet
    Err(StatusCode::NOT_IMPLEMENTED)
}

/// DELETE /workspace/domains/{domain}/data-flow-diagrams/{diagram_id} - Delete a data-flow diagram
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}/data-flow-diagrams/{diagram_id}",
    tag = "Data Flow Diagrams",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("diagram_id" = String, Path, description = "Diagram UUID")
    ),
    responses(
        (status = 200, description = "Data-flow diagram deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Diagram not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn delete_data_flow_diagram(
    State(state): State<AppState>,
    Path((domain_path, diagram_path)): Path<(DomainPath, DataFlowDiagramPath)>,
    headers: HeaderMap,
    _auth: AuthContext,
) -> Result<Json<Value>, StatusCode> {
    // Ensure domain is loaded
    let ctx = super::workspace::ensure_domain_loaded(&state, &headers, &domain_path.domain).await?;

    let diagram_id =
        Uuid::parse_str(&diagram_path.diagram_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try PostgreSQL storage first
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .delete_data_flow_diagram(ctx.domain_info.id, diagram_id, &ctx.user_context)
            .await
        {
            Ok(()) => {
                info!("Deleted data-flow diagram {} from PostgreSQL", diagram_id);
                return Ok(Json(
                    serde_json::json!({"message": "Data-flow diagram deleted successfully"}),
                ));
            }
            Err(crate::storage::StorageError::NotFound { .. }) => {
                return Err(StatusCode::NOT_FOUND);
            }
            Err(e) => {
                error!("Failed to delete data-flow diagram from PostgreSQL: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback - not implemented yet
    Err(StatusCode::NOT_IMPLEMENTED)
}
