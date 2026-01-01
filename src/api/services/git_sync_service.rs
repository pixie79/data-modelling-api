//! Git synchronization service.
//!
//! Provides Git operations for model synchronization.
//! Uses the SDK's GitService to avoid code duplication.

use data_modelling_sdk::git::GitService as SdkGitService;

/// Git synchronization service wrapper around SDK GitService
#[allow(dead_code)] // Reserved for future Git sync features
pub struct GitSyncService {
    #[allow(dead_code)] // GitService is used via direct SDK calls, kept for future use
    git_service: SdkGitService,
}

impl GitSyncService {
    /// Create a new git sync service
    #[allow(dead_code)] // Reserved for future Git sync features
    pub fn new() -> Self {
        Self {
            git_service: SdkGitService::new(),
        }
    }
}

impl Default for GitSyncService {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export types for compatibility
#[allow(dead_code)] // Reserved for future Git sync features
pub type SyncStatus = data_modelling_sdk::git::GitStatus;
#[allow(dead_code)] // Reserved for future Git sync features
pub type SyncResult = Result<(), data_modelling_sdk::git::GitError>;
#[allow(dead_code)] // Reserved for future Git sync features
pub type SyncConflict = data_modelling_sdk::git::GitError;

/// Git sync configuration
#[allow(dead_code)] // Reserved for future Git sync features
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GitSyncConfig {
    pub repository_url: Option<String>,
    pub branch: String,
    pub auto_commit: bool,
    pub auto_push: bool,
}

impl Default for GitSyncConfig {
    fn default() -> Self {
        Self {
            repository_url: None,
            branch: "main".to_string(),
            auto_commit: false,
            auto_push: false,
        }
    }
}
