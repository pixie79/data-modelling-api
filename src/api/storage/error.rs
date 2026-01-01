//! Storage error types for the API storage backends.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Storage operation errors.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageError {
    /// Entity not found
    #[error("Entity not found: {entity_type} with id {entity_id}")]
    NotFound {
        entity_type: String,
        entity_id: String,
    },
    /// Version conflict in optimistic locking
    #[error("Version conflict: expected {expected_version}, got {current_version}")]
    VersionConflict {
        entity_type: String,
        entity_id: String,
        expected_version: i32,
        current_version: i32,
        current_data: Option<serde_json::Value>,
    },
    /// Database connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
    /// General storage error
    #[error("Storage error: {0}")]
    Other(String),
}
