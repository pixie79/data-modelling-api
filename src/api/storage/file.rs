//! File-based storage backend implementation (stub).
//!
//! This is a placeholder for file-based storage. In practice, the API
//! falls back to ModelService for file operations when PostgreSQL is not available.

use super::{StorageError, traits::*};
use crate::models::{DataFlowDiagram, Relationship, Table};
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

/// File-based storage backend (stub implementation).
///
/// Note: The API actually uses ModelService for file operations when
/// PostgreSQL is not available. This is kept for trait compatibility.
#[allow(dead_code)] // Reserved for file-based storage backend
pub struct FileStorageBackend;

impl FileStorageBackend {
    /// Create a new file storage backend.
    #[allow(dead_code)] // Reserved for file-based storage backend
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileStorageBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for FileStorageBackend {
    async fn get_workspace_by_email(
        &self,
        _email: &str,
    ) -> Result<Option<WorkspaceInfo>, StorageError> {
        // File-based storage doesn't track workspaces
        Ok(None)
    }

    async fn create_workspace(
        &self,
        _email: String,
        _user_context: &UserContext,
    ) -> Result<WorkspaceInfo, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support workspace creation. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_domain_by_name(
        &self,
        _workspace_id: Uuid,
        _name: &str,
    ) -> Result<Option<DomainInfo>, StorageError> {
        // File-based storage doesn't track domains
        Ok(None)
    }

    async fn create_domain(
        &self,
        _workspace_id: Uuid,
        _name: String,
        _description: Option<String>,
        _user_context: &UserContext,
    ) -> Result<DomainInfo, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support domain creation. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_table(
        &self,
        _domain_id: Uuid,
        _table_id: Uuid,
    ) -> Result<Option<Table>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support table operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn create_table(
        &self,
        _domain_id: Uuid,
        _table: Table,
        _user_context: &UserContext,
    ) -> Result<Table, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support table operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn update_table(
        &self,
        _table: Table,
        _expected_version: Option<i32>,
        _user_context: &UserContext,
    ) -> Result<Table, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support table operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn delete_table(
        &self,
        _domain_id: Uuid,
        _table_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support table operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn list_tables(&self, _domain_id: Uuid) -> Result<Vec<Table>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support table operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_relationship(
        &self,
        _domain_id: Uuid,
        _relationship_id: Uuid,
    ) -> Result<Option<Relationship>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support relationship operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn create_relationship(
        &self,
        _domain_id: Uuid,
        _relationship: Relationship,
        _user_context: &UserContext,
    ) -> Result<Relationship, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support relationship operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn update_relationship(
        &self,
        _relationship: Relationship,
        _expected_version: Option<i32>,
        _user_context: &UserContext,
    ) -> Result<Relationship, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support relationship operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn delete_relationship(
        &self,
        _domain_id: Uuid,
        _relationship_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support relationship operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn list_relationships(
        &self,
        _domain_id: Uuid,
    ) -> Result<Vec<Relationship>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support relationship operations. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support workspace listing. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_workspaces_by_owner(
        &self,
        _owner_id: Uuid,
    ) -> Result<Vec<WorkspaceInfo>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support workspace listing by owner. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn create_workspace_with_details(
        &self,
        _email: String,
        _user_context: &UserContext,
        _name: String,
        _workspace_type: String,
    ) -> Result<WorkspaceInfo, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support workspace creation with details. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn workspace_name_exists(&self, _email: &str, _name: &str) -> Result<bool, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support workspace name checking. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_domains(&self, _workspace_id: Uuid) -> Result<Vec<DomainInfo>, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support domain listing. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn get_cross_domain_refs(
        &self,
        _domain_id: Uuid,
    ) -> Result<Vec<CrossDomainRef>, StorageError> {
        // File-based storage uses cross-domain config files
        // Return empty vec - actual implementation would read from config files
        Ok(Vec::new())
    }

    async fn add_cross_domain_ref(
        &self,
        _target_domain_id: Uuid,
        _source_domain_id: Uuid,
        _table_id: Uuid,
        _display_alias: Option<String>,
        _position: Option<PositionExport>,
        _notes: Option<String>,
    ) -> Result<CrossDomainRef, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support cross-domain refs. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn remove_cross_domain_ref(&self, _ref_id: Uuid) -> Result<(), StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support cross-domain refs. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn update_domain(
        &self,
        _domain_id: Uuid,
        _name: Option<String>,
        _description: Option<String>,
        _user_context: &UserContext,
    ) -> Result<DomainInfo, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support domain updates. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn delete_domain(
        &self,
        _domain_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support domain deletion. Use ModelService instead."
                .to_string(),
        ))
    }

    async fn update_cross_domain_ref(
        &self,
        _ref_id: Uuid,
        _display_alias: Option<String>,
        _position: Option<PositionExport>,
        _notes: Option<String>,
    ) -> Result<CrossDomainRef, StorageError> {
        Err(StorageError::Other(
            "File-based storage doesn't support cross-domain refs. Use ModelService instead."
                .to_string(),
        ))
    }

    // Data-flow diagram methods - implemented for file-based storage

    async fn get_data_flow_diagrams(
        &self,
        _domain_id: Uuid,
    ) -> Result<Vec<DataFlowDiagram>, StorageError> {
        // File-based storage reads from data-flow.yaml files
        // This is handled by ModelService, so return empty for now
        // Actual implementation would need access to workspace path
        Err(StorageError::Other(
            "File-based storage for data-flow diagrams is handled by ModelService. Use domain-scoped endpoints."
                .to_string(),
        ))
    }

    async fn get_data_flow_diagram(
        &self,
        _domain_id: Uuid,
        _diagram_id: Uuid,
    ) -> Result<Option<DataFlowDiagram>, StorageError> {
        Err(StorageError::Other(
            "File-based storage for data-flow diagrams is handled by ModelService. Use domain-scoped endpoints."
                .to_string(),
        ))
    }

    async fn create_data_flow_diagram(
        &self,
        _domain_id: Uuid,
        _name: String,
        _description: Option<String>,
        _diagram_data: Value,
        _user_context: &UserContext,
    ) -> Result<DataFlowDiagram, StorageError> {
        Err(StorageError::Other(
            "File-based storage for data-flow diagrams is handled by ModelService. Use domain-scoped endpoints."
                .to_string(),
        ))
    }

    async fn update_data_flow_diagram(
        &self,
        _diagram_id: Uuid,
        _domain_id: Uuid,
        _name: Option<String>,
        _description: Option<String>,
        _diagram_data: Option<Value>,
        _expected_version: Option<i32>,
        _user_context: &UserContext,
    ) -> Result<DataFlowDiagram, StorageError> {
        Err(StorageError::Other(
            "File-based storage for data-flow diagrams is handled by ModelService. Use domain-scoped endpoints."
                .to_string(),
        ))
    }

    async fn delete_data_flow_diagram(
        &self,
        _domain_id: Uuid,
        _diagram_id: Uuid,
        _user_context: &UserContext,
    ) -> Result<(), StorageError> {
        Err(StorageError::Other(
            "File-based storage for data-flow diagrams is handled by ModelService. Use domain-scoped endpoints."
                .to_string(),
        ))
    }
}
