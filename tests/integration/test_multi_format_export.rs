//! Integration tests for multi-format export functionality.

use data_modelling_api::api::models::{Column, DataModel, Table};
use data_modelling_api::api::services::export_service::ExportService;
use uuid::Uuid;

fn create_test_model() -> DataModel {
    let columns1 = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        Column::new("email".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table1 = Table::new("users".to_string(), columns1);

    let columns2 = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("user_id".to_string(), "INTEGER".to_string()),
        Column::new("total".to_string(), "DECIMAL(10,2)".to_string()),
    ];
    let table2 = Table::new("orders".to_string(), columns2);

    DataModel {
        id: Uuid::new_v4(),
        name: "Test Model".to_string(),
        description: Some("Test model for multi-format export".to_string()),
        git_directory_path: "/test".to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![table1, table2],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[test]
fn test_export_json_schema_valid() {
    let model = create_test_model();

    let schema = ExportService::export_json_schema(&model, None);

    // Verify JSON Schema structure
    assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["title"], "Test Model");
    assert!(schema["definitions"].is_object());

    // Verify table definitions exist
    let definitions = schema["definitions"].as_object().unwrap();
    assert!(definitions.contains_key("users"));
    assert!(definitions.contains_key("orders"));

    // Verify table schema structure
    let users_schema = &definitions["users"];
    assert_eq!(users_schema["type"], "object");
    assert!(users_schema["properties"].is_object());
}

#[test]
fn test_export_avro_valid() {
    let model = create_test_model();

    let avro = ExportService::export_avro(&model, None);

    // AVRO export returns array for multiple tables
    assert!(avro.is_array());
    let schemas = avro.as_array().unwrap();
    assert_eq!(schemas.len(), 2);

    // Verify each schema structure
    for schema in schemas {
        assert_eq!(schema["type"], "record");
        assert!(schema["name"].is_string());
        assert_eq!(schema["namespace"], "com.datamodel");
        assert!(schema["fields"].is_array());
    }
}

#[test]
fn test_export_protobuf_valid() {
    let model = create_test_model();

    let proto = ExportService::export_protobuf(&model, None);

    // Verify protobuf file structure
    assert!(proto.contains("syntax = \"proto3\";"));
    assert!(proto.contains("package com.datamodel;"));
    assert!(proto.contains("message users {"));
    assert!(proto.contains("message orders {"));

    // Verify field declarations
    assert!(proto.contains("int32"));
    assert!(proto.contains("string"));
}

#[test]
fn test_export_sql_valid() {
    let model = create_test_model();

    let sql = ExportService::export_sql(&model, None, None);

    // Verify SQL contains CREATE TABLE statements
    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("users"));
    assert!(sql.contains("orders"));

    // Verify column definitions
    assert!(sql.contains("id"));
    assert!(sql.contains("name"));
    assert!(sql.contains("email"));
    assert!(sql.contains("total"));
}

#[test]
fn test_export_sql_with_dialect() {
    let model = create_test_model();

    // Test PostgreSQL dialect
    let sql_postgres = ExportService::export_sql(&model, None, Some("postgres"));
    assert!(sql_postgres.contains("CREATE TABLE"));

    // Test MySQL dialect
    let sql_mysql = ExportService::export_sql(&model, None, Some("mysql"));
    assert!(sql_mysql.contains("CREATE TABLE"));
}

#[test]
fn test_export_odcl_valid() {
    let model = create_test_model();

    // Test ODCL v3 format
    let odcl_v3 = ExportService::export_odcl(&model, None, "odcl_v3");
    assert_eq!(odcl_v3.len(), 2);
    assert!(odcl_v3.contains_key("users"));
    assert!(odcl_v3.contains_key("orders"));

    // Verify YAML structure
    let users_yaml = &odcl_v3["users"];
    assert!(users_yaml.contains("name: users"));
    assert!(users_yaml.contains("models:"));

    // Test Data Contract format
    let datacontract = ExportService::export_odcl(&model, None, "datacontract");
    assert_eq!(datacontract.len(), 2);

    // Test simple format
    let simple = ExportService::export_odcl(&model, None, "simple");
    assert_eq!(simple.len(), 2);
}

