//! PostgreSQL storage backend implementation.
//!
//! Uses sqlx for database operations and implements the StorageBackend trait.

use super::{StorageError, traits::*};
use crate::models::{Relationship, Table};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// PostgreSQL storage backend implementation.
pub struct PostgresStorageBackend {
    pool: PgPool,
}

impl PostgresStorageBackend {
    /// Create a new PostgreSQL storage backend.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StorageBackend for PostgresStorageBackend {
    async fn get_workspace_by_email(
        &self,
        email: &str,
    ) -> Result<Option<WorkspaceInfo>, StorageError> {
        let result = sqlx::query_as!(
            WorkspaceInfo,
            r#"
            SELECT id, owner_id, email, name, type as workspace_type, created_at, updated_at
            FROM workspaces
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(result)
    }

    async fn create_workspace(
        &self,
        email: String,
        user_context: &UserContext,
    ) -> Result<WorkspaceInfo, StorageError> {
        // Legacy method - creates workspace with default name based on email
        let default_name = format!("Workspace {}", email.split('@').next().unwrap_or("default"));
        self.create_workspace_with_details(
            email,
            user_context,
            default_name,
            "personal".to_string(),
        )
        .await
    }

    async fn get_domain_by_name(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> Result<Option<DomainInfo>, StorageError> {
        let result = sqlx::query_as!(
            DomainInfo,
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at
            FROM domains
            WHERE workspace_id = $1 AND name = $2
            "#,
            workspace_id,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(result)
    }

    async fn create_domain(
        &self,
        workspace_id: Uuid,
        name: String,
        description: Option<String>,
        _user_context: &UserContext,
    ) -> Result<DomainInfo, StorageError> {
        let domain_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO domains (id, workspace_id, name, description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            domain_id,
            workspace_id,
            name,
            description,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(DomainInfo {
            id: domain_id,
            workspace_id,
            name,
            description,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_table(
        &self,
        domain_id: Uuid,
        table_id: Uuid,
    ) -> Result<Option<Table>, StorageError> {
        let row = sqlx::query!(
            r#"
            SELECT data
            FROM tables
            WHERE domain_id = $1 AND id = $2
            "#,
            domain_id,
            table_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        if let Some(row) = row {
            serde_json::from_value(row.data)
                .map_err(|e| StorageError::Other(format!("Failed to deserialize table: {}", e)))
        } else {
            Ok(None)
        }
    }

    async fn create_table(
        &self,
        domain_id: Uuid,
        table: Table,
        user_context: &UserContext,
    ) -> Result<Table, StorageError> {
        let data = serde_json::to_value(&table)
            .map_err(|e| StorageError::Other(format!("Failed to serialize table: {}", e)))?;

        sqlx::query!(
            r#"
            INSERT INTO tables (id, domain_id, name, data, version, created_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            table.id,
            domain_id,
            table.name,
            data,
            1i32,
            user_context.user_id,
            Utc::now(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(table)
    }

    async fn update_table(
        &self,
        table: Table,
        expected_version: Option<i32>,
        _user_context: &UserContext,
    ) -> Result<Table, StorageError> {
        let data = serde_json::to_value(&table)
            .map_err(|e| StorageError::Other(format!("Failed to serialize table: {}", e)))?;

        if let Some(expected_ver) = expected_version {
            // Optimistic locking check
            let current_version = sqlx::query_scalar!(
                r#"
                SELECT version FROM tables WHERE id = $1
                "#,
                table.id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound {
                entity_type: "table".to_string(),
                entity_id: table.id.to_string(),
            })?;

            if current_version != expected_ver {
                return Err(StorageError::VersionConflict {
                    entity_type: "table".to_string(),
                    entity_id: table.id.to_string(),
                    expected_version: expected_ver,
                    current_version,
                    current_data: Some(data),
                });
            }
        }

        let new_version = expected_version.map(|v| v + 1).unwrap_or(1);

        sqlx::query!(
            r#"
            UPDATE tables
            SET data = $1, version = $2, updated_by = $3, updated_at = $4
            WHERE id = $5
            "#,
            data,
            new_version,
            _user_context.user_id,
            Utc::now(),
            table.id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(table)
    }

    async fn delete_table(
        &self,
        domain_id: Uuid,
        table_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        let rows_affected = sqlx::query!(
            r#"
            DELETE FROM tables
            WHERE domain_id = $1 AND id = $2
            "#,
            domain_id,
            table_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(StorageError::NotFound {
                entity_type: "table".to_string(),
                entity_id: table_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn list_tables(&self, domain_id: Uuid) -> Result<Vec<Table>, StorageError> {
        let rows = sqlx::query!(
            r#"
            SELECT data
            FROM tables
            WHERE domain_id = $1
            ORDER BY name
            "#,
            domain_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let mut tables = Vec::new();
        for row in rows {
            let table: Table = serde_json::from_value(row.data)
                .map_err(|e| StorageError::Other(format!("Failed to deserialize table: {}", e)))?;
            tables.push(table);
        }

        Ok(tables)
    }

    async fn get_relationship(
        &self,
        domain_id: Uuid,
        relationship_id: Uuid,
    ) -> Result<Option<Relationship>, StorageError> {
        let row = sqlx::query!(
            r#"
            SELECT data
            FROM relationships
            WHERE domain_id = $1 AND id = $2
            "#,
            domain_id,
            relationship_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        if let Some(row) = row {
            serde_json::from_value(row.data).map_err(|e| {
                StorageError::Other(format!("Failed to deserialize relationship: {}", e))
            })
        } else {
            Ok(None)
        }
    }

    async fn create_relationship(
        &self,
        domain_id: Uuid,
        relationship: Relationship,
        _user_context: &UserContext,
    ) -> Result<Relationship, StorageError> {
        let data = serde_json::to_value(&relationship)
            .map_err(|e| StorageError::Other(format!("Failed to serialize relationship: {}", e)))?;

        // Generate a name from relationship ID for database indexing
        let name = format!("relationship-{}", relationship.id);

        sqlx::query!(
            r#"
            INSERT INTO relationships (id, domain_id, name, data, version, created_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            relationship.id,
            domain_id,
            name,
            data,
            1i32,
            _user_context.user_id,
            Utc::now(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(relationship)
    }

    async fn update_relationship(
        &self,
        relationship: Relationship,
        expected_version: Option<i32>,
        _user_context: &UserContext,
    ) -> Result<Relationship, StorageError> {
        let data = serde_json::to_value(&relationship)
            .map_err(|e| StorageError::Other(format!("Failed to serialize relationship: {}", e)))?;

        if let Some(expected_ver) = expected_version {
            let current_version = sqlx::query_scalar!(
                r#"
                SELECT version FROM relationships WHERE id = $1
                "#,
                relationship.id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound {
                entity_type: "relationship".to_string(),
                entity_id: relationship.id.to_string(),
            })?;

            if current_version != expected_ver {
                return Err(StorageError::VersionConflict {
                    entity_type: "relationship".to_string(),
                    entity_id: relationship.id.to_string(),
                    expected_version: expected_ver,
                    current_version,
                    current_data: Some(data),
                });
            }
        }

        let new_version = expected_version.map(|v| v + 1).unwrap_or(1);

        sqlx::query!(
            r#"
            UPDATE relationships
            SET data = $1, version = $2, updated_by = $3, updated_at = $4
            WHERE id = $5
            "#,
            data,
            new_version,
            _user_context.user_id,
            Utc::now(),
            relationship.id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(relationship)
    }

    async fn delete_relationship(
        &self,
        domain_id: Uuid,
        relationship_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        let rows_affected = sqlx::query!(
            r#"
            DELETE FROM relationships
            WHERE domain_id = $1 AND id = $2
            "#,
            domain_id,
            relationship_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(StorageError::NotFound {
                entity_type: "relationship".to_string(),
                entity_id: relationship_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn list_relationships(&self, domain_id: Uuid) -> Result<Vec<Relationship>, StorageError> {
        let rows = sqlx::query!(
            r#"
            SELECT data
            FROM relationships
            WHERE domain_id = $1
            ORDER BY name
            "#,
            domain_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let mut relationships = Vec::new();
        for row in rows {
            let relationship: Relationship = serde_json::from_value(row.data).map_err(|e| {
                StorageError::Other(format!("Failed to deserialize relationship: {}", e))
            })?;
            relationships.push(relationship);
        }

        Ok(relationships)
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, StorageError> {
        let results = sqlx::query_as!(
            WorkspaceInfo,
            r#"
            SELECT id, owner_id, email, name, type as workspace_type, created_at, updated_at
            FROM workspaces
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(results)
    }

    async fn get_workspaces_by_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<WorkspaceInfo>, StorageError> {
        let results = sqlx::query_as!(
            WorkspaceInfo,
            r#"
            SELECT id, owner_id, email, name, type as workspace_type, created_at, updated_at
            FROM workspaces
            WHERE owner_id = $1
            ORDER BY created_at DESC
            "#,
            owner_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(results)
    }

    async fn create_workspace_with_details(
        &self,
        email: String,
        user_context: &UserContext,
        name: String,
        workspace_type: String,
    ) -> Result<WorkspaceInfo, StorageError> {
        let workspace_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO workspaces (id, owner_id, email, name, type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            workspace_id,
            user_context.user_id,
            email,
            name,
            workspace_type,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(WorkspaceInfo {
            id: workspace_id,
            owner_id: user_context.user_id,
            email,
            name: Some(name),
            workspace_type: Some(workspace_type),
            created_at: now,
            updated_at: now,
        })
    }

    async fn workspace_name_exists(&self, email: &str, name: &str) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM workspaces
            WHERE email = $1 AND name = $2
            "#,
            email,
            name
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(result.count.unwrap_or(0) > 0)
    }

    async fn get_domains(&self, workspace_id: Uuid) -> Result<Vec<DomainInfo>, StorageError> {
        let results = sqlx::query_as!(
            DomainInfo,
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at
            FROM domains
            WHERE workspace_id = $1
            ORDER BY name
            "#,
            workspace_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(results)
    }

    async fn get_cross_domain_refs(
        &self,
        domain_id: Uuid,
    ) -> Result<Vec<CrossDomainRef>, StorageError> {
        // Check if cross_domain_refs table exists, if not return empty vec
        // This allows the API to work without the migration
        let rows = sqlx::query!(
            r#"
            SELECT id, target_domain_id, source_domain_id, table_id, display_alias, position_x, position_y, notes
            FROM cross_domain_refs
            WHERE target_domain_id = $1
            ORDER BY created_at
            "#,
            domain_id
        )
        .fetch_all(&self.pool)
        .await;

        match rows {
            Ok(rows) => {
                let refs = rows
                    .into_iter()
                    .map(|r| {
                        let position = if r.position_x.is_some() && r.position_y.is_some() {
                            Some(PositionExport {
                                x: r.position_x.unwrap_or(0.0),
                                y: r.position_y.unwrap_or(0.0),
                            })
                        } else {
                            None
                        };

                        CrossDomainRef {
                            id: r.id,
                            target_domain_id: r.target_domain_id,
                            source_domain_id: r.source_domain_id,
                            table_id: r.table_id,
                            display_alias: r.display_alias,
                            position,
                            notes: r.notes,
                        }
                    })
                    .collect();
                Ok(refs)
            }
            Err(_) => {
                // Table doesn't exist yet, return empty vec
                Ok(Vec::new())
            }
        }
    }

    async fn add_cross_domain_ref(
        &self,
        target_domain_id: Uuid,
        source_domain_id: Uuid,
        table_id: Uuid,
        display_alias: Option<String>,
        position: Option<PositionExport>,
        notes: Option<String>,
    ) -> Result<CrossDomainRef, StorageError> {
        let ref_id = Uuid::new_v4();
        let position_x = position.as_ref().map(|p| p.x);
        let position_y = position.as_ref().map(|p| p.y);

        sqlx::query!(
            r#"
            INSERT INTO cross_domain_refs (id, target_domain_id, source_domain_id, table_id, display_alias, position_x, position_y, notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            "#,
            ref_id,
            target_domain_id,
            source_domain_id,
            table_id,
            display_alias,
            position_x,
            position_y,
            notes
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(CrossDomainRef {
            id: ref_id,
            target_domain_id,
            source_domain_id,
            table_id,
            display_alias,
            position,
            notes,
        })
    }

    async fn remove_cross_domain_ref(&self, ref_id: Uuid) -> Result<(), StorageError> {
        let rows_affected = sqlx::query!(
            r#"
            DELETE FROM cross_domain_refs
            WHERE id = $1
            "#,
            ref_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(StorageError::NotFound {
                entity_type: "cross_domain_ref".to_string(),
                entity_id: ref_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn update_domain(
        &self,
        domain_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        _user_context: &UserContext,
    ) -> Result<DomainInfo, StorageError> {
        let now = Utc::now();

        if let Some(new_name) = name {
            sqlx::query!(
                r#"
                UPDATE domains
                SET name = $1, description = COALESCE($2, description), updated_at = $3
                WHERE id = $4
                "#,
                new_name,
                description,
                now,
                domain_id
            )
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
        } else if description.is_some() {
            sqlx::query!(
                r#"
                UPDATE domains
                SET description = $1, updated_at = $2
                WHERE id = $3
                "#,
                description,
                now,
                domain_id
            )
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
        }

        // Fetch updated domain
        let result = sqlx::query_as!(
            DomainInfo,
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at
            FROM domains
            WHERE id = $1
            "#,
            domain_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .ok_or_else(|| StorageError::NotFound {
            entity_type: "domain".to_string(),
            entity_id: domain_id.to_string(),
        })?;

        Ok(result)
    }

    async fn delete_domain(
        &self,
        domain_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        let rows_affected = sqlx::query!(
            r#"
            DELETE FROM domains
            WHERE id = $1
            "#,
            domain_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .rows_affected();

        if rows_affected == 0 {
            Err(StorageError::NotFound {
                entity_type: "domain".to_string(),
                entity_id: domain_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn update_cross_domain_ref(
        &self,
        ref_id: Uuid,
        display_alias: Option<String>,
        position: Option<PositionExport>,
        notes: Option<String>,
    ) -> Result<CrossDomainRef, StorageError> {
        let position_x = position.as_ref().map(|p| p.x);
        let position_y = position.as_ref().map(|p| p.y);

        sqlx::query!(
            r#"
            UPDATE cross_domain_refs
            SET display_alias = COALESCE($1, display_alias),
                position_x = COALESCE($2, position_x),
                position_y = COALESCE($3, position_y),
                notes = COALESCE($4, notes)
            WHERE id = $5
            "#,
            display_alias,
            position_x,
            position_y,
            notes,
            ref_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        // Fetch updated ref
        let row = sqlx::query!(
            r#"
            SELECT id, target_domain_id, source_domain_id, table_id, display_alias, position_x, position_y, notes
            FROM cross_domain_refs
            WHERE id = $1
            "#,
            ref_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::ConnectionError(e.to_string()))?
        .ok_or_else(|| StorageError::NotFound {
            entity_type: "cross_domain_ref".to_string(),
            entity_id: ref_id.to_string(),
        })?;

        let position = if row.position_x.is_some() && row.position_y.is_some() {
            Some(PositionExport {
                x: row.position_x.unwrap_or(0.0),
                y: row.position_y.unwrap_or(0.0),
            })
        } else {
            None
        };

        Ok(CrossDomainRef {
            id: row.id,
            target_domain_id: row.target_domain_id,
            source_domain_id: row.source_domain_id,
            table_id: row.table_id,
            display_alias: row.display_alias,
            position,
            notes: row.notes,
        })
    }
}
