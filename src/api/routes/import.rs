//! Import routes for SQL and ODCL file imports.

use axum::{
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info, warn};

use super::tables::AppState;
use crate::services::{AvroParser, JSONSchemaParser, ODCSParser, ProtobufParser, SQLParser};

/// Request for SQL text import
#[derive(Debug, Deserialize)]
pub struct SQLTextImportRequest {
    pub content: String,
    #[serde(default)]
    pub use_ai: bool,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub table_names: Option<HashMap<String, String>>, // Map of table_index -> table_name for dynamic names
    #[serde(default)]
    pub dialect: Option<String>, // SQL dialect name (e.g., "postgres", "mysql", "databricks", "duckdb")
}

/// Request for ODCL text import
#[derive(Debug, Deserialize)]
pub struct ODCLTextImportRequest {
    pub content: String,
    #[serde(default)]
    pub use_ai: bool,
    #[serde(default)]
    pub filename: Option<String>,
}

/// Create the import router
pub fn import_router() -> Router<AppState> {
    Router::new()
        .route("/odcl", post(import_odcl))
        .route("/odcl/text", post(import_odcl_text))
        .route("/sql", post(import_sql))
        .route(
            "/sql/text",
            post(
                |state: State<AppState>, headers: HeaderMap, request: Json<SQLTextImportRequest>| async move {
                    import_sql_text(state, headers, request).await
                },
            ),
        )
        .route("/avro", post(import_avro))
        .route("/json-schema", post(import_json_schema))
        .route("/protobuf", post(import_protobuf))
}

/// POST /import/odcl - Import tables from ODCL file
async fn import_odcl(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut yaml_content = String::new();
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            // Validate filename
            if let Some(filename) = field.file_name() {
                if !filename.ends_with(".yaml") && !filename.ends_with(".yml") {
                    return Err(StatusCode::BAD_REQUEST);
                }
            }

            if let Ok(content) = field.bytes().await {
                if content.len() > 10 * 1024 * 1024 {
                    return Err(StatusCode::BAD_REQUEST);
                }
                yaml_content = String::from_utf8_lossy(&content).to_string();
            }
        } else if name == "use_ai" {
            // Parse use_ai flag (not used yet, but parsed for future AI integration)
            let _ = field.text().await;
        }
    }

    if yaml_content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Basic sanitization
    yaml_content = yaml_content.replace('\x00', "");

    let mut parser = ODCSParser::new();
    let (table, parse_errors) = match parser.parse(&yaml_content) {
        Ok(result) => result,
        Err(e) => {
            error!("ODCL parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(std::slice::from_ref(&table));
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                    "new_table": t1.name,
                    "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": [serde_json::to_value(&table).unwrap_or(json!({}))],
            "conflicts": conflict_info,
            "errors": errors_json
        })));
    }

    // Add table to model
    let added_table = match model_service.add_table(table.clone()) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to add table: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let errors_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "type": e.error_type,
                "field": e.field,
                "message": e.message
            })
        })
        .collect();

    Ok(Json(json!({
        "tables": [serde_json::to_value(&added_table).unwrap_or(json!({}))],
        "ai_suggestions": json!([]),
        "errors": errors_json
    })))
}

