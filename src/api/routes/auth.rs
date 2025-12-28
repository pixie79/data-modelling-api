//! Authentication routes for GitHub OAuth with JWT tokens.
//! 
//! Supports both web and desktop authentication flows:
//! - Web: Direct OAuth redirect flow
//! - Desktop: Initiate OAuth, open browser, poll for completion
//!
//! Security features:
//! - Time-scoped JWT access tokens (15 minutes)
//! - Refresh tokens for session renewal (7 days)
//! - Session revocation support via blacklist

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use axum::extract::FromRef;

use super::tables::AppState;
use super::workspace;
use crate::services::oauth_service::{OAuthService, GitHubEmail};
use crate::services::jwt_service::{JwtService, SharedJwtService, TokenPair, Claims};

/// OAuth session storage - keeps track of active sessions for revocation
/// Key: session_id (from JWT), Value: session metadata
pub type SessionStore = Arc<Mutex<HashMap<String, SessionMetadata>>>;

/// Revoked sessions (for logout before token expiry)
pub type RevokedTokens = Arc<Mutex<HashSet<String>>>;

/// Pending OAuth states for desktop apps (state_id -> PendingAuth)
pub type PendingAuthStore = Arc<Mutex<HashMap<String, PendingAuth>>>;

/// Session metadata stored server-side (for revocation and tracking)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub github_id: u64,
    pub github_username: String,
    pub github_access_token: String,  // GitHub's access token (for API calls)
    pub emails: Vec<GitHubEmail>,
    pub selected_email: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

// Legacy alias for backward compatibility
pub type OAuthSession = SessionMetadata;

pub fn new_session_store() -> SessionStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn new_revoked_tokens() -> RevokedTokens {
    Arc::new(Mutex::new(HashSet::new()))
}

