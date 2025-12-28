//! Workspace operations routes.
//! Handles user workspace creation and session management based on email.
//! 
//! All endpoints require JWT authentication via Authorization header.

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

use super::tables::AppState;
use axum::http::HeaderMap;
use crate::services::jwt_service::JwtService;

#[derive(Deserialize)]
struct CreateWorkspaceRequest {
    email: String,
    domain: String,
}

#[derive(Serialize)]
struct CreateWorkspaceResponse {
    workspace_path: String,
    message: String,
}

#[derive(Serialize)]
struct WorkspaceInfo {
    workspace_path: String,
    email: String,
}

/// Profile information for a user
#[derive(Serialize)]
pub struct ProfileInfo {
    pub email: String,
    pub domains: Vec<String>,
}

/// List of profiles response
#[derive(Serialize)]
struct ProfilesResponse {
    profiles: Vec<ProfileInfo>,
}

/// Request to load or create a specific domain
#[derive(Deserialize)]
struct DomainRequest {
    domain: String,
}

/// Response for domain operations
#[derive(Serialize)]
struct DomainResponse {
    domain: String,
    workspace_path: String,
    message: String,
}

/// Response for listing domains
#[derive(Serialize)]
struct DomainsListResponse {
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
        .route("/domains/{domain}/tables/{table_id}", axum::routing::put(update_domain_table))
        .route("/domains/{domain}/tables/{table_id}", axum::routing::delete(delete_domain_table))
        // Domain-scoped relationship CRUD endpoints
        .route("/domains/{domain}/relationships", get(get_domain_relationships))
        .route("/domains/{domain}/relationships", post(create_domain_relationship))
        .route("/domains/{domain}/relationships/{relationship_id}", get(get_domain_relationship))
        .route("/domains/{domain}/relationships/{relationship_id}", axum::routing::put(update_domain_relationship))
        .route("/domains/{domain}/relationships/{relationship_id}", axum::routing::delete(delete_domain_relationship))
        // Cross-domain reference endpoints
        .route("/domains/{domain}/cross-domain", get(get_cross_domain_config))
        .route("/domains/{domain}/cross-domain/tables", get(list_cross_domain_tables))
        .route("/domains/{domain}/cross-domain/tables", post(add_cross_domain_table))
        .route("/domains/{domain}/cross-domain/tables/{table_id}", axum::routing::delete(remove_cross_domain_table))
        .route("/domains/{domain}/cross-domain/tables/{table_id}", axum::routing::put(update_cross_domain_table_ref))
        .route("/domains/{domain}/cross-domain/relationships", get(list_cross_domain_relationships))
        .route("/domains/{domain}/cross-domain/relationships/{relationship_id}", axum::routing::delete(remove_cross_domain_relationship))
        .route("/domains/{domain}/cross-domain/sync", post(sync_cross_domain_relationships))
        // Combined view endpoint (domain tables + imported tables with ownership info)
        .route("/domains/{domain}/canvas", get(get_domain_canvas))
}

