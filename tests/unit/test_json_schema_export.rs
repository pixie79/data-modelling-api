//! Unit tests for JSON Schema export validation.

use data_modelling_api::api::models::{Column, Table};
use data_modelling_api::export::json_schema::JSONSchemaExporter;
use serde_json::json;

#[test]
fn test_export_simple_table() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        Column::new("email".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let schema = JSONSchemaExporter::export_table(&table);

    // Verify schema structure
    assert_eq!(schema["$schema"], "http://json-schema.org/draft-07/schema#");
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["title"], "users");
    assert!(schema["properties"].is_object());

    // Verify properties
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("id"));
    assert!(properties.contains_key("name"));
    assert!(properties.contains_key("email"));

    // Verify data type mapping
    assert_eq!(properties["id"]["type"], "integer");
    assert_eq!(properties["name"]["type"], "string");
    assert_eq!(properties["email"]["type"], "string");
}

#[test]
fn test_export_table_with_required_fields() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.nullable = false;
    id_col.primary_key = true;

    let mut name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string());
    name_col.nullable = false;

    let email_col = Column::new("email".to_string(), "VARCHAR(255)".to_string()); // nullable by default

    let table = Table::new("users".to_string(), vec![id_col, name_col, email_col]);

    let schema = JSONSchemaExporter::export_table(&table);

    // Verify required array exists
    assert!(schema["required"].is_array());
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&json!("id")));
    assert!(required.contains(&json!("name")));
    assert!(!required.contains(&json!("email"))); // email is nullable
}

#[test]
fn test_export_table_with_descriptions() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.description = "User identifier".to_string();

    let mut name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string());
    name_col.description = "User full name".to_string();

    let table = Table::new("users".to_string(), vec![id_col, name_col]);

    let schema = JSONSchemaExporter::export_table(&table);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["id"]["description"], "User identifier");
    assert_eq!(properties["name"]["description"], "User full name");
}

#[test]
fn test_export_table_with_date_types() {
    let columns = vec![
        Column::new("created_at".to_string(), "DATE".to_string()),
        Column::new("updated_at".to_string(), "TIMESTAMP".to_string()),
        Column::new("email".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let schema = JSONSchemaExporter::export_table(&table);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["created_at"]["type"], "string");
    assert_eq!(properties["created_at"]["format"], "date");
    assert_eq!(properties["updated_at"]["type"], "string");
    assert_eq!(properties["updated_at"]["format"], "date-time");
}

#[test]
fn test_export_table_with_numeric_types() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("price".to_string(), "DECIMAL(10,2)".to_string()),
        Column::new("rating".to_string(), "FLOAT".to_string()),
    ];
    let table = Table::new("products".to_string(), columns);

    let schema = JSONSchemaExporter::export_table(&table);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["id"]["type"], "integer");
    assert_eq!(properties["price"]["type"], "number");
    assert_eq!(properties["rating"]["type"], "number");
}

#[test]
fn test_export_table_with_boolean_type() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("is_active".to_string(), "BOOLEAN".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let schema = JSONSchemaExporter::export_table(&table);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["is_active"]["type"], "boolean");
}

#[test]
fn test_export_table_with_special_formats() {
    let columns = vec![
        Column::new("id".to_string(), "UUID".to_string()),
        Column::new("website".to_string(), "URI".to_string()),
        Column::new("contact_email".to_string(), "EMAIL".to_string()),
    ];
    let table = Table::new("companies".to_string(), columns);

    let schema = JSONSchemaExporter::export_table(&table);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties["id"]["type"], "string");
    assert_eq!(properties["id"]["format"], "uuid");
    assert_eq!(properties["website"]["type"], "string");
    assert_eq!(properties["website"]["format"], "uri");
    assert_eq!(properties["contact_email"]["type"], "string");
    assert_eq!(properties["contact_email"]["format"], "email");
}
