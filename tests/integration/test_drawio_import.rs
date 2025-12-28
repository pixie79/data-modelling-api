//! Integration tests for DrawIO import functionality.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::enums::{Cardinality, RelationshipType};
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
        Some("Test model for DrawIO import".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_import_drawio_xml_restores_positions() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table with initial position
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
        position: Some(Position { x: 100.0, y: 200.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table.clone()).unwrap();

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Modify position in XML (simulate editing in draw.io)
    let modified_xml = xml.replace("x=\"100\"", "x=\"300\"")
        .replace("y=\"200\"", "y=\"400\"");

    // Import modified XML
    let document = DrawIOService::parse_drawio_xml(&modified_xml).unwrap();
    let positions = DrawIOService::extract_table_positions(&document);

    // Verify position was updated
    assert_eq!(positions.get(&table.id), Some(&(300.0, 400.0)));
}

#[test]
fn test_import_drawio_xml_restores_routing() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables
    let table1 = Table {
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
        position: Some(Position { x: 0.0, y: 0.0 }),
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
        medallion_layers: Vec::new(),
        scd_pattern: None,
        data_vault_classification: None,
        modeling_level: None,
        tags: Vec::new(),
        odcl_metadata: Default::default(),
        position: Some(Position { x: 300.0, y: 0.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table1).unwrap();
    model_service.add_table(table2).unwrap();

    let model = model_service.get_current_model().unwrap();
    let table1_id = model.tables[0].id;
    let table2_id = model.tables[1].id;

    // Create relationship with routing
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::ForeignKey),
    ).unwrap();

    // Add visual metadata
    let mut model_mut = model_service.get_current_model_mut().unwrap();
    if let Some(rel) = model_mut.relationships.iter_mut().find(|r| r.id == relationship.id) {
        rel.visual_metadata = Some(VisualMetadata {
            source_connection_point: Some("east".to_string()),
            target_connection_point: Some("west".to_string()),
            routing_waypoints: vec![
                ConnectionPoint { x: 150.0, y: 50.0 },
                ConnectionPoint { x: 200.0, y: 50.0 },
            ],
            label_position: Some(ConnectionPoint { x: 175.0, y: 30.0 }),
        });
    }

    // Export to DrawIO XML
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Import XML
    let document = DrawIOService::parse_drawio_xml(&xml).unwrap();
    let routing = DrawIOService::extract_relationship_routing(&document);

    // Verify routing was extracted
    assert!(routing.contains_key(&relationship.id));
    let waypoints = routing.get(&relationship.id).unwrap();
    assert_eq!(waypoints.len(), 2);
    assert_eq!(waypoints[0], (150.0, 50.0));
    assert_eq!(waypoints[1], (200.0, 50.0));
}

#[test]
fn test_import_validates_xml_structure() {
    // Test that invalid XML is rejected
    let invalid_xml = "<not-a-drawio-file></not-a-drawio-file>";

    let result = DrawIOService::validate_drawio_xml(invalid_xml);
    assert!(result.is_err());

    // Test that valid XML structure is accepted
    let valid_xml = r#"<mxfile host="test" modified="2024-01-01T00:00:00.000Z" version="1.0" type="device">
        <diagram id="test" name="test">
            <mxGraphModel dx="1422" dy="794" grid="1" gridSize="10">
                <root>
                    <mxCell id="0"/>
                    <mxCell id="1" parent="0"/>
                </root>
            </mxGraphModel>
        </diagram>
    </mxfile>"#;

    let result = DrawIOService::validate_drawio_xml(valid_xml);
    assert!(result.is_ok());
}

#[test]
fn test_import_edited_drawio_xml() {
    // Test the full workflow: export, edit, import
    let (mut model_service, temp_dir) = setup_test_model();

    // Create table
    let table = Table {
        id: Uuid::new_v4(),
        name: "edited_table".to_string(),
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
        position: Some(Position { x: 50.0, y: 100.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(table.clone()).unwrap();

    // Export
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Simulate editing in draw.io (change position)
    let edited_xml = xml.replace("x=\"50\"", "x=\"500\"")
        .replace("y=\"100\"", "y=\"600\"");

    // Import edited XML
    let document = DrawIOService::parse_drawio_xml(&edited_xml).unwrap();
    let positions = DrawIOService::extract_table_positions(&document);

    // Verify edited position is restored
    assert_eq!(positions.get(&table.id), Some(&(500.0, 600.0)));
}
