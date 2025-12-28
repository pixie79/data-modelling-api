//! Integration tests for schema preservation during DrawIO import.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::table::Position;
use data_modelling_api::api::models::Table;
use data_modelling_api::api::services::{DrawIOService, ModelService};
use tempfile::TempDir;
use uuid::Uuid;
use chrono::Utc;
use std::fs;
use std::path::Path;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for schema preservation".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_odcs_references_resolved() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table with YAML file path
    let table = Table {
        id: Uuid::new_v4(),
        name: "users".to_string(),
        columns: vec![
            Column::new("id".to_string(), "INT".to_string()),
            Column::new("name".to_string(), "VARCHAR(255)".to_string()),
        ],
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
        yaml_file_path: Some("tables/users.yaml".to_string()),
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table.clone()).unwrap();

    // Create ODCS YAML file
    let tables_dir = temp_dir.path().join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let yaml_content = r#"name: users
columns:
  - name: id
    data_type: INT
  - name: name
    data_type: VARCHAR(255)
"#;

    fs::write(tables_dir.join("users.yaml"), yaml_content).unwrap();

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Import and resolve ODCS references
    let document = DrawIOService::parse_drawio_xml(&xml).unwrap();
    let tables = DrawIOService::resolve_odcs_references(&document, temp_dir.path()).unwrap();

    // Verify ODCS reference was resolved
    assert!(tables.contains_key("tables/users.yaml"));
    let loaded_table = &tables["tables/users.yaml"];
    assert_eq!(loaded_table.name, "users");
    assert_eq!(loaded_table.columns.len(), 2);
}

#[test]
fn test_missing_odcs_references_handled() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table
    let table = Table {
        id: Uuid::new_v4(),
        name: "missing_table".to_string(),
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
        yaml_file_path: Some("tables/missing.yaml".to_string()),
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table).unwrap();

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Import and check for missing references
    let document = DrawIOService::parse_drawio_xml(&xml).unwrap();
    let warnings = DrawIOService::handle_missing_odcs_references(&document, temp_dir.path());

    // Verify missing reference is detected
    assert!(!warnings.is_empty());
    assert!(warnings.iter().any(|w| w.contains("missing.yaml")));
}

#[test]
fn test_schema_preserved_during_import() {
    // Test that schema from ODCS YAML is preserved when importing DrawIO XML
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table with schema
    let table = Table {
        id: Uuid::new_v4(),
        name: "schema_table".to_string(),
        columns: vec![
            Column::new("id".to_string(), "INT".to_string()),
            Column::new("email".to_string(), "VARCHAR(255)".to_string()),
            Column::new("created_at".to_string(), "TIMESTAMP".to_string()),
        ],
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
        yaml_file_path: Some("tables/schema_table.yaml".to_string()),
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table.clone()).unwrap();

    // Create ODCS YAML file
    let tables_dir = temp_dir.path().join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let yaml_content = r#"name: schema_table
columns:
  - name: id
    data_type: INT
  - name: email
    data_type: VARCHAR(255)
  - name: created_at
    data_type: TIMESTAMP
"#;

    fs::write(tables_dir.join("schema_table.yaml"), yaml_content).unwrap();

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Modify position in XML (simulate editing layout in draw.io)
    let edited_xml = xml.replace("x=\"100\"", "x=\"500\"")
        .replace("y=\"200\"", "y=\"600\"");

    // Import and resolve ODCS references
    let document = DrawIOService::parse_drawio_xml(&edited_xml).unwrap();
    let tables = DrawIOService::resolve_odcs_references(&document, temp_dir.path()).unwrap();

    // Verify schema is preserved (columns, data types, etc.)
    assert!(tables.contains_key("tables/schema_table.yaml"));
    let loaded_table = &tables["tables/schema_table.yaml"];
    assert_eq!(loaded_table.name, "schema_table");
    assert_eq!(loaded_table.columns.len(), 3);
    assert_eq!(loaded_table.columns[0].name, "id");
    assert_eq!(loaded_table.columns[0].data_type, "INT");
    assert_eq!(loaded_table.columns[1].name, "email");
    assert_eq!(loaded_table.columns[1].data_type, "VARCHAR(255)");
    assert_eq!(loaded_table.columns[2].name, "created_at");
    assert_eq!(loaded_table.columns[2].data_type, "TIMESTAMP");

    // Verify position was updated from DrawIO XML
    let positions = DrawIOService::extract_table_positions(&document);
    assert_eq!(positions.get(&table.id), Some(&(500.0, 600.0)));
}

#[test]
fn test_layout_restored_schema_unchanged() {
    // Full workflow: export, edit layout in draw.io, import, verify layout changed but schema unchanged
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table with initial schema and position
    let table = Table {
        id: Uuid::new_v4(),
        name: "layout_table".to_string(),
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
        yaml_file_path: Some("tables/layout_table.yaml".to_string()),
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table.clone()).unwrap();

    // Create ODCS YAML file
    let tables_dir = temp_dir.path().join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let yaml_content = r#"name: layout_table
columns:
  - name: id
    data_type: INT
"#;

    fs::write(tables_dir.join("layout_table.yaml"), yaml_content).unwrap();

    // Export
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Edit layout in draw.io (change position)
    let edited_xml = xml.replace("x=\"0\"", "x=\"1000\"")
        .replace("y=\"0\"", "y=\"2000\"");

    // Import
    let document = DrawIOService::parse_drawio_xml(&edited_xml).unwrap();
    let positions = DrawIOService::extract_table_positions(&document);
    let tables = DrawIOService::resolve_odcs_references(&document, temp_dir.path()).unwrap();

    // Verify layout was restored (position changed)
    assert_eq!(positions.get(&table.id), Some(&(1000.0, 2000.0)));

    // Verify schema is unchanged (from ODCS YAML)
    assert!(tables.contains_key("tables/layout_table.yaml"));
    let loaded_table = &tables["tables/layout_table.yaml"];
    assert_eq!(loaded_table.name, "layout_table");
    assert_eq!(loaded_table.columns.len(), 1);
    assert_eq!(loaded_table.columns[0].name, "id");
    assert_eq!(loaded_table.columns[0].data_type, "INT");
}
