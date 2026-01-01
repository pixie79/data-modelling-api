//! Storage module for the API.
//!
//! Provides storage backends for PostgreSQL and file-based storage.

pub mod collaboration;
pub mod error;
pub mod session_store;
pub mod traits;

// Storage backend implementations
pub mod file;
pub mod postgres;

pub use collaboration::CollaborationStore;
pub use error::StorageError;
#[allow(unused_imports)] // Re-exported for API compatibility
pub use session_store::DbSessionStore;
#[allow(unused_imports)] // Re-exported for API compatibility
pub use traits::{
    DomainInfo, EmailInfo, PositionExport, StorageBackend, UserContext, WorkspaceInfo,
};
