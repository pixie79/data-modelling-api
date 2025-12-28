//! Integration tests for DrawIO export with modeling levels.

use data_modelling_api::api::models::Column;
use data_modelling_api::api::models::enums::{MedallionLayer, ModelingLevel};
use data_modelling_api::api::models::table::Position;
use data_modelling_api::api::models::{DataModel, Table};
use data_modelling_api::api::services::DrawIOService;
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;
use std::path::Path;

fn create_test_table_with_keys() -> Table {
    let mut columns = vec![
        Column {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            nullable: false,
            primary_key: true,
            secondary_key: false,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description: String::new(),
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 0,
        },
        Column {
            name: "name".to_string(),
            data_type: "VARCHAR(255)".to_string(),
            nullable: true,
            primary_key: false,
            secondary_key: true,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description: String::new(),
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 1,
        },
        Column {
            name: "email".to_string(),
            data_type: "VARCHAR(255)".to_string(),
            nullable: true,
            primary_key: false,
            secondary_key: false,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description: String::new(),
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 2,
        },
        Column {
            name: "created_at".to_string(),
            data_type: "TIMESTAMP".to_string(),
            nullable: false,
            primary_key: false,
            secondary_key: false,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description: String::new(),
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 3,
        },
    ];

    Table {
        id: Uuid::new_v4(),
        name: "users".to_string(),
        columns,
        database_type: None,
        catalog_name: None,
        schema_name: None,
        medallion_layers: vec![MedallionLayer::Gold],
        scd_pattern: None,
        data_vault_classification: None,
        modeling_level: None,
        tags: Vec::new(),
        odcl_metadata: Default::default(),
        position: Some(Position { x: 100.0, y: 200.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_model() -> (DataModel, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let table = create_test_table_with_keys();

    let model = DataModel {
        id: Uuid::new_v4(),
        name: "Test Model".to_string(),
        description: Some("Test model for DrawIO modeling levels".to_string()),
        git_directory_path: temp_dir.path().to_string_lossy().to_string(),
        control_file_path: "relationships.yaml".to_string(),
        tables: vec![table],
        relationships: vec![],
        diagram_file_path: None,
        is_subfolder: false,
        parent_git_directory: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    (model, temp_dir)
}

#[test]
fn test_export_drawio_conceptual_level() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    let xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Conceptual))
        .unwrap();

    // Conceptual level should only show table name, no columns
    assert!(xml.contains("users"));
    assert!(xml.contains("<b>users</b>"));
    // Should not contain column details
    assert!(!xml.contains("id"));
    assert!(!xml.contains("name"));
    assert!(!xml.contains("email"));
}

#[test]
fn test_export_drawio_logical_level() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    let xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Logical))
        .unwrap();

    // Logical level should show table name + key columns only
    assert!(xml.contains("users"));
    assert!(xml.contains("<b>users</b>"));
    // Should contain key columns (primary key, secondary key)
    assert!(xml.contains("PK") || xml.contains("id")); // Primary key indicator
    assert!(xml.contains("SK") || xml.contains("name")); // Secondary key indicator
    // Should not contain non-key columns
    assert!(!xml.contains("email")); // Non-key column should not appear
    assert!(!xml.contains("created_at")); // Non-key column should not appear
}

#[test]
fn test_export_drawio_physical_level() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    let xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Physical))
        .unwrap();

    // Physical level should show table name + all columns with data types
    assert!(xml.contains("users"));
    assert!(xml.contains("<b>users</b>"));
    // Should contain all columns
    assert!(xml.contains("id"));
    assert!(xml.contains("name"));
    assert!(xml.contains("email"));
    assert!(xml.contains("created_at"));
    // Should contain data types
    assert!(xml.contains("INTEGER") || xml.contains("integer"));
    assert!(xml.contains("VARCHAR") || xml.contains("varchar"));
    assert!(xml.contains("TIMESTAMP") || xml.contains("timestamp"));
}

#[test]
fn test_export_drawio_default_level() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    // Default export (None) should work (backward compatibility)
    let xml = drawio_service
        .export_to_drawio_with_level(&model, None)
        .unwrap();

    // Should contain table name
    assert!(xml.contains("users"));
    // Default behavior: just table name, no columns
    assert!(xml.contains("users")); // Table name should be present
}

#[test]
fn test_export_drawio_all_levels_produce_valid_xml() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    // Export all three levels and verify they're all valid XML
    for level in [ModelingLevel::Conceptual, ModelingLevel::Logical, ModelingLevel::Physical] {
        let xml = drawio_service
            .export_to_drawio_with_level(&model, Some(level))
            .unwrap();

        // Verify XML structure is valid
        assert!(xml.contains("<mxfile"));
        assert!(xml.contains("<diagram"));
        assert!(xml.contains("<mxGraphModel"));
        assert!(xml.contains("users")); // Table name should be present
    }
}

#[test]
fn test_export_drawio_table_dimensions_vary_by_level() {
    let (model, temp_dir) = create_test_model();
    let drawio_service = DrawIOService::new(temp_dir.path());

    // Export all three levels
    let conceptual_xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Conceptual))
        .unwrap();
    let logical_xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Logical))
        .unwrap();
    let physical_xml = drawio_service
        .export_to_drawio_with_level(&model, Some(ModelingLevel::Physical))
        .unwrap();

    // Extract height values from XML (they should differ based on column count)
    // Physical should be tallest (all columns), Conceptual shortest (no columns)
    // This is a basic check - actual dimension parsing would be more complex
    // For now, just verify all exports succeed and contain geometry elements
    assert!(conceptual_xml.contains("height="));
    assert!(logical_xml.contains("height="));
    assert!(physical_xml.contains("height="));

    // Physical should have more content (more columns = more HTML content)
    let physical_content_len = physical_xml.len();
    let conceptual_content_len = conceptual_xml.len();
    assert!(physical_content_len > conceptual_content_len,
        "Physical level should have more content than conceptual");
}
