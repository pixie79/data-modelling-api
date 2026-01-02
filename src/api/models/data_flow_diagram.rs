//! Data-flow diagram model.
//!
//! Represents data-flow diagrams stored at the domain level.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Data-flow diagram model
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DataFlowDiagram {
    /// Unique identifier for the diagram
    pub id: Uuid,
    /// Domain ID this diagram belongs to
    pub domain_id: Uuid,
    /// Diagram name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Diagram data (JSON/YAML structure)
    pub diagram_data: serde_json::Value,
    /// Version for optimistic locking
    pub version: i32,
    /// User who created the diagram
    pub created_by: Uuid,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Request to create a data-flow diagram
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDataFlowDiagramRequest {
    /// Diagram name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Diagram data (JSON/YAML structure)
    pub diagram_data: serde_json::Value,
}

/// Request to update a data-flow diagram
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDataFlowDiagramRequest {
    /// Diagram name
    pub name: Option<String>,
    /// Optional description
    pub description: Option<String>,
    /// Diagram data (JSON/YAML structure)
    pub diagram_data: Option<serde_json::Value>,
    /// Expected version for optimistic locking
    pub expected_version: Option<i32>,
}
