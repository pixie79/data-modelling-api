//! Integration tests for individual schema file export functionality.

use data_modelling_api::api::models::{Column, DataModel, Table};
use data_modelling_api::export::avro::AvroExporter;
use data_modelling_api::export::json_schema::JSONSchemaExporter;
use data_modelling_api::export::protobuf::ProtobufExporter;
use uuid::Uuid;

fn create_test_model() -> DataModel {
    let columns1 = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("name".to_string(), "VARCHAR(255)".to_string()),
    ];
    let table1 = Table::new("users".to_string(), columns1);

    let columns2 = vec![
        Column::new("id".to_string(), "INTEGER".to_string()),
        Column::new("total".to_string(), "DECIMAL(10,2)".to_string()),
    ];
    let table2 = Table::new("orders".to_string(), columns2);

    DataModel {
        id: Uuid::new_v4(),
        name: "Test Model".to_string(),
        description: Some("Test model for individual schema export".to_string()),
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
fn test_export_individual_json_schema_files() {
    let model = create_test_model();

    // Export individual JSON Schema files per table
    for table in &model.tables {
        let schema = JSONSchemaExporter::export_table(table);

        // Verify it's a valid JSON Schema (not wrapped in definitions)
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["title"], table.name);
        assert!(schema["properties"].is_object());

        // Verify table-specific columns are present
        let properties = schema["properties"].as_object().unwrap();
        if table.name == "users" {
            assert!(properties.contains_key("id"));
            assert!(properties.contains_key("name"));
        } else if table.name == "orders" {
            assert!(properties.contains_key("id"));
            assert!(properties.contains_key("total"));
        }
    }
}

#[test]
fn test_export_individual_avro_files() {
    let model = create_test_model();

    // Export individual AVRO schema files per table
    for table in &model.tables {
        let schema = AvroExporter::export_table(table);

        // Verify it's a valid AVRO schema (not an array)
        assert_eq!(schema["type"], "record");
        assert_eq!(schema["name"], table.name);
        assert_eq!(schema["namespace"], "com.datamodel");
        assert!(schema["fields"].is_array());

        // Verify table-specific fields are present
        let fields = schema["fields"].as_array().unwrap();
        if table.name == "users" {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0]["name"], "id");
            assert_eq!(fields[1]["name"], "name");
        } else if table.name == "orders" {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0]["name"], "id");
            assert_eq!(fields[1]["name"], "total");
        }
    }
}

#[test]
fn test_export_individual_protobuf_files() {
    let model = create_test_model();

    // Export individual Protobuf files per table
    for table in &model.tables {
        let mut field_number = 0u32;
        let proto = ProtobufExporter::export_table(table, &mut field_number);

        // Verify it's a valid Protobuf message (not multiple messages)
        assert!(proto.contains(&format!("message {} {{", table.name)));
        assert!(proto.contains("syntax = \"proto3\";") || !proto.contains("syntax")); // May or may not include syntax

        // Verify table-specific fields are present
        if table.name == "users" {
            assert!(proto.contains("id"));
            assert!(proto.contains("name"));
        } else if table.name == "orders" {
            assert!(proto.contains("id"));
            assert!(proto.contains("total"));
        }
    }
}

#[test]
fn test_individual_schemas_dont_include_other_tables() {
    let model = create_test_model();

    // Export individual JSON Schema for first table only
    let users_schema = JSONSchemaExporter::export_table(&model.tables[0]);
    let schema_str = serde_json::to_string(&users_schema).unwrap();

    // Verify it doesn't contain the other table
    assert!(!schema_str.contains("orders"));
    assert!(schema_str.contains("users"));

    // Verify it doesn't have a "definitions" wrapper
    assert!(!users_schema.as_object().unwrap().contains_key("definitions"));
}