/// POST /import/odcl/text - Import tables from ODCL text
async fn import_odcl_text(
    State(state): State<AppState>,
    Json(request): Json<ODCLTextImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Basic sanitization
    let yaml_content = request.content.replace('\x00', "");
    if yaml_content.len() > 10 * 1024 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut parser = ODCSParser::new();
    let (table, parse_errors) = match parser.parse(&yaml_content) {
        Ok(result) => result,
        Err(e) => {
            error!("ODCL parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(std::slice::from_ref(&table));
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                    "new_table": t1.name,
                    "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": [serde_json::to_value(&table).unwrap_or(json!({}))],
            "conflicts": conflict_info,
            "errors": errors_json
        })));
    }

    // Add parse errors to table.errors
    let mut table_with_errors = table.clone();
    for parse_error in &parse_errors {
        let mut error_map = HashMap::new();
        error_map.insert(
            "type".to_string(),
            serde_json::Value::String(parse_error.error_type.clone()),
        );
        error_map.insert(
            "field".to_string(),
            serde_json::Value::String(parse_error.field.clone()),
        );
        error_map.insert(
            "message".to_string(),
            serde_json::Value::String(parse_error.message.clone()),
        );
        table_with_errors.errors.push(error_map);
    }

    // Add table to model - save even if it has errors
    let added_table = match model_service.add_table(table_with_errors.clone()) {
        Ok(t) => t,
        Err(e) => {
            warn!("Failed to add table normally, saving with errors: {}", e);

            // Add the add_table error to table.errors
            let mut error_map = HashMap::new();
            error_map.insert(
                "type".to_string(),
                serde_json::Value::String("import_error".to_string()),
            );
            error_map.insert(
                "message".to_string(),
                serde_json::Value::String(e.to_string()),
            );
            error_map.insert(
                "field".to_string(),
                serde_json::Value::String("table".to_string()),
            );
            table_with_errors.errors.push(error_map);

            // Save table with errors
            match model_service.add_table_with_errors(table_with_errors) {
                Ok(t) => t,
                Err(e2) => {
                    error!("Failed to save table even with errors: {}", e2);
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        }
    };

    let errors_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "type": e.error_type,
                "field": e.field,
                "message": e.message
            })
        })
        .collect();

    // Add import errors if table has errors
    let mut import_errors = errors_json;
    if !added_table.errors.is_empty() {
        import_errors.push(json!({
            "type": "table_error",
            "table": added_table.name,
            "message": format!("Table '{}' was saved but has {} error(s)", added_table.name, added_table.errors.len())
        }));
    }

    Ok(Json(json!({
        "tables": [serde_json::to_value(&added_table).unwrap_or(json!({}))],
        "ai_suggestions": json!([]),
        "errors": import_errors
    })))
}