#[test]
fn test_export_with_table_filtering() {
    let model = create_test_model();
    let table1_id = model.tables[0].id;

    // Export only first table
    let schema = ExportService::export_json_schema(&model, Some(&[table1_id]));

    let definitions = schema["definitions"].as_object().unwrap();
    assert_eq!(definitions.len(), 1);
    assert!(definitions.contains_key("users"));
    assert!(!definitions.contains_key("orders"));
}

#[test]
fn test_export_all_formats_consistent() {
    let model = create_test_model();

    // Export to all formats
    let json_schema = ExportService::export_json_schema(&model, None);
    let avro = ExportService::export_avro(&model, None);
    let protobuf = ExportService::export_protobuf(&model, None);
    let sql = ExportService::export_sql(&model, None, None);
    let odcl = ExportService::export_odcl(&model, None, "odcl_v3");

    // Verify all exports contain table names
    let json_schema_str = serde_json::to_string(&json_schema).unwrap();
    assert!(json_schema_str.contains("users"));
    assert!(json_schema_str.contains("orders"));

    let avro_str = serde_json::to_string(&avro).unwrap();
    assert!(avro_str.contains("users"));
    assert!(avro_str.contains("orders"));

    assert!(protobuf.contains("users"));
    assert!(protobuf.contains("orders"));

    assert!(sql.contains("users"));
    assert!(sql.contains("orders"));

    assert!(odcl.contains_key("users"));
    assert!(odcl.contains_key("orders"));
}

#[test]
fn test_export_empty_model() {
    let model = DataModel {
        id: Uuid::new_v4(),
        name: "Empty Model".to_string(),
        description: None,
        git_directory_path: "/test".to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // All exports should handle empty models gracefully
    let schema = ExportService::export_json_schema(&model, None);
    assert_eq!(schema["definitions"].as_object().unwrap().len(), 0);

    let avro = ExportService::export_avro(&model, None);
    assert_eq!(avro.as_array().unwrap().len(), 0);

    let protobuf = ExportService::export_protobuf(&model, None);
    assert!(protobuf.contains("syntax = \"proto3\";"));
    assert!(!protobuf.contains("message"));

    let sql = ExportService::export_sql(&model, None, None);
    assert!(sql.is_empty() || !sql.contains("CREATE TABLE"));

    let odcl = ExportService::export_odcl(&model, None, "odcl_v3");
    assert_eq!(odcl.len(), 0);
}

#[test]
fn test_export_table_with_complex_types() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("tags".to_string(), "ARRAY<STRING>".to_string()),
        Column::new("metadata".to_string(), "OBJECT".to_string()),
        Column::new("price".to_string(), "DECIMAL(10,2)".to_string()),
        Column::new("is_active".to_string(), "BOOLEAN".to_string()),
        Column::new("created_at".to_string(), "TIMESTAMP".to_string()),
    ];
    let table = Table::new("products".to_string(), columns);

    let model = DataModel {
        id: Uuid::new_v4(),
        name: "Complex Types Model".to_string(),
        description: None,
        git_directory_path: "/test".to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![table],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Test all formats handle complex types
    let schema = ExportService::export_json_schema(&model, None);
    assert!(serde_json::to_string(&schema).unwrap().contains("products"));

    let avro = ExportService::export_avro(&model, None);
    assert!(serde_json::to_string(&avro).unwrap().contains("products"));

    let protobuf = ExportService::export_protobuf(&model, None);
    assert!(protobuf.contains("products"));

    let sql = ExportService::export_sql(&model, None, None);
    assert!(sql.contains("products"));
}
