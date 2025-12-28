//! Unit tests for Protobuf export validation.

use data_modelling_api::api::models::{Column, Table};
use data_modelling_api::export::protobuf::ProtobufExporter;

#[test]
fn test_export_simple_table() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        Column::new("email".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    // Verify message structure
    assert!(proto.contains("message users {"));
    assert!(proto.contains("}"));

    // Verify field declarations
    assert!(proto.contains("int32"));
    assert!(proto.contains("string"));

    // Verify field numbers are sequential
    assert!(proto.contains("= 1;"));
    assert!(proto.contains("= 2;"));
    assert!(proto.contains("= 3;"));
}

#[test]
fn test_export_table_with_nullable_fields() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.nullable = false;

    let name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string()); // nullable by default

    let table = Table::new("users".to_string(), vec![id_col, name_col]);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    // Non-nullable field should not have "optional"
    assert!(proto.contains("int32 id = 1;"));

    // Nullable field should have "optional"
    assert!(proto.contains("optional string name = 2;"));
}

#[test]
fn test_export_table_with_descriptions() {
    let mut id_col = Column::new("id".to_string(), "INTEGER".to_string());
    id_col.description = "User identifier".to_string();

    let mut name_col = Column::new("name".to_string(), "VARCHAR(255)".to_string());
    name_col.description = "User full name".to_string();

    let table = Table::new("users".to_string(), vec![id_col, name_col]);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    // Verify comments are included
    assert!(proto.contains("// User identifier"));
    assert!(proto.contains("// User full name"));
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

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    assert!(proto.contains("int32"));
    assert!(proto.contains("int64"));
    assert!(proto.contains("double"));
    assert!(proto.contains("float"));
}

#[test]
fn test_export_table_with_boolean_type() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("is_active".to_string(), "BOOLEAN".to_string()),
    ];
    let table = Table::new("users".to_string(), columns);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    assert!(proto.contains("bool"));
}

#[test]
fn test_export_table_with_bytes_type() {
    let columns = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("data".to_string(), "BYTES".to_string()),
        Column::new("binary_data".to_string(), "BINARY".to_string()),
    ];
    let table = Table::new("files".to_string(), columns);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    assert!(proto.contains("bytes"));
}

#[test]
fn test_export_table_with_array_type() {
    let mut tags_col = Column::new("tags".to_string(), "ARRAY<STRING>".to_string());
    tags_col.data_type = "ARRAY<STRING>".to_string();

    let table = Table::new("items".to_string(), vec![tags_col]);

    let mut field_number = 0u32;
    let proto = ProtobufExporter::export_table(&table, &mut field_number);

    // Array types should have "repeated" keyword
    assert!(proto.contains("repeated"));
}

#[test]
fn test_export_model_structure() {
    let columns1 = vec![Column::new("id".to_string(), "INTEGER".to_string())];
    let table1 = Table::new("users".to_string(), columns1);

    let columns2 = vec![Column::new("id".to_string(), "INTEGER".to_string())];
    let table2 = Table::new("orders".to_string(), columns2);

    use data_modelling_api::api::models::DataModel;
    let model = DataModel {
        id: uuid::Uuid::new_v4(),
        name: "Test Model".to_string(),
        description: None,
        git_directory_path: "/test".to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![table1, table2],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let proto = ProtobufExporter::export_model(&model, None);

    // Verify protobuf file structure
    assert!(proto.contains("syntax = \"proto3\";"));
    assert!(proto.contains("package com.datamodel;"));
    assert!(proto.contains("message users {"));
    assert!(proto.contains("message orders {"));
}

#[test]
fn test_export_model_with_table_filtering() {
    let columns1 = vec![Column::new("id".to_string(), "INTEGER".to_string())];
    let table1 = Table::new("users".to_string(), columns1);
    let table1_id = table1.id;

    let columns2 = vec![Column::new("id".to_string(), "INTEGER".to_string())];
    let table2 = Table::new("orders".to_string(), columns2);

    use data_modelling_api::api::models::DataModel;
    let model = DataModel {
        id: uuid::Uuid::new_v4(),
        name: "Test Model".to_string(),
        description: None,
        git_directory_path: "/test".to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![table1, table2],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Export only table1
    let proto = ProtobufExporter::export_model(&model, Some(&[table1_id]));

    // Should only contain users table
    assert!(proto.contains("message users {"));
    assert!(!proto.contains("message orders {"));
}
