//! Integration tests for table properties saving functionality.

use data_modelling_api::api::models::{Column, DataModel, Table};
use data_modelling_api::api::services::ModelService;
use data_modelling_api::api::models::enums::{MedallionLayer, DatabaseType, SCDPattern, DataVaultClassification};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

fn setup_test_model() -> (ModelService, TempDir, Uuid) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for table properties".to_string()),
    ).unwrap();

    // Create a test table
    let table = Table {
        id: Uuid::new_v4(),
        name: "test_table".to_string(),
        columns: vec![Column::new("id".to_string(), "INTEGER".to_string())],
        database_type: None,
        catalog_name: None,
        schema_name: None,
        medallion_layers: Vec::new(),
        scd_pattern: None,
        data_vault_classification: None,
        modeling_level: None,
        tags: Vec::new(),
        odcl_metadata: std::collections::HashMap::new(),
        position: None,
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let added_table = model_service.add_table(table).unwrap();
    let table_id = added_table.id;

    (model_service, temp_dir, table_id)
}

#[test]
fn test_update_table_medallion_layers() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "medallion_layers": ["bronze", "silver"]
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.medallion_layers.len(), 2);
    assert!(updated.medallion_layers.contains(&MedallionLayer::Bronze));
    assert!(updated.medallion_layers.contains(&MedallionLayer::Silver));
}

#[test]
fn test_update_table_database_type() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "database_type": "Postgres"
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.database_type, Some(DatabaseType::Postgres));
}

#[test]
fn test_update_table_scd_pattern() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "scd_pattern": "TYPE_2"
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.scd_pattern, Some(SCDPattern::Type2));
}

#[test]
fn test_update_table_data_vault_classification() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "data_vault_classification": "Hub"
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.data_vault_classification, Some(DataVaultClassification::Hub));
}

#[test]
fn test_update_table_tags() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "tags": ["tag1", "tag2", "tag3"]
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.tags.len(), 3);
    assert!(updated.tags.contains(&"tag1".to_string()));
    assert!(updated.tags.contains(&"tag2".to_string()));
    assert!(updated.tags.contains(&"tag3".to_string()));
}

#[test]
fn test_update_table_quality_rules() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let quality_rules = vec![
        json!({
            "property": "quality",
            "value": "bronze",
            "type": "medallion_layer"
        }),
        json!({
            "property": "data_quality",
            "value": "high"
        }),
    ];

    let updates = json!({
        "quality": quality_rules
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.quality.len(), 2);
}

#[test]
fn test_update_table_odcl_metadata() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "odcl_metadata": {
            "description": "Updated description",
            "owner": "data-team",
            "version": "1.0.0"
        }
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.odcl_metadata.get("description").and_then(|v| v.as_str()), Some("Updated description"));
    assert_eq!(updated.odcl_metadata.get("owner").and_then(|v| v.as_str()), Some("data-team"));
    assert_eq!(updated.odcl_metadata.get("version").and_then(|v| v.as_str()), Some("1.0.0"));
}

#[test]
fn test_update_table_catalog_and_schema() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "catalog_name": "my_catalog",
        "schema_name": "my_schema"
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.catalog_name, Some("my_catalog".to_string()));
    assert_eq!(updated.schema_name, Some("my_schema".to_string()));
}

#[test]
fn test_update_table_all_properties() {
    let (mut model_service, _temp_dir, table_id) = setup_test_model();

    let updates = json!({
        "name": "updated_table_name",
        "medallion_layers": ["gold"],
        "database_type": "DatabricksDelta",
        "scd_pattern": "TYPE_1",
        "tags": ["production", "critical"],
        "catalog_name": "prod_catalog",
        "schema_name": "analytics",
        "quality": [{
            "property": "quality",
            "value": "gold",
            "type": "medallion_layer"
        }],
        "odcl_metadata": {
            "description": "Production table",
            "owner": "analytics-team"
        }
    });

    let updated = model_service.update_table(table_id, &updates).unwrap().unwrap();

    assert_eq!(updated.name, "updated_table_name");
    assert_eq!(updated.medallion_layers.len(), 1);
    assert!(updated.medallion_layers.contains(&MedallionLayer::Gold));
    assert_eq!(updated.database_type, Some(DatabaseType::DatabricksDelta));
    assert_eq!(updated.scd_pattern, Some(SCDPattern::Type1));
    assert_eq!(updated.tags.len(), 2);
    assert_eq!(updated.catalog_name, Some("prod_catalog".to_string()));
    assert_eq!(updated.schema_name, Some("analytics".to_string()));
    assert_eq!(updated.quality.len(), 1);
    assert_eq!(updated.odcl_metadata.get("description").and_then(|v| v.as_str()), Some("Production table"));
}
