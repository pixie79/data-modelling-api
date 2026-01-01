//! Collaboration storage for PostgreSQL.
//!
//! Provides database operations for collaboration sessions.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Collaboration store for database operations
pub struct CollaborationStore {
    pool: PgPool,
}

/// Collaboration session information
#[derive(Debug, Clone)]
pub struct CollaborationSession {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub domain_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)] // Used for session tracking
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Collaboration participant information
#[derive(Debug, Clone)]
pub struct CollaborationParticipant {
    pub id: Uuid,
    #[allow(dead_code)] // Used for participant tracking
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub permission: String,
    pub joined_at: DateTime<Utc>,
    #[allow(dead_code)] // Used for presence tracking
    pub last_seen: DateTime<Utc>,
    pub is_online: bool,
    pub cursor_x: Option<f64>,
    pub cursor_y: Option<f64>,
    pub selected_tables: Value,
    pub selected_relationships: Value,
    pub editing_table: Option<Uuid>,
}

/// Collaboration request information
#[derive(Debug, Clone)]
pub struct CollaborationRequest {
    pub id: Uuid,
    #[allow(dead_code)] // Used for request tracking
    pub session_id: Uuid,
    pub requester_id: Uuid,
    pub status: String,
    #[allow(dead_code)] // Used for request tracking
    pub requested_at: DateTime<Utc>,
    #[allow(dead_code)] // Used for request tracking
    pub responded_at: Option<DateTime<Utc>>,
    #[allow(dead_code)] // Used for request tracking
    pub responder_id: Option<Uuid>,
}

impl CollaborationStore {
    /// Create a new collaboration store
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new collaboration session
    pub async fn create_session(
        &self,
        workspace_id: Uuid,
        domain_id: Option<Uuid>,
        name: String,
        description: Option<String>,
        created_by: Uuid,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CollaborationSession, sqlx::Error> {
        let session_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO collaboration_sessions (id, workspace_id, domain_id, name, description, created_by, created_at, updated_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            session_id,
            workspace_id,
            domain_id,
            name,
            description,
            created_by,
            now,
            now,
            expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(CollaborationSession {
            id: session_id,
            workspace_id,
            domain_id,
            name,
            description,
            created_by,
            created_at: now,
            updated_at: now,
            expires_at,
        })
    }

    /// Get a collaboration session by ID
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<CollaborationSession>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT id, workspace_id, domain_id, name, description, created_by, created_at, updated_at, expires_at
            FROM collaboration_sessions
            WHERE id = $1
            "#,
            session_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            Ok(Some(CollaborationSession {
                id: r.id,
                workspace_id: r.workspace_id,
                domain_id: r.domain_id,
                name: r.name,
                description: r.description,
                created_by: r.created_by,
                created_at: r.created_at,
                updated_at: r.updated_at,
                expires_at: r.expires_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// List collaboration sessions for a workspace
    pub async fn list_sessions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<CollaborationSession>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, workspace_id, domain_id, name, description, created_by, created_at, updated_at, expires_at
            FROM collaboration_sessions
            WHERE workspace_id = $1
            ORDER BY created_at DESC
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CollaborationSession {
                id: r.id,
                workspace_id: r.workspace_id,
                domain_id: r.domain_id,
                name: r.name,
                description: r.description,
                created_by: r.created_by,
                created_at: r.created_at,
                updated_at: r.updated_at,
                expires_at: r.expires_at,
            })
            .collect())
    }

    /// Delete a collaboration session
    pub async fn delete_session(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM collaboration_sessions
            WHERE id = $1
            "#,
            session_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Add a participant to a session
    pub async fn add_participant(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        permission: String,
    ) -> Result<(), sqlx::Error> {
        let participant_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO collaboration_participants (id, session_id, user_id, permission, joined_at, last_seen, is_online)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            ON CONFLICT (session_id, user_id) DO UPDATE SET
                permission = $4,
                is_online = true,
                last_seen = $6
            "#,
            participant_id,
            session_id,
            user_id,
            permission,
            now,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List participants for a session
    pub async fn list_participants(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<CollaborationParticipant>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, session_id, user_id, permission, joined_at, last_seen, is_online, cursor_x, cursor_y, selected_tables, selected_relationships, editing_table
            FROM collaboration_participants
            WHERE session_id = $1
            ORDER BY joined_at
            "#,
            session_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CollaborationParticipant {
                id: r.id,
                session_id: r.session_id,
                user_id: r.user_id,
                permission: r.permission,
                joined_at: r.joined_at,
                last_seen: r.last_seen,
                is_online: r.is_online,
                cursor_x: r.cursor_x,
                cursor_y: r.cursor_y,
                selected_tables: r.selected_tables.unwrap_or(serde_json::json!([])),
                selected_relationships: r.selected_relationships.unwrap_or(serde_json::json!([])),
                editing_table: r.editing_table,
            })
            .collect())
    }

    /// Remove a participant from a session
    pub async fn remove_participant(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM collaboration_participants
            WHERE session_id = $1 AND user_id = $2
            "#,
            session_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Create a collaboration request
    pub async fn create_request(
        &self,
        session_id: Uuid,
        requester_id: Uuid,
    ) -> Result<CollaborationRequest, sqlx::Error> {
        let request_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO collaboration_requests (id, session_id, requester_id, status, requested_at)
            VALUES ($1, $2, $3, 'pending', $4)
            ON CONFLICT (session_id, requester_id) WHERE status = 'pending' DO NOTHING
            "#,
            request_id,
            session_id,
            requester_id,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(CollaborationRequest {
            id: request_id,
            session_id,
            requester_id,
            status: "pending".to_string(),
            requested_at: now,
            responded_at: None,
            responder_id: None,
        })
    }

    /// List pending requests for a session
    pub async fn list_pending_requests(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<CollaborationRequest>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, session_id, requester_id, status, requested_at, responded_at, responder_id
            FROM collaboration_requests
            WHERE session_id = $1 AND status = 'pending'
            ORDER BY requested_at
            "#,
            session_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CollaborationRequest {
                id: r.id,
                session_id: r.session_id,
                requester_id: r.requester_id,
                status: r.status,
                requested_at: r.requested_at,
                responded_at: r.responded_at,
                responder_id: r.responder_id,
            })
            .collect())
    }

    /// Respond to a collaboration request
    pub async fn respond_to_request(
        &self,
        request_id: Uuid,
        responder_id: Uuid,
        approved: bool,
    ) -> Result<(), sqlx::Error> {
        let status = if approved { "approved" } else { "rejected" };
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE collaboration_requests
            SET status = $1, responded_at = $2, responder_id = $3
            WHERE id = $4
            "#,
            status,
            now,
            responder_id,
            request_id
        )
        .execute(&self.pool)
        .await?;

        // If approved, add the requester as a participant
        if approved {
            let request = sqlx::query!(
                r#"
                SELECT requester_id, session_id FROM collaboration_requests WHERE id = $1
                "#,
                request_id
            )
            .fetch_one(&self.pool)
            .await?;

            let _ = self
                .add_participant(
                    request.session_id,
                    request.requester_id,
                    "viewer".to_string(),
                )
                .await;
        }

        Ok(())
    }