/// POST /import/sql - Import tables from SQL file
pub async fn import_sql(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut sql_content = String::new();
    let mut dialect = "generic".to_string(); // Default dialect
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            if let Ok(content) = field.bytes().await {
                sql_content = String::from_utf8_lossy(&content).to_string();
            }
        } else if name == "use_ai" {
            // Parse use_ai flag (not used yet, but parsed for future AI integration)
            let _ = field.text().await;
        } else if name == "dialect" {
            // Parse dialect field
            if let Ok(d) = field.text().await {
                dialect = d;
            }
        }
    }

    if sql_content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Sanitize content
    sql_content = sql_content.replace('\x00', "");
    if sql_content.len() > 10 * 1024 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse SQL before any await points to avoid Send issues
    // SQLParser contains a Box<dyn Dialect> which is not Send
    info!("[Import] Starting SQL import with dialect: '{}'", dialect);
    let (tables, tables_requiring_name) = {
        let parser = SQLParser::with_dialect_name(&dialect);
        match parser.parse(&sql_content) {
            Ok(result) => {
                info!(
                    "[Import] Parsed {} tables from SQL with dialect '{}'",
                    result.0.len(),
                    dialect
                );
                // Log database_type for each table
                for (idx, table) in result.0.iter().enumerate() {
                    if let Some(ref db_type) = table.database_type {
                        info!(
                            "[Import] Table {} '{}' has database_type: {:?}",
                            idx, table.name, db_type
                        );
                    } else {
                        warn!(
                            "[Import] Table {} '{}' has NO database_type set",
                            idx, table.name
                        );
                    }
                }
                result
            }
            Err(e) => {
                error!(
                    "[Import] SQL parsing error with dialect '{}': {}",
                    dialect, e
                );
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    };

    // If any tables require name input, return them for user confirmation
    if !tables_requiring_name.is_empty() {
        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        let name_inputs_json: Vec<Value> = tables_requiring_name
            .iter()
            .map(|tni| {
                json!({
                "table_index": tni.table_index,
                "suggested_name": tni.suggested_name,
                    "original_expression": tni.original_expression
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "tables_requiring_name": name_inputs_json,
            "requires_name_input": true,
            "ai_suggestions": json!([]),
            "errors": json!([])
        })));
    }

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(&tables);
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                "new_table": t1.name,
                "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "conflicts": conflict_info,
            "errors": json!([])
        })));
    }

    // Add tables to model - save even if they have errors
    let mut added_tables = Vec::new();
    let mut import_errors = Vec::new();

    for mut table in tables {
        let db_type_before = table.database_type.map(|dt| format!("{:?}", dt));
        info!(
            "[Import] Adding table '{}' with database_type: {:?}",
            table.name, db_type_before
        );

        // Try normal add first
        match model_service.add_table(table.clone()) {
            Ok(added_table) => {
                let db_type_after = added_table.database_type.map(|dt| format!("{:?}", dt));
                info!(
                    "[Import] Successfully added table '{}', database_type preserved: {:?} -> {:?}",
                    added_table.name, db_type_before, db_type_after
                );
                added_tables.push(added_table);
            }
            Err(e) => {
                warn!(
                    "[Import] Failed to add table {} normally, saving with errors: {}",
                    table.name, e
                );

                // Add the error to the table's errors field
                use std::collections::HashMap;
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String("import_error".to_string()),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(e.to_string()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String("table".to_string()),
                );
                table.errors.push(error_map);

                // Save table with errors
                match model_service.add_table_with_errors(table.clone()) {
                    Ok(added_table) => {
                        info!("[Import] Saved table '{}' with errors", added_table.name);
                        added_tables.push(added_table);

                        // Add to import_errors for frontend display
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Table '{}' was saved but has errors", table.name)
                        }));
                    }
                    Err(e2) => {
                        error!(
                            "[Import] Failed to save table {} even with errors: {}",
                            table.name, e2
                        );
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Failed to save table '{}': {}", table.name, e2)
                        }));
                    }
                }
            }
        }
    }

    // Ensure model persists after import - verify it's still available
    // Log model state for debugging
    if let Some(model) = model_service.get_current_model() {
        info!(
            "[Import] Model state after import: {} tables in memory, git_dir: {}",
            model.tables.len(),
            model.git_directory_path
        );
    } else {
        warn!("[Import] WARNING: Model is None after import! Attempting to reload...");
        if let Err(e) = model_service.ensure_model_available() {
            error!(
                "[Import] CRITICAL: Model not available after import and reload failed: {}",
                e
            );
        } else if let Some(model) = model_service.get_current_model() {
            info!(
                "[Import] Model reloaded successfully: {} tables",
                model.tables.len()
            );
        }
    }

    let tables_json: Vec<Value> = added_tables
        .iter()
        .map(|t| {
            let db_type_str = t.database_type.map(|dt| format!("{:?}", dt));
            info!(
                "[Import] Serializing table '{}' with database_type: {:?}, errors: {}",
                t.name,
                db_type_str,
                t.errors.len()
            );
            serde_json::to_value(t).unwrap_or(json!({}))
        })
        .collect();

    info!(
        "[Import] Returning {} tables in response ({} with errors)",
        tables_json.len(),
        added_tables.iter().filter(|t| !t.errors.is_empty()).count()
    );
    Ok(Json(json!({
        "tables": tables_json,
        "errors": import_errors
    })))
}

