//! Unit tests for AVRO export validation.

use data_modelling_api::api::models::{Column, Table};
use data_modelling_api::export::avro::AvroExporter;
use serde_json::json;

#[test]
fn test_export_simple_table() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        Column::new("email".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let schema = AvroExporter::export_table(&table);

    // Verify schema structure
    assert_eq!(schema["type"], "record");
    assert_eq!(schema["name"], "users");
    assert_eq!(schema["namespace"], "com.datamodel");
    assert!(schema["fields"].is_array());

    // Verify fields
    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0]["name"], "id");
    assert_eq!(fields[1]["name"], "name");
    assert_eq!(fields[2]["name"], "email");
}

#[test]
fn test_export_table_with_nullable_fields() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.nullable = false;

    let name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string()); // nullable by default

    let table = Table::new("users".to_string(), vec![id_col, name_col]);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();

    // Non-nullable field should be just the type
    assert_eq!(fields[0]["type"], "int");

    // Nullable field should be union with null
    assert!(fields[1]["type"].is_array());
    let nullable_type = fields[1]["type"].as_array().unwrap();
    assert_eq!(nullable_type.len(), 2);
    assert_eq!(nullable_type[0], "null");
    assert_eq!(nullable_type[1], "string");
}

#[test]
fn test_export_table_with_descriptions() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.description = "User identifier".to_string();

    let mut name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string());
    name_col.description = "User full name".to_string();

    let table = Table::new("users".to_string(), vec![id_col, name_col]);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields[0]["doc"], "User identifier");
    assert_eq!(fields[1]["doc"], "User full name");
}

#[test]
fn test_export_table_with_numeric_types() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("big_id".to_string(), "BIGINT".to_string()),
        Column::new("price".to_string(), "DECIMAL(10,2)".to_string()),
        Column::new("rating".to_string(), "FLOAT".to_string()),
    ];
    let table = Table::new("products".to_string(), columns);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields[0]["type"], "int");
    assert_eq!(fields[1]["type"], "long");
    assert_eq!(fields[2]["type"], "double");
    assert_eq!(fields[3]["type"], "float");
}

#[test]
fn test_export_table_with_boolean_type() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("is_active".to_string(), "BOOLEAN".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields[1]["type"], "boolean");
}

#[test]
fn test_export_table_with_bytes_type() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("data".to_string(), "BYTES".to_string()),
        Column::new("binary_data".to_string(), "BINARY".to_string()),
    ];
    let table = Table::new("files".to_string(), columns);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields[1]["type"], "bytes");
    assert_eq!(fields[2]["type"], "bytes");
}

#[test]
fn test_export_table_string_types() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        Column::new("description".to_string(), "TEXT".to_string()),
        Column::new("code".to_string(), "CHAR(10)".to_string()),
        Column::new("created_at".to_string(), "DATE".to_string()),
        Column::new("updated_at".to_string(), "TIMESTAMP".to_string()),
    ];
    let table = Table::new("items".to_string(), columns);

    let schema = AvroExporter::export_table(&table);

    let fields = schema["fields"].as_array().unwrap();
    // All string-like types should map to "string" in AVRO
    assert_eq!(fields[1]["type"], "string");
    assert_eq!(fields[2]["type"], "string");
    assert_eq!(fields[3]["type"], "string");
    assert_eq!(fields[4]["type"], "string");
    assert_eq!(fields[5]["type"], "string");
}
