//! Application state management.
//!
//! Defines the AppState struct that holds all shared application state including
//! model service, session store, storage backends, and database connections.

use crate::routes::collaboration::CollaborationMessage;
use crate::services::model_service::ModelService;
use crate::storage::session_store::DbSessionStore;
use crate::storage::{StorageBackend, StorageError};
use axum::extract::FromRef;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

/// Application state shared across all route handlers.
#[derive(Clone)]
pub struct AppState {
    /// Model service for managing data models
    pub model_service: Arc<Mutex<ModelService>>,
    /// Session store for authentication (in-memory or database-backed)
    pub session_store: crate::routes::auth::SessionStore,
    /// Database-backed session store (optional, for PostgreSQL mode)
    pub db_session_store: Option<Arc<DbSessionStore>>,
    /// Storage backend for PostgreSQL operations (optional)
    pub storage: Option<Arc<dyn StorageBackend>>,
    /// PostgreSQL database connection pool (optional)
    pub database: Option<PgPool>,
    /// Collaboration broadcast channels (model_id -> channel)
    pub collaboration_channels:
        Arc<Mutex<HashMap<String, broadcast::Sender<CollaborationMessage>>>>,
}

impl AppState {
    /// Create a new application state with default values.
    pub fn new() -> Self {
        Self {
            model_service: Arc::new(Mutex::new(ModelService::new())),
            session_store: crate::routes::auth::new_session_store(),
            db_session_store: None,
            storage: None,
            database: None,
            collaboration_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Initialize storage backend from environment configuration.
    ///
    /// This will attempt to connect to PostgreSQL if DATABASE_URL is set,
    /// otherwise falls back to file-based storage.
    pub async fn init_storage(&mut self) -> Result<(), StorageError> {
        // Check if DATABASE_URL is set
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            // Initialize PostgreSQL storage
            match sqlx::PgPool::connect(&database_url).await {
                Ok(pool) => {
                    // Run migrations
                    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
                        return Err(StorageError::ConnectionError(format!(
                            "Migration failed: {}",
                            e
                        )));
                    }

                    self.database = Some(pool.clone());
                    // Create database session store
                    self.db_session_store = Some(Arc::new(DbSessionStore::new(pool.clone())));
                    // Create PostgreSQL storage backend
                    let storage: Arc<dyn StorageBackend> =
                        Arc::new(crate::storage::postgres::PostgresStorageBackend::new(pool));
                    self.storage = Some(storage);
                    Ok(())
                }
                Err(e) => Err(StorageError::ConnectionError(format!(
                    "Failed to connect to database: {}",
                    e
                ))),
            }
        } else {
            // File-based storage (no database)
            Ok(())
        }
    }

    /// Get a reference to the database pool if available.
    pub fn database(&self) -> Option<&PgPool> {
        self.database.as_ref()
    }

    /// Get a reference to the storage backend if available.
    pub fn storage(&self) -> Option<&Arc<dyn StorageBackend>> {
        self.storage.as_ref()
    }

    /// Get a reference to the database session store if available.
    pub fn db_session_store(&self) -> Option<&Arc<DbSessionStore>> {
        self.db_session_store.as_ref()
    }

    /// Check if PostgreSQL storage is enabled
    pub fn is_postgres(&self) -> bool {
        self.database.is_some() && self.storage.is_some()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// Allow AppState to be extracted from references (for Axum)
impl FromRef<AppState> for Arc<Mutex<ModelService>> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.model_service.clone()
    }
}

impl FromRef<AppState> for crate::routes::auth::SessionStore {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.session_store.clone()
    }
}
