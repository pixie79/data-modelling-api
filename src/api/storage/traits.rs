//! Storage trait definitions for the API storage backends.

use crate::models::{Relationship, Table};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User context for storage operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: Uuid,
    pub email: String,
}

/// Email information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailInfo {
    pub email: String,
    pub verified: bool,
    pub primary: bool,
}

/// Workspace information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub workspace_type: Option<String>, // Using workspace_type to avoid conflict with Rust's type keyword
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainInfo {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Position export for canvas layout
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionExport {
    pub x: f64,
    pub y: f64,
}

/// Storage backend trait for database operations
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Get workspace by owner email
    async fn get_workspace_by_email(
        &self,
        email: &str,
    ) -> Result<Option<WorkspaceInfo>, super::StorageError>;

    /// Create a new workspace
    async fn create_workspace(
        &self,
        email: String,
        user_context: &UserContext,
    ) -> Result<WorkspaceInfo, super::StorageError>;

    /// Get domain by name
    async fn get_domain_by_name(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> Result<Option<DomainInfo>, super::StorageError>;

    /// Create a new domain
    async fn create_domain(
        &self,
        workspace_id: Uuid,
        name: String,
        description: Option<String>,
        user_context: &UserContext,
    ) -> Result<DomainInfo, super::StorageError>;

    /// Get table by ID
    async fn get_table(
        &self,
        domain_id: Uuid,
        table_id: Uuid,
    ) -> Result<Option<Table>, super::StorageError>;

    /// Create a new table
    async fn create_table(
        &self,
        domain_id: Uuid,
        table: Table,
        user_context: &UserContext,
    ) -> Result<Table, super::StorageError>;

    /// Update a table with optimistic locking
    async fn update_table(
        &self,
        table: Table,
        expected_version: Option<i32>,
        user_context: &UserContext,
    ) -> Result<Table, super::StorageError>;

    /// Delete a table
    async fn delete_table(
        &self,
        domain_id: Uuid,
        table_id: Uuid,
        user_context: &UserContext,
    ) -> Result<(), super::StorageError>;

    /// List tables in a domain
    async fn list_tables(&self, domain_id: Uuid) -> Result<Vec<Table>, super::StorageError>;

    /// Get relationship by ID
    async fn get_relationship(
        &self,
        domain_id: Uuid,
        relationship_id: Uuid,
    ) -> Result<Option<Relationship>, super::StorageError>;

    /// Create a new relationship
    async fn create_relationship(
        &self,
        domain_id: Uuid,
        relationship: Relationship,
        user_context: &UserContext,
    ) -> Result<Relationship, super::StorageError>;

    /// Update a relationship with optimistic locking
    async fn update_relationship(
        &self,
        relationship: Relationship,
        expected_version: Option<i32>,
        user_context: &UserContext,
    ) -> Result<Relationship, super::StorageError>;

    /// Delete a relationship
    async fn delete_relationship(
        &self,
        domain_id: Uuid,
        relationship_id: Uuid,
        user_context: &UserContext,
    ) -> Result<(), super::StorageError>;

    /// List relationships in a domain
    async fn list_relationships(
        &self,
        domain_id: Uuid,
    ) -> Result<Vec<Relationship>, super::StorageError>;

    /// Get all workspaces (for admin/list operations)
    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, super::StorageError>;

    /// Get workspaces filtered by owner_id (for /api/v1/workspaces endpoint)
    async fn get_workspaces_by_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<WorkspaceInfo>, super::StorageError>;

    /// Create workspace with name and type (for /api/v1/workspaces endpoint)
    async fn create_workspace_with_details(
        &self,
        email: String,
        user_context: &UserContext,
        name: String,
        workspace_type: String,
    ) -> Result<WorkspaceInfo, super::StorageError>;

    /// Check if workspace name already exists for given email
    async fn workspace_name_exists(
        &self,
        email: &str,
        name: &str,
    ) -> Result<bool, super::StorageError>;

    /// Get all domains in a workspace
    async fn get_domains(&self, workspace_id: Uuid)
    -> Result<Vec<DomainInfo>, super::StorageError>;

    /// Get all tables (alias for list_tables, for consistency)
    async fn get_tables(&self, domain_id: Uuid) -> Result<Vec<Table>, super::StorageError> {
        self.list_tables(domain_id).await
    }

    /// Get all relationships (alias for list_relationships, for consistency)
    async fn get_relationships(
        &self,
        domain_id: Uuid,
    ) -> Result<Vec<Relationship>, super::StorageError> {
        self.list_relationships(domain_id).await
    }

    /// Get cross-domain references for a domain
    async fn get_cross_domain_refs(
        &self,
        domain_id: Uuid,
    ) -> Result<Vec<CrossDomainRef>, super::StorageError>;

    /// Add a cross-domain reference
    async fn add_cross_domain_ref(
        &self,
        target_domain_id: Uuid,
        source_domain_id: Uuid,
        table_id: Uuid,
        display_alias: Option<String>,
        position: Option<PositionExport>,
        notes: Option<String>,
    ) -> Result<CrossDomainRef, super::StorageError>;

    /// Remove a cross-domain reference
    async fn remove_cross_domain_ref(&self, ref_id: Uuid) -> Result<(), super::StorageError>;

    /// Update a domain
    async fn update_domain(
        &self,
        domain_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        user_context: &UserContext,
    ) -> Result<DomainInfo, super::StorageError>;

    /// Delete a domain
    async fn delete_domain(
        &self,
        domain_id: Uuid,
        user_context: &UserContext,
    ) -> Result<(), super::StorageError>;

    /// Update a cross-domain reference
    async fn update_cross_domain_ref(
        &self,
        ref_id: Uuid,
        display_alias: Option<String>,
        position: Option<PositionExport>,
        notes: Option<String>,
    ) -> Result<CrossDomainRef, super::StorageError>;
}

/// Cross-domain reference information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossDomainRef {
    pub id: Uuid,
    pub target_domain_id: Uuid,
    pub source_domain_id: Uuid,
    pub table_id: Uuid,
    pub display_alias: Option<String>,
    pub position: Option<PositionExport>,
    pub notes: Option<String>,
}
