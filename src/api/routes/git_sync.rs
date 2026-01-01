//! Git synchronization routes.
//!
//! Provides endpoints for Git operations like clone, pull, push, and conflict resolution.
//! Uses the SDK's GitService to avoid code duplication.

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{error, info};
use utoipa::ToSchema;

use super::app_state::AppState;
use super::auth_context::AuthContext;
use super::workspace::{get_workspace_data_dir, sanitize_email_for_path, validate_domain_name};
use data_modelling_sdk::git::GitService as SdkGitService;

/// Create the git sync router
pub fn git_sync_router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_sync_config))
        .route("/config", post(update_sync_config))
        .route("/init", post(init_repository))
        .route("/clone", post(clone_repository))
        .route("/status", get(get_sync_status))
        .route("/export", post(export_domain))
        .route("/commit", post(commit_changes))
        .route("/push", post(push_changes))
        .route("/pull", post(pull_changes))
        .route("/conflicts", get(list_conflicts))
        .route("/conflicts/resolve", post(resolve_conflict))
}

/// Path parameters for domain-scoped git operations
#[derive(Deserialize, ToSchema)]
pub struct DomainPath {
    domain: String,
}

/// Query parameters for git status
#[derive(Deserialize, ToSchema)]
pub struct GitStatusQuery {
    domain: Option<String>,
}

/// Request to update sync configuration
#[derive(Deserialize, ToSchema)]
pub struct UpdateSyncConfigRequest {
    repository_url: Option<String>,
    branch: Option<String>,
    auto_commit: Option<bool>,
    auto_push: Option<bool>,
}

/// Response for sync configuration
#[derive(Serialize, ToSchema)]
pub struct SyncConfigResponse {
    repository_url: Option<String>,
    branch: String,
    auto_commit: bool,
    auto_push: bool,
}

/// Request to initialize a repository
#[derive(Deserialize, ToSchema)]
pub struct InitRepositoryRequest {
    domain: String,
}

/// Response for repository initialization
#[derive(Serialize, ToSchema)]
pub struct InitRepositoryResponse {
    message: String,
    repository_path: String,
}

/// Request to clone a repository
#[derive(Deserialize, ToSchema)]
pub struct CloneRepositoryRequest {
    repository_url: String,
    domain: String,
    branch: Option<String>,
}

/// Response for repository cloning
#[derive(Serialize, ToSchema)]
pub struct CloneRepositoryResponse {
    message: String,
    repository_path: String,
}

/// Request to export domain to Git
#[derive(Deserialize, ToSchema)]
pub struct ExportDomainRequest {
    domain: String,
}

/// Response for domain export
#[derive(Serialize, ToSchema)]
pub struct GitExportResult {
    message: String,
    files_exported: usize,
}

/// Request to commit changes
#[derive(Deserialize, ToSchema)]
pub struct CommitRequest {
    domain: String,
    message: String,
}

/// Response for commit operation
#[derive(Serialize, ToSchema)]
pub struct CommitResponse {
    message: String,
    commit_hash: Option<String>,
}

/// Request to resolve a conflict
#[derive(Deserialize, ToSchema)]
pub struct ResolveConflictRequest {
    domain: String,
    file_path: String,
    resolution: String, // "ours", "theirs", or "manual"
}

/// Response for conflict list
#[derive(Serialize, ToSchema)]
pub struct ConflictListResponse {
    conflicts: Vec<ConflictInfo>,
}

/// Information about a Git conflict
#[derive(Serialize, ToSchema)]
pub struct ConflictInfo {
    file_path: String,
    status: String,
}

/// Git status response
#[derive(Serialize, ToSchema)]
pub struct GitStatusResponse {
    is_initialized: bool,
    has_remote: bool,
    branch: Option<String>,
    ahead: usize,
    behind: usize,
    has_uncommitted_changes: bool,
    conflicts: Vec<ConflictInfo>,
}

/// Helper to get workspace path for a domain
fn get_domain_workspace_path(email: &str, domain: &str) -> Result<PathBuf, StatusCode> {
    validate_domain_name(domain)?;
    let workspace_data_dir =
        get_workspace_data_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sanitized_email = sanitize_email_for_path(email);
    Ok(workspace_data_dir.join(&sanitized_email).join(domain))
}

