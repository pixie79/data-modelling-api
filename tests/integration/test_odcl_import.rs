//! Integration tests for ODCL import workflow.

use data_modelling_api::api::models::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, SCDPattern,
};
use data_modelling_api::api::services::{ModelService, ODCLParser};
use tempfile::TempDir;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service
        .create_model(
            "Test Model".to_string(),
            temp_dir.path().to_path_buf(),
            Some("Test model for integration tests".to_string()),
        )
        .unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_import_odcl_file_success() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
    nullable: false
    primary_key: true
  - name: name
    data_type: VARCHAR(255)
    nullable: false
database_type: Postgres
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.database_type, Some(DatabaseType::Postgres));

    // Add to model
    let added_table = model_service.add_table(table.clone()).unwrap();
    assert_eq!(added_table.name, "users");

    // Verify table is in model
    let retrieved = model_service.get_table(added_table.id).unwrap();
    assert_eq!(retrieved.name, "users");
    assert_eq!(retrieved.columns.len(), 2);
}

#[test]
fn test_import_odcl_with_metadata() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
medallion_layer: gold
scd_pattern: TYPE_2
odcl_metadata:
  description: "User table"
  owner: "data-team"
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.medallion_layers.len(), 1);
    assert_eq!(table.medallion_layers[0], MedallionLayer::Gold);
    assert_eq!(table.scd_pattern, Some(SCDPattern::Type2));

    if let Some(serde_json::Value::String(desc)) = table.odcl_metadata.get("description") {
        assert_eq!(desc, "User table");
    }

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_odcl_with_data_vault() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: hub_customer
columns:
  - name: customer_key
    data_type: VARCHAR(50)
data_vault_classification: Hub
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(
        table.data_vault_classification,
        Some(DataVaultClassification::Hub)
    );

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_odcl_invalid_yaml() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let invalid_yaml = "not: valid: yaml: structure:";

    let mut parser = ODCSParser::new();
    let result = parser.parse(invalid_yaml);

    // Should fail to parse YAML
    assert!(result.is_err());
}

#[test]
fn test_import_odcl_missing_required_fields() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let invalid_odcl = r#"
name: users
# Missing columns field
"#;

    let mut parser = ODCSParser::new();
    let result = parser.parse(invalid_odcl);

    // Should fail with missing columns
    assert!(result.is_err());
}

#[test]
fn test_import_odcl_with_naming_conflict() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
"#;

    // First import
    let mut parser = ODCSParser::new();
    let (table1, _) = parser.parse(odcl_content).unwrap();
    model_service.add_table(table1.clone()).unwrap();

    // Second import with same name
    let (table2, _) = parser.parse(odcl_content).unwrap();

    // Detect conflicts before adding
    let conflicts = model_service.detect_naming_conflicts(&[table2.clone()]);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].0.name, "users");
    assert_eq!(conflicts[0].1.name, "users");

    // Attempting to add should fail
    let result = model_service.add_table(table2);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_import_odcl_with_medallion_layers_plural() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
medallion_layers:
  - bronze
  - silver
  - gold
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.medallion_layers.len(), 3);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));
    assert!(table.medallion_layers.contains(&MedallionLayer::Silver));
    assert!(table.medallion_layers.contains(&MedallionLayer::Gold));

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_odcl_with_foreign_key() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: orders
columns:
  - name: id
    data_type: INT
    primary_key: true
  - name: user_id
    data_type: INT
    foreign_key:
      table_id: users
      column_name: id
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.columns.len(), 2);
    let user_id_col = table.columns.iter().find(|c| c.name == "user_id").unwrap();
    assert!(user_id_col.foreign_key.is_some());
    assert_eq!(user_id_col.foreign_key.as_ref().unwrap().table_id, "users");

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_data_contract_format() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let data_contract_content = r#"
dataContractSpecification: "1.2.1"
id: "test-contract-001"
info:
  title: "Test Data Contract"
  version: "1.0.0"
models:
  users:
    fields:
      id:
        type: "INTEGER"
        required: true
        description: "User identifier"
      name:
        type: "VARCHAR(255)"
        required: true
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(data_contract_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 2);

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_odcs_v3_format() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcs_v3_content = r#"
apiVersion: "v3.0.2"
kind: "DataContract"
id: "test-odcs-001"
version: "1.0.0"
status: "draft"
name: "users"
schema:
  - name: "users"
    properties:
      id:
        type: "INTEGER"
        required: true
        description: "User identifier"
      name:
        type: "VARCHAR(255)"
        required: true
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcs_v3_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 2);

    model_service.add_table(table).unwrap();
}

#[test]
fn test_import_odcl_scd_and_data_vault_mutually_exclusive() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
scd_pattern: TYPE_2
data_vault_classification: Hub
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    // Should have an error about mutual exclusivity
    assert!(errors.len() > 0);
    assert!(errors
        .iter()
        .any(|e| e.message.contains("mutually exclusive")));

    // Table should still be parseable, but with errors
    assert_eq!(table.name, "users");
}

#[test]
fn test_import_odcl_with_complex_data_types() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcl_content = r#"
name: complex_table
columns:
  - name: id
    data_type: INT
  - name: tags
    data_type: ARRAY<VARCHAR(50)>
  - name: metadata
    data_type: STRUCT<key VARCHAR(255), value VARCHAR(255)>
  - name: coordinates
    data_type: MAP<VARCHAR(50), DOUBLE>
"#;

    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcl_content).unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(table.columns.len(), 4);

    let tags_col = table.columns.iter().find(|c| c.name == "tags").unwrap();
    assert!(tags_col.data_type.starts_with("ARRAY"));

    let metadata_col = table.columns.iter().find(|c| c.name == "metadata").unwrap();
    assert!(metadata_col.data_type.starts_with("STRUCT"));

    model_service.add_table(table).unwrap();
}
