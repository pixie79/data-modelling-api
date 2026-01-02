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
//!
//! Storage backends:
//! - In-memory: Default for file-based storage (legacy)
//! - PostgreSQL: Database-backed sessions for production

use axum::extract::FromRef;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use super::app_state::AppState;
use super::workspace;
use crate::services::jwt_service::{Claims, JwtService, SharedJwtService, TokenPair};
use crate::services::oauth_service::{GitHubEmail, OAuthService};
use url::Url;

/// OAuth session storage - keeps track of active sessions for revocation
/// Key: session_id (from JWT), Value: session metadata
pub type SessionStore = Arc<Mutex<HashMap<String, SessionMetadata>>>;

/// Revoked sessions (for logout before token expiry)
pub type RevokedTokens = Arc<Mutex<HashSet<String>>>;

/// Pending OAuth states for desktop apps (state_id -> PendingAuth)
pub type PendingAuthStore = Arc<Mutex<HashMap<String, PendingAuth>>>;

/// OAuth state store: CSRF state -> source metadata (web vs desktop polling id).
pub type OAuthStateStore = Arc<Mutex<HashMap<String, OAuthStateEntry>>>;

/// One-time auth code exchange store: code -> token payload (short-lived).
pub type TokenExchangeStore = Arc<Mutex<HashMap<String, TokenExchangeEntry>>>;

#[derive(Clone, Debug)]
pub struct TokenExchangeEntry {
    pub tokens: TokenPair,
    pub emails: Vec<GitHubEmail>,
    pub select_email: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub github_id: u64,
    pub github_username: String,
    pub session_id: String,
}

#[derive(Clone, Debug)]
pub enum OAuthSource {
    Web,
    Desktop { state_id: String },
}

#[derive(Clone, Debug)]
pub struct OAuthStateEntry {
    pub source: OAuthSource,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Custom redirect URI provided by the client (optional)
    pub redirect_uri: Option<String>,
}

/// Session metadata stored server-side (for revocation and tracking)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// UUIDv4 user id for file-mode requests (stable via mapping).
    pub user_id: Uuid,
    pub github_id: u64,
    pub github_username: String,
    pub github_access_token: String, // GitHub's access token (for API calls)
    pub emails: Vec<GitHubEmail>,
    pub selected_email: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

// Legacy alias for backward compatibility
#[allow(dead_code)]
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

pub fn new_oauth_state_store() -> OAuthStateStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn new_token_exchange_store() -> TokenExchangeStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Pending authentication for desktop apps
#[derive(Clone, Debug)]
pub struct PendingAuth {
    #[allow(dead_code)]
    pub state_id: String,
    #[allow(dead_code)]
    pub oauth_state: String,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed: bool,
    pub session_id: Option<String>,
    pub tokens: Option<TokenPair>,
    pub emails: Vec<GitHubEmail>,
}

#[derive(Deserialize, ToSchema)]
pub struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

/// Query parameters for GitHub OAuth login initiation
#[derive(Deserialize, ToSchema)]
pub struct GitHubLoginQuery {
    /// Optional redirect URI to use after OAuth callback completion
    /// If not provided, uses FRONTEND_URL environment variable or default
    #[serde(default)]
    redirect_uri: Option<String>,
}

/// Response for desktop auth initiation
#[derive(Serialize, ToSchema)]
pub struct DesktopAuthInitResponse {
    state_id: String,
    auth_url: String,
}

