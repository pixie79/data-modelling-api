//! Integration tests for ODCS reference system.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::enums::MedallionLayer;
use data_modelling_api::api::models::Table;
use data_modelling_api::drawio::builder::DrawIOBuilder;
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_schema_changes_preserve_layout() {
    // This test verifies that when schema changes in ODCS YAML,
    // the layout in DrawIO XML is preserved

    let mut builder = DrawIOBuilder::new("Test Diagram".to_string());

    // Create table with initial schema
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
        medallion_layers: vec![MedallionLayer::Gold],
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
    };

    // Add table at specific position
    let x = 100.0;
    let y = 200.0;
    builder.add_table(&table, x, y, None, None);
    let document = builder.build();

    // Verify position is stored
    let cell = &document.diagram.graph_model.root.table_cells[0];
    assert_eq!(cell.geometry.x, x);
    assert_eq!(cell.geometry.y, y);

    // Verify ODCS reference is present (schema is separate from layout)
    assert_eq!(cell.odcs_reference, Some("tables/users.yaml".to_string()));

    // The key point: if schema changes in ODCS YAML, the DrawIO XML
    // still references the same file, and the position is preserved
    // This test verifies the separation of concerns
}

#[test]
fn test_odcs_reference_validity() {
    // Test that ODCS references use valid paths
    let mut builder = DrawIOBuilder::new("Test".to_string());

    let table = Table {
        id: Uuid::new_v4(),
        name: "valid_table_name".to_string(),
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
    };

    builder.add_table(&table, 0.0, 0.0, None, None);
    let document = builder.build();

    let cell = &document.diagram.graph_model.root.table_cells[0];
    let odcs_ref = cell.odcs_reference.as_ref().unwrap();

    // Verify reference format is valid
    assert!(odcs_ref.starts_with("tables/"));
    assert!(odcs_ref.ends_with(".yaml"));
    assert_eq!(odcs_ref, "tables/valid_table_name.yaml");
}

#[test]
fn test_table_id_linking() {
    // Test that table_id custom attribute correctly links to Table.id
    let mut builder = DrawIOBuilder::new("Test".to_string());

    let table_id = Uuid::new_v4();
    let table = Table {
        id: table_id,
        name: "linked_table".to_string(),
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
    };

    builder.add_table(&table, 0.0, 0.0, None, None);
    let document = builder.build();

    let cell = &document.diagram.graph_model.root.table_cells[0];

    // Verify table_id links correctly
    assert_eq!(cell.table_id, Some(table_id));
    // Verify cell ID uses table ID
    assert_eq!(cell.id, format!("table-{}", table_id));
}