/// POST /import/sql/text - Import tables from SQL text
pub async fn import_sql_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SQLTextImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Ensure workspace is loaded from session before importing
    if let Err(e) = super::workspace::ensure_workspace_loaded(&state, &headers).await {
        warn!("[Import] Failed to ensure workspace loaded: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Basic sanitization
    let sql_content = request.content.replace('\x00', "");
    if sql_content.len() > 10 * 1024 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse SQL before any await points to avoid Send issues
    // SQLParser contains a Box<dyn Dialect> which is not Send
    let dialect = request.dialect.as_deref().unwrap_or("generic");
    let (mut tables, tables_requiring_name) = {
        let parser = SQLParser::with_dialect_name(dialect);
        match parser.parse(&sql_content) {
            Ok(result) => result,
            Err(e) => {
                error!("SQL parsing error: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    };

    // If table_names are provided, update table names using the table_index from tables_requiring_name
    let mut all_names_provided = true;
    if let Some(ref table_names) = request.table_names {
        for name_input in &tables_requiring_name {
            let table_index_str = name_input.table_index.to_string();
            if let Some(new_name) = table_names.get(&table_index_str) {
                if let Some(table) = tables.get_mut(name_input.table_index) {
                    info!(
                        "[Import] Updating table name from '{}' to '{}' (index: {})",
                        table.name, new_name, name_input.table_index
                    );
                    table.name = new_name.clone();
                } else {
                    warn!(
                        "[Import] Table index {} out of range (total tables: {})",
                        name_input.table_index,
                        tables.len()
                    );
                    all_names_provided = false;
                }
            } else {
                warn!(
                    "[Import] Table name not provided for table index {}",
                    name_input.table_index
                );
                all_names_provided = false;
            }
        }
    } else if !tables_requiring_name.is_empty() {
        all_names_provided = false;
    }

    // If any tables still require name input AND not all names were provided, return them for user confirmation
    if !tables_requiring_name.is_empty() && !all_names_provided {
        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        let name_inputs_json: Vec<Value> = tables_requiring_name
            .iter()
            .map(|tni| {
                json!({
                "table_index": tni.table_index,
                "suggested_name": tni.suggested_name,
                    "original_expression": tni.original_expression
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "tables_requiring_name": name_inputs_json,
            "requires_name_input": true,
            "ai_suggestions": json!([]),
            "errors": json!([])
        })));
    }

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(&tables);
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                "new_table": t1.name,
                "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "conflicts": conflict_info,
            "errors": json!([])
        })));
    }

    // Add tables to model - save even if they have errors
    let mut added_tables = Vec::new();
    let mut import_errors = Vec::new();

    for mut table in tables {
        match model_service.add_table(table.clone()) {
            Ok(added_table) => {
                added_tables.push(added_table);
            }
            Err(e) => {
                warn!(
                    "Failed to add table {} normally, saving with errors: {}",
                    table.name, e
                );

                // Add the add_table error to table.errors
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String("import_error".to_string()),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(e.to_string()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String("table".to_string()),
                );
                table.errors.push(error_map);

                // Save table with errors
                match model_service.add_table_with_errors(table.clone()) {
                    Ok(added_table) => {
                        added_tables.push(added_table);
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Table '{}' was saved but has errors", table.name)
                        }));
                    }
                    Err(e2) => {
                        error!(
                            "Failed to save table {} even with errors: {}",
                            table.name, e2
                        );
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Failed to save table '{}': {}", table.name, e2)
                        }));
                    }
                }
            }
        }
    }

    // Ensure model persists after import - verify it's still available
    // Log model state for debugging
    if let Some(model) = model_service.get_current_model() {
        info!(
            "[Import] Model state after import_sql_text: {} tables in memory, git_dir: {}",
            model.tables.len(),
            model.git_directory_path
        );
    } else {
        warn!("[Import] WARNING: Model is None after import_sql_text! Attempting to reload...");
        if let Err(e) = model_service.ensure_model_available() {
            error!("[Import] CRITICAL: Model not available after import_sql_text and reload failed: {}", e);
        } else if let Some(model) = model_service.get_current_model() {
            info!(
                "[Import] Model reloaded successfully after import_sql_text: {} tables",
                model.tables.len()
            );
        }
    }

    let tables_json: Vec<Value> = added_tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    Ok(Json(json!({
        "tables": tables_json,
        "ai_suggestions": json!([]),
        "errors": import_errors
    })))
}

