//! Git operations routes.

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use super::tables::AppState;
use crate::services::{cache_service::CacheService, git_service::GitService};
use tracing::info;

#[derive(Deserialize)]
struct MapGitDirectoryRequest {
    git_directory_path: String,
}

#[derive(Serialize)]
struct MapGitDirectoryResponse {
    model: Value,
    orphaned_relationships: Vec<Value>,
    message: String,
}

#[derive(Serialize)]
struct SubfoldersResponse {
    subfolders: Vec<String>,
}

#[derive(Serialize)]
struct LoadCacheResponse {
    model_id: String,
    name: String,
    tables_count: usize,
    relationships_count: usize,
    git_directory_path: String,
}

/// Create the Git operations router
pub fn git_router() -> Router<AppState> {
    Router::new()
        .route("/map", post(map_git_directory))
        .route("/subfolders", get(list_subfolders))
        .route("/cache/load", get(load_cache))
        .route("/cache/clear", post(clear_cache))
}

/// POST /git/map - Map a Git directory and load model
async fn map_git_directory(
    State(state): State<AppState>,
    Json(request): Json<MapGitDirectoryRequest>,
) -> Result<Json<MapGitDirectoryResponse>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let git_path = PathBuf::from(&request.git_directory_path);

    let mut git_service = GitService::new();
    let (model, orphaned_relationships) = git_service
        .map_git_directory(&git_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(
        "[Git Route] Loaded model with {} tables and {} relationships ({} orphaned)",
        model.tables.len(),
        model.relationships.len(),
        orphaned_relationships.len()
    );

    // Set the model in the model service
    model_service.set_current_model(model.clone());

    let model_json = serde_json::to_value(&model).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Convert orphaned relationships to JSON values
    let orphaned_rels_json: Vec<Value> = orphaned_relationships
        .into_iter()
        .map(|rel| serde_json::to_value(&rel).unwrap_or(Value::Null))
        .collect();

    Ok(Json(MapGitDirectoryResponse {
        model: model_json,
        orphaned_relationships: orphaned_rels_json,
        message: format!(
            "Successfully mapped Git directory: {}",
            request.git_directory_path
        ),
    }))
}

/// GET /git/subfolders - List subfolders (focus areas) in Git directory
async fn list_subfolders(
    State(state): State<AppState>,
) -> Result<Json<SubfoldersResponse>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let git_path = PathBuf::from(&model.git_directory_path);
    let _git_service = GitService::new();

    // Note: We need to map the directory first to get subfolders
    // For now, we'll create a temporary GitService instance
    let mut temp_git_service = GitService::new();
    let _ = temp_git_service.map_git_directory(&git_path).map(|(_, _)| ());

    let subfolders = temp_git_service
        .list_subfolders()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SubfoldersResponse { subfolders }))
}

/// GET /git/cache/load - Load model metadata from cache
async fn load_cache(_state: State<AppState>) -> Result<Json<LoadCacheResponse>, StatusCode> {
    let cache_path = std::env::temp_dir().join("modelling_cache.db");
    let cache_service =
        CacheService::new(&cache_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match cache_service.get_model_metadata() {
        Ok(Some((model_id, name, git_directory_path))) => match cache_service.get_cache_counts() {
            Ok((tables_count, relationships_count)) => Ok(Json(LoadCacheResponse {
                model_id,
                name,
                tables_count,
                relationships_count,
                git_directory_path,
            })),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
struct ClearCacheResponse {
    message: String,
}

/// POST /git/cache/clear - Clear all cache and reset model state
async fn clear_cache(
    State(state): State<AppState>,
) -> Result<Json<ClearCacheResponse>, StatusCode> {
    // Clear database cache - use default cache path
    let cache_path = std::env::temp_dir().join("modelling_cache.db");
    let cache_service =
        CacheService::new(&cache_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    cache_service
        .clear_cache()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Clear in-memory model state
    let mut model_service = state.model_service.lock().await;

    // If there's a current model, also clean up its on-disk files
    if let Some(model) = model_service.get_current_model() {
        let git_path = PathBuf::from(&model.git_directory_path);

        // Remove table YAML files
        let tables_dir = git_path.join("tables");
        if tables_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&tables_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "yaml" || ext == "yml")
                        .unwrap_or(false)
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }

        // Remove relationships file
        let relationships_file = git_path.join("relationships.yaml");
        if relationships_file.exists() {
            let _ = std::fs::remove_file(&relationships_file);
        }

        // If it's a default temp model directory, remove the entire directory
        if git_path.to_string_lossy().contains("modelling_default_") {
            let _ = std::fs::remove_dir_all(&git_path);
        }
    }

    // Clear the in-memory model state
    model_service.clear_model();
    info!("Cache and model state cleared (including on-disk files)");

    Ok(Json(ClearCacheResponse {
        message: "Cache cleared successfully".to_string(),
    }))
}
