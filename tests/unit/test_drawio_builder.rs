//! Unit tests for DrawIO builder verifying ODCS references are included.

use data_modelling_api::api::models::column::Column;
use data_modelling_api::api::models::enums::{Cardinality, MedallionLayer, RelationshipType};
use data_modelling_api::api::models::{Relationship, Table};
use data_modelling_api::drawio::builder::DrawIOBuilder;
use data_modelling_api::drawio::document::DrawIODocument;
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_odcs_reference_in_table_cell() {
    let mut builder = DrawIOBuilder::new("Test Diagram".to_string());

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

    builder.add_table(&table, 100.0, 200.0, None, None);
    let document = builder.build();

    // Verify ODCS reference is included
    assert_eq!(document.diagram.graph_model.root.table_cells.len(), 1);
    let cell = &document.diagram.graph_model.root.table_cells[0];
    assert_eq!(cell.odcs_reference, Some("tables/users.yaml".to_string()));
    assert_eq!(cell.table_id, Some(table.id));
}

#[test]
fn test_odcs_reference_format() {
    let mut builder = DrawIOBuilder::new("Test".to_string());

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
    // Verify format: tables/{table_name}.yaml
    assert_eq!(cell.odcs_reference, Some("tables/orders.yaml".to_string()));
}

#[test]
fn test_table_id_custom_attribute() {
    let mut builder = DrawIOBuilder::new("Test".to_string());

    let table_id = Uuid::new_v4();
    let table = Table {
        id: table_id,
        name: "products".to_string(),
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
    // Verify table_id custom attribute links to Table.id
    assert_eq!(cell.table_id, Some(table_id));
    assert_eq!(cell.id, format!("table-{}", table_id));
}

#[test]
fn test_relationship_id_custom_attribute() {
    let mut builder = DrawIOBuilder::new("Test".to_string());

    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let relationship_id = Uuid::new_v4();

    let relationship = Relationship {
        id: relationship_id,
        source_table_id: source_id,
        target_table_id: target_id,
        cardinality: Some(Cardinality::OneToMany),
        foreign_key_details: None,
        etl_job_metadata: None,
        relationship_type: Some(RelationshipType::ForeignKey),
        visual_metadata: None,
        drawio_edge_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    builder.add_relationship(&relationship, None);
    let document = builder.build();

    // Verify relationship_id custom attribute links to Relationship.id
    assert_eq!(document.diagram.graph_model.root.relationship_edges.len(), 1);
    let edge = &document.diagram.graph_model.root.relationship_edges[0];
    assert_eq!(edge.relationship_id, Some(relationship_id));
    assert_eq!(edge.id, format!("edge-{}", relationship_id));
}

#[test]
fn test_multiple_tables_with_odcs_references() {
    let mut builder = DrawIOBuilder::new("Test".to_string());

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

    builder.add_table(&table1, 0.0, 0.0, None, None);
    builder.add_table(&table2, 300.0, 0.0, None, None);
    let document = builder.build();

    // Verify both tables have ODCS references
    assert_eq!(document.diagram.graph_model.root.table_cells.len(), 2);
    assert_eq!(
        document.diagram.graph_model.root.table_cells[0].odcs_reference,
        Some("tables/users.yaml".to_string())
    );
    assert_eq!(
        document.diagram.graph_model.root.table_cells[1].odcs_reference,
        Some("tables/orders.yaml".to_string())
    );
}

#[test]
fn test_xml_generation_includes_custom_attributes() {
    let mut builder = DrawIOBuilder::new("Test".to_string());

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

    // Generate XML and verify custom attributes are included
    let xml = document.to_xml().unwrap();

    // Verify ODCS reference is in XML
    assert!(xml.contains("odcs_reference"));
    assert!(xml.contains("tables/test_table.yaml"));

    // Verify table_id is in XML
    assert!(xml.contains("table_id"));
}