/// POST /import/avro - Import tables from AVRO schema file
async fn import_avro(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut avro_content = String::new();
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            if let Ok(content) = field.bytes().await {
                if content.len() > 10 * 1024 * 1024 {
                    return Err(StatusCode::BAD_REQUEST);
                }
                avro_content = String::from_utf8_lossy(&content).to_string();
            }
        } else if name == "use_ai" {
            let _ = field.text().await;
        }
    }

    if avro_content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Sanitize content
    avro_content = avro_content.replace('\x00', "");

    // Parse AVRO
    let parser = AvroParser::new();
    let (tables, parse_errors) = match parser.parse(&avro_content) {
        Ok(result) => result,
        Err(e) => {
            error!("AVRO parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    if tables.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(&tables);
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                    "new_table": t1.name,
                    "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "conflicts": conflict_info,
            "errors": errors_json
        })));
    }

    // Add parse errors to tables
    let mut tables_with_errors = tables;
    for parse_error in &parse_errors {
        // Find the table this error belongs to (if field indicates a table)
        if let Some(table_name) = parse_error.field.as_ref().and_then(|f| {
            // Try to extract table name from field if it's in format "table_name.field"
            f.split('.').next()
        }) {
            if let Some(table) = tables_with_errors.iter_mut().find(|t| t.name == table_name) {
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String(parse_error.error_type.clone()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String(
                        parse_error
                            .field
                            .as_deref()
                            .unwrap_or("unknown")
                            .to_string(),
                    ),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(parse_error.message.clone()),
                );
                table.errors.push(error_map);
            }
        }
    }

    // Add tables to model - save even if they have errors
    let mut added_tables = Vec::new();
    let mut import_errors = Vec::new();

    for mut table in tables_with_errors {
        match model_service.add_table(table.clone()) {
            Ok(added_table) => {
                added_tables.push(added_table);
            }
            Err(e) => {
                warn!(
                    "Failed to add table {} normally, saving with errors: {}",
                    table.name, e
                );

                // Add the add_table error to table.errors
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String("import_error".to_string()),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(e.to_string()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String("table".to_string()),
                );
                table.errors.push(error_map);

                // Save table with errors
                match model_service.add_table_with_errors(table.clone()) {
                    Ok(added_table) => {
                        added_tables.push(added_table);
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Table '{}' was saved but has errors", table.name)
                        }));
                    }
                    Err(e2) => {
                        error!(
                            "Failed to save table {} even with errors: {}",
                            table.name, e2
                        );
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Failed to save table '{}': {}", table.name, e2)
                        }));
                    }
                }
            }
        }
    }

    let tables_json: Vec<Value> = added_tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    let errors_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "type": e.error_type,
                "field": e.field,
                "message": e.message
            })
        })
        .collect();

    // Combine parse errors with import errors
    let mut all_errors = errors_json;
    all_errors.extend(import_errors);

    Ok(Json(json!({
        "tables": tables_json,
        "ai_suggestions": json!([]),
        "errors": all_errors
    })))
}

