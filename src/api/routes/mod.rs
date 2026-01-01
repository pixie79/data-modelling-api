//! API routes module - organizes all route handlers.
//!
//! All table and relationship operations are now domain-scoped under /workspace/domains/{domain}/

pub mod ai;
pub mod app_state;
pub mod audit;
pub mod auth;
pub mod auth_context;
pub mod error;
// DrawIO routes - uses crate::drawio from lib.rs
pub mod collaboration;
pub mod collaboration_sessions;
pub mod drawio;
pub mod git_sync;
pub mod import;
pub mod models;
pub mod openapi;
// Legacy routes kept for AppState definition but not mounted
pub mod relationships;
pub mod tables;
pub mod workspace;

use axum::Router;
// Re-export AppState from app_state module for backwards compatibility
pub use app_state::AppState;
// Legacy AppState export kept for potential backwards compatibility
#[allow(unused_imports)]
pub use tables::AppState as LegacyAppState;

/// Create the main API router combining all route modules
///
/// Note: Legacy /tables and /relationships endpoints have been removed.
/// All table/relationship operations are now domain-scoped under:
/// - /workspace/domains/{domain}/tables
/// - /workspace/domains/{domain}/relationships
pub fn create_api_router(app_state: AppState) -> Router<AppState> {
    use crate::services::oauth_service::OAuthService;
    use std::env;
    use std::sync::Arc;

    // Initialize OAuth service
    let github_client_id = env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| "".to_string());
    let github_client_secret = env::var("GITHUB_CLIENT_SECRET").unwrap_or_else(|_| "".to_string());
    // GitHub callback MUST point to the API server, not the web client
    // The API processes the callback and then redirects to the web client
    let github_redirect_uri = env::var("GITHUB_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8081/api/v1/auth/github/callback".to_string());

    let oauth_service = Arc::new(OAuthService::new(
        github_client_id,
        github_client_secret,
        github_redirect_uri,
    ));

    Router::new()
        // All table/relationship operations are now under /workspace/domains/{domain}/
        .nest("/workspace", workspace::workspace_router())
        .nest("/import", import::import_router())
        .nest("/export", drawio::drawio_router())
        .nest("/models", models::models_router())
        .nest(
            "/auth",
            auth::auth_router(
                app_state.session_store.clone(),
                oauth_service,
                app_state.clone(),
            ),
        )
        .nest("/ai", ai::ai_router())
        .nest(
            "/collaboration",
            collaboration_sessions::collaboration_sessions_router(),
        )
        .nest("/audit", audit::audit_router())
        .nest("/git", git_sync::git_sync_router())
        .merge(collaboration::collaboration_router())
        // OpenAPI documentation endpoints
        .merge(openapi::openapi_router())
    // Note: State is applied by callers who need it (e.g., TestServer)
    // For production use, call .with_state(app_state) after creating the router
}

/// Create the application state (synchronous, for backwards compatibility).
///
/// Note: For PostgreSQL storage, call `init_storage()` on the returned state.
pub fn create_app_state() -> AppState {
    AppState::new()
}

/// Create the application state with storage initialization (async).
///
/// This is the preferred method for production use.
pub async fn create_app_state_with_storage() -> Result<AppState, crate::storage::StorageError> {
    let mut state = AppState::new();
    state.init_storage().await?;
    Ok(state)
}
