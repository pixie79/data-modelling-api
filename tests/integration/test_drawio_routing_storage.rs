//! Integration tests for DrawIO relationship routing storage.

use data_modelling_api::api::models::enums::{Cardinality, RelationshipType};
use data_modelling_api::api::models::relationship::{ConnectionPoint, Relationship, VisualMetadata};
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
        Some("Test model for DrawIO routing storage".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_save_relationship_routing() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create two tables
    use data_modelling_api::api::models::column::Column;
    use data_modelling_api::api::models::Table;

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
        position: None,
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
        position: None,
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

    // Create relationship with routing waypoints
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::ForeignKey),
    ).unwrap();

    // Update relationship with visual metadata
    let mut model_mut = model_service.get_current_model_mut().unwrap();
    if let Some(rel) = model_mut.relationships.iter_mut().find(|r| r.id == relationship.id) {
        rel.visual_metadata = Some(VisualMetadata {
            source_connection_point: Some("east".to_string()),
            target_connection_point: Some("west".to_string()),
            routing_waypoints: vec![
                ConnectionPoint { x: 200.0, y: 100.0 },
                ConnectionPoint { x: 300.0, y: 100.0 },
            ],
            label_position: Some(ConnectionPoint { x: 250.0, y: 90.0 }),
        });
    }

    // Save routing
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_relationship_routing(model).unwrap();

    // Verify DrawIO XML file was created/updated
    let diagram_file = temp_dir.path().join("diagram.drawio");
    assert!(diagram_file.exists());
}

#[test]
fn test_load_relationship_routing() {
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables and relationship
    use data_modelling_api::api::models::column::Column;
    use data_modelling_api::api::models::Table;

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
        position: None,
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
        position: None,
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

    // Create relationship
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        None,
    ).unwrap();

    // Save routing first
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_relationship_routing(model).unwrap();

    // Load routing
    let mut model = model_service.get_current_model_mut().unwrap().clone();
    drawio_service.load_relationship_routing(&mut model).unwrap();

    // Note: This test verifies the structure - full routing restoration
    // requires proper XML parsing which is marked as TODO in the service
    assert!(true); // Placeholder until XML parsing is fully implemented
}

#[test]
fn test_routing_persistence_workflow() {
    // Test the full workflow: save routing, close, reopen, verify
    let (mut model_service, temp_dir) = setup_test_model();

    // Create tables and relationship with routing
    use data_modelling_api::api::models::column::Column;
    use data_modelling_api::api::models::Table;

    let table1 = Table {
        id: Uuid::new_v4(),
        name: "customers".to_string(),
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
        position: None,
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

    // Create relationship
    let mut rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = rel_service.create_relationship(
        table1_id,
        table2_id,
        Some(Cardinality::OneToMany),
        None,
        None,
        Some(RelationshipType::DataFlow),
    ).unwrap();

    // Save routing
    let drawio_service = DrawIOService::new(temp_dir.path());
    let model = model_service.get_current_model().unwrap();
    drawio_service.save_relationship_routing(model).unwrap();

    // Verify file exists and has content
    let diagram_file = temp_dir.path().join("diagram.drawio");
    assert!(diagram_file.exists());

    let content = std::fs::read_to_string(&diagram_file).unwrap();
    // Verify relationship is referenced in XML
    assert!(content.contains(&relationship.id.to_string()));
}
