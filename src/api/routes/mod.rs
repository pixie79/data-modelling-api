//! API routes module - organizes all route handlers.
//! 
//! All table and relationship operations are now domain-scoped under /workspace/domains/{domain}/

pub mod ai;
pub mod auth;
// DrawIO routes - uses crate::drawio from lib.rs
pub mod collaboration;
pub mod drawio;
pub mod import;
pub mod models;
// Legacy routes kept for AppState definition but not mounted
pub mod relationships;
pub mod tables;
pub mod workspace;

use axum::Router;
pub use tables::AppState;

/// Create the main API router combining all route modules
/// 
/// Note: Legacy /tables and /relationships endpoints have been removed.
/// All table/relationship operations are now domain-scoped under:
/// - /workspace/domains/{domain}/tables
/// - /workspace/domains/{domain}/relationships
pub fn create_api_router(app_state: AppState) -> Router<AppState> {
    use crate::services::oauth_service::OAuthService;
    use std::sync::Arc;
    use std::env;

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
        .nest("/auth", auth::auth_router(app_state.session_store.clone(), oauth_service, app_state.clone()))
        .nest("/ai", ai::ai_router())
        .merge(collaboration::collaboration_router())
}

/// Create the application state
pub fn create_app_state() -> AppState {
    use std::collections::HashMap;
    
    let model_service = std::sync::Arc::new(tokio::sync::Mutex::new(
        crate::services::ModelService::new(),
    ));
    
    // Workspace will be created when user provides email via /workspace/create endpoint
    // No auto-initialization needed - user must create workspace explicitly
    
    AppState {
        model_service,
        collaboration_channels: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        session_store: auth::new_session_store(),
    }
}
