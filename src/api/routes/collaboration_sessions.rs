//! Collaboration session management routes.
//!
//! Provides endpoints for creating and managing shared collaboration sessions.

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::app_state::AppState;
use super::auth_context::AuthContext;
use crate::storage::collaboration::CollaborationStore;

/// Create the collaboration sessions router
pub fn collaboration_sessions_router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{session_id}", get(get_session))
        .route("/sessions/{session_id}", delete(end_session))
        .route(
            "/sessions/{session_id}/participants",
            get(list_participants),
        )
        .route("/sessions/{session_id}/invite", post(invite_user))
        .route(
            "/sessions/{session_id}/participants/{user_id}",
            delete(remove_participant),
        )
        .route(
            "/sessions/{session_id}/requests",
            get(list_pending_requests),
        )
        .route("/sessions/{session_id}/requests", post(request_access))
        .route(
            "/sessions/{session_id}/requests/{request_id}",
            post(respond_to_request),
        )
        .route("/sessions/{session_id}/presence", get(get_presence))
}

/// Request to create a collaboration session
#[derive(Deserialize, ToSchema)]
pub struct CreateSessionRequest {
    workspace_id: Uuid,
    domain_id: Option<Uuid>,
    name: String,
    description: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

/// Response for session creation
#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    id: Uuid,
    workspace_id: Uuid,
    domain_id: Option<Uuid>,
    name: String,
    description: Option<String>,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

/// Request to invite a user
#[derive(Deserialize, ToSchema)]
pub struct InviteRequest {
    user_id: Uuid,
    permission: String, // "viewer", "editor", "owner"
}

/// Response for participant information
#[derive(Serialize, ToSchema)]
pub struct ParticipantResponse {
    id: Uuid,
    user_id: Uuid,
    permission: String,
    joined_at: DateTime<Utc>,
    is_online: bool,
}

/// Request for access
#[derive(Deserialize, ToSchema)]
pub struct AccessRequestRequest {
    #[allow(dead_code)] // Message may be used in future for access request context
    message: Option<String>,
}

/// Response for access request
#[derive(Serialize, ToSchema)]
pub struct AccessRequestResponse {
    request_id: Uuid,
    status: String,
    message: String,
}

/// Request to respond to access request
#[derive(Deserialize, ToSchema)]
pub struct RespondToRequestRequest {
    approved: bool,
}

/// Response for presence information
#[derive(Serialize, ToSchema)]
pub struct PresenceResponse {
    user_id: Uuid,
    is_online: bool,
    cursor_x: Option<f64>,
    cursor_y: Option<f64>,
    selected_tables: Vec<String>,
    selected_relationships: Vec<String>,
    editing_table: Option<Uuid>,
}

/// POST /collaboration/sessions - Create a new collaboration session
#[utoipa::path(
    post,
    path = "/collaboration/sessions",
    tag = "Collaboration",
    request_body = CreateSessionRequest,
    responses(
        (status = 200, description = "Session created successfully", body = SessionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_session(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let session = store
        .create_session(
            request.workspace_id,
            request.domain_id,
            request.name,
            request.description,
            auth.user_context.user_id,
            request.expires_at,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Add creator as owner participant
    let _ = store
        .add_participant(session.id, auth.user_context.user_id, "owner".to_string())
        .await;

    Ok(Json(SessionResponse {
        id: session.id,
        workspace_id: session.workspace_id,
        domain_id: session.domain_id,
        name: session.name,
        description: session.description,
        created_by: session.created_by,
        created_at: session.created_at,
        expires_at: session.expires_at,
    }))
}

/// GET /collaboration/sessions - List collaboration sessions
#[utoipa::path(
    get,
    path = "/collaboration/sessions",
    tag = "Collaboration",
    params(
        ("workspace_id" = Option<Uuid>, Query, description = "Filter by workspace ID")
    ),
    responses(
        (status = 200, description = "Sessions retrieved successfully", body = Vec<SessionResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    auth: AuthContext,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<SessionResponse>>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    // Get workspace_id from query params or use user's workspace
    let workspace_id = if let Some(ws_id_str) = params.get("workspace_id") {
        Uuid::parse_str(ws_id_str).map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
        // Get user's workspace_id from storage - requires workspace lookup by email
        let storage = state.storage().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let workspace = storage
            .get_workspace_by_email(&auth.email)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        workspace.id
    };

    let sessions = store.list_sessions(workspace_id).await.map_err(|e| {
        tracing::error!("Failed to list sessions: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionResponse {
                id: s.id,
                workspace_id: s.workspace_id,
                domain_id: s.domain_id,
                name: s.name,
                description: s.description,
                created_by: s.created_by,
                created_at: s.created_at,
                expires_at: s.expires_at,
            })
            .collect(),
    ))
}

/// GET /collaboration/sessions/{session_id} - Get a collaboration session
#[utoipa::path(
    get,
    path = "/collaboration/sessions/{session_id}",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    responses(
        (status = 200, description = "Session retrieved successfully", body = SessionResponse),
        (status = 404, description = "Session not found"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_session(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let session = store
        .get_session(session_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get session: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionResponse {
        id: session.id,
        workspace_id: session.workspace_id,
        domain_id: session.domain_id,
        name: session.name,
        description: session.description,
        created_by: session.created_by,
        created_at: session.created_at,
        expires_at: session.expires_at,
    }))
}

/// DELETE /collaboration/sessions/{session_id} - End a collaboration session
#[utoipa::path(
    delete,
    path = "/collaboration/sessions/{session_id}",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    responses(
        (status = 204, description = "Session ended successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn end_session(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    store.delete_session(session_id).await.map_err(|e| {
        tracing::error!("Failed to end session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /collaboration/sessions/{session_id}/participants - List participants
#[utoipa::path(
    get,
    path = "/collaboration/sessions/{session_id}/participants",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    responses(
        (status = 200, description = "Participants retrieved successfully", body = Vec<ParticipantResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_participants(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<ParticipantResponse>>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let participants = store.list_participants(session_id).await.map_err(|e| {
        tracing::error!("Failed to list participants: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        participants
            .into_iter()
            .map(|p| ParticipantResponse {
                id: p.id,
                user_id: p.user_id,
                permission: p.permission,
                joined_at: p.joined_at,
                is_online: p.is_online,
            })
            .collect(),
    ))
}

/// POST /collaboration/sessions/{session_id}/invite - Invite a user to a session
#[utoipa::path(
    post,
    path = "/collaboration/sessions/{session_id}/invite",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    request_body = InviteRequest,
    responses(
        (status = 200, description = "User invited successfully", body = ParticipantResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn invite_user(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<ParticipantResponse>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    store
        .add_participant(session_id, request.user_id, request.permission)
        .await
        .map_err(|e| {
            tracing::error!("Failed to invite user: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get the participant to return
    let participants = store
        .list_participants(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let participant = participants
        .into_iter()
        .find(|p| p.user_id == request.user_id)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ParticipantResponse {
        id: participant.id,
        user_id: participant.user_id,
        permission: participant.permission,
        joined_at: participant.joined_at,
        is_online: participant.is_online,
    }))
}

/// DELETE /collaboration/sessions/{session_id}/participants/{user_id} - Remove participant
#[utoipa::path(
    delete,
    path = "/collaboration/sessions/{session_id}/participants/{user_id}",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID"),
        ("user_id" = Uuid, Path, description = "User UUID")
    ),
    responses(
        (status = 204, description = "Participant removed successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove_participant(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path((session_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    store
        .remove_participant(session_id, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to remove participant: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /collaboration/sessions/{session_id}/requests - List pending requests
#[utoipa::path(
    get,
    path = "/collaboration/sessions/{session_id}/requests",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    responses(
        (status = 200, description = "Requests retrieved successfully", body = Vec<AccessRequestResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_pending_requests(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<AccessRequestResponse>>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let requests = store.list_pending_requests(session_id).await.map_err(|e| {
        tracing::error!("Failed to list requests: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        requests
            .into_iter()
            .map(|r| AccessRequestResponse {
                request_id: r.id,
                status: r.status,
                message: format!("Access request from user {}", r.requester_id),
            })
            .collect(),
    ))
}

/// POST /collaboration/sessions/{session_id}/requests - Request access
#[utoipa::path(
    post,
    path = "/collaboration/sessions/{session_id}/requests",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    request_body = AccessRequestRequest,
    responses(
        (status = 200, description = "Access requested successfully", body = AccessRequestResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn request_access(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(session_id): Path<Uuid>,
    Json(_request): Json<AccessRequestRequest>,
) -> Result<Json<AccessRequestResponse>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let request = store
        .create_request(session_id, auth.user_context.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(AccessRequestResponse {
        request_id: request.id,
        status: request.status,
        message: "Access request created".to_string(),
    }))
}

/// POST /collaboration/sessions/{session_id}/requests/{request_id} - Respond to request
#[utoipa::path(
    post,
    path = "/collaboration/sessions/{session_id}/requests/{request_id}",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID"),
        ("request_id" = Uuid, Path, description = "Request UUID")
    ),
    request_body = RespondToRequestRequest,
    responses(
        (status = 200, description = "Request responded to successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn respond_to_request(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((_session_id, request_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<RespondToRequestRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    store
        .respond_to_request(request_id, auth.user_context.user_id, request.approved)
        .await
        .map_err(|e| {
            tracing::error!("Failed to respond to request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(serde_json::json!({
        "message": if request.approved { "Request approved" } else { "Request rejected" }
    })))
}

/// GET /collaboration/sessions/{session_id}/presence - Get presence information
#[utoipa::path(
    get,
    path = "/collaboration/sessions/{session_id}/presence",
    tag = "Collaboration",
    params(
        ("session_id" = Uuid, Path, description = "Session UUID")
    ),
    responses(
        (status = 200, description = "Presence retrieved successfully", body = Vec<PresenceResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_presence(
    State(state): State<AppState>,
    _auth: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<PresenceResponse>>, StatusCode> {
    let db = state.database().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let store = CollaborationStore::new(db.clone());

    let participants = store.get_presence(session_id).await.map_err(|e| {
        tracing::error!("Failed to get presence: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        participants
            .into_iter()
            .map(|p| {
                let selected_tables: Vec<String> =
                    serde_json::from_value(p.selected_tables.clone()).unwrap_or_default();
                let selected_relationships: Vec<String> =
                    serde_json::from_value(p.selected_relationships.clone()).unwrap_or_default();

                PresenceResponse {
                    user_id: p.user_id,
                    is_online: p.is_online,
                    cursor_x: p.cursor_x,
                    cursor_y: p.cursor_y,
                    selected_tables,
                    selected_relationships,
                    editing_table: p.editing_table,
                }
            })
            .collect(),
    ))
}
