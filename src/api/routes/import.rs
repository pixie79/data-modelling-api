//! Import routes for SQL and ODCS/ODCL file imports.
//!
//! Primary format: ODCS (Open Data Contract Standard) v3.1.0
//! Legacy format: ODCL (Data Contract Specification) - deprecated, support ends 31/12/26
//!
//! All import routes require authentication via JWT token.
//! Parsed data is validated for security before being stored.

use axum::{
    Router,
    extract::{Multipart, State},
    http::StatusCode,
    response::Json,
    routing::post,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info, warn};
use utoipa::ToSchema;

use super::auth_context::AuthContext;
use super::tables::AppState;
use crate::models::Table;
use crate::services::{AvroParser, JSONSchemaParser, ODCSParser, ProtobufParser, SQLParser};

/// Validation errors from import validation.
#[derive(Debug, Clone)]
pub struct ImportValidationError {
    pub table_name: String,
    pub field: String,
    pub message: String,
}

/// Validate imported tables for security.
///
/// This function checks:
/// - Table names are valid identifiers (no SQL injection)
/// - Column names are valid identifiers
/// - Data types are valid
/// - No excessively long strings
fn validate_imported_tables(tables: &[Table]) -> Vec<ImportValidationError> {
    let mut errors = Vec::new();

    for table in tables {
        // Validate table name
        if let Err(msg) = validate_identifier(&table.name, "table") {
            errors.push(ImportValidationError {
                table_name: table.name.clone(),
                field: "name".to_string(),
                message: msg,
            });
        }

        // Validate column names
        for column in &table.columns {
            if let Err(msg) = validate_identifier(&column.name, "column") {
                errors.push(ImportValidationError {
                    table_name: table.name.clone(),
                    field: format!("column.{}", column.name),
                    message: msg,
                });
            }

            // Validate data type isn't excessively long or contains suspicious patterns
            if column.data_type.len() > 255 {
                errors.push(ImportValidationError {
                    table_name: table.name.clone(),
                    field: format!("column.{}.data_type", column.name),
                    message: "Data type exceeds maximum length".to_string(),
                });
            }
        }
    }

    errors
}

/// Validate an identifier (table or column name) for security.
fn validate_identifier(name: &str, identifier_type: &str) -> Result<(), String> {
    // Check empty
    if name.is_empty() {
        return Err(format!("{} name cannot be empty", identifier_type));
    }

    // Check length
    if name.len() > 255 {
        return Err(format!(
            "{} name exceeds maximum length of 255",
            identifier_type
        ));
    }

    // Check for SQL injection patterns
    let suspicious_patterns = [
        "--",
        "/*",
        "*/",
        ";",
        "\'",
        "\"\"",
        "DROP ",
        "DELETE ",
        "INSERT ",
        "UPDATE ",
        "EXEC ",
        "EXECUTE ",
        "UNION ",
        "SELECT ",
        "<script",
        "javascript:",
    ];

    let name_upper = name.to_uppercase();
    for pattern in suspicious_patterns {
        if name_upper.contains(pattern) {
            return Err(format!(
                "{} name contains suspicious pattern: {}",
                identifier_type,
                pattern.trim()
            ));
        }
    }

    // Allow alphanumeric, underscores, and hyphens
    // Also allow dots for schema-qualified names (they should be split first)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(format!(
            "{} name contains invalid characters",
            identifier_type
        ));
    }

    Ok(())
}

/// Request for SQL text import
#[derive(Debug, Deserialize, ToSchema)]
pub struct SQLTextImportRequest {
    pub content: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub use_ai: bool,
    #[allow(dead_code)]
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub table_names: Option<HashMap<String, String>>, // Map of table_index -> table_name for dynamic names
    #[serde(default)]
    pub dialect: Option<String>, // SQL dialect name (e.g., "postgres", "mysql", "databricks", "duckdb")
}

/// Request for ODCS/ODCL text import
///
/// Supports ODCS v3.1.0 (primary) and legacy ODCL formats (deprecated, support ends 31/12/26)
#[derive(Debug, Deserialize, ToSchema)]
pub struct ODCLTextImportRequest {
    pub content: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub use_ai: bool,
    #[allow(dead_code)]
    #[serde(default)]
    pub filename: Option<String>,
}

/// Create the import router
///
/// All routes require JWT authentication.
pub fn import_router() -> Router<AppState> {
    Router::new()
        // ODCS v3.1.0 (primary) and legacy ODCL (deprecated, support ends 31/12/26)
        .route("/odcl", post(import_odcl)) // Legacy endpoint name kept for backward compatibility
        .route("/odcl/text", post(import_odcl_text)) // Legacy endpoint name kept for backward compatibility
        .route("/sql", post(import_sql))
        .route("/sql/text", post(import_sql_text))
        .route("/avro", post(import_avro))
        .route("/json-schema", post(import_json_schema))
        .route("/protobuf", post(import_protobuf))
}

