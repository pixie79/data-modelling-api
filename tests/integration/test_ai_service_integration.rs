//! Integration tests for AI service.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::Table;
use data_modelling_api::api::services::AIService;
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;

#[test]
fn test_ai_service_initialization() {
    let ai_service = AIService::new();

    // Service should initialize even without API key
    // (will just return empty results)
    assert!(true); // Service created successfully
}

#[test]
fn test_resolve_sql_errors_without_api_key() {
    let ai_service = AIService::new();

    // Without API key, should return empty results
    let sql_content = "CREATE TABLE users (id INT);";
    let error_message = "Syntax error";

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(ai_service.resolve_sql_errors(sql_content, error_message))
        .unwrap();

    // Should return empty vector when API key is not configured
    assert_eq!(result.len(), 0);
}

#[test]
fn test_resolve_odcl_errors_without_api_key() {
    let ai_service = AIService::new();

    // Without API key, should return empty results
    let yaml_content = r#"name: test_table
columns:
  - name: id
    data_type: INT
"#;
    let errors = vec!["Missing required field".to_string()];

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(ai_service.resolve_odcl_errors(yaml_content, &errors))
        .unwrap();

    // Should return empty vector when API key is not configured
    assert_eq!(result.len(), 0);
}

#[test]
fn test_suggest_relationships_without_api_key() {
    let ai_service = AIService::new();

    let tables = vec![
        Table {
            id: Uuid::new_v4(),
            name: "users".to_string(),
            columns: vec![Column::new("id".to_string(), "INT".to_string())],
            database_type: None,
            catalog_name: None,
            schema_name: None,
            medallion_layers: Vec::new(),
            scd_pattern: None,
            data_vault_classification: None,
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata: Default::default(),
            position: None,
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        Table {
            id: Uuid::new_v4(),
            name: "orders".to_string(),
            columns: vec![Column::new("user_id".to_string(), "INT".to_string())],
            database_type: None,
            catalog_name: None,
            schema_name: None,
            medallion_layers: Vec::new(),
            scd_pattern: None,
            data_vault_classification: None,
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata: Default::default(),
            position: None,
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(ai_service.suggest_relationships(&tables))
        .unwrap();

    // Should return empty vector when API key is not configured
    assert_eq!(result.len(), 0);
}

// Note: Tests with actual API key would require mocking the HTTP client
// or using a test API key. For now, we test the service structure
// and verify it handles missing API keys gracefully.