    /// Update user presence
    #[allow(clippy::too_many_arguments)]
    pub async fn update_presence(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        is_online: bool,
        cursor_x: Option<f64>,
        cursor_y: Option<f64>,
        selected_tables: &[Uuid],
        selected_relationships: &[Uuid],
        editing_table: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let selected_tables_json =
            serde_json::to_value(selected_tables).unwrap_or(serde_json::json!([]));
        let selected_relationships_json =
            serde_json::to_value(selected_relationships).unwrap_or(serde_json::json!([]));
        let editing_table_uuid = editing_table;

        sqlx::query!(
            r#"
            INSERT INTO collaboration_participants (id, session_id, user_id, is_online, cursor_x, cursor_y, selected_tables, selected_relationships, editing_table, last_seen)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (session_id, user_id)
            DO UPDATE SET
                is_online = $3,
                cursor_x = $4,
                cursor_y = $5,
                selected_tables = $6,
                selected_relationships = $7,
                editing_table = $8,
                last_seen = NOW()
            "#,
            session_id,
            user_id,
            is_online,
            cursor_x,
            cursor_y,
            selected_tables_json,
            selected_relationships_json,
            editing_table_uuid
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get user presence for a session
    pub async fn get_presence(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<CollaborationParticipant>, sqlx::Error> {
        self.list_participants(session_id).await
    }

    /// Mark user as offline
    pub async fn mark_offline(&self, session_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE collaboration_participants
            SET is_online = false, last_seen = NOW()
            WHERE session_id = $1 AND user_id = $2
            "#,
            session_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Set user offline (alias for mark_offline)
    pub async fn set_user_offline(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        self.mark_offline(session_id, user_id).await
    }
}