/// POST /import/odcl - Import tables from ODCS/ODCL file
///
/// Supports:
/// - ODCS (Open Data Contract Standard) v3.1.0 (primary format)
/// - Legacy ODCL formats (deprecated, support ends 31/12/26)
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/odcl",
    tag = "Import",
    request_body(content = Multipart, description = "ODCS/ODCL YAML file"),
    responses(
        (status = 200, description = "ODCS/ODCL file imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid file or format"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn import_odcl(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    info!(
        "[Import] ODCS/ODCL import by user {} (ODCS v3.1.0 is primary, ODCL is legacy)",
        auth.email
    );
    let mut yaml_content = String::new();
    let _use_ai = false;

    // Parse multipart form data
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("");

        if name == "file" {
            // Validate filename
            if let Some(filename) = field.file_name()
                && !filename.ends_with(".yaml")
                && !filename.ends_with(".yml")
            {
                return Err(StatusCode::BAD_REQUEST);
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
            error!("ODCS/ODCL parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(std::slice::from_ref(&table));
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for ODCS/ODCL import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
        })));
    }

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
                    "field": e.field.clone(),
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

/// POST /import/odcl/text - Import tables from ODCS/ODCL text
///
/// Supports:
/// - ODCS (Open Data Contract Standard) v3.1.0 (primary format)
/// - Legacy ODCL formats (deprecated, support ends 31/12/26)
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/odcl/text",
    tag = "Import",
    request_body = ODCLTextImportRequest,
    responses(
        (status = 200, description = "ODCS/ODCL text imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid content or format"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn import_odcl_text(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<ODCLTextImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    info!(
        "[Import] ODCS/ODCL text import by user {} (ODCS v3.1.0 is primary, ODCL is legacy)",
        auth.email
    );
    // Basic sanitization
    let yaml_content = request.content.replace('\x00', "");
    if yaml_content.len() > 10 * 1024 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut parser = ODCSParser::new();
    let (table, parse_errors) = match parser.parse(&yaml_content) {
        Ok(result) => result,
        Err(e) => {
            error!("ODCS/ODCL parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(std::slice::from_ref(&table));
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for ODCS/ODCL text import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
        })));
    }

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
                    "field": e.field.clone(),
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
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/sql",
    tag = "Import",
    request_body(content = Multipart, description = "SQL file"),
    responses(
        (status = 200, description = "SQL file imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid file or SQL syntax"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn import_sql(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    info!("[Import] SQL import by user {}", auth.email);
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

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(&tables);
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for SQL import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
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
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/sql/text",
    tag = "Import",
    request_body = SQLTextImportRequest,
    responses(
        (status = 200, description = "SQL text imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid SQL syntax"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn import_sql_text(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<SQLTextImportRequest>,
) -> Result<Json<Value>, StatusCode> {
    info!("[Import] SQL text import by user {}", auth.email);

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

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(&tables);
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for SQL text import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
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
            error!(
                "[Import] CRITICAL: Model not available after import_sql_text and reload failed: {}",
                e
            );
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
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/avro",
    tag = "Import",
    request_body(content = Multipart, description = "AVRO schema file"),
    responses(
        (status = 200, description = "AVRO schema imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid AVRO schema"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn import_avro(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    info!("[Import] Avro import by user {}", auth.email);
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

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(&tables);
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for Avro import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
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

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field.clone(),
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
        if let Some(field_str) = parse_error.field.as_ref()
            && let Some(table_name) = field_str.split('.').next()
            && let Some(table) = tables_with_errors.iter_mut().find(|t| t.name == table_name)
        {
            let mut error_map = HashMap::new();
            error_map.insert(
                "type".to_string(),
                serde_json::Value::String(parse_error.error_type.clone()),
            );
            error_map.insert(
                "field".to_string(),
                serde_json::Value::String(field_str.clone()),
            );
            error_map.insert(
                "message".to_string(),
                serde_json::Value::String(parse_error.message.clone()),
            );
            table.errors.push(error_map);
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
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/json-schema",
    tag = "Import",
    request_body(content = Multipart, description = "JSON Schema file"),
    responses(
        (status = 200, description = "JSON Schema imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid JSON Schema"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn import_json_schema(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    info!("[Import] JSON Schema import by user {}", auth.email);
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

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(&tables);
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for JSON Schema import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
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

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field.clone(),
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
        if let Some(field_str) = parse_error.field.as_ref()
            && let Some(table_name) = field_str.split('.').next()
            && let Some(table) = tables_with_errors.iter_mut().find(|t| t.name == table_name)
        {
            let mut error_map = HashMap::new();
            error_map.insert(
                "type".to_string(),
                serde_json::Value::String(parse_error.error_type.clone()),
            );
            error_map.insert(
                "field".to_string(),
                serde_json::Value::String(field_str.clone()),
            );
            error_map.insert(
                "message".to_string(),
                serde_json::Value::String(parse_error.message.clone()),
            );
            table.errors.push(error_map);
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
///
/// Requires JWT authentication.
#[utoipa::path(
    post,
    path = "/import/protobuf",
    tag = "Import",
    request_body(content = Multipart, description = "Protobuf schema file"),
    responses(
        (status = 200, description = "Protobuf schema imported successfully", body = Object),
        (status = 400, description = "Bad request - invalid Protobuf schema"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
async fn import_protobuf(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    info!("[Import] Protobuf import by user {}", auth.email);
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
    let (tables, parse_error_strings) = match parser.parse(&proto_content).await {
        Ok(result) => result,
        Err(e) => {
            error!("Protobuf parsing error: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Convert Vec<String> to Vec<ParserError> for consistency
    let parse_errors: Vec<crate::services::avro_parser::ParserError> = parse_error_strings
        .into_iter()
        .map(|msg| crate::services::avro_parser::ParserError {
            error_type: "parse_error".to_string(),
            field: None,
            message: msg,
        })
        .collect();

    if tables.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate imported tables for security
    let validation_errors = validate_imported_tables(&tables);
    if !validation_errors.is_empty() {
        let errors_json: Vec<Value> = validation_errors
            .iter()
            .map(|e| {
                json!({
                    "type": "validation_error",
                    "table": e.table_name,
                    "field": e.field,
                    "message": e.message
                })
            })
            .collect();
        warn!(
            "[Import] Validation failed for Protobuf import: {:?}",
            validation_errors
        );
        return Ok(Json(json!({
            "tables": [],
            "errors": errors_json
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

        let errors_json: Vec<Value> = parse_errors
            .iter()
            .map(|e| {
                json!({
                    "type": e.error_type,
                    "field": e.field.clone(),
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