/// POST /import/json-schema - Import tables from JSON Schema file
async fn import_json_schema(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut json_content = String::new();
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            if let Ok(content) = field.bytes().await {
                if content.len() > 10 * 1024 * 1024 {
                    return Err(StatusCode::BAD_REQUEST);
                }
                json_content = String::from_utf8_lossy(&content).to_string();
            }
        } else if name == "use_ai" {
            let _ = field.text().await;
        }
    }

    if json_content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Sanitize content
    json_content = json_content.replace('\x00', "");

    // Parse JSON Schema
    let parser = JSONSchemaParser::new();
    let (tables, parse_errors) = match parser.parse(&json_content) {
        Ok(result) => result,
        Err(e) => {
            error!("JSON Schema parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    if tables.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(&tables);
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                    "new_table": t1.name,
                    "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "conflicts": conflict_info,
            "errors": errors_json
        })));
    }

    // Add parse errors to tables
    let mut tables_with_errors = tables;
    for parse_error in &parse_errors {
        // Find the table this error belongs to (if field indicates a table)
        if let Some(table_name) = parse_error.field.as_ref().and_then(|f| {
            // Try to extract table name from field if it's in format "table_name.field"
            f.split('.').next()
        }) {
            if let Some(table) = tables_with_errors.iter_mut().find(|t| t.name == table_name) {
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String(parse_error.error_type.clone()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String(
                        parse_error
                            .field
                            .as_deref()
                            .unwrap_or("unknown")
                            .to_string(),
                    ),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(parse_error.message.clone()),
                );
                table.errors.push(error_map);
            }
        }
    }

    // Add tables to model - save even if they have errors
    let mut added_tables = Vec::new();
    let mut import_errors = Vec::new();

    for mut table in tables_with_errors {
        match model_service.add_table(table.clone()) {
            Ok(added_table) => {
                added_tables.push(added_table);
            }
            Err(e) => {
                warn!(
                    "Failed to add table {} normally, saving with errors: {}",
                    table.name, e
                );

                // Add the add_table error to table.errors
                let mut error_map = HashMap::new();
                error_map.insert(
                    "type".to_string(),
                    serde_json::Value::String("import_error".to_string()),
                );
                error_map.insert(
                    "message".to_string(),
                    serde_json::Value::String(e.to_string()),
                );
                error_map.insert(
                    "field".to_string(),
                    serde_json::Value::String("table".to_string()),
                );
                table.errors.push(error_map);

                // Save table with errors
                match model_service.add_table_with_errors(table.clone()) {
                    Ok(added_table) => {
                        added_tables.push(added_table);
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Table '{}' was saved but has errors", table.name)
                        }));
                    }
                    Err(e2) => {
                        error!(
                            "Failed to save table {} even with errors: {}",
                            table.name, e2
                        );
                        import_errors.push(json!({
                            "type": "table_error",
                            "table": table.name,
                            "message": format!("Failed to save table '{}': {}", table.name, e2)
                        }));
                    }
                }
            }
        }
    }

    let tables_json: Vec<Value> = added_tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    let errors_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "type": e.error_type,
                "field": e.field,
                "message": e.message
            })
        })
        .collect();

    // Combine parse errors with import errors
    let mut all_errors = errors_json;
    all_errors.extend(import_errors);

    Ok(Json(json!({
        "tables": tables_json,
        "ai_suggestions": json!([]),
        "errors": all_errors
    })))
}

/// POST /import/protobuf - Import tables from Protobuf .proto file
async fn import_protobuf(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut proto_content = String::new();
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            if let Ok(content) = field.bytes().await {
                if content.len() > 10 * 1024 * 1024 {
                    return Err(StatusCode::BAD_REQUEST);
                }
                proto_content = String::from_utf8_lossy(&content).to_string();
            }
        } else if name == "use_ai" {
            let _ = field.text().await;
        }
    }

    if proto_content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Sanitize content
    proto_content = proto_content.replace('\x00', "");

    // Parse Protobuf
    let parser = ProtobufParser::new();
    let (tables, parse_errors) = match parser.parse(&proto_content) {
        Ok(result) => result,
        Err(e) => {
            error!("Protobuf parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    if tables.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut model_service = state.model_service.lock().await;

    // Check for naming conflicts
    let conflicts = model_service.detect_naming_conflicts(&tables);
    if !conflicts.is_empty() {
        let conflict_info: Vec<Value> = conflicts
            .iter()
            .map(|(t1, t2)| {
                json!({
                    "new_table": t1.name,
                    "existing_table": t2.name,
                    "message": format!("Table '{}' conflicts with existing table", t1.name)
                })
            })
            .collect();

        let tables_json: Vec<Value> = tables
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
            .collect();

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();

        return Ok(Json(json!({
            "tables": tables_json,
            "conflicts": conflict_info,
            "errors": errors_json
        })));
    }

    // Add tables to model
    let mut added_tables = Vec::new();
    for table in tables {
        match model_service.add_table(table.clone()) {
            Ok(added_table) => added_tables.push(added_table),
            Err(e) => {
                warn!("Failed to add table {}: {}", table.name, e);
            }
        }
    }

    let tables_json: Vec<Value> = added_tables
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
        .collect();

    let errors_json: Vec<Value> = parse_errors
        .iter()
        .map(|e| {
            json!({
                "type": e.error_type,
                "field": e.field,
                "message": e.message
            })
        })
        .collect();

    Ok(Json(json!({
        "tables": tables_json,
        "ai_suggestions": json!([]),
        "errors": errors_json
    })))
}
