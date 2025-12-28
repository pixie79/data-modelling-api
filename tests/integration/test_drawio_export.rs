//! Integration tests for DrawIO export functionality.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::enums::{Cardinality, MedallionLayer, RelationshipType};
use data_modelling_api::api::models::relationship::{ConnectionPoint, Relationship, VisualMetadata};
use data_modelling_api::api::models::table::Position;
use data_modelling_api::api::models::{DataModel, Table};
use data_modelling_api::api::services::{DrawIOService, ModelService, RelationshipService};
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for DrawIO export".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_export_to_drawio_xml_valid() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables with positions
    let table1 = Table {
        id: Uuid::new_v4(),
        name: "users".to_string(),
        columns: vec![Column::new("id".to_string(), "INT".to_string())],
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
    };

    let table2 = Table {
        id: Uuid::new_v4(),
        name: "orders".to_string(),
        columns: vec![Column::new("id".to_string(), "INT".to_string())],
        database_type: None,
        catalog_name: None,
        schema_name: None,
        medallion_layers: vec![MedallionLayer::Silver],
        scd_pattern: None,
        data_vault_classification: None,
        modeling_level: None,
        tags: Vec::new(),
        odcl_metadata: Default::default(),
        position: Some(Position { x: 400.0, y: 200.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table1).unwrap();
    model_service.add_table(table2).unwrap();

    // Create relationship
    let model = model_service.get_current_model().unwrap();
    let table1_id = model.tables[0].id;
    let table2_id = model.tables[1].id;

    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::ForeignKey),
    ).unwrap();

    // Update model with relationship
    let mut model_mut = model_service.get_current_model_mut().unwrap();
    model_mut.relationships.push(relationship);

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify XML is valid and contains expected elements
    assert!(xml.contains("<mxfile"));
    assert!(xml.contains("users"));
    assert!(xml.contains("orders"));
    assert!(xml.contains("tables/users.yaml")); // ODCS reference
    assert!(xml.contains("tables/orders.yaml")); // ODCS reference
}

#[test]
fn test_export_opens_in_drawio() {
    // This test verifies that the exported XML can be opened in draw.io
    // by checking for required DrawIO XML structure elements
    let (mut model_service, temp_dir) = setup_test_model();

    let table = Table {
        id: Uuid::new_v4(),
        name: "test_table".to_string(),
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
        position: Some(Position { x: 0.0, y: 0.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table).unwrap();

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify required DrawIO XML structure
    assert!(xml.contains("<mxfile"));
    assert!(xml.contains("<diagram"));
    assert!(xml.contains("<mxGraphModel"));
    assert!(xml.contains("<root>"));
    assert!(xml.contains("id=\"0\"")); // Root cell
    assert!(xml.contains("id=\"1\"")); // Layer cell
}

#[test]
fn test_export_preserves_table_positions() {
    let (mut model_service, temp_dir) = setup_test_model();

    let table = Table {
        id: Uuid::new_v4(),
        name: "positioned_table".to_string(),
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
        position: Some(Position { x: 250.0, y: 350.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table).unwrap();

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify position is in XML
    assert!(xml.contains("x=\"250"));
    assert!(xml.contains("y=\"350"));
}

#[test]
fn test_export_includes_all_tables() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create multiple tables
    for i in 0..5 {
        let table = Table {
            id: Uuid::new_v4(),
            name: format!("table_{}", i),
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
            position: Some(Position { x: (i as f64) * 300.0, y: 0.0 }),
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        model_service.add_table(table).unwrap();
    }

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify all tables are in XML
    for i in 0..5 {
        assert!(xml.contains(&format!("table_{}", i)));
        assert!(xml.contains(&format!("tables/table_{}.yaml", i)));
    }
}
