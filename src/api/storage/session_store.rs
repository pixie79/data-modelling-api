//! Session store for PostgreSQL.
//!
//! Provides database-backed session storage.

use crate::routes::auth::SessionMetadata;
use crate::storage::traits::EmailInfo;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// Session creation parameters
pub struct CreateSessionParams {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub github_id: u64,
    pub github_username: String,
    pub github_access_token: String,
    pub emails: Vec<EmailInfo>,
    pub selected_email: Option<String>,
}

/// Database-backed session store
pub struct DbSessionStore {
    pool: PgPool,
}

impl DbSessionStore {
    /// Create a new database session store
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a session by ID
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<SessionMetadata>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT id, user_id, github_id, github_username, github_access_token, emails, selected_email, created_at, last_activity, expires_at
            FROM sessions
            WHERE id = $1 AND expires_at > NOW()
            "#,
            session_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            let emails: Vec<serde_json::Value> =
                serde_json::from_value(r.emails).unwrap_or_default();
            let email_infos: Vec<EmailInfo> = emails
                .into_iter()
                .filter_map(|e| serde_json::from_value(e).ok())
                .collect();

            // Convert EmailInfo to GitHubEmail
            let github_emails: Vec<crate::services::oauth_service::GitHubEmail> = email_infos
                .into_iter()
                .map(|e| crate::services::oauth_service::GitHubEmail {
                    email: e.email,
                    verified: e.verified,
                    primary: e.primary,
                })
                .collect();

            Ok(Some(SessionMetadata {
                user_id: r.user_id,
                github_id: r.github_id as u64,
                github_username: r.github_username,
                github_access_token: r.github_access_token,
                emails: github_emails,
                selected_email: r.selected_email,
                created_at: r.created_at,
                last_activity: r.last_activity,
                revoked_at: None, // Not stored in DB currently
                expires_at: r.expires_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create a new session
    pub async fn create_session(&self, params: CreateSessionParams) -> Result<(), sqlx::Error> {
        let emails_json = serde_json::to_value(params.emails).unwrap_or(serde_json::json!([]));
        let expires_at = Utc::now() + chrono::Duration::days(7); // 7 days expiry

        sqlx::query!(
            r#"
            INSERT INTO sessions (id, user_id, github_id, github_username, github_access_token, emails, selected_email, created_at, last_activity, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW(), $8)
            ON CONFLICT (id) DO UPDATE SET
                last_activity = NOW(),
                expires_at = $8
            "#,
            params.session_id,
            params.user_id,
            params.github_id as i64,
            params.github_username,
            params.github_access_token,
            emails_json,
            params.selected_email,
            expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update session activity timestamp
    pub async fn update_session_activity(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE sessions
            SET last_activity = NOW()
            WHERE id = $1 AND expires_at > NOW()
            "#,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update selected email for a session
    pub async fn update_selected_email(
        &self,
        session_id: Uuid,
        selected_email: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE sessions
            SET selected_email = $1, last_activity = NOW()
            WHERE id = $2 AND expires_at > NOW()
            "#,
            selected_email,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if session is valid
    pub async fn is_session_valid(&self, session_id: Uuid) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM sessions
            WHERE id = $1 AND expires_at > NOW()
            "#,
            session_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0) > 0)
    }

    /// Revoke a session
    pub async fn revoke_session(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM sessions
            WHERE id = $1
            "#,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Start background task to clean up expired sessions
pub async fn start_session_cleanup_task(pool: PgPool) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Run every hour

    loop {
        interval.tick().await;

        // Delete expired sessions
        if let Err(e) = sqlx::query!(
            r#"
            DELETE FROM sessions
            WHERE expires_at < NOW()
            "#,
        )
        .execute(&pool)
        .await
        {
            tracing::error!("Failed to cleanup expired sessions: {}", e);
        }
    }
}
