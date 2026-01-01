//! Authentication context utilities.
//!
//! Provides helper functions and extractors for authentication context.

use super::app_state::AppState;
use crate::services::jwt_service::JwtService;
use crate::storage::traits::UserContext;
use axum::extract::FromRequestParts;
use axum::http::{StatusCode, request::Parts};

/// Authentication context extracted from request
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub user_context: UserContext,
    #[allow(dead_code)] // Used for session tracking in future features
    pub session_id: Option<String>,
    pub email: String,
}

impl AuthContext {
    /// Create from user context
    #[allow(dead_code)] // Reserved for future use
    pub fn from_user_context(user_context: UserContext, session_id: Option<String>) -> Self {
        Self {
            email: user_context.email.clone(),
            user_context,
            session_id,
        }
    }
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Use AppState directly from router state
        let app_state = state;

        let headers = parts.headers.clone();
        let jwt_service = JwtService::from_env();

        // Try Authorization header first (preferred)
        let token =
            if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
                JwtService::extract_bearer_token(auth_header)
            } else {
                headers.get("x-session-id").and_then(|h| h.to_str().ok())
            };

        let token = token.ok_or_else(|| {
            tracing::warn!("No authorization token provided");
            StatusCode::UNAUTHORIZED
        })?;

        let claims = jwt_service.validate_access_token(token).map_err(|e| {
            tracing::warn!("JWT validation failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;

        if claims.sub.is_empty() {
            tracing::warn!("JWT has empty subject claim");
            return Err(StatusCode::BAD_REQUEST);
        }

        // For file-based mode, verify session exists in memory store
        let sessions = app_state.session_store.lock().await;
        if !sessions.contains_key(&claims.session_id) {
            tracing::warn!("Session {} not found in store", claims.session_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
        drop(sessions);

        // For file-based mode, we use a deterministic UUID based on email
        let user_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, claims.sub.as_bytes());

        let user_context = UserContext {
            user_id,
            email: claims.sub.clone(),
        };

        Ok(AuthContext {
            user_context,
            session_id: Some(claims.session_id),
            email: claims.sub,
        })
    }
}
