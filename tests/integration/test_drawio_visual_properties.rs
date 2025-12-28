//! Integration tests for DrawIO visual properties preservation.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::enums::{Cardinality, MedallionLayer, RelationshipType};
use data_modelling_api::api::models::relationship::{ConnectionPoint, Relationship, VisualMetadata};
use data_modelling_api::api::models::table::Position;
use data_modelling_api::api::models::Table;
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
        Some("Test model for visual properties".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_medallion_layer_colors_preserved() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables with different medallion layers
    let bronze_table = Table {
        id: Uuid::new_v4(),
        name: "bronze_table".to_string(),
        columns: vec![Column::new("id".to_string(), "INT".to_string())],
        database_type: None,
        catalog_name: None,
        schema_name: None,
        medallion_layers: vec![MedallionLayer::Bronze],
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

    let gold_table = Table {
        id: Uuid::new_v4(),
        name: "gold_table".to_string(),
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
        position: Some(Position { x: 300.0, y: 0.0 }),
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    model_service.add_table(bronze_table).unwrap();
    model_service.add_table(gold_table).unwrap();

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify medallion layer colors are in XML
    assert!(xml.contains("#CD7F32")); // Bronze color
    assert!(xml.contains("#FFD700")); // Gold color
}

#[test]
fn test_rounded_rectangles_shape() {
    let (mut model_service, temp_dir) = setup_test_model();

    let table = Table {
        id: Uuid::new_v4(),
        name: "rounded_table".to_string(),
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

    // Verify rounded rectangle style
    assert!(xml.contains("rounded=1")); // Rounded rectangles
}

#[test]
fn test_relationship_edge_colors() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create two tables
    let table1 = Table {
        id: Uuid::new_v4(),
        name: "source".to_string(),
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
        name: "target".to_string(),
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

    // Create relationship with DataFlow type (blue)
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::DataFlow),
    ).unwrap();

    let mut model_mut = model_service.get_current_model_mut().unwrap();
    model_mut.relationships.push(relationship);

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify DataFlow color (blue) is in XML
    assert!(xml.contains("#0066CC")); // DataFlow blue color
}

#[test]
fn test_relationship_dash_patterns() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables
    let table1 = Table {
        id: Uuid::new_v4(),
        name: "table1".to_string(),
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
        name: "table2".to_string(),
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

    // Create relationship with Dependency type (dashed)
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::Dependency),
    ).unwrap();

    let mut model_mut = model_service.get_current_model_mut().unwrap();
    model_mut.relationships.push(relationship);

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify dashed pattern is in XML
    assert!(xml.contains("dashed=1")); // Dependency uses dashed pattern
}

#[test]
fn test_relationship_label_positioning() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables
    let table1 = Table {
        id: Uuid::new_v4(),
        name: "table1".to_string(),
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
        name: "table2".to_string(),
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

    // Create relationship with label position
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        None,
    ).unwrap();

    // Add visual metadata with label position
    let mut model_mut = model_service.get_current_model_mut().unwrap();
    if let Some(rel) = model_mut.relationships.iter_mut().find(|r| r.id == relationship.id) {
        rel.visual_metadata = Some(VisualMetadata {
            source_connection_point: Some("east".to_string()),
            target_connection_point: Some("west".to_string()),
            routing_waypoints: vec![
                ConnectionPoint { x: 150.0, y: 50.0 },
            ],
            label_position: Some(ConnectionPoint { x: 150.0, y: 30.0 }),
        });
    }

    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    let xml = drawio_service.export_to_drawio(model).unwrap();

    // Verify waypoints are in XML (label position is part of visual metadata)
    assert!(xml.contains("x=\"150")); // Waypoint x coordinate
    assert!(xml.contains("y=\"50")); // Waypoint y coordinate
}
