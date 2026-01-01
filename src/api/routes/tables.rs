//! Table routes for managing tables.

use axum::routing::put;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, ModelingLevel, SCDPattern,
};
use crate::models::{Column, Position, Table};
use crate::services::FilterService;

use super::collaboration;

// Re-export AppState from app_state module for backwards compatibility
pub use super::app_state::AppState;

/// Query parameters for GET /tables
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetTablesQuery {
    modeling_level: Option<String>,
    medallion_layer: Option<String>,
    #[serde(default)]
    table_ids: Vec<String>,
}

/// Request body for creating a table
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateTableRequest {
    pub name: String,
    pub columns: Vec<Value>,
    #[serde(default)]
    pub database_type: Option<String>,
    #[serde(default)]
    pub catalog_name: Option<String>,
    #[serde(default)]
    pub schema_name: Option<String>,
    #[serde(default)]
    pub medallion_layers: Vec<String>,
    #[serde(default)]
    pub medallion_layer: Option<String>, // Backward compatibility
    #[serde(default)]
    pub scd_pattern: Option<String>,
    #[serde(default)]
    pub data_vault_classification: Option<String>,
    #[serde(default)]
    pub modeling_level: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub odcl_metadata: HashMap<String, Value>,
    #[serde(default)]
    pub position: Option<Value>,
}

/// Request body for filtering tables
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FilterTablesRequest {
    #[serde(default)]
    pub table_ids: Vec<String>,
    pub modeling_level: Option<String>,
    #[serde(default)]
    pub medallion_layers: Vec<String>,
    #[serde(default)]
    pub database_types: Vec<String>,
    #[serde(default)]
    pub scd_patterns: Vec<String>,
    #[serde(default)]
    pub data_vault_classifications: Vec<String>,
}

/// Create the tables router
#[allow(dead_code)]
pub fn tables_router() -> Router<AppState> {
    // In axum 0.8, path parameters use curly braces {} instead of colons :
    Router::new()
        .route("/", get(get_tables).post(create_table))
        .route("/filter", post(filter_tables))
        .route("/stats", get(get_table_stats))
        .route(
            "/{table_id}",
            get(get_table).put(update_table).delete(delete_table),
        )
        .route("/{table_id}/position", put(update_table_position))
}

/// GET /tables - Get all tables, optionally filtered
#[allow(dead_code)]
async fn get_tables(
    State(state): State<AppState>,
    Query(query): Query<GetTablesQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Ensure workspace is loaded from session
    if let Err(e) = super::workspace::ensure_workspace_loaded(&state, &headers).await {
        warn!("[GET /tables] Failed to ensure workspace loaded: {}", e);
        return Ok(Json(json!([])));
    }

    let mut model_service = state.model_service.lock().await;

    // Try to ensure model is available (reload from temp directories if needed)
    // This handles the case where the model was created during import but lost
    if let Err(e) = model_service.ensure_model_available() {
        warn!("[GET /tables] No model available and reload failed: {}", e);
        return Ok(Json(json!([])));
    }

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!([]))),
    };

    let filter_service = FilterService::new(Some(model.clone()));

    // Parse modeling level
    let parsed_level =
        query
            .modeling_level
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "CONCEPTUAL" => Some(ModelingLevel::Conceptual),
                "LOGICAL" => Some(ModelingLevel::Logical),
                "PHYSICAL" => Some(ModelingLevel::Physical),
                _ => None,
            });

    // Parse medallion layers
    let parsed_layers =
        query
            .medallion_layer
            .as_ref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "bronze" => Some(vec![MedallionLayer::Bronze]),
                "silver" => Some(vec![MedallionLayer::Silver]),
                "gold" => Some(vec![MedallionLayer::Gold]),
                "operational" => Some(vec![MedallionLayer::Operational]),
                _ => None,
            });

    let table_ids = if query.table_ids.is_empty() {
        None
    } else {
        Some(query.table_ids.as_slice())
    };

    let tables = filter_service.filter_tables(
        table_ids,
        parsed_level,
        parsed_layers.as_deref(),
        None,
        None,
        None,
    );

    let tables_json: Vec<Value> = tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    Ok(Json(json!(tables_json)))
}