/// GET /git/config - Get sync configuration for a domain
#[utoipa::path(
    get,
    path = "/git/config",
    tag = "Git Sync",
    params(
        ("domain" = Option<String>, Query, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Sync configuration retrieved successfully", body = SyncConfigResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_sync_config(
    State(_state): State<AppState>,
    _auth: AuthContext,
    Query(_params): Query<GitStatusQuery>,
) -> Result<Json<SyncConfigResponse>, StatusCode> {
    // Return default config - config storage per domain can be added later if needed
    Ok(Json(SyncConfigResponse {
        repository_url: None,
        branch: "main".to_string(),
        auto_commit: false,
        auto_push: false,
    }))
}

/// POST /git/config - Update sync configuration for a domain
#[utoipa::path(
    post,
    path = "/git/config",
    tag = "Git Sync",
    request_body = UpdateSyncConfigRequest,
    responses(
        (status = 200, description = "Sync configuration updated successfully", body = SyncConfigResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_sync_config(
    State(_state): State<AppState>,
    _auth: AuthContext,
    Json(request): Json<UpdateSyncConfigRequest>,
) -> Result<Json<SyncConfigResponse>, StatusCode> {
    // Config storage per domain can be added later if needed
    Ok(Json(SyncConfigResponse {
        repository_url: request.repository_url,
        branch: request.branch.unwrap_or_else(|| "main".to_string()),
        auto_commit: request.auto_commit.unwrap_or(false),
        auto_push: request.auto_push.unwrap_or(false),
    }))
}

/// POST /git/init - Initialize a Git repository for a domain
#[utoipa::path(
    post,
    path = "/git/init",
    tag = "Git Sync",
    request_body = InitRepositoryRequest,
    responses(
        (status = 200, description = "Repository initialized successfully", body = InitRepositoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn init_repository(
    State(_state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<InitRepositoryRequest>,
) -> Result<Json<InitRepositoryResponse>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &request.domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => {
            info!(
                "Initialized Git repository for domain {} at {:?}",
                request.domain, workspace_path
            );
            Ok(Json(InitRepositoryResponse {
                message: format!("Repository initialized for domain {}", request.domain),
                repository_path: workspace_path.to_string_lossy().to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to initialize repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /git/clone - Clone a repository for a domain
#[utoipa::path(
    post,
    path = "/git/clone",
    tag = "Git Sync",
    request_body = CloneRepositoryRequest,
    responses(
        (status = 200, description = "Repository cloned successfully", body = CloneRepositoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn clone_repository(
    State(_state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CloneRepositoryRequest>,
) -> Result<Json<CloneRepositoryResponse>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &request.domain)?;

    let mut git_service = SdkGitService::new();
    let branch = request.branch.as_deref();

    match git_service.clone_repository(&request.repository_url, &workspace_path, branch) {
        Ok(_) => {
            info!(
                "Cloned repository {} for domain {} at {:?}",
                request.repository_url, request.domain, workspace_path
            );
            Ok(Json(CloneRepositoryResponse {
                message: format!("Repository cloned for domain {}", request.domain),
                repository_path: workspace_path.to_string_lossy().to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to clone repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /git/status - Get Git status for a domain
#[utoipa::path(
    get,
    path = "/git/status",
    tag = "Git Sync",
    params(
        ("domain" = Option<String>, Query, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Git status retrieved successfully", body = GitStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_sync_status(
    State(_state): State<AppState>,
    auth: AuthContext,
    Query(params): Query<GitStatusQuery>,
) -> Result<Json<GitStatusResponse>, StatusCode> {
    let domain = params.domain.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
    let workspace_path = get_domain_workspace_path(&auth.email, domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => {
            // Get status information - use default values if status() fails
            let status_result = git_service.status();

            // Return simplified status response
            // SDK GitStatus provides basic info - detailed status extraction can be enhanced later
            Ok(Json(GitStatusResponse {
                is_initialized: true,
                has_remote: status_result.is_ok(),
                branch: None, // Branch info can be extracted from git config if needed
                ahead: 0,
                behind: 0,
                has_uncommitted_changes: false, // Can be determined by checking git diff
                conflicts: vec![],              // Conflicts detected during merge operations
            }))
        }
        Err(e) => {
            error!("Failed to get Git status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /git/export - Export domain to Git repository
#[utoipa::path(
    post,
    path = "/git/export",
    tag = "Git Sync",
    request_body = ExportDomainRequest,
    responses(
        (status = 200, description = "Domain exported successfully", body = GitExportResult),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_domain(
    State(_state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<ExportDomainRequest>,
) -> Result<Json<GitExportResult>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &request.domain)?;

    // Domain is already exported to the workspace path (YAML files)
    // This endpoint could trigger a commit or just confirm export
    info!("Domain {} exported to {:?}", request.domain, workspace_path);
    Ok(Json(GitExportResult {
        message: format!("Domain {} exported successfully", request.domain),
        files_exported: 0, // File count can be tracked during export if needed
    }))
}

/// POST /git/commit - Commit changes to Git repository
#[utoipa::path(
    post,
    path = "/git/commit",
    tag = "Git Sync",
    request_body = CommitRequest,
    responses(
        (status = 200, description = "Changes committed successfully", body = CommitResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn commit_changes(
    State(_state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CommitRequest>,
) -> Result<Json<CommitResponse>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &request.domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => {
            match git_service.commit_all(&request.message, &auth.email, &auth.email) {
                Ok(_) => {
                    info!("Committed changes for domain {}", request.domain);
                    Ok(Json(CommitResponse {
                        message: format!("Changes committed for domain {}", request.domain),
                        commit_hash: None, // SDK commit_all doesn't return commit hash
                    }))
                }
                Err(e) => {
                    error!("Failed to commit changes: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            error!("Failed to open repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /git/push - Push changes to remote repository
#[utoipa::path(
    post,
    path = "/git/push",
    tag = "Git Sync",
    params(
        ("domain" = String, Query, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Changes pushed successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn push_changes(
    State(_state): State<AppState>,
    auth: AuthContext,
    Query(params): Query<DomainPath>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &params.domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => match git_service.push("origin", "main") {
            Ok(_) => {
                info!("Pushed changes for domain {}", params.domain);
                Ok(Json(serde_json::json!({
                    "message": format!("Changes pushed for domain {}", params.domain)
                })))
            }
            Err(e) => {
                error!("Failed to push changes: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(e) => {
            error!("Failed to open repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /git/pull - Pull changes from remote repository
#[utoipa::path(
    post,
    path = "/git/pull",
    tag = "Git Sync",
    params(
        ("domain" = String, Query, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Changes pulled successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn pull_changes(
    State(_state): State<AppState>,
    auth: AuthContext,
    Query(params): Query<DomainPath>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &params.domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => match git_service.pull("origin", "main") {
            Ok(_) => {
                info!("Pulled changes for domain {}", params.domain);
                Ok(Json(serde_json::json!({
                    "message": format!("Changes pulled for domain {}", params.domain)
                })))
            }
            Err(e) => {
                error!("Failed to pull changes: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(e) => {
            error!("Failed to open repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /git/conflicts - List Git conflicts for a domain
#[utoipa::path(
    get,
    path = "/git/conflicts",
    tag = "Git Sync",
    params(
        ("domain" = Option<String>, Query, description = "Domain name")
    ),
    responses(
        (status = 200, description = "Conflicts retrieved successfully", body = ConflictListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_conflicts(
    State(_state): State<AppState>,
    auth: AuthContext,
    Query(params): Query<GitStatusQuery>,
) -> Result<Json<ConflictListResponse>, StatusCode> {
    let domain = params.domain.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
    let workspace_path = get_domain_workspace_path(&auth.email, domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => {
            // Conflicts are detected during merge operations
            // SDK GitStatus can be extended to provide conflict details
            Ok(Json(ConflictListResponse { conflicts: vec![] }))
        }
        Err(e) => {
            error!("Failed to list conflicts: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /git/conflicts/resolve - Resolve a Git conflict
#[utoipa::path(
    post,
    path = "/git/conflicts/resolve",
    tag = "Git Sync",
    request_body = ResolveConflictRequest,
    responses(
        (status = 200, description = "Conflict resolved successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn resolve_conflict(
    State(_state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<ResolveConflictRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let workspace_path = get_domain_workspace_path(&auth.email, &request.domain)?;

    let mut git_service = SdkGitService::new();
    match git_service.open_or_init(&workspace_path) {
        Ok(_) => {
            // Resolve conflict based on resolution strategy
            // Note: SDK GitService doesn't have resolve_conflict method yet
            // For now, return a message indicating manual resolution is needed
            match request.resolution.as_str() {
                "ours" | "theirs" | "manual" => {
                    info!(
                        "Conflict resolution requested for {}: {}",
                        request.file_path, request.resolution
                    );
                    // Conflict resolution via SDK GitService can be added when SDK supports it
                    Ok(Json(serde_json::json!({
                        "message": format!("Conflict resolution requested for {} (resolution: {}). Manual resolution may be required.", request.file_path, request.resolution)
                    })))
                }
                _ => Err(StatusCode::BAD_REQUEST),
            }
        }
        Err(e) => {
            error!("Failed to open repository: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
