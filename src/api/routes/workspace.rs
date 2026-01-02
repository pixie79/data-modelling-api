//! Workspace operations routes.
//! Handles user workspace creation and session management based on email.
//!
//! All endpoints require JWT authentication via Authorization header.
//!
//! This module supports two storage backends:
//! - PostgreSQL: Uses the `StorageBackend` trait for database operations
//! - File: Falls back to file-based operations for backward compatibility

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};
use utoipa::ToSchema;

use super::app_state::AppState;
use super::data_flow;
use super::git_sync;
use super::import;
use super::models;
use crate::services::jwt_service::JwtService;
use crate::storage::{
    StorageError,
    traits::{DomainInfo, PositionExport, UserContext, WorkspaceInfo as StorageWorkspaceInfo},
};
use axum::http::HeaderMap;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
pub struct CreateWorkspaceRequest {
    email: String,
    domain: String,
}

/// Request for creating workspace via /api/v1/workspaces endpoint
#[derive(Deserialize, ToSchema)]
pub struct CreateWorkspaceV1Request {
    name: String,
    #[serde(rename = "type")]
    workspace_type: String,
}

/// Response for workspace creation via /api/v1/workspaces endpoint
#[derive(Serialize, ToSchema)]
pub struct WorkspaceResponse {
    id: uuid::Uuid,
    name: String,
    #[serde(rename = "type")]
    workspace_type: String,
    email: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Response for listing workspaces
#[derive(Serialize, ToSchema)]
pub struct WorkspacesListResponse {
    workspaces: Vec<WorkspaceResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct CreateWorkspaceResponse {
    workspace_path: String,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub struct WorkspaceInfoResponse {
    workspace_path: String,
    email: String,
}

/// Profile information for a user
#[derive(Serialize, ToSchema)]
pub struct ProfileInfo {
    pub email: String,
    pub domains: Vec<String>,
}

/// List of profiles response
#[derive(Serialize, ToSchema)]
pub struct ProfilesResponse {
    profiles: Vec<ProfileInfo>,
}

/// Request to load or create a specific domain
#[derive(Deserialize, ToSchema)]
pub struct DomainRequest {
    domain: String,
}

/// Response for domain operations
#[derive(Serialize, ToSchema)]
pub struct DomainResponse {
    domain: String,
    workspace_path: String,
    message: String,
}

/// Response for listing domains
#[derive(Serialize, ToSchema)]
pub struct DomainsListResponse {
    domains: Vec<String>,
}

/// Create the workspace router
pub fn workspace_router() -> Router<AppState> {
    Router::new()
        .route("/create", post(create_workspace))
        .route("/info", get(get_workspace_info))
        .route("/profiles", get(list_profiles))
        // Domain CRUD endpoints
        .route("/domains", get(list_domains))
        .route("/domains", post(create_domain))
        .route("/domains/{domain}", get(get_domain))
        .route("/domains/{domain}", axum::routing::put(update_domain))
        .route("/domains/{domain}", axum::routing::delete(delete_domain))
        .route("/load-domain", post(load_domain))
        // Domain-scoped table CRUD endpoints
        .route("/domains/{domain}/tables", get(get_domain_tables))
        .route("/domains/{domain}/tables", post(create_domain_table))
        .route("/domains/{domain}/tables/{table_id}", get(get_domain_table))
        .route(
            "/domains/{domain}/tables/{table_id}",
            axum::routing::put(update_domain_table),
        )
        .route(
            "/domains/{domain}/tables/{table_id}",
            axum::routing::delete(delete_domain_table),
        )
        // Domain-scoped relationship CRUD endpoints
        .route(
            "/domains/{domain}/relationships",
            get(get_domain_relationships),
        )
        .route(
            "/domains/{domain}/relationships",
            post(create_domain_relationship),
        )
        .route(
            "/domains/{domain}/relationships/{relationship_id}",
            get(get_domain_relationship),
        )
        .route(
            "/domains/{domain}/relationships/{relationship_id}",
            axum::routing::put(update_domain_relationship),
        )
        .route(
            "/domains/{domain}/relationships/{relationship_id}",
            axum::routing::delete(delete_domain_relationship),
        )
        // Cross-domain reference endpoints
        .route(
            "/domains/{domain}/cross-domain",
            get(get_cross_domain_config),
        )
        .route(
            "/domains/{domain}/cross-domain/tables",
            get(list_cross_domain_tables),
        )
        .route(
            "/domains/{domain}/cross-domain/tables",
            post(add_cross_domain_table),
        )
        .route(
            "/domains/{domain}/cross-domain/tables/{table_id}",
            axum::routing::delete(remove_cross_domain_table),
        )
        .route(
            "/domains/{domain}/cross-domain/tables/{table_id}",
            axum::routing::put(update_cross_domain_table_ref),
        )
        .route(
            "/domains/{domain}/cross-domain/relationships",
            get(list_cross_domain_relationships),
        )
        .route(
            "/domains/{domain}/cross-domain/relationships/{relationship_id}",
            axum::routing::delete(remove_cross_domain_relationship),
        )
        .route(
            "/domains/{domain}/cross-domain/sync",
            post(sync_cross_domain_relationships),
        )
        // Combined view endpoint (domain tables + imported tables with ownership info)
        .route("/domains/{domain}/canvas", get(get_domain_canvas))
        // Domain-scoped import endpoints
        .nest("/domains/{domain}/import", import::domain_import_router())
        // Domain-scoped export endpoints (added directly to ensure domain path parameter is available)
        .route(
            "/domains/{domain}/export/{format}",
            get(models::domain_export_format),
        )
        .route(
            "/domains/{domain}/export/all",
            get(models::domain_export_all),
        )
        // Domain-scoped git sync endpoints
        .nest("/domains/{domain}/git", git_sync::domain_git_router())
        // Domain-scoped data-flow diagram endpoints
        .nest(
            "/domains/{domain}/data-flow-diagrams",
            data_flow::data_flow_router(),
        )
}

/// Get the workspace data directory from environment variable
pub fn get_workspace_data_dir() -> Result<PathBuf, String> {
    let workspace_data = std::env::var("WORKSPACE_DATA")
        .map_err(|_| "WORKSPACE_DATA environment variable not set".to_string())?;

    let path = PathBuf::from(workspace_data);
    if !path.exists() {
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create workspace data directory: {}", e))?;
    }

    Ok(path)
}

/// Sanitize email for use as directory name
pub fn sanitize_email_for_path(email: &str) -> String {
    // Replace invalid characters with safe alternatives
    email
        .replace('@', "_at_")
        .replace(['.', '/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
}

/// Validate domain name for use in URL paths and file system.
///
/// Prevents path traversal attacks and ensures domain names are safe.
pub fn validate_domain_name(domain: &str) -> Result<(), StatusCode> {
    // Check empty
    if domain.is_empty() {
        warn!("Domain name is empty");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check length
    if domain.len() > 100 {
        warn!("Domain name too long: {} chars", domain.len());
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for path traversal patterns
    if domain.contains("..") || domain.contains('/') || domain.contains('\\') {
        warn!("Domain name contains path traversal characters: {}", domain);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for hidden file patterns
    if domain.starts_with('.') {
        warn!("Domain name starts with dot: {}", domain);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Only allow alphanumeric, hyphens, and underscores
    if !domain
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        warn!("Domain name contains invalid characters: {}", domain);
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}

/// File-mode user id mapping (UUIDv4) persisted under WORKSPACE_DATA.
///
/// This avoids deriving ids from email while keeping user ids stable across sessions.
pub fn get_or_create_file_user_id(email: &str) -> Result<Uuid, String> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Err("Email cannot be empty".to_string());
    }

    let workspace_data_dir = get_workspace_data_dir()?;
    let mapping_path = workspace_data_dir.join(".users.json");

    let mut map: HashMap<String, String> = if mapping_path.exists() {
        let content = std::fs::read_to_string(&mapping_path)
            .map_err(|e| format!("Failed to read user mapping: {}", e))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    if let Some(existing) = map.get(&email)
        && let Ok(id) = Uuid::parse_str(existing)
    {
        return Ok(id);
    }

    let id = Uuid::new_v4();
    map.insert(email, id.to_string());

    let json = serde_json::to_string_pretty(&map)
        .map_err(|e| format!("Failed to serialize user mapping: {}", e))?;
    std::fs::write(&mapping_path, json)
        .map_err(|e| format!("Failed to write user mapping: {}", e))?;

    Ok(id)
}

/// Create workspace for email and domain (shared function for use by auth routes)
pub async fn create_workspace_for_email_and_domain(
    model_service: &mut crate::services::ModelService,
    email: &str,
    domain: &str,
) -> Result<String, String> {
    let email = email.trim().to_lowercase();
    let domain = domain.trim();

    if email.is_empty() {
        return Err("Email cannot be empty".to_string());
    }

    if domain.is_empty() {
        return Err("Domain cannot be empty".to_string());
    }

    // Validate email format (basic check)
    if !email.contains('@') || !email.contains('.') {
        return Err("Invalid email format".to_string());
    }

    // Get workspace data directory
    let workspace_data_dir = match get_workspace_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            return Err(format!("Failed to get workspace data directory: {}", e));
        }
    };

    // Create user workspace directory with domain subdirectory
    // Structure: {WORKSPACE_DATA}/{email}/{domain}/
    let sanitized_email = sanitize_email_for_path(&email);
    let user_workspace = workspace_data_dir.join(&sanitized_email).join(domain);

    // Create workspace directory structure
    let tables_dir = user_workspace.join("tables");

    if let Err(e) = std::fs::create_dir_all(&tables_dir) {
        return Err(format!("Failed to create workspace directory: {}", e));
    }

    // Load or create model - will load existing tables from YAML if they exist
    let model_result = model_service.load_or_create_model(
        format!("Workspace for {} - {}", email, domain),
        user_workspace.clone(),
        Some(format!("User workspace for {} in domain {}", email, domain)),
    );

    match model_result {
        Ok(_) => {
            info!(
                "Created/loaded workspace for user: {} domain: {} at {:?}",
                email, domain, user_workspace
            );
            Ok(user_workspace.to_string_lossy().to_string())
        }
        Err(e) => Err(format!("Failed to create/load model in workspace: {}", e)),
    }
}

/// Create workspace for email with default domain (for backwards compatibility)
pub async fn create_workspace_for_email(
    model_service: &mut crate::services::ModelService,
    email: &str,
) -> Result<String, String> {
    // Use "default" as the domain for backwards compatibility
    create_workspace_for_email_and_domain(model_service, email, "default").await
}

/// POST /workspace/create - Create or get workspace for user email and domain
#[utoipa::path(
    post,
    path = "/workspace/create",
    tag = "Workspace",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 200, description = "Workspace created or retrieved successfully", body = CreateWorkspaceResponse),
        (status = 400, description = "Bad request - invalid email or domain"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_workspace(
    State(state): State<AppState>,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<Json<CreateWorkspaceResponse>, StatusCode> {
    let email = request.email.trim().to_lowercase();
    let domain = request.domain.trim();

    if email.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate email format (basic check)
    if !email.contains('@') || !email.contains('.') {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if workspace already exists and has model data
    let mut model_service = state.model_service.lock().await;

    match create_workspace_for_email_and_domain(&mut model_service, &email, domain).await {
        Ok(workspace_path) => Ok(Json(CreateWorkspaceResponse {
            workspace_path,
            message: format!("Workspace ready for {} in domain {}", email, domain),
        })),
        Err(e) => {
            warn!("Failed to create/load workspace: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Ensure workspace is loaded for the current session.
/// This function checks if a model is already loaded, and if not, tries to load it
/// from the session's selected email.
pub async fn ensure_workspace_loaded(
    app_state: &AppState,
    headers: &HeaderMap,
) -> Result<(), String> {
    // Try to get session ID from headers
    let session_id = headers
        .get("x-session-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    ensure_workspace_loaded_with_session_id(app_state, session_id).await
}

/// Ensure workspace is loaded using a session ID string (for WebSocket connections).
pub async fn ensure_workspace_loaded_with_session_id(
    app_state: &AppState,
    session_id: Option<String>,
) -> Result<(), String> {
    let model_service = app_state.model_service.lock().await;

    // If model is already loaded, we're good
    if model_service.get_current_model().is_some() {
        return Ok(());
    }

    drop(model_service); // Release lock before async operation

    let email = if let Some(session_id) = session_id {
        // Try database session first, then in-memory
        let session_uuid = Uuid::parse_str(&session_id).ok();

        if let Some(uuid) = session_uuid {
            if let Some(db_session_store) = app_state.db_session_store.as_ref() {
                match db_session_store.get_session(uuid).await {
                    Ok(Some(session)) => {
                        // Check if session is valid
                        if session.revoked_at.is_none() && session.expires_at > chrono::Utc::now() {
                            session.selected_email.clone()
                        } else {
                            None
                        }
                    }
                    Ok(None) => {
                        // Session not found in database, try in-memory
                        let sessions = app_state.session_store.lock().await;
                        sessions
                            .get(&session_id)
                            .and_then(|s| s.selected_email.clone())
                    }
                    Err(e) => {
                        warn!("Failed to get session from database: {}", e);
                        // Fall through to in-memory check
                        let sessions = app_state.session_store.lock().await;
                        sessions
                            .get(&session_id)
                            .and_then(|s| s.selected_email.clone())
                    }
                }
            } else {
                // No database session store, use in-memory
                let sessions = app_state.session_store.lock().await;
                sessions
                    .get(&session_id)
                    .and_then(|s| s.selected_email.clone())
            }
        } else {
            // Invalid UUID format, try in-memory fallback
            let sessions = app_state.session_store.lock().await;
            sessions
                .get(&session_id)
                .and_then(|s| s.selected_email.clone())
        }
    } else {
        None
    };

    if let Some(email) = email {
        // Load workspace for this email
        let mut model_service = app_state.model_service.lock().await;
        create_workspace_for_email(&mut model_service, &email).await?;
        Ok(())
    } else {
        Err(
            "No session or email available. Please authenticate and select an email first."
                .to_string(),
        )
    }
}

/// GET /workspace/info - Get current workspace information
#[utoipa::path(
    get,
    path = "/workspace/info",
    tag = "Workspace",
    responses(
        (status = 200, description = "Workspace information retrieved successfully", body = WorkspaceInfoResponse),
        (status = 404, description = "Workspace not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_workspace_info(
    State(state): State<AppState>,
) -> Result<Json<WorkspaceInfoResponse>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Extract email from workspace path
    // The path format is: /workspace_data/user_at_example_com
    // We need to reverse the sanitization: _at_ -> @, but be careful with dots
    let workspace_path = PathBuf::from(&model.git_directory_path);
    let sanitized = workspace_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Reverse sanitization: _at_ -> @
    // Note: We can't perfectly reverse dots since both . and other chars become _
    // So we'll use a simple approach: _at_ -> @, keep rest as-is
    let email = sanitized.replace("_at_", "@");

    Ok(Json(WorkspaceInfoResponse {
        workspace_path: model.git_directory_path.clone(),
        email,
    }))
}

/// GET /api/v1/workspaces - List all workspaces for the authenticated user
#[utoipa::path(
    get,
    path = "/workspaces",
    tag = "Workspace",
    responses(
        (status = 200, description = "List of workspaces retrieved successfully", body = WorkspacesListResponse),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WorkspacesListResponse>, StatusCode> {
    // Get user context from JWT token
    let user_context = get_user_context(&state, &headers).await?;

    // Get workspaces for this user
    let workspaces: Vec<StorageWorkspaceInfo> = if let Some(storage) = state.storage.as_ref() {
        storage
            .get_workspaces_by_owner(user_context.user_id)
            .await
            .map_err(|e| {
                warn!("Failed to get workspaces: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    } else {
        // File-based mode - read from .workspaces.json file
        match get_workspace_data_dir() {
            Ok(workspace_data_dir) => {
                let sanitized_email = sanitize_email_for_path(&user_context.email);
                let user_workspace_base = workspace_data_dir.join(&sanitized_email);
                let workspaces_file = user_workspace_base.join(".workspaces.json");

                if workspaces_file.exists() {
                    match std::fs::read_to_string(&workspaces_file) {
                        Ok(content) => {
                            let workspaces_map: HashMap<String, serde_json::Value> =
                                serde_json::from_str(&content).unwrap_or_default();

                            workspaces_map
                                .values()
                                .filter_map(|v| {
                                    Some(StorageWorkspaceInfo {
                                        id: uuid::Uuid::parse_str(v.get("id")?.as_str()?).ok()?,
                                        owner_id: user_context.user_id,
                                        email: v.get("email")?.as_str()?.to_string(),
                                        name: v.get("name")?.as_str().map(|s| s.to_string()),
                                        workspace_type: v
                                            .get("type")?
                                            .as_str()
                                            .map(|s| s.to_string()),
                                        created_at: chrono::DateTime::parse_from_rfc3339(
                                            v.get("created_at")?.as_str()?,
                                        )
                                        .ok()?
                                        .with_timezone(&chrono::Utc),
                                        updated_at: chrono::Utc::now(), // Use current time as fallback
                                    })
                                })
                                .collect()
                        }
                        Err(e) => {
                            warn!("Failed to read workspaces file: {}", e);
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            }
            Err(_) => {
                // WORKSPACE_DATA not set - return empty list
                Vec::new()
            }
        }
    };

    // Convert to response format
    let workspace_responses: Vec<WorkspaceResponse> = workspaces
        .into_iter()
        .map(|w| WorkspaceResponse {
            id: w.id,
            name: w.name.unwrap_or_else(|| "Unnamed Workspace".to_string()),
            workspace_type: w.workspace_type.unwrap_or_else(|| "personal".to_string()),
            email: w.email,
            created_at: w.created_at,
        })
        .collect();

    Ok(Json(WorkspacesListResponse {
        workspaces: workspace_responses,
    }))
}

/// POST /api/v1/workspaces - Create a new workspace for the authenticated user
#[utoipa::path(
    post,
    path = "/workspaces",
    tag = "Workspace",
    request_body = CreateWorkspaceV1Request,
    responses(
        (status = 200, description = "Workspace created successfully", body = WorkspaceResponse),
        (status = 400, description = "Bad request - validation failed"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 409, description = "Conflict - workspace name already exists for this email"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_workspace_v1(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkspaceV1Request>,
) -> Result<Json<WorkspaceResponse>, StatusCode> {
    // Validate request
    let name = request.name.trim();
    let workspace_type = request.workspace_type.trim().to_lowercase();

    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if workspace_type != "personal" && workspace_type != "organization" {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get user context from JWT token
    let user_context = get_user_context(&state, &headers).await?;
    let email = user_context.email.clone();

    // Check if workspace name already exists for this email
    if let Some(storage) = state.storage.as_ref() {
        let name_exists = storage
            .workspace_name_exists(&email, name)
            .await
            .map_err(|e| {
                warn!("Failed to check workspace name: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if name_exists {
            return Err(StatusCode::CONFLICT);
        }

        // Create workspace
        let workspace = storage
            .create_workspace_with_details(
                email.clone(),
                &user_context,
                name.to_string(),
                workspace_type.clone(),
            )
            .await
            .map_err(|e| {
                warn!("Failed to create workspace: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        Ok(Json(WorkspaceResponse {
            id: workspace.id,
            name: workspace.name.unwrap_or_else(|| name.to_string()),
            workspace_type: workspace.workspace_type.unwrap_or(workspace_type),
            email: workspace.email,
            created_at: workspace.created_at,
        }))
    } else {
        // File-based mode - use ModelService to create workspace
        // Check if workspace name already exists by checking directory structure
        let workspace_data_dir = match get_workspace_data_dir() {
            Ok(dir) => dir,
            Err(_) => {
                warn!("WORKSPACE_DATA not set for file-based workspace creation");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let sanitized_email = sanitize_email_for_path(&email);
        let user_workspace_base = workspace_data_dir.join(&sanitized_email);

        // Ensure user workspace base directory exists
        if let Err(e) = std::fs::create_dir_all(&user_workspace_base) {
            warn!("Failed to create user workspace directory: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        // Check if workspace name already exists for this email
        // Track workspace names in a JSON file: {WORKSPACE_DATA}/{email}/.workspaces.json
        let workspaces_file = user_workspace_base.join(".workspaces.json");
        let mut workspaces: HashMap<String, serde_json::Value> = if workspaces_file.exists() {
            match std::fs::read_to_string(&workspaces_file) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(e) => {
                    warn!("Failed to read workspaces file: {}", e);
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        // Check for duplicate workspace name
        if workspaces.contains_key(name) {
            return Err(StatusCode::CONFLICT);
        }

        // Create workspace directory structure using name as domain identifier
        // Structure: {WORKSPACE_DATA}/{email}/{name}/
        let workspace_dir = user_workspace_base.join(name);
        let tables_dir = workspace_dir.join("tables");

        if let Err(e) = std::fs::create_dir_all(&tables_dir) {
            warn!("Failed to create workspace directory: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        // Create workspace using ModelService
        let mut model_service = state.model_service.lock().await;
        match model_service.load_or_create_model(
            format!("{} - {}", name, email),
            workspace_dir.clone(),
            Some(format!("Workspace {} for {}", name, email)),
        ) {
            Ok(_) => {
                info!(
                    "Created file-based workspace: {} for user: {} at {:?}",
                    name, email, workspace_dir
                );
            }
            Err(e) => {
                warn!("Failed to create model in workspace: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        // Generate workspace ID (deterministic based on email + name)
        let workspace_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_DNS,
            format!("{}:{}", email, name).as_bytes(),
        );

        // Get or create user ID for file-based mode
        let _owner_id = match get_or_create_file_user_id(&email) {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to get/create user ID: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // Store workspace metadata
        let workspace_metadata = serde_json::json!({
            "id": workspace_id.to_string(),
            "name": name,
            "type": workspace_type,
            "email": email,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "workspace_path": workspace_dir.to_string_lossy().to_string(),
        });
        workspaces.insert(name.to_string(), workspace_metadata);

        // Save workspace metadata
        if let Err(e) = std::fs::write(
            &workspaces_file,
            serde_json::to_string_pretty(&workspaces).unwrap_or_default(),
        ) {
            warn!("Failed to save workspace metadata: {}", e);
            // Continue anyway - workspace is created
        }

        Ok(Json(WorkspaceResponse {
            id: workspace_id,
            name: name.to_string(),
            workspace_type: workspace_type.clone(),
            email: email.clone(),
            created_at: chrono::Utc::now(),
        }))
    }
}

/// GET /workspace/profiles - List all profiles (email/domain combinations) for the authenticated user
#[utoipa::path(
    get,
    path = "/workspace/profiles",
    tag = "Workspace",
    responses(
        (status = 200, description = "List of profiles retrieved successfully", body = ProfilesResponse),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProfilesResponse>, StatusCode> {
    // Initialize JWT service and validate token
    let jwt_service = JwtService::from_env();

    // Try Authorization header first (preferred)
    let token =
        if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
            JwtService::extract_bearer_token(auth_header)
        } else {
            // Fall back to x-session-id header
            headers.get("x-session-id").and_then(|h| h.to_str().ok())
        };

    let token = match token {
        Some(t) => t,
        None => {
            info!("No authorization token provided for list_profiles");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Validate the token
    let claims = match jwt_service.validate_access_token(token) {
        Ok(c) => c,
        Err(e) => {
            warn!("JWT validation failed for list_profiles: {}", e);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let session_id = claims.session_id.clone();

    // Try storage backend first (PostgreSQL mode)
    if let Some(storage) = state.storage.as_ref()
        && let Some(db_session_store) = state.db_session_store.as_ref()
    {
        // Parse session UUID
        if let Ok(session_uuid) = uuid::Uuid::parse_str(&session_id) {
            // Get session from database
            if let Ok(Some(_db_session)) = db_session_store.get_session(session_uuid).await {
                // Get workspaces for this user
                match storage.get_workspaces().await {
                    Ok(workspaces) => {
                        let mut profiles = Vec::new();

                        for workspace in workspaces {
                            // Get domains for this workspace
                            let domains = match storage.get_domains(workspace.id).await {
                                Ok(domain_infos) => {
                                    domain_infos.iter().map(|d| d.name.clone()).collect()
                                }
                                Err(_) => Vec::new(),
                            };

                            profiles.push(ProfileInfo {
                                email: workspace.email.clone(),
                                domains,
                            });
                        }

                        info!(
                            "Listed {} profiles for session {} from database",
                            profiles.len(),
                            session_id
                        );
                        return Ok(Json(ProfilesResponse { profiles }));
                    }
                    Err(e) => {
                        warn!("Failed to get workspaces from database: {}", e);
                        // Fall through to file-based approach
                    }
                }
            }
        }
    }

    // File-based fallback
    // Get session to find user's emails
    let sessions = state.session_store.lock().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s.clone(),
        None => {
            info!("Session not found for list_profiles: {}", session_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    drop(sessions);

    // Get workspace data directory
    let workspace_data_dir = match get_workspace_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            warn!("Failed to get workspace data directory: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Build list of profiles from user's emails
    let mut profiles = Vec::new();

    for email_info in &session.emails {
        if !email_info.verified {
            continue;
        }

        let email = &email_info.email;
        let sanitized_email = sanitize_email_for_path(email);
        let user_workspace = workspace_data_dir.join(&sanitized_email);

        // List domains (subdirectories) in the user's workspace
        let mut domains = Vec::new();
        if user_workspace.exists()
            && let Ok(entries) = std::fs::read_dir(&user_workspace)
        {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                    && let Some(name) = entry.file_name().to_str()
                {
                    // Skip hidden directories and special directories
                    if !name.starts_with('.') && name != "tables" {
                        domains.push(name.to_string());
                    }
                }
            }
        }

        profiles.push(ProfileInfo {
            email: email.clone(),
            domains,
        });
    }

    info!(
        "Listed {} profiles for session {} from file system",
        profiles.len(),
        session_id
    );

    Ok(Json(ProfilesResponse { profiles }))
}

/// Helper to get session email from JWT token in headers
///
/// Validates the JWT token and returns the email (subject claim).
/// Supports both Authorization: Bearer `token` and x-session-id header.
async fn get_session_email(state: &AppState, headers: &HeaderMap) -> Result<String, StatusCode> {
    // Initialize JWT service
    let jwt_service = JwtService::from_env();

    // Try Authorization header first (preferred)
    let token =
        if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
            JwtService::extract_bearer_token(auth_header)
        } else {
            // Fall back to x-session-id header
            headers.get("x-session-id").and_then(|h| h.to_str().ok())
        };

    let token = token.ok_or_else(|| {
        warn!("No authorization token provided");
        StatusCode::UNAUTHORIZED
    })?;

    // Validate the token
    let claims = jwt_service.validate_access_token(token).map_err(|e| {
        warn!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Check if the session is still valid (not revoked)
    // The subject claim contains the email
    if claims.sub.is_empty() {
        warn!("JWT has empty subject claim");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify session still exists - check database first, then in-memory
    let session_uuid = match Uuid::parse_str(&claims.session_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            // Invalid UUID format, try in-memory fallback
            let sessions = state.session_store.lock().await;
            if !sessions.contains_key(&claims.session_id) {
                warn!("Session {} not found in store", claims.session_id);
                return Err(StatusCode::UNAUTHORIZED);
            }
            return Ok(claims.sub);
        }
    };

    // Try database session first
    if let Some(db_session_store) = state.db_session_store.as_ref() {
        match db_session_store.get_session(session_uuid).await {
            Ok(Some(session)) => {
                // Check if session is valid
                if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
                    warn!("Session {} is expired or revoked", claims.session_id);
                    return Err(StatusCode::UNAUTHORIZED);
                }
                // Session is valid, return email from claims
                return Ok(claims.sub);
            }
            Ok(None) => {
                // Session not found in database, try in-memory
            }
            Err(e) => {
                warn!("Failed to get session from database: {}", e);
                // Fall through to in-memory check
            }
        }
    }

    // Fall back to in-memory session store
    let sessions = state.session_store.lock().await;
    if !sessions.contains_key(&claims.session_id) {
        warn!("Session {} not found in store", claims.session_id);
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(claims.sub)
}

/// Helper to get user context (user_id and email) from JWT token in headers.
///
/// This is used for storage backend operations that require user attribution.
/// For PostgreSQL mode, it also retrieves the user_id from the database session.
pub async fn get_user_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<UserContext, StatusCode> {
    let jwt_service = JwtService::from_env();

    // Try Authorization header first (preferred)
    let token =
        if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
            JwtService::extract_bearer_token(auth_header)
        } else {
            headers.get("x-session-id").and_then(|h| h.to_str().ok())
        };

    let token = token.ok_or_else(|| {
        warn!("No authorization token provided");
        StatusCode::UNAUTHORIZED
    })?;

    let claims = jwt_service.validate_access_token(token).map_err(|e| {
        warn!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    if claims.sub.is_empty() {
        warn!("JWT has empty subject claim");
        return Err(StatusCode::BAD_REQUEST);
    }

    // For PostgreSQL mode, get user_id from DB session
    if let Some(db_session_store) = state.db_session_store.as_ref()
        && let Ok(session_id) = uuid::Uuid::parse_str(&claims.session_id)
        && let Ok(Some(session)) = db_session_store.get_session(session_id).await
    {
        return Ok(UserContext {
            user_id: session.user_id,
            email: claims.sub,
        });
    }

    // For file-based mode, verify session exists in memory store
    let sessions = state.session_store.lock().await;
    if !sessions.contains_key(&claims.session_id) {
        warn!("Session {} not found in store", claims.session_id);
        return Err(StatusCode::UNAUTHORIZED);
    }

    // For file-based mode, we use a deterministic UUID based on email
    // This ensures consistent user IDs for the same email across sessions
    let user_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, claims.sub.as_bytes());

    Ok(UserContext {
        user_id,
        email: claims.sub,
    })
}

/// Helper to get workspace for a user, creating it if it doesn't exist.
///
/// This uses the storage backend for PostgreSQL mode, or file-based operations otherwise.
async fn get_or_create_workspace(
    state: &AppState,
    user_context: &UserContext,
) -> Result<StorageWorkspaceInfo, StatusCode> {
    if let Some(storage) = state.storage.as_ref() {
        // Try to get existing workspace
        match storage.get_workspace_by_email(&user_context.email).await {
            Ok(Some(workspace)) => return Ok(workspace),
            Ok(None) => {
                // Create new workspace
                match storage
                    .create_workspace(user_context.email.clone(), user_context)
                    .await
                {
                    Ok(workspace) => return Ok(workspace),
                    Err(e) => {
                        warn!("Failed to create workspace: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get workspace: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback: create a synthetic workspace info
    let sanitized_email = sanitize_email_for_path(&user_context.email);
    let workspace_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, sanitized_email.as_bytes());

    Ok(StorageWorkspaceInfo {
        id: workspace_id,
        owner_id: user_context.user_id,
        email: user_context.email.clone(),
        name: Some(format!(
            "Workspace {}",
            user_context.email.split('@').next().unwrap_or("default")
        )),
        workspace_type: Some("personal".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Helper to get or create a domain in a workspace.
async fn get_or_create_domain(
    state: &AppState,
    workspace: &StorageWorkspaceInfo,
    domain_name: &str,
    user_context: &UserContext,
) -> Result<DomainInfo, StatusCode> {
    if let Some(storage) = state.storage.as_ref() {
        // Try to get existing domain
        match storage.get_domain_by_name(workspace.id, domain_name).await {
            Ok(Some(domain)) => return Ok(domain),
            Ok(None) => {
                // Create new domain
                match storage
                    .create_domain(workspace.id, domain_name.to_string(), None, user_context)
                    .await
                {
                    Ok(domain) => return Ok(domain),
                    Err(e) => {
                        warn!("Failed to create domain: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get domain: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // File-based fallback: create a synthetic domain info
    let domain_id = uuid::Uuid::new_v5(&workspace.id, domain_name.as_bytes());

    Ok(DomainInfo {
        id: domain_id,
        workspace_id: workspace.id,
        name: domain_name.to_string(),
        description: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

/// Helper to get user workspace path
fn get_user_workspace_path(email: &str) -> Result<PathBuf, StatusCode> {
    let workspace_data_dir =
        get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(email);
    Ok(workspace_data_dir.join(&sanitized_email))
}

/// GET /workspace/domains - List all domains for the authenticated user
#[utoipa::path(
    get,
    path = "/workspace/domains",
    tag = "Workspace",
    responses(
        (status = 200, description = "List of domains retrieved successfully", body = DomainsListResponse),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DomainsListResponse>, StatusCode> {
    let user_context = get_user_context(&state, &headers).await?;

    // Try storage backend first (PostgreSQL or file-based)
    if let Some(storage) = state.storage.as_ref() {
        // Get workspace for user
        let workspace = get_or_create_workspace(&state, &user_context).await?;

        match storage.get_domains(workspace.id).await {
            Ok(domain_infos) => {
                let mut domains: Vec<String> =
                    domain_infos.iter().map(|d| d.name.clone()).collect();
                domains.sort();
                info!(
                    "Listed {} domains for user {} from storage",
                    domains.len(),
                    user_context.email
                );
                return Ok(Json(DomainsListResponse { domains }));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let user_workspace = get_user_workspace_path(&user_context.email)?;

    let mut domains = Vec::new();
    if user_workspace.exists()
        && let Ok(entries) = std::fs::read_dir(&user_workspace)
    {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                // Skip hidden directories
                if !name.starts_with('.') {
                    domains.push(name.to_string());
                }
            }
        }
    }

    domains.sort();
    info!(
        "Listed {} domains for user {} from file system",
        domains.len(),
        user_context.email
    );

    Ok(Json(DomainsListResponse { domains }))
}

/// POST /workspace/domains - Create a new domain for the authenticated user
#[utoipa::path(
    post,
    path = "/workspace/domains",
    tag = "Workspace",
    request_body = DomainRequest,
    responses(
        (status = 200, description = "Domain created successfully", body = DomainResponse),
        (status = 400, description = "Bad request - invalid domain name"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 409, description = "Conflict - domain already exists"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DomainRequest>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let user_context = get_user_context(&state, &headers).await?;

    let domain_name = request.domain.trim();
    if domain_name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate domain name (alphanumeric, hyphens, underscores only)
    if !domain_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        warn!("Invalid domain name: {}", domain_name);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        let workspace = get_or_create_workspace(&state, &user_context).await?;

        // Check if domain already exists
        if let Ok(Some(_)) = storage.get_domain_by_name(workspace.id, domain_name).await {
            return Err(StatusCode::CONFLICT);
        }

        match storage
            .create_domain(workspace.id, domain_name.to_string(), None, &user_context)
            .await
        {
            Ok(domain_info) => {
                let workspace_path =
                    format!("db://workspace/{}/domain/{}", workspace.id, domain_info.id);
                info!(
                    "Created domain {} for user {} in storage",
                    domain_name, user_context.email
                );
                return Ok(Json(DomainResponse {
                    domain: domain_name.to_string(),
                    workspace_path,
                    message: format!("Created domain {}", domain_name),
                }));
            }
            Err(e) => {
                warn!("Storage backend failed to create domain: {}", e);
                // Fall through to file-based fallback
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;

    match create_workspace_for_email_and_domain(
        &mut model_service,
        &user_context.email,
        domain_name,
    )
    .await
    {
        Ok(workspace_path) => {
            info!(
                "Created domain {} for user {} at {}",
                domain_name, user_context.email, workspace_path
            );
            Ok(Json(DomainResponse {
                domain: domain_name.to_string(),
                workspace_path,
                message: format!("Created domain {}", domain_name),
            }))
        }
        Err(e) => {
            warn!("Failed to create domain: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Domain info response with metadata (for API responses)
#[derive(Serialize)]
pub struct DomainInfoResponse {
    name: String,
    workspace_path: String,
    table_count: usize,
    relationship_count: usize,
    imported_table_count: usize,
    created_at: Option<String>,
    modified_at: Option<String>,
}

/// Request to update a domain
#[derive(Deserialize)]
pub struct UpdateDomainRequest {
    /// New name for the domain (rename)
    #[serde(default)]
    new_name: Option<String>,
}

/// GET /workspace/domains/:domain - Get domain info
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Domain information retrieved successfully", body = DomainInfoResponse),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
async fn get_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
) -> Result<Json<DomainInfoResponse>, StatusCode> {
    let user_context = get_user_context(&state, &headers).await?;

    let domain_name = domain.trim();
    validate_domain_name(domain_name)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        let workspace = get_or_create_workspace(&state, &user_context).await?;

        if let Ok(Some(domain_info)) = storage.get_domain_by_name(workspace.id, domain_name).await {
            // Get counts from storage
            let table_count = storage
                .get_tables(domain_info.id)
                .await
                .map(|t| t.len())
                .unwrap_or(0);
            let relationship_count = storage
                .get_relationships(domain_info.id)
                .await
                .map(|r| r.len())
                .unwrap_or(0);
            let imported_table_count = storage
                .get_cross_domain_refs(domain_info.id)
                .await
                .map(|r| r.len())
                .unwrap_or(0);

            return Ok(Json(DomainInfoResponse {
                name: domain_info.name,
                workspace_path: format!(
                    "db://workspace/{}/domain/{}",
                    workspace.id, domain_info.id
                ),
                table_count,
                relationship_count,
                imported_table_count,
                created_at: Some(domain_info.created_at.to_rfc3339()),
                modified_at: Some(domain_info.updated_at.to_rfc3339()),
            }));
        }
    }

    // File-based fallback
    let user_workspace = get_user_workspace_path(&user_context.email)?;
    let domain_path = user_workspace.join(domain_name);

    if !domain_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Count tables
    let tables_dir = domain_path.join("tables");
    let table_count = if tables_dir.exists() {
        std::fs::read_dir(&tables_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "yaml" || ext == "yml")
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    // Count relationships
    let relationships_file = domain_path.join("relationships.yaml");
    let relationship_count = if relationships_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&relationships_file) {
            // Simple count: number of "- id:" lines
            content
                .lines()
                .filter(|l| l.trim().starts_with("- id:"))
                .count()
        } else {
            0
        }
    } else {
        0
    };

    // Count imported tables from cross-domain config
    let cross_domain_path = domain_path.join("cross_domain.yaml");
    let imported_table_count = if cross_domain_path.exists() {
        let config = load_cross_domain_config(&cross_domain_path);
        config.imported_tables.len()
    } else {
        0
    };

    // Get timestamps
    let metadata = std::fs::metadata(&domain_path).ok();
    let created_at = metadata
        .as_ref()
        .and_then(|m| m.created().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
    let modified_at = metadata
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

    Ok(Json(DomainInfoResponse {
        name: domain_name.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        table_count,
        relationship_count,
        imported_table_count,
        created_at,
        modified_at,
    }))
}

/// PUT /workspace/domains/:domain - Update/rename a domain
#[utoipa::path(
    put,
    path = "/workspace/domains/{domain}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    request_body = UpdateDomainRequest,
    responses(
        (status = 200, description = "Domain updated successfully", body = DomainResponse),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 409, description = "Conflict - new domain name already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
    Json(request): Json<UpdateDomainRequest>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let user_context = get_user_context(&state, &headers).await?;

    let domain_name = domain.trim();
    validate_domain_name(domain_name)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        let workspace = get_or_create_workspace(&state, &user_context).await?;

        if let Ok(Some(domain_info)) = storage.get_domain_by_name(workspace.id, domain_name).await {
            // Handle rename
            if let Some(new_name) = request.new_name.as_ref() {
                let new_name = new_name.trim();
                if new_name.is_empty() {
                    return Err(StatusCode::BAD_REQUEST);
                }

                // Validate new name
                if !new_name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    warn!("Invalid new domain name: {}", new_name);
                    return Err(StatusCode::BAD_REQUEST);
                }

                // Check if new name already exists
                if let Ok(Some(_)) = storage.get_domain_by_name(workspace.id, new_name).await {
                    warn!("Domain already exists: {}", new_name);
                    return Err(StatusCode::CONFLICT);
                }

                match storage
                    .update_domain(
                        domain_info.id,
                        Some(new_name.to_string()),
                        None,
                        &user_context,
                    )
                    .await
                {
                    Ok(updated_domain) => {
                        let workspace_path = format!(
                            "db://workspace/{}/domain/{}",
                            workspace.id, updated_domain.id
                        );
                        info!(
                            "Renamed domain {} to {} for user {}",
                            domain_name, new_name, user_context.email
                        );
                        return Ok(Json(DomainResponse {
                            domain: new_name.to_string(),
                            workspace_path,
                            message: format!("Renamed domain {} to {}", domain_name, new_name),
                        }));
                    }
                    Err(e) => {
                        warn!("Failed to rename domain: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }

            // No changes requested
            let workspace_path =
                format!("db://workspace/{}/domain/{}", workspace.id, domain_info.id);
            return Ok(Json(DomainResponse {
                domain: domain_name.to_string(),
                workspace_path,
                message: "No changes".to_string(),
            }));
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // File-based fallback
    let user_workspace = get_user_workspace_path(&user_context.email)?;
    let domain_path = user_workspace.join(domain_name);

    if !domain_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Handle rename
    if let Some(new_name) = request.new_name {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }

        // Validate new name (alphanumeric, hyphens, underscores only)
        if !new_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            warn!("Invalid new domain name: {}", new_name);
            return Err(StatusCode::BAD_REQUEST);
        }

        let new_domain_path = user_workspace.join(new_name);

        // Check if new name already exists
        if new_domain_path.exists() {
            warn!("Domain already exists: {}", new_name);
            return Err(StatusCode::CONFLICT);
        }

        // Rename the directory
        if let Err(e) = std::fs::rename(&domain_path, &new_domain_path) {
            warn!(
                "Failed to rename domain {} to {}: {}",
                domain_name, new_name, e
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        info!(
            "Renamed domain {} to {} for user {}",
            domain_name, new_name, user_context.email
        );

        return Ok(Json(DomainResponse {
            domain: new_name.to_string(),
            workspace_path: new_domain_path.to_string_lossy().to_string(),
            message: format!("Renamed domain {} to {}", domain_name, new_name),
        }));
    }

    // No changes requested
    Ok(Json(DomainResponse {
        domain: domain_name.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        message: "No changes".to_string(),
    }))
}

/// DELETE /workspace/domains/:domain - Delete a domain for the authenticated user
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Domain deleted successfully", body = DomainResponse),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let user_context = get_user_context(&state, &headers).await?;

    let domain_name = domain.trim();
    validate_domain_name(domain_name)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        let workspace = get_or_create_workspace(&state, &user_context).await?;

        if let Ok(Some(domain_info)) = storage.get_domain_by_name(workspace.id, domain_name).await {
            match storage.delete_domain(domain_info.id, &user_context).await {
                Ok(()) => {
                    let workspace_path =
                        format!("db://workspace/{}/domain/{}", workspace.id, domain_info.id);
                    info!(
                        "Deleted domain {} for user {} from storage",
                        domain_name, user_context.email
                    );
                    return Ok(Json(DomainResponse {
                        domain: domain_name.to_string(),
                        workspace_path,
                        message: format!("Deleted domain {}", domain_name),
                    }));
                }
                Err(e) => {
                    warn!("Failed to delete domain from storage: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // File-based fallback
    let user_workspace = get_user_workspace_path(&user_context.email)?;
    let domain_path = user_workspace.join(domain_name);

    // Check if domain exists
    if !domain_path.exists() {
        warn!(
            "Domain not found: {} for user {}",
            domain_name, user_context.email
        );
        return Err(StatusCode::NOT_FOUND);
    }

    // Delete the domain directory
    if let Err(e) = std::fs::remove_dir_all(&domain_path) {
        warn!("Failed to delete domain {}: {}", domain_name, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    info!(
        "Deleted domain {} for user {}",
        domain_name, user_context.email
    );

    Ok(Json(DomainResponse {
        domain: domain_name.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        message: format!("Deleted domain {}", domain),
    }))
}

/// POST /workspace/load-domain - Load a specific domain for the authenticated user
/// This endpoint forces a reload from disk to ensure latest data is loaded
#[utoipa::path(
    post,
    path = "/workspace/load-domain",
    tag = "Workspace",
    request_body = DomainRequest,
    responses(
        (status = 200, description = "Domain loaded successfully", body = CreateWorkspaceResponse),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn load_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DomainRequest>,
) -> Result<Json<CreateWorkspaceResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;

    let domain = request.domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get workspace path
    let workspace_data_dir =
        get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(&email);
    let workspace_path = workspace_data_dir.join(&sanitized_email).join(domain);

    // Create tables directory if needed
    let tables_dir = workspace_path.join("tables");
    if let Err(e) = std::fs::create_dir_all(&tables_dir) {
        warn!("Failed to create workspace directory: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Force reload when explicitly calling load-domain endpoint
    let mut model_service = state.model_service.lock().await;
    match model_service.load_or_create_model_with_reload(
        format!("Workspace for {} - {}", email, domain),
        workspace_path.clone(),
        Some(format!("User workspace for {} in domain {}", email, domain)),
        true, // Force reload from disk
    ) {
        Ok(_) => {
            info!(
                "Loaded domain {} for user {} at {:?}",
                domain, email, workspace_path
            );
            Ok(Json(CreateWorkspaceResponse {
                workspace_path: workspace_path.to_string_lossy().to_string(),
                message: format!("Loaded domain {} for {}", domain, email),
            }))
        }
        Err(e) => {
            warn!("Failed to load domain: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Domain-scoped Table CRUD handlers
// ============================================================================

use crate::models::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, ModelingLevel, SCDPattern,
};
use crate::models::{Column, Position, Table};
use serde_json::{Value, json};

/// Request body for creating a table
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTableRequest {
    pub name: String,
    pub columns: Vec<Value>,
    #[serde(default)]
    pub database_type: Option<String>,
    #[serde(default)]
    pub catalog_name: Option<String>,
    #[serde(default)]
    pub schema_name: Option<String>,
    #[serde(default)]
    pub medallion_layers: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub medallion_layer: Option<String>,
    #[serde(default)]
    pub scd_pattern: Option<String>,
    #[serde(default)]
    pub data_vault_classification: Option<String>,
    #[serde(default)]
    pub modeling_level: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub odcl_metadata: std::collections::HashMap<String, Value>,
    #[serde(default)]
    pub position: Option<Value>,
}

/// Result of ensuring a domain is loaded, with context for storage operations.
#[allow(dead_code)]
/// Context for domain operations
///
/// This struct is public to allow domain-scoped handlers to access domain context.
pub struct DomainContext {
    /// The domain info (for storage operations).
    pub domain_info: DomainInfo,
    /// The user context (for attribution).
    pub user_context: UserContext,
    /// The workspace info.
    pub workspace: StorageWorkspaceInfo,
}

/// Helper to ensure domain is loaded for the current session.
/// Returns the domain context for storage operations.
///
/// This function is public to allow domain-scoped handlers to ensure
/// the domain is loaded before operating on it.
pub async fn ensure_domain_loaded(
    state: &AppState,
    headers: &HeaderMap,
    domain: &str,
) -> Result<DomainContext, StatusCode> {
    ensure_domain_loaded_with_reload(state, headers, domain, false).await
}

/// Helper to ensure domain is loaded with option to force reload from disk.
/// Returns the domain context for storage operations.
async fn ensure_domain_loaded_with_reload(
    state: &AppState,
    headers: &HeaderMap,
    domain: &str,
    force_reload: bool,
) -> Result<DomainContext, StatusCode> {
    // Validate domain name to prevent path traversal and injection
    validate_domain_name(domain)?;

    let user_context = get_user_context(state, headers).await?;
    let workspace = get_or_create_workspace(state, &user_context).await?;
    let domain_info = get_or_create_domain(state, &workspace, domain, &user_context).await?;

    // For file-based storage, also load the model service
    if state.storage.is_none() || !state.is_postgres() {
        let mut model_service = state.model_service.lock().await;
        let workspace_data_dir =
            get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let sanitized_email = sanitize_email_for_path(&user_context.email);
        let workspace_path = workspace_data_dir.join(&sanitized_email).join(domain);

        // Create tables directory if needed
        let tables_dir = workspace_path.join("tables");
        if let Err(e) = std::fs::create_dir_all(&tables_dir) {
            warn!("Failed to create workspace directory: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        // Load model, optionally forcing reload from disk
        let _ = model_service
            .load_or_create_model_with_reload(
                format!("Workspace for {} - {}", user_context.email, domain),
                workspace_path,
                Some(format!(
                    "User workspace for {} in domain {}",
                    user_context.email, domain
                )),
                force_reload, // Force reload if requested (e.g., for relationship operations to get latest tables)
            )
            .map_err(|e| {
                warn!("Failed to load domain {}: {}", domain, e);
                StatusCode::NOT_FOUND
            })?;
    }

    Ok(DomainContext {
        domain_info,
        user_context,
        workspace,
    })
}

/// Path parameters for domain-scoped routes
#[derive(Deserialize)]
pub struct DomainPath {
    pub domain: String,
}

/// Path parameters for domain + table routes
#[derive(Deserialize)]
pub struct DomainTablePath {
    pub domain: String,
    pub table_id: String,
}

/// Path parameters for domain + relationship routes
#[derive(Deserialize)]
pub struct DomainRelationshipPath {
    pub domain: String,
    pub relationship_id: String,
}

/// Path parameters for domain + export format routes
#[derive(Deserialize)]
pub struct DomainExportPath {
    pub domain: String,
    pub format: String,
}

/// Helper function to serialize table with database_type as "PostgreSQL" instead of "POSTGRES"
/// and medallion_layers with proper capitalization
fn serialize_table_with_database_type(table: &crate::models::table::Table) -> Value {
    let mut table_json = serde_json::to_value(table).unwrap_or(json!({}));

    // Convert database_type from enum to display string
    if let Some(db_type) = table.database_type {
        let db_type_str = match db_type {
            DatabaseType::Postgres => "PostgreSQL",
            DatabaseType::Mysql => "MySQL",
            DatabaseType::SqlServer => "SQL Server",
            DatabaseType::DatabricksDelta => "Databricks Delta",
            DatabaseType::DatabricksIceberg => "Databricks Iceberg",
            DatabaseType::AwsGlue => "AWS Glue",
            DatabaseType::DatabricksLakebase => "Databricks Lakebase",
            DatabaseType::Dynamodb => "DynamoDB",
            DatabaseType::Cassandra => "Cassandra",
            DatabaseType::Kafka => "Kafka",
            DatabaseType::Pulsar => "Pulsar",
        };
        table_json["database_type"] = json!(db_type_str);
    }

    // Convert medallion_layers to capitalized strings
    if let Some(layers) = table_json
        .get("medallion_layers")
        .and_then(|v| v.as_array())
    {
        let capitalized_layers: Vec<Value> = layers
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| match s.to_lowercase().as_str() {
                "bronze" => "Bronze",
                "silver" => "Silver",
                "gold" => "Gold",
                "operational" => "Operational",
                _ => s,
            })
            .map(|s| json!(s))
            .collect();
        table_json["medallion_layers"] = json!(capitalized_layers);
    }

    table_json
}

/// GET /workspace/domains/{domain}/tables - Get all tables in a domain
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/tables",
    tag = "Tables",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "List of tables retrieved successfully", body = Object),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_domain_tables(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_tables(ctx.domain_info.id).await {
            Ok(tables) => {
                let tables_json: Vec<Value> = tables
                    .iter()
                    .map(serialize_table_with_database_type)
                    .collect();
                return Ok(Json(json!({"tables": tables_json})));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let model_service = state.model_service.lock().await;
    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!({"tables": []}))),
    };

    let tables_json: Vec<Value> = model
        .tables
        .iter()
        .map(serialize_table_with_database_type)
        .collect();

    Ok(Json(json!({"tables": tables_json})))
}

/// POST /workspace/domains/{domain}/tables - Create a new table in a domain
#[utoipa::path(
    post,
    path = "/workspace/domains/{domain}/tables",
    tag = "Tables",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    request_body = CreateTableRequest,
    responses(
        (status = 200, description = "Table created successfully", body = Object),
        (status = 400, description = "Bad request - invalid table data"),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    request: Result<Json<CreateTableRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, StatusCode> {
    let request = request.map_err(|_| StatusCode::BAD_REQUEST)?;
    let request = request.0;
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Validate required fields
    if request.name.trim().is_empty() || request.columns.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse columns
    let mut columns: Vec<Column> = Vec::new();
    for (idx, col_data) in request.columns.iter().enumerate() {
        match serde_json::from_value::<Column>(col_data.clone()) {
            Ok(mut col) => {
                col.column_order = idx as i32;
                columns.push(col);
            }
            Err(_) => {
                // Fallback: extract basic fields
                let name = col_data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let data_type = col_data
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("STRING")
                    .to_string();
                if !name.is_empty() {
                    let mut col = Column::new(name, data_type);
                    col.column_order = idx as i32;
                    columns.push(col);
                }
            }
        }
    }

    if columns.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse medallion layers
    let medallion_layers = if !request.medallion_layers.is_empty() {
        request
            .medallion_layers
            .iter()
            .filter_map(|s| match s.to_lowercase().as_str() {
                "bronze" => Some(MedallionLayer::Bronze),
                "silver" => Some(MedallionLayer::Silver),
                "gold" => Some(MedallionLayer::Gold),
                "operational" => Some(MedallionLayer::Operational),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };

    // Parse database type
    let database_type =
        request
            .database_type
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "POSTGRES" | "POSTGRESQL" => Some(DatabaseType::Postgres),
                "MYSQL" => Some(DatabaseType::Mysql),
                "SQL_SERVER" | "SQLSERVER" => Some(DatabaseType::SqlServer),
                "DATABRICKS" | "DATABRICKS_DELTA" => Some(DatabaseType::DatabricksDelta),
                "AWS_GLUE" | "GLUE" => Some(DatabaseType::AwsGlue),
                _ => None,
            });

    // Parse SCD pattern
    let scd_pattern = request
        .scd_pattern
        .as_ref()
        .and_then(|s| match s.to_uppercase().as_str() {
            "TYPE_1" => Some(SCDPattern::Type1),
            "TYPE_2" => Some(SCDPattern::Type2),
            _ => None,
        });

    // Parse Data Vault classification
    let data_vault_classification =
        request
            .data_vault_classification
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "HUB" => Some(DataVaultClassification::Hub),
                "LINK" => Some(DataVaultClassification::Link),
                "SATELLITE" => Some(DataVaultClassification::Satellite),
                _ => None,
            });

    // Parse modeling level
    let modeling_level =
        request
            .modeling_level
            .as_ref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "conceptual" => Some(ModelingLevel::Conceptual),
                "logical" => Some(ModelingLevel::Logical),
                "physical" => Some(ModelingLevel::Physical),
                _ => None,
            });

    // Parse position
    let position = request.position.and_then(|pos_val| {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            Some(Position { x, y })
        } else {
            None
        }
    });

    let table = Table {
        id: Uuid::new_v4(),
        name: request.name.trim().to_string(),
        columns,
        database_type,
        catalog_name: request.catalog_name.filter(|s| !s.trim().is_empty()),
        schema_name: request.schema_name.filter(|s| !s.trim().is_empty()),
        medallion_layers,
        scd_pattern,
        data_vault_classification,
        modeling_level,
        tags: request.tags,
        odcl_metadata: request.odcl_metadata,
        position,
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .create_table(ctx.domain_info.id, table.clone(), &ctx.user_context)
            .await
        {
            Ok(created_table) => {
                return Ok(Json(serialize_table_with_database_type(&created_table)));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;
    match model_service.add_table(table.clone()) {
        Ok(added_table) => Ok(Json(serialize_table_with_database_type(&added_table))),
        Err(e) => {
            warn!("Failed to add table: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /workspace/domains/{domain}/tables/{table_id} - Get a single table
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/tables/{table_id}",
    tag = "Tables",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("table_id" = String, Path, description = "Table UUID")
    ),
    responses(
        (status = 200, description = "Table retrieved successfully", body = Object),
        (status = 404, description = "Table not found"),
        (status = 400, description = "Bad request - invalid table ID"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
async fn get_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_table(ctx.domain_info.id, table_uuid).await {
            Ok(Some(table)) => {
                // Verify table belongs to this domain
                // Note: get_table doesn't check domain, so we verify by checking if it's in domain's tables
                match storage.get_tables(ctx.domain_info.id).await {
                    Ok(tables) => {
                        if tables.iter().any(|t| t.id == table_uuid) {
                            return Ok(Json(serialize_table_with_database_type(&table)));
                        } else {
                            return Err(StatusCode::NOT_FOUND);
                        }
                    }
                    Err(_) => {
                        // If we can't verify, return the table anyway (it was found by ID)
                        return Ok(Json(serialize_table_with_database_type(&table)));
                    }
                }
            }
            Ok(None) => return Err(StatusCode::NOT_FOUND),
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    // ctx already ensures domain is loaded, so model_service should have the model
    let model_service = state.model_service.lock().await;
    let table = model_service
        .get_table(table_uuid)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serialize_table_with_database_type(table)))
}

/// PUT /workspace/domains/{domain}/tables/{table_id} - Update a table
#[utoipa::path(
    put,
    path = "/workspace/domains/{domain}/tables/{table_id}",
    tag = "Tables",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("table_id" = String, Path, description = "Table UUID")
    ),
    request_body(content = Object, description = "Table update fields"),
    responses(
        (status = 200, description = "Table updated successfully", body = Object),
        (status = 404, description = "Table not found"),
        (status = 400, description = "Bad request - invalid table ID or update data"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
    Json(updates): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Get existing table
        match storage.get_table(ctx.domain_info.id, table_uuid).await {
            Ok(Some(mut table)) => {
                // Apply updates to the table
                if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
                    table.name = name.to_string();
                }
                if let Some(columns) = updates.get("columns")
                    && let Ok(parsed_columns) =
                        serde_json::from_value::<Vec<Column>>(columns.clone())
                {
                    table.columns = parsed_columns;
                }
                if let Some(position) = updates.get("position")
                    && let (Some(x), Some(y)) = (
                        position.get("x").and_then(|v| v.as_f64()),
                        position.get("y").and_then(|v| v.as_f64()),
                    )
                {
                    table.position = Some(Position { x, y });
                }
                table.updated_at = chrono::Utc::now();

                // Get version from updates for optimistic locking
                let expected_version = updates
                    .get("version")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32);

                match storage
                    .update_table(table, expected_version, &ctx.user_context)
                    .await
                {
                    Ok(updated_table) => {
                        return Ok(Json(serialize_table_with_database_type(&updated_table)));
                    }
                    Err(StorageError::VersionConflict { .. }) => {
                        return Err(StatusCode::CONFLICT);
                    }
                    Err(e) => {
                        warn!("Storage backend failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }
            Ok(None) => return Err(StatusCode::NOT_FOUND),
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;
    match model_service.update_table(table_uuid, &updates) {
        Ok(Some(table)) => Ok(Json(serialize_table_with_database_type(&table))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!("Failed to update table: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// DELETE /workspace/domains/{domain}/tables/{table_id} - Delete a table
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}/tables/{table_id}",
    tag = "Tables",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("table_id" = String, Path, description = "Table UUID")
    ),
    responses(
        (status = 200, description = "Table deleted successfully", body = Object),
        (status = 404, description = "Table not found"),
        (status = 400, description = "Bad request - invalid table ID"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Verify table exists and belongs to this domain before deleting
        match storage.get_tables(ctx.domain_info.id).await {
            Ok(tables) => {
                if !tables.iter().any(|t| t.id == table_uuid) {
                    return Err(StatusCode::NOT_FOUND);
                }
            }
            Err(_) => {
                // If we can't verify, continue with delete attempt
            }
        }

        match storage
            .delete_table(ctx.domain_info.id, table_uuid, &ctx.user_context)
            .await
        {
            Ok(()) => {
                return Ok(Json(json!({"message": "Table deleted successfully"})));
            }
            Err(StorageError::NotFound { .. }) => {
                return Err(StatusCode::NOT_FOUND);
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    // ctx already ensures domain is loaded, so model_service should have the model
    let mut model_service = state.model_service.lock().await;
    // Check if table exists first
    if model_service.get_table(table_uuid).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    match model_service.delete_table(table_uuid) {
        Ok(true) => Ok(Json(json!({"message": "Table deleted successfully"}))),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

// ============================================================================
// Domain-scoped Relationship CRUD handlers
// ============================================================================

use crate::models::enums::{Cardinality, RelationshipType};
use crate::models::relationship::{ETLJobMetadata, ForeignKeyDetails};
use crate::services::RelationshipService;

/// Request to create a relationship
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRelationshipRequest {
    pub source_table_id: String,
    pub target_table_id: String,
    #[serde(default)]
    pub cardinality: Option<String>,
    #[serde(default)]
    pub foreign_key_details: Option<Value>,
    #[serde(default)]
    pub etl_job_metadata: Option<Value>,
    #[serde(default)]
    pub relationship_type: Option<String>,
}

/// Request to update a relationship
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRelationshipRequest {
    #[serde(default)]
    pub cardinality: Option<String>,
    #[serde(default)]
    pub source_optional: Option<bool>,
    #[serde(default)]
    pub target_optional: Option<bool>,
    #[serde(default)]
    pub foreign_key_details: Option<Value>,
    #[serde(default)]
    pub etl_job_metadata: Option<Value>,
    #[serde(default)]
    pub relationship_type: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// GET /workspace/domains/{domain}/relationships - Get all relationships in a domain
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/relationships",
    tag = "Relationships",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "List of relationships retrieved successfully", body = Object),
        (status = 404, description = "Domain not found"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_relationships(ctx.domain_info.id).await {
            Ok(relationships) => {
                let relationships_json: Vec<Value> = relationships
                    .iter()
                    .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
                    .collect();
                return Ok(Json(json!(relationships_json)));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let model_service = state.model_service.lock().await;
    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!([]))),
    };

    let relationships_json: Vec<Value> = model
        .relationships
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .collect();

    Ok(Json(json!(relationships_json)))
}

/// POST /workspace/domains/{domain}/relationships - Create a new relationship
#[utoipa::path(
    post,
    path = "/workspace/domains/{domain}/relationships",
    tag = "Relationships",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    request_body = CreateRelationshipRequest,
    responses(
        (status = 200, description = "Relationship created successfully", body = Object),
        (status = 400, description = "Bad request - invalid relationship data"),
        (status = 404, description = "Domain or referenced tables not found"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    Json(request): Json<CreateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    use crate::models::relationship::Relationship;

    // Force reload from disk to ensure we have latest tables (which are auto-saved)
    let ctx = ensure_domain_loaded_with_reload(&state, &headers, &path.domain, true).await?;

    let source_table_id =
        Uuid::parse_str(&request.source_table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let target_table_id =
        Uuid::parse_str(&request.target_table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Parse cardinality
    let cardinality = request.cardinality.as_ref().and_then(|s| match s.as_str() {
        "OneToOne" => Some(Cardinality::OneToOne),
        "OneToMany" => Some(Cardinality::OneToMany),
        "ManyToOne" => Some(Cardinality::ManyToOne),
        "ManyToMany" => Some(Cardinality::ManyToMany),
        _ => None,
    });

    // Parse relationship type
    let relationship_type = request
        .relationship_type
        .as_ref()
        .and_then(|s| match s.as_str() {
            "DataFlow" => Some(RelationshipType::DataFlow),
            "Dependency" => Some(RelationshipType::Dependency),
            "ForeignKey" => Some(RelationshipType::ForeignKey),
            "EtlTransformation" => Some(RelationshipType::EtlTransformation),
            _ => None,
        });

    let foreign_key_details = request
        .foreign_key_details
        .as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());

    let etl_job_metadata = request
        .etl_job_metadata
        .as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Check for duplicate
        match storage.get_relationships(ctx.domain_info.id).await {
            Ok(relationships) => {
                if relationships.iter().any(|r| {
                    r.source_table_id == source_table_id && r.target_table_id == target_table_id
                }) {
                    return Err(StatusCode::CONFLICT);
                }
            }
            Err(e) => {
                warn!("Failed to check for duplicate relationships: {}", e);
            }
        }

        // Create relationship
        let relationship = Relationship {
            id: Uuid::new_v4(),
            source_table_id,
            target_table_id,
            cardinality,
            source_optional: Some(false),
            target_optional: Some(false),
            foreign_key_details: foreign_key_details.clone(),
            etl_job_metadata: etl_job_metadata.clone(),
            relationship_type,
            visual_metadata: None,
            notes: None,
            drawio_edge_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        match storage
            .create_relationship(ctx.domain_info.id, relationship, &ctx.user_context)
            .await
        {
            Ok(created_relationship) => {
                return Ok(Json(
                    serde_json::to_value(created_relationship).unwrap_or(json!({})),
                ));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model_mut()
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Validate tables exist before creating relationship
    if model.get_table_by_id(source_table_id).is_none() {
        warn!("Source table {} not found in model", source_table_id);
        return Err(StatusCode::BAD_REQUEST);
    }
    if model.get_table_by_id(target_table_id).is_none() {
        warn!("Target table {} not found in model", target_table_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for duplicate
    if model
        .relationships
        .iter()
        .any(|r| r.source_table_id == source_table_id && r.target_table_id == target_table_id)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut rel_service = RelationshipService::new(Some(model.clone()));

    // Check circular dependency
    if let Ok((is_circular, _)) =
        rel_service.check_circular_dependency(source_table_id, target_table_id)
        && is_circular
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    rel_service.set_model(model.clone());

    match rel_service.create_relationship(
        source_table_id,
        target_table_id,
        cardinality,
        foreign_key_details,
        etl_job_metadata,
        relationship_type,
    ) {
        Ok(relationship) => {
            model.relationships.push(relationship.clone());

            // Auto-save relationships to YAML file (similar to how tables are auto-saved)
            let git_directory_path = model.git_directory_path.clone();
            if !git_directory_path.is_empty() {
                use crate::services::git_service::GitService;
                use std::path::Path;

                let mut git_service = GitService::new();
                if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
                    warn!("Failed to set git directory for relationship save: {}", e);
                } else {
                    // Save all relationships including the newly created one
                    if let Err(e) =
                        git_service.save_relationships_to_yaml(&model.relationships, &model.tables)
                    {
                        warn!("Failed to auto-save relationships to YAML: {}", e);
                    } else {
                        info!(
                            "Auto-saved {} relationships to YAML",
                            model.relationships.len()
                        );
                    }
                }
            }

            Ok(Json(
                serde_json::to_value(relationship).unwrap_or(json!({})),
            ))
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// GET /workspace/domains/{domain}/relationships/{relationship_id} - Get a single relationship
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/relationships/{relationship_id}",
    tag = "Relationships",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("relationship_id" = String, Path, description = "Relationship UUID")
    ),
    responses(
        (status = 200, description = "Relationship retrieved successfully", body = Object),
        (status = 404, description = "Relationship not found"),
        (status = 400, description = "Bad request - invalid relationship ID"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
async fn get_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let relationship_uuid =
        Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .get_relationship(ctx.domain_info.id, relationship_uuid)
            .await
        {
            Ok(Some(relationship)) => {
                return Ok(Json(
                    serde_json::to_value(relationship).unwrap_or(json!({})),
                ));
            }
            Ok(None) => return Err(StatusCode::NOT_FOUND),
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service
        .get_relationship(relationship_uuid)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(
        serde_json::to_value(relationship).unwrap_or(json!({})),
    ))
}

/// PUT /workspace/domains/{domain}/relationships/{relationship_id} - Update a relationship
#[utoipa::path(
    put,
    path = "/workspace/domains/{domain}/relationships/{relationship_id}",
    tag = "Relationships",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("relationship_id" = String, Path, description = "Relationship UUID")
    ),
    request_body = UpdateRelationshipRequest,
    responses(
        (status = 200, description = "Relationship updated successfully", body = Object),
        (status = 404, description = "Relationship not found"),
        (status = 400, description = "Bad request - invalid relationship ID or update data"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
    Json(request): Json<UpdateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let relationship_uuid =
        Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Parse cardinality
    let cardinality = request.cardinality.as_ref().and_then(|s| {
        if s.is_empty() {
            None
        } else {
            match s.as_str() {
                "OneToOne" => Some(Cardinality::OneToOne),
                "OneToMany" => Some(Cardinality::OneToMany),
                "ManyToOne" => Some(Cardinality::ManyToOne),
                "ManyToMany" => Some(Cardinality::ManyToMany),
                _ => None,
            }
        }
    });

    let relationship_type = request
        .relationship_type
        .as_ref()
        .and_then(|s| match s.as_str() {
            "DataFlow" => Some(RelationshipType::DataFlow),
            "Dependency" => Some(RelationshipType::Dependency),
            "ForeignKey" => Some(RelationshipType::ForeignKey),
            "EtlTransformation" => Some(RelationshipType::EtlTransformation),
            _ => None,
        });

    let foreign_key_details = request
        .foreign_key_details
        .as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());

    let etl_job_metadata = request
        .etl_job_metadata
        .as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .get_relationship(ctx.domain_info.id, relationship_uuid)
            .await
        {
            Ok(Some(mut relationship)) => {
                // Apply updates
                if request.cardinality.is_some() {
                    relationship.cardinality = cardinality;
                }
                if let Some(source_optional) = request.source_optional {
                    relationship.source_optional = Some(source_optional);
                }
                if let Some(target_optional) = request.target_optional {
                    relationship.target_optional = Some(target_optional);
                }
                if request.foreign_key_details.is_some() {
                    relationship.foreign_key_details = foreign_key_details.clone();
                }
                if request.etl_job_metadata.is_some() {
                    relationship.etl_job_metadata = etl_job_metadata.clone();
                }
                if request.relationship_type.is_some() {
                    relationship.relationship_type = relationship_type;
                }
                if let Some(ref notes) = request.notes {
                    relationship.notes = if notes.is_empty() {
                        None
                    } else {
                        Some(notes.clone())
                    };
                }
                relationship.updated_at = chrono::Utc::now();

                match storage
                    .update_relationship(relationship, None, &ctx.user_context)
                    .await
                {
                    Ok(updated_relationship) => {
                        return Ok(Json(
                            serde_json::to_value(updated_relationship).unwrap_or(json!({})),
                        ));
                    }
                    Err(e) => {
                        warn!("Storage backend failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }
            Ok(None) => return Err(StatusCode::NOT_FOUND),
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model_mut()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let mut rel_service = RelationshipService::new(Some(model.clone()));

    // Parse cardinality with Option<Option<Cardinality>> semantics for file-based fallback
    let cardinality_option: Option<Option<Cardinality>> = if request.cardinality.is_some() {
        let card_value = request.cardinality.as_ref().unwrap();
        if card_value.is_empty() {
            Some(None)
        } else {
            Some(match card_value.as_str() {
                "OneToOne" => Some(Cardinality::OneToOne),
                "OneToMany" => Some(Cardinality::OneToMany),
                "ManyToOne" => Some(Cardinality::ManyToOne),
                "ManyToMany" => Some(Cardinality::ManyToMany),
                _ => None,
            })
        }
    } else {
        None
    };

    rel_service.set_model(model.clone());

    match rel_service.update_relationship(
        relationship_uuid,
        cardinality_option,
        request.source_optional,
        request.target_optional,
        foreign_key_details,
        etl_job_metadata,
        relationship_type,
        request.notes.clone(),
    ) {
        Ok(Some(relationship)) => {
            // Update in model
            if let Some(existing) = model
                .relationships
                .iter_mut()
                .find(|r| r.id == relationship_uuid)
            {
                *existing = relationship.clone();
            }

            // Auto-save relationships to YAML file (similar to how tables are auto-saved)
            let git_directory_path = model.git_directory_path.clone();
            if !git_directory_path.is_empty() {
                use crate::services::git_service::GitService;
                use std::path::Path;

                let mut git_service = GitService::new();
                if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
                    warn!("Failed to set git directory for relationship save: {}", e);
                } else {
                    // Save all relationships including the updated one
                    if let Err(e) =
                        git_service.save_relationships_to_yaml(&model.relationships, &model.tables)
                    {
                        warn!("Failed to auto-save relationships to YAML: {}", e);
                    } else {
                        info!(
                            "Auto-saved {} relationships to YAML after update",
                            model.relationships.len()
                        );
                    }
                }
            }

            Ok(Json(
                serde_json::to_value(relationship).unwrap_or(json!({})),
            ))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// DELETE /workspace/domains/{domain}/relationships/{relationship_id} - Delete a relationship
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}/relationships/{relationship_id}",
    tag = "Relationships",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("relationship_id" = String, Path, description = "Relationship UUID")
    ),
    responses(
        (status = 200, description = "Relationship deleted successfully", body = Object),
        (status = 404, description = "Relationship not found"),
        (status = 400, description = "Bad request - invalid relationship ID"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let relationship_uuid =
        Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage
            .delete_relationship(ctx.domain_info.id, relationship_uuid, &ctx.user_context)
            .await
        {
            Ok(()) => {
                return Ok(Json(json!({"message": "Relationship deleted"})));
            }
            Err(StorageError::NotFound { .. }) => {
                return Err(StatusCode::NOT_FOUND);
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let mut model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model_mut()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let mut rel_service = RelationshipService::new(Some(model.clone()));
    rel_service.set_model(model.clone());

    match rel_service.delete_relationship(relationship_uuid) {
        Ok(true) => {
            model.relationships.retain(|r| r.id != relationship_uuid);

            // Auto-save relationships to YAML file after deletion
            let git_directory_path = model.git_directory_path.clone();
            if !git_directory_path.is_empty() {
                use crate::services::git_service::GitService;
                use std::path::Path;

                let mut git_service = GitService::new();
                if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
                    warn!("Failed to set git directory for relationship save: {}", e);
                } else {
                    // Save all remaining relationships
                    if let Err(e) =
                        git_service.save_relationships_to_yaml(&model.relationships, &model.tables)
                    {
                        warn!("Failed to auto-save relationships to YAML: {}", e);
                    } else {
                        info!(
                            "Auto-saved {} relationships to YAML after deletion",
                            model.relationships.len()
                        );
                    }
                }
            }

            Ok(Json(json!({"message": "Relationship deleted"})))
        }
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

// ============================================================================
// Cross-Domain Reference handlers
// ============================================================================

use data_modelling_sdk::models::{CrossDomainConfig, CrossDomainTableRef, Position as SdkPosition};

/// Request to add a cross-domain table reference
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AddCrossDomainTableRequest {
    source_domain: String,
    table_id: String,
    #[serde(default)]
    display_alias: Option<String>,
    #[serde(default)]
    position: Option<Value>,
    #[serde(default)]
    notes: Option<String>,
}

/// Request to update a cross-domain table reference
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateCrossDomainTableRequest {
    #[serde(default)]
    display_alias: Option<String>,
    #[serde(default)]
    position: Option<Value>,
    #[serde(default)]
    notes: Option<String>,
}

/// Response for canvas view (combined domain + imported tables)
#[derive(Serialize, utoipa::ToSchema)]
pub struct CanvasResponse {
    /// Tables owned by this domain
    owned_tables: Vec<Value>,
    /// Tables imported from other domains (read-only in this domain)
    imported_tables: Vec<ImportedTableInfo>,
    /// Relationships owned by this domain
    owned_relationships: Vec<Value>,
    /// Relationships imported from other domains (read-only)
    imported_relationships: Vec<ImportedRelationshipInfo>,
}

/// Info about an imported table
#[derive(Serialize, utoipa::ToSchema)]
pub struct ImportedTableInfo {
    /// The table data
    table: Value,
    /// Domain that owns this table
    source_domain: String,
    /// Reference ID in this domain's cross-domain config
    reference_id: String,
    /// Optional display alias
    display_alias: Option<String>,
    /// Position override for this domain
    position_override: Option<Value>,
    /// Notes about why imported
    notes: Option<String>,
}

/// Info about an imported relationship
#[derive(Serialize, utoipa::ToSchema)]
pub struct ImportedRelationshipInfo {
    /// The relationship data
    relationship: Value,
    /// Domain that owns this relationship
    source_domain: String,
    /// Reference ID
    reference_id: String,
}

/// Get path to cross-domain config file
fn get_cross_domain_config_path(email: &str, domain: &str) -> Result<PathBuf, StatusCode> {
    let workspace_data_dir =
        get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(email);
    Ok(workspace_data_dir
        .join(&sanitized_email)
        .join(domain)
        .join("cross_domain.yaml"))
}

/// Load cross-domain config from file
fn load_cross_domain_config(path: &PathBuf) -> CrossDomainConfig {
    if path.exists()
        && let Ok(content) = std::fs::read_to_string(path)
        && let Ok(config) = serde_yaml::from_str(&content)
    {
        return config;
    }
    CrossDomainConfig::default()
}

/// Save cross-domain config to file
fn save_cross_domain_config(path: &PathBuf, config: &CrossDomainConfig) -> Result<(), StatusCode> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let yaml = serde_yaml::to_string(config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(path, yaml).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

/// GET /workspace/domains/{domain}/cross-domain - Get cross-domain configuration
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/cross-domain",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Cross-domain configuration retrieved successfully", body = CrossDomainConfig),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_cross_domain_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<CrossDomainConfig>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_cross_domain_refs(ctx.domain_info.id).await {
            Ok(refs) => {
                // Convert to CrossDomainConfig format
                let imported_tables: Vec<CrossDomainTableRef> = refs
                    .iter()
                    .map(|r| {
                        let mut table_ref = CrossDomainTableRef::new(
                            format!("domain-{}", r.source_domain_id), // Domain name lookup can be added if needed
                            r.table_id,
                        );
                        table_ref.id = r.id;
                        table_ref.display_alias = r.display_alias.clone();
                        table_ref.notes = r.notes.clone();
                        if let Some(ref pos) = r.position {
                            table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
                        }
                        table_ref
                    })
                    .collect();

                let config = CrossDomainConfig {
                    schema_version: "1.0".to_string(),
                    imported_tables,
                    imported_relationships: Vec::new(), // Relationship refs can be added when cross-domain relationships are supported
                };
                return Ok(Json(config));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(config))
}

/// GET /workspace/domains/{domain}/cross-domain/tables - List imported tables
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/cross-domain/tables",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Imported tables retrieved successfully", body = Vec<CrossDomainTableRef>),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_cross_domain_tables(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Vec<CrossDomainTableRef>>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        match storage.get_cross_domain_refs(ctx.domain_info.id).await {
            Ok(refs) => {
                let imported_tables: Vec<CrossDomainTableRef> = refs
                    .iter()
                    .map(|r| {
                        let mut table_ref = CrossDomainTableRef::new(
                            format!("domain-{}", r.source_domain_id),
                            r.table_id,
                        );
                        table_ref.id = r.id;
                        table_ref.display_alias = r.display_alias.clone();
                        table_ref.notes = r.notes.clone();
                        if let Some(ref pos) = r.position {
                            table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
                        }
                        table_ref
                    })
                    .collect();
                return Ok(Json(imported_tables));
            }
            Err(e) => {
                warn!("Storage backend failed, falling back to file system: {}", e);
            }
        }
    }

    // File-based fallback
    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(config.imported_tables))
}

/// POST /workspace/domains/{domain}/cross-domain/tables - Add a table from another domain
#[utoipa::path(
    post,
    path = "/workspace/domains/{domain}/cross-domain/tables",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    request_body = AddCrossDomainTableRequest,
    responses(
        (status = 200, description = "Table imported successfully", body = CrossDomainTableRef),
        (status = 400, description = "Bad request - invalid table ID"),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain or source table not found"),
        (status = 409, description = "Conflict - table already imported"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_cross_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    Json(request): Json<AddCrossDomainTableRequest>,
) -> Result<Json<CrossDomainTableRef>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&request.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Parse position if provided
    let position = request.position.as_ref().and_then(|pos_val| {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            Some(PositionExport { x, y })
        } else {
            None
        }
    });

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Get source domain ID
        if let Ok(Some(source_domain_info)) = storage
            .get_domain_by_name(ctx.workspace.id, &request.source_domain)
            .await
        {
            // Check if already imported
            if let Ok(refs) = storage.get_cross_domain_refs(ctx.domain_info.id).await
                && refs.iter().any(|r| r.table_id == table_uuid)
            {
                warn!("Table {} already imported", table_uuid);
                return Err(StatusCode::CONFLICT);
            }

            match storage
                .add_cross_domain_ref(
                    ctx.domain_info.id,
                    source_domain_info.id,
                    table_uuid,
                    request.display_alias.clone(),
                    position.clone(),
                    request.notes.clone(),
                )
                .await
            {
                Ok(ref_info) => {
                    let mut table_ref =
                        CrossDomainTableRef::new(request.source_domain.clone(), table_uuid);
                    table_ref.id = ref_info.id;
                    table_ref.display_alias = ref_info.display_alias;
                    table_ref.notes = ref_info.notes;
                    if let Some(ref pos) = ref_info.position {
                        table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
                    }
                    info!(
                        "Added cross-domain table reference: {} from {} to {}",
                        table_uuid, request.source_domain, path.domain
                    );
                    return Ok(Json(table_ref));
                }
                Err(e) => {
                    warn!("Storage backend failed: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            warn!("Source domain not found: {}", request.source_domain);
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // File-based fallback
    let source_domain_path =
        get_user_workspace_path(&ctx.user_context.email)?.join(&request.source_domain);
    if !source_domain_path.exists() {
        warn!("Source domain does not exist: {}", request.source_domain);
        return Err(StatusCode::NOT_FOUND);
    }

    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);

    // Check if already imported
    if config
        .imported_tables
        .iter()
        .any(|t| t.table_id == table_uuid)
    {
        warn!(
            "Table {} already imported from {}",
            table_uuid, request.source_domain
        );
        return Err(StatusCode::CONFLICT);
    }

    // Create the reference
    let source_domain = request.source_domain.clone();
    let mut table_ref = CrossDomainTableRef::new(request.source_domain, table_uuid);
    table_ref.display_alias = request.display_alias;
    table_ref.notes = request.notes;

    if let Some(pos) = position {
        table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
    }

    config.imported_tables.push(table_ref.clone());
    save_cross_domain_config(&config_path, &config)?;

    info!(
        "Added cross-domain table reference: {} from {} to {}",
        table_uuid, source_domain, path.domain
    );

    Ok(Json(table_ref))
}

/// PUT /workspace/domains/{domain}/cross-domain/tables/{table_id} - Update a table reference
#[utoipa::path(
    put,
    path = "/workspace/domains/{domain}/cross-domain/tables/{table_id}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("table_id" = String, Path, description = "Table UUID")
    ),
    request_body = UpdateCrossDomainTableRequest,
    responses(
        (status = 200, description = "Table reference updated successfully", body = CrossDomainTableRef),
        (status = 400, description = "Bad request - invalid table ID"),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain or table reference not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_cross_domain_table_ref(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
    Json(request): Json<UpdateCrossDomainTableRequest>,
) -> Result<Json<CrossDomainTableRef>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Parse position if provided
    let position = request.position.as_ref().and_then(|pos_val| {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            Some(PositionExport { x, y })
        } else {
            None
        }
    });

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Find the reference by table_id
        if let Ok(refs) = storage.get_cross_domain_refs(ctx.domain_info.id).await {
            if let Some(ref_info) = refs.iter().find(|r| r.table_id == table_uuid) {
                let display_alias = match request.display_alias.as_ref() {
                    Some(a) if a.is_empty() => None,
                    Some(a) => Some(a.clone()),
                    None => ref_info.display_alias.clone(),
                };

                let notes = match request.notes.as_ref() {
                    Some(n) if n.is_empty() => None,
                    Some(n) => Some(n.clone()),
                    None => ref_info.notes.clone(),
                };

                match storage
                    .update_cross_domain_ref(ref_info.id, display_alias, position.clone(), notes)
                    .await
                {
                    Ok(updated_info) => {
                        let mut table_ref = CrossDomainTableRef::new(
                            format!("domain-{}", updated_info.source_domain_id),
                            updated_info.table_id,
                        );
                        table_ref.id = updated_info.id;
                        table_ref.display_alias = updated_info.display_alias;
                        table_ref.notes = updated_info.notes;
                        if let Some(ref pos) = updated_info.position {
                            table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
                        }
                        return Ok(Json(table_ref));
                    }
                    Err(e) => {
                        warn!("Storage backend failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            } else {
                return Err(StatusCode::NOT_FOUND);
            }
        }
    }

    // File-based fallback
    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);

    // Find and update the reference
    let table_ref = config
        .imported_tables
        .iter_mut()
        .find(|t| t.table_id == table_uuid)
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(alias) = request.display_alias {
        table_ref.display_alias = if alias.is_empty() { None } else { Some(alias) };
    }
    if let Some(notes) = request.notes {
        table_ref.notes = if notes.is_empty() { None } else { Some(notes) };
    }
    if let Some(pos) = position {
        table_ref.position = Some(SdkPosition { x: pos.x, y: pos.y });
    }

    let updated_ref = table_ref.clone();
    save_cross_domain_config(&config_path, &config)?;

    Ok(Json(updated_ref))
}

/// DELETE /workspace/domains/{domain}/cross-domain/tables/{table_id} - Remove a table reference
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}/cross-domain/tables/{table_id}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("table_id" = String, Path, description = "Table UUID")
    ),
    responses(
        (status = 200, description = "Table reference removed successfully", body = Object),
        (status = 400, description = "Bad request - invalid table ID"),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain or table reference not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove_cross_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try storage backend first (PostgreSQL)
    if let Some(storage) = state.storage.as_ref() {
        // Find the reference by table_id
        if let Ok(refs) = storage.get_cross_domain_refs(ctx.domain_info.id).await {
            if let Some(ref_info) = refs.iter().find(|r| r.table_id == table_uuid) {
                match storage.remove_cross_domain_ref(ref_info.id).await {
                    Ok(()) => {
                        info!(
                            "Removed cross-domain table reference: {} from {}",
                            table_uuid, path.domain
                        );
                        return Ok(Json(json!({"message": "Table reference removed"})));
                    }
                    Err(e) => {
                        warn!("Storage backend failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            } else {
                return Err(StatusCode::NOT_FOUND);
            }
        }
    }

    // File-based fallback
    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);

    if !config.remove_table_ref(table_uuid) {
        return Err(StatusCode::NOT_FOUND);
    }

    save_cross_domain_config(&config_path, &config)?;

    info!(
        "Removed cross-domain table reference: {} from {}",
        table_uuid, path.domain
    );

    Ok(Json(json!({"message": "Table reference removed"})))
}

/// GET /workspace/domains/{domain}/cross-domain/relationships - List imported relationships
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/cross-domain/relationships",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Imported relationships retrieved successfully", body = Object),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_cross_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;

    // Note: Cross-domain relationship references use a different structure than table references.
    // Table references (CrossDomainRef) are stored in PostgreSQL via get_cross_domain_refs().
    // Relationship references (ImportedRelationshipInfo) are currently file-based only.
    // This is intentional - relationship refs are managed differently and may be migrated to PostgreSQL in a future enhancement.

    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(
        serde_json::to_value(config.imported_relationships).unwrap_or(json!([])),
    ))
}

/// DELETE /workspace/domains/{domain}/cross-domain/relationships/{relationship_id} - Remove an imported relationship reference
#[utoipa::path(
    delete,
    path = "/workspace/domains/{domain}/cross-domain/relationships/{relationship_id}",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name"),
        ("relationship_id" = String, Path, description = "Relationship UUID")
    ),
    responses(
        (status = 200, description = "Relationship reference removed successfully", body = Object),
        (status = 400, description = "Bad request - invalid relationship ID"),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain or relationship reference not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove_cross_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    let ctx = ensure_domain_loaded(&state, &headers, &path.domain).await?;
    let relationship_uuid =
        Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Note: Cross-domain relationship references use a different structure than table references.
    // Table references (CrossDomainRef) are stored in PostgreSQL via remove_cross_domain_ref().
    // Relationship references (ImportedRelationshipInfo) are currently file-based only.
    // This is intentional - relationship refs are managed differently and may be migrated to PostgreSQL in a future enhancement.

    let config_path = get_cross_domain_config_path(&ctx.user_context.email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);

    if !config.remove_relationship_ref(relationship_uuid) {
        return Err(StatusCode::NOT_FOUND);
    }

    save_cross_domain_config(&config_path, &config)?;

    info!(
        "Removed cross-domain relationship reference: {} from {}",
        relationship_uuid, path.domain
    );

    Ok(Json(json!({"message": "Relationship reference removed"})))
}

/// POST /workspace/domains/{domain}/cross-domain/sync - Sync imported relationships
///
/// This automatically discovers and imports relationships from source domains
/// when both ends of the relationship are imported into this domain.
#[utoipa::path(
    post,
    path = "/workspace/domains/{domain}/cross-domain/sync",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Relationships synced successfully", body = Object),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_cross_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);

    let mut synced_count = 0;

    // Group imported tables by source domain
    let mut tables_by_domain: std::collections::HashMap<String, Vec<Uuid>> =
        std::collections::HashMap::new();
    for table_ref in &config.imported_tables {
        tables_by_domain
            .entry(table_ref.source_domain.clone())
            .or_default()
            .push(table_ref.table_id);
    }

    // For each source domain, load its relationships and find ones where both ends are imported
    for (source_domain, table_ids) in tables_by_domain {
        // Load the source domain's model
        let mut model_service = state.model_service.lock().await;
        if let Ok(_) =
            create_workspace_for_email_and_domain(&mut model_service, &email, &source_domain).await
            && let Some(model) = model_service.get_current_model()
        {
            for relationship in &model.relationships {
                // Check if both ends are in our imported tables
                if table_ids.contains(&relationship.source_table_id)
                    && table_ids.contains(&relationship.target_table_id)
                {
                    // Check if not already imported
                    if !config
                        .imported_relationships
                        .iter()
                        .any(|r| r.relationship_id == relationship.id)
                    {
                        config.add_relationship_ref(
                            source_domain.clone(),
                            relationship.id,
                            relationship.source_table_id,
                            relationship.target_table_id,
                        );
                        synced_count += 1;
                    }
                }
            }
        }
        drop(model_service);
    }

    if synced_count > 0 {
        save_cross_domain_config(&config_path, &config)?;
    }

    // Reload the current domain
    let mut model_service = state.model_service.lock().await;
    let _ = create_workspace_for_email_and_domain(&mut model_service, &email, &path.domain).await;

    info!(
        "Synced {} cross-domain relationships for domain {}",
        synced_count, path.domain
    );

    Ok(Json(json!({
        "message": format!("Synced {} relationships", synced_count),
        "synced_count": synced_count
    })))
}

/// GET /workspace/domains/{domain}/canvas - Get combined canvas view
///
/// Returns all tables and relationships for the domain canvas, including:
/// - Owned tables (editable)
/// - Imported tables from other domains (read-only)
/// - Owned relationships (editable)
/// - Imported relationships (read-only, between imported tables from same source domain)
#[utoipa::path(
    get,
    path = "/workspace/domains/{domain}/canvas",
    tag = "Workspace",
    params(
        ("domain" = String, Path, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Canvas view retrieved successfully", body = CanvasResponse),
        (status = 403, description = "Forbidden - domain access denied"),
        (status = 404, description = "Domain not found"),
        (status = 503, description = "Service unavailable - database not available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_domain_canvas(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<CanvasResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;

    // Load cross-domain config
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);

    // Load this domain's model
    ensure_domain_loaded(&state, &headers, &path.domain).await?;

    let model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model()
        .ok_or(StatusCode::NOT_FOUND)?;

    // Owned tables and relationships
    let owned_tables: Vec<Value> = model
        .tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    let owned_relationships: Vec<Value> = model
        .relationships
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .collect();

    drop(model_service);

    // Load imported tables from their source domains
    let mut imported_tables: Vec<ImportedTableInfo> = Vec::new();
    let mut imported_relationships: Vec<ImportedRelationshipInfo> = Vec::new();

    // Group by source domain for efficient loading
    let mut tables_by_domain: std::collections::HashMap<String, Vec<&CrossDomainTableRef>> =
        std::collections::HashMap::new();
    for table_ref in &config.imported_tables {
        tables_by_domain
            .entry(table_ref.source_domain.clone())
            .or_default()
            .push(table_ref);
    }

    for (source_domain, table_refs) in tables_by_domain {
        // Load source domain model
        let mut model_service = state.model_service.lock().await;
        if let Ok(_) =
            create_workspace_for_email_and_domain(&mut model_service, &email, &source_domain).await
            && let Some(source_model) = model_service.get_current_model()
        {
            for table_ref in table_refs {
                if let Some(table) = source_model
                    .tables
                    .iter()
                    .find(|t| t.id == table_ref.table_id)
                {
                    let mut table_json = serde_json::to_value(table).unwrap_or(json!({}));

                    // Apply position override if specified
                    if let Some(ref pos) = table_ref.position
                        && let Some(obj) = table_json.as_object_mut()
                    {
                        obj.insert("position".to_string(), json!({"x": pos.x, "y": pos.y}));
                    }

                    imported_tables.push(ImportedTableInfo {
                        table: table_json,
                        source_domain: source_domain.clone(),
                        reference_id: table_ref.id.to_string(),
                        display_alias: table_ref.display_alias.clone(),
                        position_override: table_ref
                            .position
                            .as_ref()
                            .map(|p| json!({"x": p.x, "y": p.y})),
                        notes: table_ref.notes.clone(),
                    });
                }
            }
        }
        drop(model_service);
    }

    // Load imported relationships
    for rel_ref in &config.imported_relationships {
        let mut model_service = state.model_service.lock().await;
        if let Ok(_) = create_workspace_for_email_and_domain(
            &mut model_service,
            &email,
            &rel_ref.source_domain,
        )
        .await
            && let Some(source_model) = model_service.get_current_model()
            && let Some(relationship) = source_model
                .relationships
                .iter()
                .find(|r| r.id == rel_ref.relationship_id)
        {
            imported_relationships.push(ImportedRelationshipInfo {
                relationship: serde_json::to_value(relationship).unwrap_or(json!({})),
                source_domain: rel_ref.source_domain.clone(),
                reference_id: rel_ref.id.to_string(),
            });
        }
        drop(model_service);
    }

    // Reload the current domain to restore context
    let mut model_service = state.model_service.lock().await;
    let _ = create_workspace_for_email_and_domain(&mut model_service, &email, &path.domain).await;

    Ok(Json(CanvasResponse {
        owned_tables,
        imported_tables,
        owned_relationships,
        imported_relationships,
    }))
}