/// Get the workspace data directory from environment variable
fn get_workspace_data_dir() -> Result<PathBuf, String> {
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
fn sanitize_email_for_path(email: &str) -> String {
    // Replace invalid characters with safe alternatives
    email
        .replace('@', "_at_")
        .replace('.', "_")
        .replace('/', "_")
        .replace('\\', "_")
        .replace(':', "_")
        .replace('*', "_")
        .replace('?', "_")
        .replace('"', "_")
        .replace('<', "_")
        .replace('>', "_")
        .replace('|', "_")
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
            info!("Created/loaded workspace for user: {} domain: {} at {:?}", email, domain, user_workspace);
            Ok(user_workspace.to_string_lossy().to_string())
        }
        Err(e) => {
            Err(format!("Failed to create/load model in workspace: {}", e))
        }
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
async fn create_workspace(
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
    
    match create_workspace_for_email_and_domain(&mut model_service, &email, &domain).await {
        Ok(workspace_path) => {
            Ok(Json(CreateWorkspaceResponse {
                workspace_path,
                message: format!("Workspace ready for {} in domain {}", email, domain),
            }))
        }
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
        // Get session from store
        let sessions = app_state.session_store.lock().await;
        if let Some(session) = sessions.get(&session_id) {
            session.selected_email.clone()
        } else {
            None
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
        Err("No session or email available. Please authenticate and select an email first.".to_string())
    }
}

/// GET /workspace/info - Get current workspace information
async fn get_workspace_info(
    State(state): State<AppState>,
) -> Result<Json<WorkspaceInfo>, StatusCode> {
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
    
    Ok(Json(WorkspaceInfo {
        workspace_path: model.git_directory_path.clone(),
        email,
    }))
}

/// GET /workspace/profiles - List all profiles (email/domain combinations) for the authenticated user
async fn list_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ProfilesResponse>, StatusCode> {
    // Initialize JWT service and validate token
    let jwt_service = JwtService::from_env();
    
    // Try Authorization header first (preferred)
    let token = if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
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
        if user_workspace.exists() {
            if let Ok(entries) = std::fs::read_dir(&user_workspace) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                // Skip hidden directories and special directories
                                if !name.starts_with('.') && name != "tables" {
                                    domains.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        profiles.push(ProfileInfo {
            email: email.clone(),
            domains,
        });
    }
    
    info!("Listed {} profiles for session {}", profiles.len(), session_id);
    
    Ok(Json(ProfilesResponse { profiles }))
}

/// Helper to get session email from JWT token in headers
/// 
/// Validates the JWT token and returns the email (subject claim).
/// Supports both Authorization: Bearer <token> and x-session-id header.
async fn get_session_email(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, StatusCode> {
    // Initialize JWT service
    let jwt_service = JwtService::from_env();
    
    // Try Authorization header first (preferred)
    let token = if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
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
    
    // Verify session still exists in store
    let sessions = state.session_store.lock().await;
    if !sessions.contains_key(&claims.session_id) {
        warn!("Session {} not found in store", claims.session_id);
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(claims.sub)
}

/// Helper to get user workspace path
fn get_user_workspace_path(email: &str) -> Result<PathBuf, StatusCode> {
    let workspace_data_dir = get_workspace_data_dir()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(email);
    Ok(workspace_data_dir.join(&sanitized_email))
}

/// GET /workspace/domains - List all domains for the authenticated user
async fn list_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DomainsListResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let user_workspace = get_user_workspace_path(&email)?;
    
    let mut domains = Vec::new();
    if user_workspace.exists() {
        if let Ok(entries) = std::fs::read_dir(&user_workspace) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            // Skip hidden directories
                            if !name.starts_with('.') {
                                domains.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    
    domains.sort();
    info!("Listed {} domains for user {}", domains.len(), email);
    
    Ok(Json(DomainsListResponse { domains }))
}

/// POST /workspace/domains - Create a new domain for the authenticated user
async fn create_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DomainRequest>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    
    let domain = request.domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Validate domain name (alphanumeric, hyphens, underscores only)
    if !domain.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        warn!("Invalid domain name: {}", domain);
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let mut model_service = state.model_service.lock().await;
    
    match create_workspace_for_email_and_domain(&mut model_service, &email, domain).await {
        Ok(workspace_path) => {
            info!("Created domain {} for user {} at {}", domain, email, workspace_path);
            Ok(Json(DomainResponse {
                domain: domain.to_string(),
                workspace_path,
                message: format!("Created domain {}", domain),
            }))
        }
        Err(e) => {
            warn!("Failed to create domain: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Domain info response with metadata
#[derive(Serialize)]
struct DomainInfo {
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
struct UpdateDomainRequest {
    /// New name for the domain (rename)
    #[serde(default)]
    new_name: Option<String>,
}

/// GET /workspace/domains/:domain - Get domain info
async fn get_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
) -> Result<Json<DomainInfo>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let user_workspace = get_user_workspace_path(&email)?;
    
    let domain = domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let domain_path = user_workspace.join(domain);
    
    if !domain_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Count tables
    let tables_dir = domain_path.join("tables");
    let table_count = if tables_dir.exists() {
        std::fs::read_dir(&tables_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).filter(|e| {
                e.path().extension().map(|ext| ext == "yaml" || ext == "yml").unwrap_or(false)
            }).count())
            .unwrap_or(0)
    } else {
        0
    };
    
    // Count relationships
    let relationships_file = domain_path.join("relationships.yaml");
    let relationship_count = if relationships_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&relationships_file) {
            // Simple count: number of "- id:" lines
            content.lines().filter(|l| l.trim().starts_with("- id:")).count()
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
    let created_at = metadata.as_ref()
        .and_then(|m| m.created().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
    let modified_at = metadata
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
    
    Ok(Json(DomainInfo {
        name: domain.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        table_count,
        relationship_count,
        imported_table_count,
        created_at,
        modified_at,
    }))
}

/// PUT /workspace/domains/:domain - Update/rename a domain
async fn update_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
    Json(request): Json<UpdateDomainRequest>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let user_workspace = get_user_workspace_path(&email)?;
    
    let domain = domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let domain_path = user_workspace.join(domain);
    
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
        if !new_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
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
            warn!("Failed to rename domain {} to {}: {}", domain, new_name, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        
        info!("Renamed domain {} to {} for user {}", domain, new_name, email);
        
        return Ok(Json(DomainResponse {
            domain: new_name.to_string(),
            workspace_path: new_domain_path.to_string_lossy().to_string(),
            message: format!("Renamed domain {} to {}", domain, new_name),
        }));
    }
    
    // No changes requested
    Ok(Json(DomainResponse {
        domain: domain.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        message: "No changes".to_string(),
    }))
}

/// DELETE /workspace/domains/:domain - Delete a domain for the authenticated user
async fn delete_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
) -> Result<Json<DomainResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let user_workspace = get_user_workspace_path(&email)?;
    
    let domain = domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let domain_path = user_workspace.join(domain);
    
    // Check if domain exists
    if !domain_path.exists() {
        warn!("Domain not found: {} for user {}", domain, email);
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Delete the domain directory
    if let Err(e) = std::fs::remove_dir_all(&domain_path) {
        warn!("Failed to delete domain {}: {}", domain, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    info!("Deleted domain {} for user {}", domain, email);
    
    Ok(Json(DomainResponse {
        domain: domain.to_string(),
        workspace_path: domain_path.to_string_lossy().to_string(),
        message: format!("Deleted domain {}", domain),
    }))
}

/// POST /workspace/load-domain - Load a specific domain for the authenticated user
async fn load_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DomainRequest>,
) -> Result<Json<CreateWorkspaceResponse>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    
    let domain = request.domain.trim();
    if domain.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Load workspace for this email and domain
    let mut model_service = state.model_service.lock().await;
    
    match create_workspace_for_email_and_domain(&mut model_service, &email, domain).await {
        Ok(workspace_path) => {
            info!("Loaded domain {} for user {} at {}", domain, email, workspace_path);
            Ok(Json(CreateWorkspaceResponse {
                workspace_path,
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

use crate::models::{Column, Position, Table};
use crate::models::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, ModelingLevel, SCDPattern,
};
use serde_json::{json, Value};
use uuid::Uuid;

/// Request body for creating a table
#[derive(Debug, Deserialize)]
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

/// Helper to ensure domain is loaded for the current session
async fn ensure_domain_loaded(
    state: &AppState,
    headers: &HeaderMap,
    domain: &str,
) -> Result<String, StatusCode> {
    let email = get_session_email(state, headers).await?;
    let mut model_service = state.model_service.lock().await;
    
    create_workspace_for_email_and_domain(&mut model_service, &email, domain)
        .await
        .map_err(|e| {
            warn!("Failed to load domain {}: {}", domain, e);
            StatusCode::NOT_FOUND
        })
}

/// Path parameters for domain-scoped routes
#[derive(Deserialize)]
struct DomainPath {
    domain: String,
}

/// Path parameters for domain + table routes
#[derive(Deserialize)]
struct DomainTablePath {
    domain: String,
    table_id: String,
}

/// Path parameters for domain + relationship routes
#[derive(Deserialize)]
struct DomainRelationshipPath {
    domain: String,
    relationship_id: String,
}

/// GET /workspace/domains/{domain}/tables - Get all tables in a domain
async fn get_domain_tables(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let model_service = state.model_service.lock().await;
    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!([]))),
    };
    
    let tables_json: Vec<Value> = model.tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();
    
    Ok(Json(json!(tables_json)))
}

/// POST /workspace/domains/{domain}/tables - Create a new table in a domain
async fn create_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    
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
                let name = col_data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data_type = col_data.get("data_type").and_then(|v| v.as_str()).unwrap_or("STRING").to_string();
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
        request.medallion_layers.iter()
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
    let database_type = request.database_type.as_ref().and_then(|s| match s.to_uppercase().as_str() {
        "POSTGRES" | "POSTGRESQL" => Some(DatabaseType::Postgres),
        "MYSQL" => Some(DatabaseType::Mysql),
        "SQL_SERVER" | "SQLSERVER" => Some(DatabaseType::SqlServer),
        "DATABRICKS" | "DATABRICKS_DELTA" => Some(DatabaseType::DatabricksDelta),
        "AWS_GLUE" | "GLUE" => Some(DatabaseType::AwsGlue),
        _ => None,
    });
    
    // Parse SCD pattern
    let scd_pattern = request.scd_pattern.as_ref().and_then(|s| match s.to_uppercase().as_str() {
        "TYPE_1" => Some(SCDPattern::Type1),
        "TYPE_2" => Some(SCDPattern::Type2),
        _ => None,
    });
    
    // Parse Data Vault classification
    let data_vault_classification = request.data_vault_classification.as_ref().and_then(|s| match s.to_uppercase().as_str() {
        "HUB" => Some(DataVaultClassification::Hub),
        "LINK" => Some(DataVaultClassification::Link),
        "SATELLITE" => Some(DataVaultClassification::Satellite),
        _ => None,
    });
    
    // Parse modeling level
    let modeling_level = request.modeling_level.as_ref().and_then(|s| match s.to_lowercase().as_str() {
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
    
    match model_service.add_table(table.clone()) {
        Ok(added_table) => {
            let table_json = serde_json::to_value(&added_table).unwrap_or(json!({}));
            Ok(Json(table_json))
        }
        Err(e) => {
            warn!("Failed to add table: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /workspace/domains/{domain}/tables/{table_id} - Get a single table
async fn get_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let model_service = state.model_service.lock().await;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let table = model_service.get_table(table_uuid).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::to_value(table).unwrap_or(json!({}))))
}

/// PUT /workspace/domains/{domain}/tables/{table_id} - Update a table
async fn update_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
    Json(updates): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match model_service.update_table(table_uuid, &updates) {
        Ok(Some(table)) => {
            let table_json = serde_json::to_value(&table).unwrap_or(json!({}));
            Ok(Json(table_json))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!("Failed to update table: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// DELETE /workspace/domains/{domain}/tables/{table_id} - Delete a table
async fn delete_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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
async fn get_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let model_service = state.model_service.lock().await;
    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!([]))),
    };
    
    let relationships_json: Vec<Value> = model.relationships
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .collect();
    
    Ok(Json(json!(relationships_json)))
}

/// POST /workspace/domains/{domain}/relationships - Create a new relationship
async fn create_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    Json(request): Json<CreateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    let model = model_service.get_current_model_mut().ok_or(StatusCode::BAD_REQUEST)?;
    
    let source_table_id = Uuid::parse_str(&request.source_table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let target_table_id = Uuid::parse_str(&request.target_table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Check for duplicate
    if model.relationships.iter().any(|r| r.source_table_id == source_table_id && r.target_table_id == target_table_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    
    // Check circular dependency
    if let Ok((is_circular, _)) = rel_service.check_circular_dependency(source_table_id, target_table_id) {
        if is_circular {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    
    // Parse cardinality
    let cardinality = request.cardinality.as_ref().and_then(|s| match s.as_str() {
        "OneToOne" => Some(Cardinality::OneToOne),
        "OneToMany" => Some(Cardinality::OneToMany),
        "ManyToOne" => Some(Cardinality::ManyToOne),
        "ManyToMany" => Some(Cardinality::ManyToMany),
        _ => None,
    });
    
    // Parse relationship type
    let relationship_type = request.relationship_type.as_ref().and_then(|s| match s.as_str() {
        "DataFlow" => Some(RelationshipType::DataFlow),
        "Dependency" => Some(RelationshipType::Dependency),
        "ForeignKey" => Some(RelationshipType::ForeignKey),
        "EtlTransformation" => Some(RelationshipType::EtlTransformation),
        _ => None,
    });
    
    let foreign_key_details = request.foreign_key_details.as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());
    
    let etl_job_metadata = request.etl_job_metadata.as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());
    
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
            Ok(Json(serde_json::to_value(relationship).unwrap_or(json!({}))))
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// GET /workspace/domains/{domain}/relationships/{relationship_id} - Get a single relationship
async fn get_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let model_service = state.model_service.lock().await;
    let model = model_service.get_current_model().ok_or(StatusCode::BAD_REQUEST)?;
    
    let relationship_uuid = Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.get_relationship(relationship_uuid).ok_or(StatusCode::NOT_FOUND)?;
    
    Ok(Json(serde_json::to_value(relationship).unwrap_or(json!({}))))
}

/// PUT /workspace/domains/{domain}/relationships/{relationship_id} - Update a relationship
async fn update_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
    Json(request): Json<UpdateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    let model = model_service.get_current_model_mut().ok_or(StatusCode::BAD_REQUEST)?;
    
    let relationship_uuid = Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    
    // Parse cardinality with Option<Option<Cardinality>> semantics
    let cardinality: Option<Option<Cardinality>> = if request.cardinality.is_some() {
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
    
    let relationship_type = request.relationship_type.as_ref().and_then(|s| match s.as_str() {
        "DataFlow" => Some(RelationshipType::DataFlow),
        "Dependency" => Some(RelationshipType::Dependency),
        "ForeignKey" => Some(RelationshipType::ForeignKey),
        "EtlTransformation" => Some(RelationshipType::EtlTransformation),
        _ => None,
    });
    
    let foreign_key_details = request.foreign_key_details.as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());
    
    let etl_job_metadata = request.etl_job_metadata.as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());
    
    rel_service.set_model(model.clone());
    
    match rel_service.update_relationship(
        relationship_uuid,
        cardinality,
        request.source_optional,
        request.target_optional,
        foreign_key_details,
        etl_job_metadata,
        relationship_type,
        request.notes.clone(),
    ) {
        Ok(Some(relationship)) => {
            // Update in model
            if let Some(existing) = model.relationships.iter_mut().find(|r| r.id == relationship_uuid) {
                *existing = relationship.clone();
            }
            Ok(Json(serde_json::to_value(relationship).unwrap_or(json!({}))))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// DELETE /workspace/domains/{domain}/relationships/{relationship_id} - Delete a relationship
async fn delete_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    ensure_domain_loaded(&state, &headers, &path.domain).await?;
    
    let mut model_service = state.model_service.lock().await;
    let model = model_service.get_current_model_mut().ok_or(StatusCode::BAD_REQUEST)?;
    
    let relationship_uuid = Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    rel_service.set_model(model.clone());
    
    match rel_service.delete_relationship(relationship_uuid) {
        Ok(true) => {
            model.relationships.retain(|r| r.id != relationship_uuid);
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
#[derive(Debug, Deserialize)]
struct AddCrossDomainTableRequest {
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
#[derive(Debug, Deserialize)]
struct UpdateCrossDomainTableRequest {
    #[serde(default)]
    display_alias: Option<String>,
    #[serde(default)]
    position: Option<Value>,
    #[serde(default)]
    notes: Option<String>,
}

/// Response for canvas view (combined domain + imported tables)
#[derive(Serialize)]
struct CanvasResponse {
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
#[derive(Serialize)]
struct ImportedTableInfo {
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
#[derive(Serialize)]
struct ImportedRelationshipInfo {
    /// The relationship data
    relationship: Value,
    /// Domain that owns this relationship
    source_domain: String,
    /// Reference ID
    reference_id: String,
}

/// Get path to cross-domain config file
fn get_cross_domain_config_path(email: &str, domain: &str) -> Result<PathBuf, StatusCode> {
    let workspace_data_dir = get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(email);
    Ok(workspace_data_dir.join(&sanitized_email).join(domain).join("cross_domain.yaml"))
}

/// Load cross-domain config from file
fn load_cross_domain_config(path: &PathBuf) -> CrossDomainConfig {
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_yaml::from_str(&content) {
                return config;
            }
        }
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
async fn get_cross_domain_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<CrossDomainConfig>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(config))
}

/// GET /workspace/domains/{domain}/cross-domain/tables - List imported tables
async fn list_cross_domain_tables(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Vec<CrossDomainTableRef>>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(config.imported_tables))
}

/// POST /workspace/domains/{domain}/cross-domain/tables - Add a table from another domain
async fn add_cross_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
    Json(request): Json<AddCrossDomainTableRequest>,
) -> Result<Json<CrossDomainTableRef>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    
    // Validate source domain exists and table exists in it
    let source_domain_path = get_user_workspace_path(&email)?.join(&request.source_domain);
    if !source_domain_path.exists() {
        warn!("Source domain does not exist: {}", request.source_domain);
        return Err(StatusCode::NOT_FOUND);
    }
    
    let table_uuid = Uuid::parse_str(&request.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Load cross-domain config
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);
    
    // Check if already imported
    if config.imported_tables.iter().any(|t| t.table_id == table_uuid) {
        warn!("Table {} already imported from {}", table_uuid, request.source_domain);
        return Err(StatusCode::CONFLICT);
    }
    
    // Create the reference
    let source_domain = request.source_domain.clone();
    let mut table_ref = CrossDomainTableRef::new(request.source_domain, table_uuid);
    table_ref.display_alias = request.display_alias;
    table_ref.notes = request.notes;
    
    // Parse position if provided
    if let Some(pos_val) = request.position {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            table_ref.position = Some(SdkPosition { x, y });
        }
    }
    
    config.imported_tables.push(table_ref.clone());
    save_cross_domain_config(&config_path, &config)?;
    
    info!("Added cross-domain table reference: {} from {} to {}", table_uuid, source_domain, path.domain);
    
    Ok(Json(table_ref))
}

/// PUT /workspace/domains/{domain}/cross-domain/tables/{table_id} - Update a table reference
async fn update_cross_domain_table_ref(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
    Json(request): Json<UpdateCrossDomainTableRequest>,
) -> Result<Json<CrossDomainTableRef>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);
    
    // Find and update the reference
    let table_ref = config.imported_tables
        .iter_mut()
        .find(|t| t.table_id == table_uuid)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    if let Some(alias) = request.display_alias {
        table_ref.display_alias = if alias.is_empty() { None } else { Some(alias) };
    }
    if let Some(notes) = request.notes {
        table_ref.notes = if notes.is_empty() { None } else { Some(notes) };
    }
    if let Some(pos_val) = request.position {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            table_ref.position = Some(SdkPosition { x, y });
        }
    }
    
    let updated_ref = table_ref.clone();
    save_cross_domain_config(&config_path, &config)?;
    
    Ok(Json(updated_ref))
}

/// DELETE /workspace/domains/{domain}/cross-domain/tables/{table_id} - Remove a table reference
async fn remove_cross_domain_table(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainTablePath>,
) -> Result<Json<Value>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let table_uuid = Uuid::parse_str(&path.table_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);
    
    if !config.remove_table_ref(table_uuid) {
        return Err(StatusCode::NOT_FOUND);
    }
    
    save_cross_domain_config(&config_path, &config)?;
    
    info!("Removed cross-domain table reference: {} from {}", table_uuid, path.domain);
    
    Ok(Json(json!({"message": "Table reference removed"})))
}

/// GET /workspace/domains/{domain}/cross-domain/relationships - List imported relationships
async fn list_cross_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let config = load_cross_domain_config(&config_path);
    Ok(Json(serde_json::to_value(config.imported_relationships).unwrap_or(json!([]))))
}

/// DELETE /workspace/domains/{domain}/cross-domain/relationships/{relationship_id} - Remove an imported relationship reference
async fn remove_cross_domain_relationship(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainRelationshipPath>,
) -> Result<Json<Value>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let relationship_uuid = Uuid::parse_str(&path.relationship_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);
    
    if !config.remove_relationship_ref(relationship_uuid) {
        return Err(StatusCode::NOT_FOUND);
    }
    
    save_cross_domain_config(&config_path, &config)?;
    
    info!("Removed cross-domain relationship reference: {} from {}", relationship_uuid, path.domain);
    
    Ok(Json(json!({"message": "Relationship reference removed"})))
}

/// POST /workspace/domains/{domain}/cross-domain/sync - Sync imported relationships
/// 
/// This automatically discovers and imports relationships from source domains
/// when both ends of the relationship are imported into this domain.
async fn sync_cross_domain_relationships(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<DomainPath>,
) -> Result<Json<Value>, StatusCode> {
    let email = get_session_email(&state, &headers).await?;
    let config_path = get_cross_domain_config_path(&email, &path.domain)?;
    let mut config = load_cross_domain_config(&config_path);
    
    let mut synced_count = 0;
    
    // Group imported tables by source domain
    let mut tables_by_domain: std::collections::HashMap<String, Vec<Uuid>> = std::collections::HashMap::new();
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
        if let Ok(_) = create_workspace_for_email_and_domain(&mut model_service, &email, &source_domain).await {
            if let Some(model) = model_service.get_current_model() {
                for relationship in &model.relationships {
                    // Check if both ends are in our imported tables
                    if table_ids.contains(&relationship.source_table_id) && 
                       table_ids.contains(&relationship.target_table_id) {
                        // Check if not already imported
                        if !config.imported_relationships.iter().any(|r| r.relationship_id == relationship.id) {
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
        }
        drop(model_service);
    }
    
    if synced_count > 0 {
        save_cross_domain_config(&config_path, &config)?;
    }
    
    // Reload the current domain
    let mut model_service = state.model_service.lock().await;
    let _ = create_workspace_for_email_and_domain(&mut model_service, &email, &path.domain).await;
    
    info!("Synced {} cross-domain relationships for domain {}", synced_count, path.domain);
    
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
async fn get_domain_canvas(
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
    let model = model_service.get_current_model().ok_or(StatusCode::NOT_FOUND)?;
    
    // Owned tables and relationships
    let owned_tables: Vec<Value> = model.tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();
    
    let owned_relationships: Vec<Value> = model.relationships
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .collect();
    
    drop(model_service);
    
    // Load imported tables from their source domains
    let mut imported_tables: Vec<ImportedTableInfo> = Vec::new();
    let mut imported_relationships: Vec<ImportedRelationshipInfo> = Vec::new();
    
    // Group by source domain for efficient loading
    let mut tables_by_domain: std::collections::HashMap<String, Vec<&CrossDomainTableRef>> = std::collections::HashMap::new();
    for table_ref in &config.imported_tables {
        tables_by_domain
            .entry(table_ref.source_domain.clone())
            .or_default()
            .push(table_ref);
    }
    
    for (source_domain, table_refs) in tables_by_domain {
        // Load source domain model
        let mut model_service = state.model_service.lock().await;
        if let Ok(_) = create_workspace_for_email_and_domain(&mut model_service, &email, &source_domain).await {
            if let Some(source_model) = model_service.get_current_model() {
                for table_ref in table_refs {
                    if let Some(table) = source_model.tables.iter().find(|t| t.id == table_ref.table_id) {
                        let mut table_json = serde_json::to_value(table).unwrap_or(json!({}));
                        
                        // Apply position override if specified
                        if let Some(ref pos) = table_ref.position {
                            if let Some(obj) = table_json.as_object_mut() {
                                obj.insert("position".to_string(), json!({"x": pos.x, "y": pos.y}));
                            }
                        }
                        
                        imported_tables.push(ImportedTableInfo {
                            table: table_json,
                            source_domain: source_domain.clone(),
                            reference_id: table_ref.id.to_string(),
                            display_alias: table_ref.display_alias.clone(),
                            position_override: table_ref.position.as_ref().map(|p| json!({"x": p.x, "y": p.y})),
                            notes: table_ref.notes.clone(),
                        });
                    }
                }
            }
        }
        drop(model_service);
    }
    
    // Load imported relationships
    for rel_ref in &config.imported_relationships {
        let mut model_service = state.model_service.lock().await;
        if let Ok(_) = create_workspace_for_email_and_domain(&mut model_service, &email, &rel_ref.source_domain).await {
            if let Some(source_model) = model_service.get_current_model() {
                if let Some(relationship) = source_model.relationships.iter().find(|r| r.id == rel_ref.relationship_id) {
                    imported_relationships.push(ImportedRelationshipInfo {
                        relationship: serde_json::to_value(relationship).unwrap_or(json!({})),
                        source_domain: rel_ref.source_domain.clone(),
                        reference_id: rel_ref.id.to_string(),
                    });
                }
            }
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