/// Response for polling auth status (now includes JWT tokens)
#[derive(Serialize, ToSchema)]
pub struct PollAuthResponse {
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

#[derive(Serialize, ToSchema)]
pub struct AuthStatusResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    github_username: Option<String>,
    emails: Vec<GitHubEmail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_expires_at: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct SelectEmailRequest {
    email: String,
    /// Optional exchange code - allows selecting email without Bearer token
    /// This is used when the initial exchange returned empty tokens with select_email=true
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SelectEmailResponse {
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

#[derive(Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    refresh_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ExchangeAuthCodeRequest {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ExchangeAuthCodeResponse {
    access_token: String,
    refresh_token: String,
    access_token_expires_at: i64,
    refresh_token_expires_at: i64,
    token_type: String,
    emails: Vec<GitHubEmail>,
    select_email: bool,
}

#[derive(Serialize, ToSchema)]
pub struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
    access_token_expires_at: i64,
    refresh_token_expires_at: i64,
    token_type: String,
}

/// Response for GET /api/v1/auth/me endpoint
#[derive(Serialize, ToSchema)]
pub struct UserInfoResponse {
    user: UserInfo,
}

#[derive(Serialize, ToSchema)]
pub struct UserInfo {
    id: String,
    name: String,
    email: String,
}

/// Auth router state
#[derive(Clone)]
pub struct AuthState {
    pub session_store: SessionStore,
    pub revoked_tokens: RevokedTokens,
    pub pending_auth_store: PendingAuthStore,
    pub oauth_state_store: OAuthStateStore,
    pub token_exchange_store: TokenExchangeStore,
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
pub fn auth_router(
    session_store: SessionStore,
    oauth_service: Arc<OAuthService>,
    app_state: AppState,
) -> Router<AppState> {
    let jwt_service = Arc::new(JwtService::from_env());

    let auth_state = AuthState {
        session_store,
        revoked_tokens: new_revoked_tokens(),
        pending_auth_store: new_pending_auth_store(),
        oauth_state_store: new_oauth_state_store(),
        token_exchange_store: new_token_exchange_store(),
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
        // Web auth code exchange (avoid tokens-in-URL)
        .route("/exchange", post(exchange_auth_code))
        // Token management
        .route("/refresh", post(refresh_token))
        // Common endpoints
        .route("/status", get(get_auth_status))
        .route("/select-email", post(select_email))
        .route("/logout", post(logout))
        // New /api/v1/auth/me endpoint
        .route("/me", get(get_current_user))
        .with_state(auth_state)
}

/// GET /auth/github/login - Initiate GitHub OAuth flow (web - direct redirect)
#[utoipa::path(
    get,
    path = "/auth/github/login",
    tag = "Authentication",
    params(
        ("redirect_uri" = Option<String>, Query, description = "Optional redirect URI after OAuth completion")
    ),
    responses(
        (status = 302, description = "Redirect to GitHub OAuth authorization page"),
        (status = 400, description = "Bad request - invalid redirect_uri"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn initiate_github_login(
    State(auth_state): State<AuthState>,
    Query(params): Query<GitHubLoginQuery>,
) -> Result<Redirect, StatusCode> {
    // Validate redirect_uri if provided
    let redirect_uri = if let Some(ref uri) = params.redirect_uri {
        if !validate_redirect_uri(uri) {
            warn!("Invalid redirect_uri provided: {}", uri);
            return Err(StatusCode::BAD_REQUEST);
        }
        Some(uri.clone())
    } else {
        None
    };

    let csrf_state = Uuid::new_v4().to_string();
    auth_state.oauth_state_store.lock().await.insert(
        csrf_state.clone(),
        OAuthStateEntry {
            source: OAuthSource::Web,
            created_at: chrono::Utc::now(),
            redirect_uri,
        },
    );

    match auth_state
        .oauth_service
        .get_authorize_url_with_state(&csrf_state)
    {
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
#[utoipa::path(
    get,
    path = "/auth/github/login/desktop",
    tag = "Authentication",
    responses(
        (status = 200, description = "Desktop OAuth flow initiated successfully", body = DesktopAuthInitResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn initiate_desktop_github_login(
    State(auth_state): State<AuthState>,
) -> Result<Json<DesktopAuthInitResponse>, StatusCode> {
    let state_id = uuid::Uuid::new_v4().to_string();
    let csrf_state = Uuid::new_v4().to_string();

    auth_state.oauth_state_store.lock().await.insert(
        csrf_state.clone(),
        OAuthStateEntry {
            source: OAuthSource::Desktop {
                state_id: state_id.clone(),
            },
            created_at: chrono::Utc::now(),
            redirect_uri: None, // Desktop flow doesn't use redirect_uri
        },
    );

    match auth_state
        .oauth_service
        .get_authorize_url_with_state(&csrf_state)
    {
        Ok(auth_url) => {
            let pending = PendingAuth {
                state_id: state_id.clone(),
                oauth_state: csrf_state,
                created_at: chrono::Utc::now(),
                completed: false,
                session_id: None,
                tokens: None,
                emails: Vec::new(),
            };

            auth_state
                .pending_auth_store
                .lock()
                .await
                .insert(state_id.clone(), pending);

            info!(
                "Initiating GitHub OAuth flow (desktop), state_id: {}",
                state_id
            );

            Ok(Json(DesktopAuthInitResponse { state_id, auth_url }))
        }
        Err(e) => {
            warn!("Failed to generate GitHub OAuth URL for desktop: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /auth/poll/{state_id} - Poll for desktop auth completion
#[utoipa::path(
    get,
    path = "/auth/poll/{state_id}",
    tag = "Authentication",
    params(
        ("state_id" = String, Path, description = "Desktop auth state ID")
    ),
    responses(
        (status = 200, description = "Auth status retrieved successfully", body = PollAuthResponse),
        (status = 404, description = "State ID not found")
    )
)]
pub async fn poll_auth_status(
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

/// POST /auth/exchange - Exchange a short-lived one-time code for JWT tokens (web flow).
#[utoipa::path(
    post,
    path = "/auth/exchange",
    tag = "Authentication",
    request_body = ExchangeAuthCodeRequest,
    responses(
        (status = 200, description = "Auth code exchanged successfully", body = ExchangeAuthCodeResponse),
        (status = 400, description = "Bad request - invalid or expired code")
    )
)]
pub async fn exchange_auth_code(
    State(auth_state): State<AuthState>,
    Json(request): Json<ExchangeAuthCodeRequest>,
) -> Result<Json<ExchangeAuthCodeResponse>, StatusCode> {
    if request.code.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get entry without removing it yet - we'll remove it only after successful token generation
    let mut store = auth_state.token_exchange_store.lock().await;
    let entry = match store.get(&request.code) {
        Some(e) => e.clone(),
        None => return Err(StatusCode::BAD_REQUEST),
    };

    if chrono::Utc::now() > entry.expires_at {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Handle email selection when select_email=true
    let (tokens, selected_email, should_remove_code) =
        if entry.select_email && entry.emails.len() > 1 {
            // Email selection required
            let email = match request.email {
                Some(e) => e.trim().to_string(),
                None => {
                    // Return emails for selection without tokens
                    // Don't remove the code yet - allow re-exchange with email
                    drop(store);
                    return Ok(Json(ExchangeAuthCodeResponse {
                        access_token: String::new(),
                        refresh_token: String::new(),
                        access_token_expires_at: 0,
                        refresh_token_expires_at: 0,
                        token_type: "Bearer".to_string(),
                        emails: entry.emails,
                        select_email: true,
                    }));
                }
            };

            // Validate email is in verified emails list
            let email_valid = entry.emails.iter().any(|e| e.email == email && e.verified);

            if !email_valid {
                drop(store);
                return Err(StatusCode::BAD_REQUEST);
            }

            // Regenerate tokens with selected email using GitHub info from TokenExchangeEntry
            let new_tokens = auth_state
                .jwt_service
                .generate_token_pair(
                    &email,
                    entry.github_id,
                    &entry.github_username,
                    &entry.session_id,
                )
                .map_err(|e| {
                    warn!("Failed to generate tokens with selected email: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Update session selected_email in memory store
            let mut sessions = auth_state.session_store.lock().await;
            if let Some(session) = sessions.get_mut(&entry.session_id) {
                session.selected_email = Some(email.clone());
            }
            drop(sessions);

            // Also update database session if available
            if let Some(db_session_store) = auth_state.app_state.db_session_store()
                && let Ok(session_uuid) = Uuid::parse_str(&entry.session_id)
            {
                let _ = db_session_store
                    .update_selected_email(session_uuid, &email)
                    .await;
            }

            (new_tokens, Some(email), true) // Remove code after successful token generation
        } else {
            // Auto-select primary email or use existing tokens
            let primary_email = entry
                .emails
                .iter()
                .find(|e| e.primary && e.verified)
                .or_else(|| entry.emails.iter().find(|e| e.verified))
                .map(|e| e.email.clone())
                .unwrap_or_else(String::new);

            if entry.select_email && entry.emails.len() == 1 {
                // Single email - auto-select
                (entry.tokens, Some(primary_email), true) // Remove code after successful token generation
            } else {
                // No selection needed - use existing tokens
                (entry.tokens, None, true) // Remove code after successful token generation
            }
        };

    // Remove the code from store only after successful token generation
    if should_remove_code {
        store.remove(&request.code);
    }
    drop(store);

    Ok(Json(ExchangeAuthCodeResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        access_token_expires_at: tokens.access_token_expires_at,
        refresh_token_expires_at: tokens.refresh_token_expires_at,
        token_type: tokens.token_type,
        emails: if selected_email.is_some() {
            Vec::new() // Don't return emails if already selected
        } else {
            entry.emails
        },
        select_email: selected_email.is_none() && entry.select_email,
    }))
}

fn legacy_token_redirect_enabled() -> bool {
    match std::env::var("AUTH_LEGACY_TOKEN_REDIRECT") {
        Ok(v) => matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

fn build_web_exchange_redirect(frontend_url: &str, code: &str, select_email: bool) -> String {
    let encoded_code = urlencoding::encode(code);
    let select_email = if select_email { "true" } else { "false" };
    format!(
        "{}/auth/complete?code={}&select_email={}",
        frontend_url, encoded_code, select_email
    )
}

/// Validate redirect_uri to prevent open redirect vulnerabilities
fn validate_redirect_uri(uri: &str) -> bool {
    // Parse URL
    let parsed = match Url::parse(uri) {
        Ok(url) => url,
        Err(_) => return false,
    };

    // Must be http or https
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }

    // In production, enforce HTTPS (optional - can be configured)
    let enforce_https = std::env::var("ENFORCE_HTTPS_REDIRECT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);
    if enforce_https && parsed.scheme() != "https" {
        return false;
    }

    // Allow localhost for development
    if matches!(
        parsed.host_str(),
        Some(host) if host == "localhost" || host == "127.0.0.1" || host == "::1"
    ) {
        return true;
    }

    // Check against whitelist if configured
    if let Ok(whitelist) = std::env::var("REDIRECT_URI_WHITELIST") {
        let allowed: Vec<&str> = whitelist.split(',').map(|s| s.trim()).collect();
        if let Some(host) = parsed.host_str() {
            return allowed.iter().any(|&allowed_host| {
                host == allowed_host || host.ends_with(&format!(".{}", allowed_host))
            });
        }
    }

    // Default: allow localhost only (safe default)
    // In production, configure REDIRECT_URI_WHITELIST
    if let Some(host) = parsed.host_str() {
        return host == "localhost" || host == "127.0.0.1" || host == "::1";
    }

    false
}

/// GET /auth/github/callback - Handle GitHub OAuth callback
///
/// This handler supports both in-memory (file storage) and database-backed (PostgreSQL) sessions.
#[utoipa::path(
    get,
    path = "/auth/github/callback",
    tag = "Authentication",
    responses(
        (status = 302, description = "Redirect to frontend with auth code or error"),
        (status = 400, description = "Bad request - invalid callback parameters")
    )
)]
pub async fn handle_github_callback(
    State(auth_state): State<AuthState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<Redirect, StatusCode> {
    let code = match params.code.as_ref() {
        Some(c) if !c.is_empty() => c.as_str(),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let state = params.state.as_deref().unwrap_or("");
    if state.is_empty() {
        warn!("OAuth callback missing state");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate state and resolve source (CSRF protection).
    let entry = auth_state.oauth_state_store.lock().await.remove(state);
    let entry = match entry {
        Some(e) => e,
        None => {
            warn!("OAuth callback with unknown/expired state");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    info!("Received GitHub OAuth callback (validated state)");

    // Exchange code for GitHub access token
    let github_access_token = match auth_state.oauth_service.exchange_code(code).await {
        Ok(token) => token,
        Err(e) => {
            warn!("Failed to exchange OAuth code: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Fetch user info from GitHub
    let (github_id, username, emails) = match auth_state
        .oauth_service
        .fetch_user_info(&github_access_token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            warn!("Failed to fetch user info from GitHub: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Generate session ID
    let session_uuid = Uuid::new_v4();
    let session_id = session_uuid.to_string();

    // Use primary email or first verified email as initial subject
    let primary_email = emails
        .iter()
        .find(|e| e.primary && e.verified)
        .or_else(|| emails.iter().find(|e| e.verified))
        .map(|e| e.email.clone())
        .unwrap_or_else(|| format!("{}@github", username));

    let selected_email = if emails.len() == 1 {
        Some(primary_email.clone())
    } else {
        None
    };

    // Generate JWT token pair
    let tokens = auth_state
        .jwt_service
        .generate_token_pair(&primary_email, github_id, &username, &session_id)
        .map_err(|e| {
            warn!("Failed to generate JWT tokens: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store session - use database if available, otherwise in-memory
    if let Some(db_session_store) = auth_state.app_state.db_session_store() {
        // PostgreSQL storage mode - create session in database
        // Use deterministic user_id based on email for consistency
        let user_id = Uuid::new_v5(&Uuid::NAMESPACE_DNS, primary_email.as_bytes());

        // Convert emails to EmailInfo
        let email_infos: Vec<crate::storage::traits::EmailInfo> = emails
            .iter()
            .map(|e| crate::storage::traits::EmailInfo {
                email: e.email.clone(),
                verified: e.verified,
                primary: e.primary,
            })
            .collect();

        // Create session in database
        db_session_store
            .create_session(crate::storage::session_store::CreateSessionParams {
                session_id: session_uuid,
                user_id,
                github_id,
                github_username: username.clone(),
                github_access_token: github_access_token.clone(),
                emails: email_infos,
                selected_email: selected_email.clone(),
            })
            .await
            .map_err(|e| {
                warn!("Failed to create session in database: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            "Created database session for GitHub user: {} (session: {}, user_id: {})",
            username, session_id, user_id
        );
    } else {
        // In-memory storage mode (legacy)
        let user_id = crate::routes::workspace::get_or_create_file_user_id(&primary_email)
            .unwrap_or_else(|_| Uuid::new_v4());
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::days(7); // 7 days expiry
        let session = SessionMetadata {
            user_id,
            github_id,
            github_username: username.clone(),
            github_access_token,
            emails: emails.clone(),
            selected_email: selected_email.clone(),
            created_at: now,
            last_activity: now,
            revoked_at: None,
            expires_at,
        };
        auth_state
            .session_store
            .lock()
            .await
            .insert(session_id.clone(), session);
        info!(
            "Created in-memory session for GitHub user: {} (session: {})",
            username, session_id
        );
    }

    // Determine redirect URL: use stored redirect_uri, fallback to FRONTEND_URL, then default
    let frontend_url = entry
        .redirect_uri
        .clone()
        .or_else(|| std::env::var("FRONTEND_URL").ok())
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    // Check if this is a desktop auth flow
    if let OAuthSource::Desktop { state_id } = entry.source {
        // Update pending auth with tokens
        let mut pending_store = auth_state.pending_auth_store.lock().await;
        if let Some(pending) = pending_store.get_mut(&state_id) {
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
        if legacy_token_redirect_enabled() {
            // Legacy mode (unsafe): include tokens in URL.
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
            info!("Redirecting web client to auth complete page (legacy token redirect enabled)");
            return Ok(Redirect::temporary(&redirect_url));
        }

        // Secure mode: store tokens server-side and redirect with short-lived one-time code.
        let exchange_code = Uuid::new_v4().to_string();
        auth_state.token_exchange_store.lock().await.insert(
            exchange_code.clone(),
            TokenExchangeEntry {
                tokens: tokens.clone(),
                emails: emails.clone(),
                select_email: emails.len() > 1,
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(2),
                github_id,
                github_username: username.clone(),
                session_id: session_id.clone(),
            },
        );

        let redirect_url =
            build_web_exchange_redirect(&frontend_url, &exchange_code, emails.len() > 1);
        info!("Redirecting web client to auth complete page (code exchange)");
        Ok(Redirect::temporary(&redirect_url))
    }
}

/// POST /auth/refresh - Refresh access token using refresh token
///
/// Supports both in-memory and database-backed session validation.
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "Authentication",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refreshed successfully", body = RefreshTokenResponse),
        (status = 401, description = "Unauthorized - invalid or expired refresh token")
    )
)]
async fn refresh_token(
    State(auth_state): State<AuthState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<RefreshTokenResponse>, StatusCode> {
    // Validate and decode refresh token
    let claims = auth_state
        .jwt_service
        .validate_refresh_token(&request.refresh_token)
        .map_err(|e| {
            warn!("Invalid refresh token: {}", e);
            StatusCode::UNAUTHORIZED
        })?;

    let session_uuid = Uuid::parse_str(&claims.session_id).map_err(|_| {
        warn!("Invalid session ID format: {}", claims.session_id);
        StatusCode::UNAUTHORIZED
    })?;

    // Check session validity - use database if available, otherwise in-memory
    if let Some(db_session_store) = auth_state.app_state.db_session_store() {
        // Database-backed session validation
        let is_valid = db_session_store
            .is_session_valid(session_uuid)
            .await
            .map_err(|e| {
                warn!("Failed to check session validity in database: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if !is_valid {
            warn!(
                "Session {} is not valid (expired or revoked)",
                claims.session_id
            );
            return Err(StatusCode::UNAUTHORIZED);
        }

        // Update last activity in database
        if let Err(e) = db_session_store.update_session_activity(session_uuid).await {
            warn!("Failed to update session activity: {}", e);
            // Non-fatal, continue with refresh
        }
    } else {
        // In-memory session validation (legacy)
        let revoked = auth_state.revoked_tokens.lock().await;
        if revoked.contains(&claims.session_id) {
            warn!(
                "Attempted to refresh revoked session: {}",
                claims.session_id
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
        drop(revoked);

        let sessions = auth_state.session_store.lock().await;
        if !sessions.contains_key(&claims.session_id) {
            warn!("Session not found for refresh: {}", claims.session_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
        drop(sessions);

        // Update last activity in memory
        let mut sessions = auth_state.session_store.lock().await;
        if let Some(session) = sessions.get_mut(&claims.session_id) {
            session.last_activity = chrono::Utc::now();
        }
    }

    // Generate new token pair
    let new_tokens = auth_state
        .jwt_service
        .refresh_access_token(&request.refresh_token)
        .map_err(|e| {
            warn!("Failed to refresh token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

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
///
/// Supports both in-memory and database-backed session queries.
#[utoipa::path(
    get,
    path = "/auth/status",
    tag = "Authentication",
    responses(
        (status = 200, description = "Auth status retrieved successfully", body = AuthStatusResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_auth_status(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AuthStatusResponse>, StatusCode> {
    // Try to get token from Authorization header (preferred) or x-session-id (legacy)
    let claims = extract_and_validate_token(&auth_state, &headers).await;

    if let Some(claims) = claims {
        let session_uuid = match Uuid::parse_str(&claims.session_id) {
            Ok(uuid) => uuid,
            Err(_) => {
                return Ok(Json(AuthStatusResponse {
                    authenticated: false,
                    github_username: None,
                    emails: Vec::new(),
                    selected_email: None,
                    token_expires_at: None,
                }));
            }
        };

        // Check session validity - use database if available, otherwise in-memory
        if let Some(db_session_store) = auth_state.app_state.db_session_store() {
            // Database-backed session query
            match db_session_store.get_session(session_uuid).await {
                Ok(Some(session)) => {
                    // Check if session is valid (not revoked, not expired)
                    if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
                        return Ok(Json(AuthStatusResponse {
                            authenticated: false,
                            github_username: None,
                            emails: Vec::new(),
                            selected_email: None,
                            token_expires_at: None,
                        }));
                    }

                    // Update last activity (fire-and-forget)
                    let _ = db_session_store.update_session_activity(session_uuid).await;

                    // For database mode, we need to fetch user info separately
                    // The session only stores selected_email, not full user data
                    // For now, return what we have from the JWT claims
                    return Ok(Json(AuthStatusResponse {
                        authenticated: true,
                        github_username: Some(claims.github_username.clone()),
                        emails: session.emails.clone(),
                        selected_email: session.selected_email,
                        token_expires_at: Some(claims.exp),
                    }));
                }
                Ok(None) => {
                    warn!("Session {} not found in database", claims.session_id);
                }
                Err(e) => {
                    warn!("Failed to get session from database: {}", e);
                }
            }
        } else {
            // In-memory session query (legacy)
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
    }

    Ok(Json(AuthStatusResponse {
        authenticated: false,
        github_username: None,
        emails: Vec::new(),
        selected_email: None,
        token_expires_at: None,
    }))
}

/// GET /api/v1/auth/me - Get current authenticated user information
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "Authentication",
    responses(
        (status = 200, description = "User information retrieved successfully", body = UserInfoResponse),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_current_user(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<UserInfoResponse>, StatusCode> {
    // Extract and validate token
    let claims = extract_and_validate_token(&auth_state, &headers)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if session is revoked
    let revoked = auth_state.revoked_tokens.lock().await;
    if revoked.contains(&claims.session_id) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    drop(revoked);

    // Get session to extract user information
    let session_uuid = match Uuid::parse_str(&claims.session_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Try database session first, then in-memory
    let (github_username, email) =
        if let Some(db_session_store) = auth_state.app_state.db_session_store() {
            match db_session_store.get_session(session_uuid).await {
                Ok(Some(session)) => {
                    // Check if session is valid
                    if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                    let email = session.selected_email.unwrap_or_else(|| claims.sub.clone());
                    (session.github_username, email)
                }
                _ => {
                    // Fall back to JWT claims
                    (claims.github_username.clone(), claims.sub.clone())
                }
            }
        } else {
            // In-memory session
            let sessions = auth_state.session_store.lock().await;
            if let Some(session) = sessions.get(&claims.session_id) {
                if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                let email = session
                    .selected_email
                    .clone()
                    .unwrap_or_else(|| claims.sub.clone());
                (session.github_username.clone(), email)
            } else {
                // Fall back to JWT claims
                (claims.github_username.clone(), claims.sub.clone())
            }
        };

    // Generate user ID from email (consistent with existing pattern)
    let user_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, email.as_bytes());

    Ok(Json(UserInfoResponse {
        user: UserInfo {
            id: user_id.to_string(),
            name: github_username,
            email,
        },
    }))
}

/// POST /auth/select-email - Select email for workspace creation
#[utoipa::path(
    post,
    path = "/auth/select-email",
    tag = "Authentication",
    request_body = SelectEmailRequest,
    responses(
        (status = 200, description = "Email selected successfully", body = SelectEmailResponse),
        (status = 400, description = "Bad request - invalid email"),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
async fn select_email(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<SelectEmailRequest>,
) -> Result<Json<SelectEmailResponse>, StatusCode> {
    // Support both Bearer token auth and exchange code auth
    // This allows selecting email when initial exchange returned empty tokens
    let (session_id, github_id, github_username, _emails) = if let Some(code) = &request.code {
        // Use exchange code for authentication
        let mut store = auth_state.token_exchange_store.lock().await;
        let entry = match store.get(code) {
            Some(e) => e.clone(),
            None => {
                drop(store);
                return Err(StatusCode::BAD_REQUEST);
            }
        };

        if chrono::Utc::now() > entry.expires_at {
            drop(store);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Validate email is in verified emails list
        let email_valid = entry
            .emails
            .iter()
            .any(|e| e.email == request.email && e.verified);
        if !email_valid {
            drop(store);
            return Err(StatusCode::BAD_REQUEST);
        }

        let session_id = entry.session_id.clone();
        let github_id = entry.github_id;
        let github_username = entry.github_username.clone();
        let emails = entry.emails.clone();

        // Remove the code after use
        store.remove(code);
        drop(store);

        (session_id, github_id, github_username, emails)
    } else {
        // Use Bearer token for authentication
        let claims = extract_and_validate_token(&auth_state, &headers)
            .await
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Check if session is revoked
        let revoked = auth_state.revoked_tokens.lock().await;
        if revoked.contains(&claims.session_id) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        drop(revoked);

        // Get session to extract user information and validate email
        let session_uuid = match Uuid::parse_str(&claims.session_id) {
            Ok(uuid) => uuid,
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        };

        // Try database session first, then in-memory
        let (github_id, github_username, emails) =
            if let Some(db_session_store) = auth_state.app_state.db_session_store() {
                match db_session_store.get_session(session_uuid).await {
                    Ok(Some(session)) => {
                        // Check if session is valid
                        if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
                            return Err(StatusCode::UNAUTHORIZED);
                        }

                        // Validate email is in user's GitHub emails
                        let email_valid = session
                            .emails
                            .iter()
                            .any(|e| e.email == request.email && e.verified);

                        if !email_valid {
                            return Err(StatusCode::BAD_REQUEST);
                        }

                        // Update selected email in database
                        if let Err(e) = db_session_store
                            .update_selected_email(session_uuid, &request.email)
                            .await
                        {
                            warn!("Failed to update selected email in database: {}", e);
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }

                        (
                            session.github_id,
                            session.github_username,
                            session.emails.clone(),
                        )
                    }
                    Ok(None) => {
                        // Session not found in database, try in-memory fallback
                        let sessions = auth_state.session_store.lock().await;
                        let session = sessions
                            .get(&claims.session_id)
                            .ok_or(StatusCode::UNAUTHORIZED)?;

                        // Validate email is in user's GitHub emails
                        let email_valid = session
                            .emails
                            .iter()
                            .any(|e| e.email == request.email && e.verified);

                        if !email_valid {
                            return Err(StatusCode::BAD_REQUEST);
                        }

                        let github_id = session.github_id;
                        let github_username = session.github_username.clone();
                        let emails = session.emails.clone();
                        drop(sessions);

                        // Update in-memory session
                        let mut sessions = auth_state.session_store.lock().await;
                        if let Some(session) = sessions.get_mut(&claims.session_id) {
                            session.selected_email = Some(request.email.clone());
                        }
                        drop(sessions);

                        (github_id, github_username, emails)
                    }
                    Err(e) => {
                        warn!("Failed to get session from database: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            } else {
                // In-memory session (file-based mode)
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
                let emails = session.emails.clone();
                drop(sessions);

                (github_id, github_username, emails)
            };

        (claims.session_id, github_id, github_username, emails)
    };

    // Generate new tokens with the selected email as subject
    let new_tokens = auth_state
        .jwt_service
        .generate_token_pair(&request.email, github_id, &github_username, &session_id)
        .map_err(|e| {
            warn!("Failed to generate new tokens: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Create workspace with selected email
    let mut model_service = auth_state.app_state.model_service.lock().await;

    match workspace::create_workspace_for_email(&mut model_service, &request.email).await {
        Ok(workspace_path) => {
            info!(
                "Created workspace for GitHub user {} with email {}",
                github_username, request.email
            );
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
///
/// Supports both in-memory and database-backed session revocation.
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "Authentication",
    responses(
        (status = 200, description = "Logged out successfully", body = Object),
        (status = 401, description = "Unauthorized - invalid or missing token")
    ),
    security(("bearer_auth" = []))
)]
async fn logout(
    State(auth_state): State<AuthState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(claims) = extract_and_validate_token(&auth_state, &headers).await {
        let session_uuid = Uuid::parse_str(&claims.session_id).ok();

        // Revoke session - use database if available, otherwise in-memory
        if let Some(db_session_store) = auth_state.app_state.db_session_store() {
            // Database-backed session revocation
            if let Some(uuid) = session_uuid {
                if let Err(e) = db_session_store.revoke_session(uuid).await {
                    warn!("Failed to revoke session in database: {}", e);
                } else {
                    info!("Revoked session in database: {}", claims.session_id);
                }
            }
        } else {
            // In-memory session revocation (legacy)
            auth_state
                .revoked_tokens
                .lock()
                .await
                .insert(claims.session_id.clone());
            auth_state
                .session_store
                .lock()
                .await
                .remove(&claims.session_id);
            info!(
                "Logged out and revoked in-memory session: {}",
                claims.session_id
            );
        }
    }

    Ok(Json(
        serde_json::json!({ "message": "Logged out successfully" }),
    ))
}

/// Extract and validate JWT token from request headers
/// Supports both Authorization: Bearer `token` and x-session-id (legacy)
async fn extract_and_validate_token(
    auth_state: &AuthState,
    headers: &axum::http::HeaderMap,
) -> Option<Claims> {
    // Try Authorization header first (preferred)
    if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok())
        && let Some(token) = JwtService::extract_bearer_token(auth_header)
        && let Ok(claims) = auth_state.jwt_service.validate_access_token(token)
    {
        return Some(claims);
    }

    // Try x-session-id header (legacy - treat as token)
    if let Some(token) = headers.get("x-session-id").and_then(|h| h.to_str().ok()) {
        // Check if it looks like a JWT (contains dots)
        if token.contains('.')
            && let Ok(claims) = auth_state.jwt_service.validate_access_token(token)
        {
            return Some(claims);
        }
    }

    None
}

/// Helper function to validate token and get claims (for use in other modules)
///
/// This is the legacy version for in-memory session validation.
#[allow(dead_code)]
pub async fn validate_request_token(
    session_store: &SessionStore,
    revoked_tokens: &RevokedTokens,
    jwt_service: &JwtService,
    headers: &axum::http::HeaderMap,
) -> Result<Claims, StatusCode> {
    // Try Authorization header first
    let token =
        if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
            JwtService::extract_bearer_token(auth_header)
        } else {
            // Try x-session-id as fallback
            headers.get("x-session-id").and_then(|h| h.to_str().ok())
        };

    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let claims = jwt_service.validate_access_token(token).map_err(|e| {
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

/// Helper function to validate token with database-backed session store.
///
/// This version supports both in-memory and database-backed sessions.
#[allow(dead_code)]
pub async fn validate_request_token_with_db(
    app_state: &AppState,
    jwt_service: &JwtService,
    headers: &axum::http::HeaderMap,
) -> Result<Claims, StatusCode> {
    // Try Authorization header first
    let token =
        if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
            JwtService::extract_bearer_token(auth_header)
        } else {
            // Try x-session-id as fallback
            headers.get("x-session-id").and_then(|h| h.to_str().ok())
        };

    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token
    let claims = jwt_service.validate_access_token(token).map_err(|e| {
        warn!("Token validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let session_uuid = Uuid::parse_str(&claims.session_id).map_err(|_| {
        warn!("Invalid session ID format: {}", claims.session_id);
        StatusCode::UNAUTHORIZED
    })?;

    // Check session validity - use database if available, otherwise in-memory
    if let Some(db_session_store) = app_state.db_session_store() {
        // Database-backed session validation
        let is_valid = db_session_store
            .is_session_valid(session_uuid)
            .await
            .map_err(|e| {
                warn!("Failed to check session validity in database: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if !is_valid {
            warn!(
                "Session {} is not valid (expired or revoked)",
                claims.session_id
            );
            return Err(StatusCode::UNAUTHORIZED);
        }

        // Update last activity in database (fire-and-forget)
        let _ = db_session_store.update_session_activity(session_uuid).await;
    } else {
        // In-memory session validation (legacy)
        // Note: This path requires access to session_store and revoked_tokens
        // which are not available here. Use validate_request_token for legacy mode.
        warn!("validate_request_token_with_db called without database session store");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(claims)
}