pub fn new_pending_auth_store() -> PendingAuthStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Pending authentication for desktop apps
#[derive(Clone, Debug)]
pub struct PendingAuth {
    pub state_id: String,
    pub oauth_state: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed: bool,
    pub session_id: Option<String>,
    pub tokens: Option<TokenPair>,
    pub emails: Vec<GitHubEmail>,
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

/// Response for desktop auth initiation
#[derive(Serialize)]
struct DesktopAuthInitResponse {
    state_id: String,
    auth_url: String,
}

/// Response for polling auth status (now includes JWT tokens)
#[derive(Serialize)]
struct PollAuthResponse {
    completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token_expires_at: Option<i64>,
    emails: Vec<GitHubEmail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    github_username: Option<String>,
}

#[derive(Serialize)]
struct AuthStatusResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    github_username: Option<String>,
    emails: Vec<GitHubEmail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct SelectEmailRequest {
    email: String,
}

#[derive(Serialize)]
struct SelectEmailResponse {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_path: Option<String>,
    /// New tokens with the selected email as subject
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token_expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct RefreshTokenRequest {
    refresh_token: String,
}

#[derive(Serialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
    access_token_expires_at: i64,
    refresh_token_expires_at: i64,
    token_type: String,
}

/// Auth router state
#[derive(Clone)]
pub struct AuthState {
    pub session_store: SessionStore,
    pub revoked_tokens: RevokedTokens,
    pub pending_auth_store: PendingAuthStore,
    pub oauth_service: Arc<OAuthService>,
    pub jwt_service: SharedJwtService,
    pub app_state: AppState,
}

impl FromRef<AuthState> for AppState {
    fn from_ref(auth_state: &AuthState) -> Self {
        auth_state.app_state.clone()
    }
}

/// Create the auth router
pub fn auth_router(session_store: SessionStore, oauth_service: Arc<OAuthService>, app_state: AppState) -> Router<AppState> {
    let jwt_service = Arc::new(JwtService::from_env());
    
    let auth_state = AuthState {
        session_store,
        revoked_tokens: new_revoked_tokens(),
        pending_auth_store: new_pending_auth_store(),
        oauth_service,
        jwt_service,
        app_state: app_state.clone(),
    };
    
    Router::new()
        // Web OAuth flow
        .route("/github/login", get(initiate_github_login))
        .route("/github/callback", get(handle_github_callback))
        // Desktop OAuth flow
        .route("/github/login/desktop", get(initiate_desktop_github_login))
        .route("/poll/{state_id}", get(poll_auth_status))
        // Token management
        .route("/refresh", post(refresh_token))
        // Common endpoints
        .route("/status", get(get_auth_status))
        .route("/select-email", post(select_email))
        .route("/logout", post(logout))
        .with_state(auth_state)
}

/// GET /auth/github/login - Initiate GitHub OAuth flow (web - direct redirect)
async fn initiate_github_login(
    State(auth_state): State<AuthState>,
) -> Result<Redirect, StatusCode> {
    match auth_state.oauth_service.get_authorize_url_with_source("web") {
        Ok(url) => {
            info!("Initiating GitHub OAuth flow (web)");
            Ok(Redirect::temporary(&url))
        }
        Err(e) => {
            warn!("Failed to generate GitHub OAuth URL: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /auth/github/login/desktop - Initiate GitHub OAuth flow for desktop apps
async fn initiate_desktop_github_login(
    State(auth_state): State<AuthState>,
) -> Result<Json<DesktopAuthInitResponse>, StatusCode> {
    let state_id = uuid::Uuid::new_v4().to_string();
    
    match auth_state.oauth_service.get_authorize_url_with_source(&format!("desktop:{}", state_id)) {
        Ok(auth_url) => {
            let oauth_state = auth_url
                .split("state=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .unwrap_or("")
                .to_string();
            
            let pending = PendingAuth {
                state_id: state_id.clone(),
                oauth_state,
                created_at: chrono::Utc::now(),
                completed: false,
                session_id: None,
                tokens: None,
                emails: Vec::new(),
            };
            
            auth_state.pending_auth_store.lock().await.insert(state_id.clone(), pending);
            
            info!("Initiating GitHub OAuth flow (desktop), state_id: {}", state_id);
            
            Ok(Json(DesktopAuthInitResponse {
                state_id,
                auth_url,
            }))
        }
        Err(e) => {
            warn!("Failed to generate GitHub OAuth URL for desktop: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /auth/poll/{state_id} - Poll for desktop auth completion
async fn poll_auth_status(
    State(auth_state): State<AuthState>,
    Path(state_id): Path<String>,
) -> Result<Json<PollAuthResponse>, StatusCode> {
    let pending_store = auth_state.pending_auth_store.lock().await;
    
    if let Some(pending) = pending_store.get(&state_id) {
        if pending.completed {
            if let Some(tokens) = &pending.tokens {
                return Ok(Json(PollAuthResponse {
                    completed: true,
                    access_token: Some(tokens.access_token.clone()),
                    refresh_token: Some(tokens.refresh_token.clone()),
                    access_token_expires_at: Some(tokens.access_token_expires_at),
                    emails: pending.emails.clone(),
                    github_username: None, // Username is available via /auth/status
                }));
            }
            
            return Ok(Json(PollAuthResponse {
                completed: true,
                access_token: None,
                refresh_token: None,
                access_token_expires_at: None,
                emails: pending.emails.clone(),
                github_username: None,
            }));
        }
        
        return Ok(Json(PollAuthResponse {
            completed: false,
            access_token: None,
            refresh_token: None,
            access_token_expires_at: None,
            emails: Vec::new(),
            github_username: None,
        }));
    }
    
    Err(StatusCode::NOT_FOUND)
}

/// GET /auth/github/callback - Handle GitHub OAuth callback
async fn handle_github_callback(
    State(auth_state): State<AuthState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<Redirect, StatusCode> {
    let code = match params.code.as_ref() {
        Some(c) if !c.is_empty() => c.as_str(),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    let state = params.state.as_deref().unwrap_or("");
    info!("Received GitHub OAuth callback with code, state: {}", state);

    // Exchange code for GitHub access token
    let github_access_token = match auth_state.oauth_service.exchange_code(code).await {
        Ok(token) => token,
        Err(e) => {
            warn!("Failed to exchange OAuth code: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Fetch user info from GitHub
    let (github_id, username, emails) = match auth_state.oauth_service.fetch_user_info(&github_access_token).await {
        Ok(info) => info,
        Err(e) => {
            warn!("Failed to fetch user info from GitHub: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Generate session ID and JWT tokens
    let session_id = uuid::Uuid::new_v4().to_string();
    
    // Use primary email or first verified email as initial subject
    let primary_email = emails.iter()
        .find(|e| e.primary && e.verified)
        .or_else(|| emails.iter().find(|e| e.verified))
        .map(|e| e.email.clone())
        .unwrap_or_else(|| format!("{}@github", username));
    
    // Generate JWT token pair
    let tokens = auth_state.jwt_service.generate_token_pair(
        &primary_email,
        github_id,
        &username,
        &session_id,
    ).map_err(|e| {
        warn!("Failed to generate JWT tokens: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store session metadata
    let session = SessionMetadata {
        github_id,
        github_username: username.clone(),
        github_access_token,
        emails: emails.clone(),
        selected_email: if emails.len() == 1 { Some(primary_email.clone()) } else { None },
        created_at: chrono::Utc::now(),
        last_activity: chrono::Utc::now(),
    };

    auth_state.session_store.lock().await.insert(session_id.clone(), session);

    info!("Created JWT session for GitHub user: {} (session: {})", username, session_id);

    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    // Check if this is a desktop auth flow
    if state.starts_with("desktop:") {
        let state_id = state.strip_prefix("desktop:").unwrap_or("");
        
        // Update pending auth with tokens
        let mut pending_store = auth_state.pending_auth_store.lock().await;
        if let Some(pending) = pending_store.get_mut(state_id) {
            pending.completed = true;
            pending.session_id = Some(session_id.clone());
            pending.tokens = Some(tokens.clone());
            pending.emails = emails.clone();
            info!("Desktop auth completed for state_id: {}", state_id);
        }
        
        // Redirect to success page
        let redirect_url = format!("{}/auth/success?source=desktop", frontend_url);
        Ok(Redirect::temporary(&redirect_url))
    } else {
        // Web flow - include token in redirect URL (will be stored in localStorage)
        let select_email = if emails.len() > 1 { "true" } else { "false" };
        // Encode token for URL safety
        let encoded_token = urlencoding::encode(&tokens.access_token);
        let encoded_refresh = urlencoding::encode(&tokens.refresh_token);
        let redirect_url = format!(
            "{}/auth/complete/{}/{}/{}/{}",
            frontend_url, 
            encoded_token,
            encoded_refresh,
            tokens.access_token_expires_at,
            select_email
        );
        info!("Redirecting web client to auth complete page");
        Ok(Redirect::temporary(&redirect_url))
    }
}

/// POST /auth/refresh - Refresh access token using refresh token
async fn refresh_token(
    State(auth_state): State<AuthState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<RefreshTokenResponse>, StatusCode> {
    // Validate and decode refresh token
    let claims = auth_state.jwt_service.validate_refresh_token(&request.refresh_token)
        .map_err(|e| {
            warn!("Invalid refresh token: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    // Check if session is revoked
    let revoked = auth_state.revoked_tokens.lock().await;
    if revoked.contains(&claims.session_id) {
        warn!("Attempted to refresh revoked session: {}", claims.session_id);
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(revoked);
    
    // Check if session still exists
    let sessions = auth_state.session_store.lock().await;
    if !sessions.contains_key(&claims.session_id) {
        warn!("Session not found for refresh: {}", claims.session_id);
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(sessions);
    
    // Generate new token pair
    let new_tokens = auth_state.jwt_service.refresh_access_token(&request.refresh_token)
        .map_err(|e| {
            warn!("Failed to refresh token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // Update last activity
    let mut sessions = auth_state.session_store.lock().await;
    if let Some(session) = sessions.get_mut(&claims.session_id) {
        session.last_activity = chrono::Utc::now();
    }
    
    info!("Refreshed tokens for session: {}", claims.session_id);
    
    Ok(Json(RefreshTokenResponse {
        access_token: new_tokens.access_token,
        refresh_token: new_tokens.refresh_token,
        access_token_expires_at: new_tokens.access_token_expires_at,
        refresh_token_expires_at: new_tokens.refresh_token_expires_at,
        token_type: "Bearer".to_string(),
    }))
}

/// GET /auth/status - Get current authentication status
async fn get_auth_status(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AuthStatusResponse>, StatusCode> {
    // Try to get token from Authorization header (preferred) or x-session-id (legacy)
    let claims = extract_and_validate_token(&auth_state, &headers).await;
    
    if let Some(claims) = claims {
        // Check if session is revoked
        let revoked = auth_state.revoked_tokens.lock().await;
        if revoked.contains(&claims.session_id) {
            return Ok(Json(AuthStatusResponse {
                authenticated: false,
                github_username: None,
                emails: Vec::new(),
                selected_email: None,
                token_expires_at: None,
            }));
        }
        drop(revoked);
        
        // Get session metadata
        let sessions = auth_state.session_store.lock().await;
        if let Some(session) = sessions.get(&claims.session_id) {
            return Ok(Json(AuthStatusResponse {
                authenticated: true,
                github_username: Some(session.github_username.clone()),
                emails: session.emails.clone(),
                selected_email: session.selected_email.clone(),
                token_expires_at: Some(claims.exp),
            }));
        }
    }

    Ok(Json(AuthStatusResponse {
        authenticated: false,
        github_username: None,
        emails: Vec::new(),
        selected_email: None,
        token_expires_at: None,
    }))
}

/// POST /auth/select-email - Select email for workspace creation
async fn select_email(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<SelectEmailRequest>,
) -> Result<Json<SelectEmailResponse>, StatusCode> {
    let claims = extract_and_validate_token(&auth_state, &headers).await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Check if session is revoked
    let revoked = auth_state.revoked_tokens.lock().await;
    if revoked.contains(&claims.session_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(revoked);

    let mut sessions = auth_state.session_store.lock().await;
    let session = sessions
        .get_mut(&claims.session_id)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate email is in user's GitHub emails
    let email_valid = session
        .emails
        .iter()
        .any(|e| e.email == request.email && e.verified);

    if !email_valid {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Update session with selected email
    session.selected_email = Some(request.email.clone());
    let github_id = session.github_id;
    let github_username = session.github_username.clone();
    drop(sessions);

    // Generate new tokens with the selected email as subject
    let new_tokens = auth_state.jwt_service.generate_token_pair(
        &request.email,
        github_id,
        &github_username,
        &claims.session_id,
    ).map_err(|e| {
        warn!("Failed to generate new tokens: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create workspace with selected email
    let mut model_service = auth_state.app_state.model_service.lock().await;
    
    match workspace::create_workspace_for_email(&mut model_service, &request.email).await {
        Ok(workspace_path) => {
            info!("Created workspace for GitHub user {} with email {}", github_username, request.email);
            Ok(Json(SelectEmailResponse {
                message: format!("Workspace created for {}", request.email),
                workspace_path: Some(workspace_path),
                access_token: Some(new_tokens.access_token),
                refresh_token: Some(new_tokens.refresh_token),
                access_token_expires_at: Some(new_tokens.access_token_expires_at),
            }))
        }
        Err(e) => {
            warn!("Failed to create workspace: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /auth/logout - Logout and revoke session
async fn logout(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(claims) = extract_and_validate_token(&auth_state, &headers).await {
        // Add session to revoked list
        auth_state.revoked_tokens.lock().await.insert(claims.session_id.clone());
        
        // Remove session metadata
        auth_state.session_store.lock().await.remove(&claims.session_id);
        
        info!("Logged out and revoked session: {}", claims.session_id);
    }

    Ok(Json(serde_json::json!({ "message": "Logged out successfully" })))
}

/// Extract and validate JWT token from request headers
/// Supports both Authorization: Bearer <token> and x-session-id (legacy)
async fn extract_and_validate_token(
    auth_state: &AuthState,
    headers: &axum::http::HeaderMap,
) -> Option<Claims> {
    // Try Authorization header first (preferred)
    if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(token) = JwtService::extract_bearer_token(auth_header) {
            if let Ok(claims) = auth_state.jwt_service.validate_access_token(token) {
                return Some(claims);
            }
        }
    }
    
    // Try x-session-id header (legacy - treat as token)
    if let Some(token) = headers.get("x-session-id").and_then(|h| h.to_str().ok()) {
        // Check if it looks like a JWT (contains dots)
        if token.contains('.') {
            if let Ok(claims) = auth_state.jwt_service.validate_access_token(token) {
                return Some(claims);
            }
        }
    }
    
    None
}

/// Helper function to validate token and get claims (for use in other modules)
pub async fn validate_request_token(
    session_store: &SessionStore,
    revoked_tokens: &RevokedTokens,
    jwt_service: &JwtService,
    headers: &axum::http::HeaderMap,
) -> Result<Claims, StatusCode> {
    // Try Authorization header first
    let token = if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        JwtService::extract_bearer_token(auth_header)
    } else {
        // Try x-session-id as fallback
        headers.get("x-session-id").and_then(|h| h.to_str().ok())
    };
    
    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Validate token
    let claims = jwt_service.validate_access_token(token)
        .map_err(|e| {
            warn!("Token validation failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    // Check if revoked
    let revoked = revoked_tokens.lock().await;
    if revoked.contains(&claims.session_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(revoked);
    
    // Check if session exists
    let sessions = session_store.lock().await;
    if !sessions.contains_key(&claims.session_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(claims)
}