/// GET /tables/:table_id - Get a single table by ID
#[allow(dead_code)]
async fn get_table(
    State(state): State<AppState>,
    Path(table_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let table_uuid = match Uuid::parse_str(&table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let table = match model_service.get_table(table_uuid) {
        Some(t) => t,
        None => return Err(StatusCode::NOT_FOUND),
    };

    Ok(Json(serde_json::to_value(table).unwrap_or(json!({}))))
}

/// POST /tables - Create a new table manually
#[allow(dead_code)]
async fn create_table(
    State(state): State<AppState>,
    Json(request): Json<CreateTableRequest>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    // Validate required fields
    if request.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.columns.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse columns from JSON
    let mut columns: Vec<Column> = Vec::new();
    for (idx, col_data) in request.columns.iter().enumerate() {
        // Try to deserialize the column directly
        match serde_json::from_value::<Column>(col_data.clone()) {
            Ok(mut col) => {
                // Ensure column_order is set
                col.column_order = idx as i32;
                columns.push(col);
            }
            Err(e) => {
                warn!(
                    "Failed to deserialize column {} directly: {}, using fallback parser",
                    idx, e
                );
                // Fallback: try to extract basic fields manually
                let name = col_data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let data_type = col_data
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("STRING")
                    .to_string();

                if !name.is_empty() {
                    let mut col = Column::new(name, data_type);
                    col.column_order = idx as i32;

                    // Extract optional fields
                    if let Some(nullable) = col_data.get("nullable").and_then(|v| v.as_bool()) {
                        col.nullable = nullable;
                    }
                    if let Some(pk) = col_data.get("primary_key").and_then(|v| v.as_bool()) {
                        col.primary_key = pk;
                    }
                    if let Some(sk) = col_data.get("secondary_key").and_then(|v| v.as_bool()) {
                        col.secondary_key = sk;
                    }
                    if let Some(desc) = col_data.get("description").and_then(|v| v.as_str()) {
                        col.description = desc.to_string();
                    }
                    if let Some(constraints) =
                        col_data.get("constraints").and_then(|v| v.as_array())
                    {
                        col.constraints = constraints
                            .iter()
                            .filter_map(|c| c.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                    if let Some(enum_vals) = col_data.get("enum_values").and_then(|v| v.as_array())
                    {
                        col.enum_values = enum_vals
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }

                    columns.push(col);
                } else {
                    warn!("Skipping column {}: missing name field", idx);
                }
            }
        }
    }

    if columns.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse medallion layers (support both plural and singular)
    let medallion_layers = if !request.medallion_layers.is_empty() {
        request
            .medallion_layers
            .iter()
            .filter_map(|s| match s.to_lowercase().as_str() {
                "bronze" => Some(MedallionLayer::Bronze),
                "silver" => Some(MedallionLayer::Silver),
                "gold" => Some(MedallionLayer::Gold),
                "operational" => Some(MedallionLayer::Operational),
                _ => None,
            })
            .collect()
    } else if let Some(ref layer) = request.medallion_layer {
        match layer.to_lowercase().as_str() {
            "bronze" => vec![MedallionLayer::Bronze],
            "silver" => vec![MedallionLayer::Silver],
            "gold" => vec![MedallionLayer::Gold],
            "operational" => vec![MedallionLayer::Operational],
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // Parse database type
    let database_type =
        request
            .database_type
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "POSTGRES" | "POSTGRESQL" => Some(DatabaseType::Postgres),
                "MYSQL" => Some(DatabaseType::Mysql),
                "SQL_SERVER" | "SQLSERVER" => Some(DatabaseType::SqlServer),
                "DATABRICKS" | "DATABRICKS_DELTA" => Some(DatabaseType::DatabricksDelta),
                "AWS_GLUE" | "GLUE" => Some(DatabaseType::AwsGlue),
                _ => None,
            });

    // Parse SCD pattern
    let scd_pattern = request
        .scd_pattern
        .as_ref()
        .and_then(|s| match s.to_uppercase().as_str() {
            "TYPE_1" => Some(SCDPattern::Type1),
            "TYPE_2" => Some(SCDPattern::Type2),
            _ => None,
        });

    // Parse Data Vault classification
    let data_vault_classification =
        request
            .data_vault_classification
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "HUB" => Some(DataVaultClassification::Hub),
                "LINK" => Some(DataVaultClassification::Link),
                "SATELLITE" => Some(DataVaultClassification::Satellite),
                _ => None,
            });

    // Parse modeling level
    let modeling_level =
        request
            .modeling_level
            .as_ref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "conceptual" => Some(ModelingLevel::Conceptual),
                "logical" => Some(ModelingLevel::Logical),
                "physical" => Some(ModelingLevel::Physical),
                _ => None,
            });

    // Parse position
    let position = request.position.and_then(|pos_val| {
        if let (Some(x), Some(y)) = (
            pos_val.get("x").and_then(|v| v.as_f64()),
            pos_val.get("y").and_then(|v| v.as_f64()),
        ) {
            Some(Position { x, y })
        } else {
            None
        }
    });

    // Create the table
    let table = Table {
        id: uuid::Uuid::new_v4(),
        name: request.name.trim().to_string(),
        columns,
        database_type,
        catalog_name: request.catalog_name.filter(|s| !s.trim().is_empty()),
        schema_name: request.schema_name.filter(|s| !s.trim().is_empty()),
        medallion_layers,
        scd_pattern,
        data_vault_classification,
        modeling_level,
        tags: request.tags,
        odcl_metadata: request.odcl_metadata,
        position,
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Add table to model
    match model_service.add_table(table.clone()) {
        Ok(added_table) => {
            let table_json = serde_json::to_value(&added_table).unwrap_or(json!({}));

            // Broadcast creation via WebSocket
            let model_id = "default"; // TODO: Get actual model_id from context
            collaboration::broadcast_table_create(&state, model_id, &table_json).await;

            Ok(Json(table_json))
        }
        Err(e) => {
            error!("Failed to add table '{}': {}", table.name, e);
            // Return error details in response for debugging
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// PUT /tables/:table_id - Update table properties
#[allow(dead_code)]
async fn update_table(
    State(state): State<AppState>,
    Path(table_id): Path<String>,
    Json(updates): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    info!("[Tables] Update request for table_id: {}", table_id);

    // Log database_type update if present
    if let Some(db_type_val) = updates.get("database_type") {
        info!("[Tables] Update includes database_type: {:?}", db_type_val);
    }

    let mut model_service = state.model_service.lock().await;

    let table_uuid = match Uuid::parse_str(&table_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            warn!("[Tables] Invalid table_id format: {}", table_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match model_service.update_table(table_uuid, &updates) {
        Ok(Some(table)) => {
            let db_type_str = table.database_type.map(|dt| format!("{:?}", dt));
            info!(
                "[Tables] Successfully updated table '{}', database_type: {:?}",
                table.name, db_type_str
            );

            // Broadcast update via WebSocket
            let table_json = serde_json::to_value(&table).unwrap_or(json!({}));
            let model_id = "default"; // TODO: Get actual model_id from context
            collaboration::broadcast_table_update(&state, model_id, &table_json).await;

            Ok(Json(table_json))
        }
        Ok(None) => {
            warn!("[Tables] Table not found: {}", table_id);
            // Log model state for debugging
            let model_service_debug = state.model_service.lock().await;
            if let Some(model) = model_service_debug.get_current_model() {
                warn!(
                    "[Tables] Model has {} tables. Table IDs: {:?}",
                    model.tables.len(),
                    model
                        .tables
                        .iter()
                        .map(|t| t.id.to_string())
                        .collect::<Vec<_>>()
                );
            } else {
                warn!("[Tables] No model available");
            }
            let _ = model_service_debug;
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("[Tables] Failed to update table {}: {}", table_id, e);
            // Log the full error message for debugging
            warn!("[Tables] Update error details: {}", e);

            // If the error is "No model available", return a more specific status code
            let error_msg = e.to_string();
            if error_msg.contains("No model available") {
                warn!(
                    "[Tables] Model is missing. This can happen if the backend restarted. User should re-import or load the model."
                );
                return Err(StatusCode::PRECONDITION_FAILED); // 428 - Precondition Required
            }

            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// DELETE /tables/:table_id - Delete a table
#[allow(dead_code)]
async fn delete_table(
    State(state): State<AppState>,
    Path(table_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let table_uuid = match Uuid::parse_str(&table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Get relationships to delete BEFORE deletion (for broadcasting)
    let deleted_relationship_ids: Vec<String> = {
        let model = model_service.get_current_model();
        if let Some(model_ref) = model {
            model_ref
                .relationships
                .iter()
                .filter(|r| r.source_table_id == table_uuid || r.target_table_id == table_uuid)
                .map(|r| r.id.to_string())
                .collect()
        } else {
            Vec::new()
        }
    };

    // Get git directory path before deletion (for saving relationships)
    let git_directory_path = {
        let model = model_service.get_current_model();
        model
            .as_ref()
            .map(|m| m.git_directory_path.clone())
            .unwrap_or_default()
    };

    match model_service.delete_table(table_uuid) {
        Ok(true) => {
            // Broadcast relationship deletions via WebSocket
            let model_id = "default"; // TODO: Get actual model_id from context
            for rel_id in &deleted_relationship_ids {
                collaboration::broadcast_relationship_delete(&state, model_id, rel_id).await;
            }

            // Broadcast table deletion via WebSocket
            collaboration::broadcast_table_delete(&state, model_id, &table_id).await;

            // Save updated relationships to YAML (after cascade delete)
            if !git_directory_path.is_empty() {
                let model = model_service.get_current_model();
                if let Some(model_ref) = model {
                    use crate::services::git_service::GitService;
                    use std::path::Path;

                    let mut git_service = GitService::new();
                    if let Err(e) =
                        git_service.set_git_directory_path(Path::new(&git_directory_path))
                    {
                        warn!(
                            "Failed to set git directory for relationship save after table delete: {}",
                            e
                        );
                    } else {
                        // Save remaining relationships (those not involving the deleted table)
                        if let Err(e) = git_service
                            .save_relationships_to_yaml(&model_ref.relationships, &model_ref.tables)
                        {
                            warn!(
                                "Failed to auto-save relationships to YAML after table delete: {}",
                                e
                            );
                        }
                    }
                }
            }

            Ok(Json(json!({"message": "Table deleted successfully"})))
        }
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// POST /tables/filter - Filter tables by multiple criteria
#[allow(dead_code)]
async fn filter_tables(
    State(state): State<AppState>,
    Json(request): Json<FilterTablesRequest>,
) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Ok(Json(json!([]))),
    };

    let filter_service = FilterService::new(Some(model.clone()));

    // Parse enums
    let parsed_level =
        request
            .modeling_level
            .as_ref()
            .and_then(|s| match s.to_uppercase().as_str() {
                "CONCEPTUAL" => Some(ModelingLevel::Conceptual),
                "LOGICAL" => Some(ModelingLevel::Logical),
                "PHYSICAL" => Some(ModelingLevel::Physical),
                _ => None,
            });

    let parsed_layers: Vec<crate::models::enums::MedallionLayer> = request
        .medallion_layers
        .iter()
        .filter_map(|s| match s.to_lowercase().as_str() {
            "bronze" => Some(MedallionLayer::Bronze),
            "silver" => Some(MedallionLayer::Silver),
            "gold" => Some(MedallionLayer::Gold),
            "operational" => Some(MedallionLayer::Operational),
            _ => None,
        })
        .collect();

    let table_ids = if request.table_ids.is_empty() {
        None
    } else {
        Some(request.table_ids.as_slice())
    };

    let database_types = if request.database_types.is_empty() {
        None
    } else {
        Some(request.database_types.as_slice())
    };

    let scd_patterns = if request.scd_patterns.is_empty() {
        None
    } else {
        Some(request.scd_patterns.as_slice())
    };

    let data_vault_classifications = if request.data_vault_classifications.is_empty() {
        None
    } else {
        Some(request.data_vault_classifications.as_slice())
    };

    let tables = filter_service.filter_tables(
        table_ids,
        parsed_level,
        if parsed_layers.is_empty() {
            None
        } else {
            Some(parsed_layers.as_slice())
        },
        database_types,
        scd_patterns,
        data_vault_classifications,
    );

    let tables_json: Vec<Value> = tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    Ok(Json(json!(tables_json)))
}

/// GET /tables/stats - Get statistics about tables
#[allow(dead_code)]
async fn get_table_stats(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => {
            return Ok(Json(json!({
                "total_tables": 0,
                "by_modeling_level": {},
                "by_medallion_layer": {}
            })));
        }
    };

    let filter_service = FilterService::new(Some(model.clone()));

    let by_level = filter_service.get_table_count_by_level();
    let by_layer = filter_service.get_table_count_by_layer();
    let available_levels = filter_service.get_available_modeling_levels();
    let available_layers = filter_service.get_available_medallion_layers();

    Ok(Json(json!({
        "total_tables": model.tables.len(),
        "by_modeling_level": by_level,
        "by_medallion_layer": by_layer,
        "available_levels": available_levels,
        "available_layers": available_layers
    })))
}

/// PUT /tables/:table_id/position - Update table position
#[allow(dead_code)]
async fn update_table_position(
    State(state): State<AppState>,
    Path(table_id): Path<String>,
    Json(position): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let table_uuid = match Uuid::parse_str(&table_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            warn!("Invalid table UUID format: {}", table_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Parse position from JSON
    let x = position.get("x").and_then(|v| v.as_f64()).ok_or_else(|| {
        warn!("Missing or invalid 'x' position value");
        StatusCode::BAD_REQUEST
    })?;
    let y = position.get("y").and_then(|v| v.as_f64()).ok_or_else(|| {
        warn!("Missing or invalid 'y' position value");
        StatusCode::BAD_REQUEST
    })?;

    // Get model - ensure one exists
    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => {
            warn!("No model loaded when trying to update table position");
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Find and update table position
    let table_result = if let Some(table) = model.tables.iter_mut().find(|t| t.id == table_uuid) {
        use crate::models::Position;
        table.position = Some(Position { x, y });
        info!(
            "Updated position for table {} to ({}, {})",
            table.name, x, y
        );
        Ok(serde_json::to_value(table).unwrap_or(json!({})))
    } else {
        warn!(
            "Table {} not found in model (total tables: {})",
            table_uuid,
            model.tables.len()
        );
        return Err(StatusCode::NOT_FOUND);
    };

    // Auto-update canvas layout YAML if model has git directory (after mutable borrow is released)
    let git_directory_path = model.git_directory_path.clone();
    let position_to_save = Position { x, y };
    let _ = model; // Explicitly release the mutable borrow before canvas layout operations

    if !git_directory_path.is_empty() {
        use crate::services::canvas_layout_service::CanvasLayoutService;
        use std::path::Path;

        let canvas_layout_service = CanvasLayoutService::new(Path::new(&git_directory_path));
        // Get model immutably for canvas layout update
        if let Some(model) = model_service.get_current_model()
            && let Err(e) =
                canvas_layout_service.update_table_position(model, table_uuid, position_to_save)
        {
            warn!("Failed to auto-update canvas layout YAML: {}", e);
        }
    }

    table_result.map(Json)
}
