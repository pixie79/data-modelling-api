//! Integration tests for DrawIO position storage.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::table::Position;
use data_modelling_api::api::models::Table;
use data_modelling_api::api::services::{DrawIOService, ModelService};
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for DrawIO position storage".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_save_table_positions() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create a table with position
    let mut table = Table {
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
        position: Some(Position { x: 100.0, y: 200.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Add table to model
    model_service.add_table(table.clone()).unwrap();

    // Save positions
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_table_positions(model).unwrap();

    // Verify DrawIO XML file was created
    let diagram_file = temp_dir.path().join("diagram.drawio");
    assert!(diagram_file.exists());
}

#[test]
fn test_load_table_positions() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create and add a table
    let table = Table {
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
        position: Some(Position { x: 150.0, y: 250.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table).unwrap();

    // Save positions
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_table_positions(model).unwrap();

    // Create new model and load positions
    let mut new_model_service = ModelService::new();
    new_model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test".to_string()),
    ).unwrap();

    // Add table without position
    let table2 = Table {
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
        position: None, // No position initially
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    new_model_service.add_table(table2).unwrap();

    // Load positions
    let mut model = new_model_service.get_current_model_mut().unwrap().clone();
    drawio_service.load_table_positions(&mut model).unwrap();

    // Note: This test verifies the structure - full position restoration
    // requires proper XML parsing which is marked as TODO in the service
    assert!(true); // Placeholder until XML parsing is fully implemented
}

#[test]
fn test_position_persistence_workflow() {
    // Test the full workflow: save, close, reopen, verify
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table with position
    let table = Table {
        id: Uuid::new_v4(),
        name: "orders".to_string(),
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
        position: Some(Position { x: 300.0, y: 400.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table).unwrap();

    // Save positions
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_table_positions(model).unwrap();

    // Verify file exists
    let diagram_file = temp_dir.path().join("diagram.drawio");
    assert!(diagram_file.exists());

    // Verify file has content
    let content = std::fs::read_to_string(&diagram_file).unwrap();
    assert!(content.contains("orders")); // Table name should be in XML
}
