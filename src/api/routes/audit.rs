//! Audit trail routes.
//!
//! Provides endpoints for querying audit history of changes to domains, tables, and relationships.

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::app_state::AppState;
use crate::routes::workspace;

/// Create the audit router
pub fn audit_router() -> Router<AppState> {
    Router::new()
        .route("/domains/{domain_id}/history", get(get_domain_history))
        .route("/tables/{table_id}/history", get(get_table_history))
        .route(
            "/relationships/{relationship_id}/history",
            get(get_relationship_history),
        )
        .route(
            "/workspaces/{workspace_id}/history",
            get(get_workspace_history),
        )
        .route("/entries/{entry_id}", get(get_audit_entry))
}

/// Query parameters for audit history
#[derive(Deserialize, IntoParams)]
pub struct AuditQueryParams {
    /// Limit number of results (default: 100)
    #[param(default = 100)]
    limit: Option<i64>,
    /// Offset for pagination (default: 0)
    #[param(default = 0)]
    offset: Option<i64>,
}

/// Audit entry response
#[derive(Serialize, ToSchema)]
pub struct AuditEntryResponse {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: String,
    pub user_id: Uuid,
    pub user_email: String,
    pub created_at: DateTime<Utc>,
}

/// Detailed audit entry response
#[derive(Serialize, ToSchema)]
pub struct AuditEntryDetailResponse {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub domain_id: Option<Uuid>,
    pub action: String,
    pub user_id: Uuid,
    pub user_email: String,
    pub changes: Option<serde_json::Value>,
    pub previous_data: Option<serde_json::Value>,
    pub new_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Audit history response
#[derive(Serialize, ToSchema)]
pub struct AuditHistoryResponse {
    pub entries: Vec<AuditEntryResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// GET /audit/domains/{domain_id}/history
#[utoipa::path(
    get,
    path = "/audit/domains/{domain_id}/history",
    tag = "Audit",
    params(
        ("domain_id" = Uuid, Path, description = "Domain UUID"),
        ("limit" = Option<i64>, Query, description = "Limit number of results"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Audit history retrieved successfully", body = AuditHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Domain not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_domain_history(
    State(state): State<AppState>,
    Path(domain_id): Path<Uuid>,
    Query(params): Query<AuditQueryParams>,
    headers: HeaderMap,
) -> Result<Json<AuditHistoryResponse>, StatusCode> {
    let _user_context = workspace::get_user_context(&state, &headers).await?;

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    if let Some(db) = state.database() {
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM audit_entries
            WHERE entity_type = 'domain' AND entity_id = $1
            "#,
            domain_id
        )
        .fetch_one(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let rows = sqlx::query!(
            r#"
            SELECT id, entity_type, entity_id, action, user_id, user_email, created_at
            FROM audit_entries
            WHERE entity_type = 'domain' AND entity_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            domain_id,
            limit,
            offset
        )
        .fetch_all(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let entries = rows
            .into_iter()
            .map(|r| AuditEntryResponse {
                id: r.id,
                entity_type: r.entity_type,
                entity_id: r.entity_id,
                action: r.action,
                user_id: r.user_id,
                user_email: r.user_email,
                created_at: r.created_at,
            })
            .collect();

        Ok(Json(AuditHistoryResponse {
            entries,
            total: total.unwrap_or(0),
            limit,
            offset,
        }))
    } else {
        // File-based mode - return empty history
        Ok(Json(AuditHistoryResponse {
            entries: Vec::new(),
            total: 0,
            limit,
            offset,
        }))
    }
}

/// GET /audit/tables/{table_id}/history
#[utoipa::path(
    get,
    path = "/audit/tables/{table_id}/history",
    tag = "Audit",
    params(
        ("table_id" = Uuid, Path, description = "Table UUID"),
        ("limit" = Option<i64>, Query, description = "Limit number of results"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Audit history retrieved successfully", body = AuditHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Table not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_table_history(
    State(state): State<AppState>,
    Path(table_id): Path<Uuid>,
    Query(params): Query<AuditQueryParams>,
    headers: HeaderMap,
) -> Result<Json<AuditHistoryResponse>, StatusCode> {
    let _user_context = workspace::get_user_context(&state, &headers).await?;

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    if let Some(db) = state.database() {
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM audit_entries
            WHERE entity_type = 'table' AND entity_id = $1
            "#,
            table_id
        )
        .fetch_one(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let rows = sqlx::query!(
            r#"
            SELECT id, entity_type, entity_id, action, user_id, user_email, created_at
            FROM audit_entries
            WHERE entity_type = 'table' AND entity_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            table_id,
            limit,
            offset
        )
        .fetch_all(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let entries = rows
            .into_iter()
            .map(|r| AuditEntryResponse {
                id: r.id,
                entity_type: r.entity_type,
                entity_id: r.entity_id,
                action: r.action,
                user_id: r.user_id,
                user_email: r.user_email,
                created_at: r.created_at,
            })
            .collect();

        Ok(Json(AuditHistoryResponse {
            entries,
            total: total.unwrap_or(0),
            limit,
            offset,
        }))
    } else {
        Ok(Json(AuditHistoryResponse {
            entries: Vec::new(),
            total: 0,
            limit,
            offset,
        }))
    }
}

/// GET /audit/relationships/{relationship_id}/history
#[utoipa::path(
    get,
    path = "/audit/relationships/{relationship_id}/history",
    tag = "Audit",
    params(
        ("relationship_id" = Uuid, Path, description = "Relationship UUID"),
        ("limit" = Option<i64>, Query, description = "Limit number of results"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Audit history retrieved successfully", body = AuditHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Relationship not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_relationship_history(
    State(state): State<AppState>,
    Path(relationship_id): Path<Uuid>,
    Query(params): Query<AuditQueryParams>,
    headers: HeaderMap,
) -> Result<Json<AuditHistoryResponse>, StatusCode> {
    let _user_context = workspace::get_user_context(&state, &headers).await?;

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    if let Some(db) = state.database() {
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM audit_entries
            WHERE entity_type = 'relationship' AND entity_id = $1
            "#,
            relationship_id
        )
        .fetch_one(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let rows = sqlx::query!(
            r#"
            SELECT id, entity_type, entity_id, action, user_id, user_email, created_at
            FROM audit_entries
            WHERE entity_type = 'relationship' AND entity_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            relationship_id,
            limit,
            offset
        )
        .fetch_all(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let entries = rows
            .into_iter()
            .map(|r| AuditEntryResponse {
                id: r.id,
                entity_type: r.entity_type,
                entity_id: r.entity_id,
                action: r.action,
                user_id: r.user_id,
                user_email: r.user_email,
                created_at: r.created_at,
            })
            .collect();

        Ok(Json(AuditHistoryResponse {
            entries,
            total: total.unwrap_or(0),
            limit,
            offset,
        }))
    } else {
        Ok(Json(AuditHistoryResponse {
            entries: Vec::new(),
            total: 0,
            limit,
            offset,
        }))
    }
}

/// GET /audit/workspaces/{workspace_id}/history
#[utoipa::path(
    get,
    path = "/audit/workspaces/{workspace_id}/history",
    tag = "Audit",
    params(
        ("workspace_id" = Uuid, Path, description = "Workspace UUID"),
        ("limit" = Option<i64>, Query, description = "Limit number of results"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination")
    ),
    responses(
        (status = 200, description = "Audit history retrieved successfully", body = AuditHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Workspace not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_workspace_history(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<AuditQueryParams>,
    headers: HeaderMap,
) -> Result<Json<AuditHistoryResponse>, StatusCode> {
    let _user_context = workspace::get_user_context(&state, &headers).await?;

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    if let Some(db) = state.database() {
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM audit_entries
            WHERE workspace_id = $1
            "#,
            workspace_id
        )
        .fetch_one(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let rows = sqlx::query!(
            r#"
            SELECT id, entity_type, entity_id, action, user_id, user_email, created_at
            FROM audit_entries
            WHERE workspace_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            workspace_id,
            limit,
            offset
        )
        .fetch_all(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch audit entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let entries = rows
            .into_iter()
            .map(|r| AuditEntryResponse {
                id: r.id,
                entity_type: r.entity_type,
                entity_id: r.entity_id,
                action: r.action,
                user_id: r.user_id,
                user_email: r.user_email,
                created_at: r.created_at,
            })
            .collect();

        Ok(Json(AuditHistoryResponse {
            entries,
            total: total.unwrap_or(0),
            limit,
            offset,
        }))
    } else {
        Ok(Json(AuditHistoryResponse {
            entries: Vec::new(),
            total: 0,
            limit,
            offset,
        }))
    }
}

/// GET /audit/entries/{entry_id}
#[utoipa::path(
    get,
    path = "/audit/entries/{entry_id}",
    tag = "Audit",
    params(
        ("entry_id" = Uuid, Path, description = "Audit entry UUID")
    ),
    responses(
        (status = 200, description = "Audit entry retrieved successfully", body = AuditEntryDetailResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Audit entry not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_audit_entry(
    State(state): State<AppState>,
    Path(entry_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AuditEntryDetailResponse>, StatusCode> {
    let _user_context = workspace::get_user_context(&state, &headers).await?;

    if let Some(db) = state.database() {
        let row = sqlx::query!(
            r#"
            SELECT id, entity_type, entity_id, workspace_id, domain_id, action, user_id, user_email, changes, previous_data, new_data, created_at
            FROM audit_entries
            WHERE id = $1
            "#,
            entry_id
        )
        .fetch_optional(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch audit entry: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some(r) = row {
            Ok(Json(AuditEntryDetailResponse {
                id: r.id,
                entity_type: r.entity_type,
                entity_id: r.entity_id,
                workspace_id: r.workspace_id,
                domain_id: r.domain_id,
                action: r.action,
                user_id: r.user_id,
                user_email: r.user_email,
                changes: r.changes,
                previous_data: r.previous_data,
                new_data: r.new_data,
                created_at: r.created_at,
            }))
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
