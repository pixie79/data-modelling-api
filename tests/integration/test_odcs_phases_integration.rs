//! Integration tests for ODCS Phase 1, 2, and 3 fields.
//!
//! Tests cover end-to-end import/export workflows for all phase fields.

use data_modelling_api::api::services::{ODCSParser, ModelService};
use data_modelling_api::api::export::odcs::ODCSExporter;
use tempfile::TempDir;
use serde_yaml;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for phase integration tests".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_import_export_phase1_fields_roundtrip() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: phase1-contract
name: Phase 1 Contract
version: 1.0.0
status: active
domain: ecommerce
dataProduct: customer-analytics
tenant: acme-corp
pricing:
  model: subscription
  currency: USD
  amount: 100.00
team:
  - name: John Doe
    email: john@example.com
roles:
  viewer:
    description: Can view data
    permissions: [read]
terms:
  usage: Internal use only
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
        required: true
"#;

    // Import
    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Verify Phase 1 fields are parsed
    assert_eq!(table.odcl_metadata.get("domain").and_then(|v| v.as_str()), Some("ecommerce"));
    assert_eq!(table.odcl_metadata.get("dataProduct").and_then(|v| v.as_str()), Some("customer-analytics"));
    assert_eq!(table.odcl_metadata.get("tenant").and_then(|v| v.as_str()), Some("acme-corp"));
    assert!(table.odcl_metadata.get("pricing").is_some());
    assert!(table.odcl_metadata.get("team").is_some());
    assert!(table.odcl_metadata.get("roles").is_some());
    assert!(table.odcl_metadata.get("terms").is_some());

    // Add to model
    let added_table = model_service.add_table(table).unwrap();

    // Export
    let exported_yaml = ODCSExporter::export_table(&added_table, "odcs");
    let exported: serde_yaml::Value = serde_yaml::from_str(&exported_yaml).unwrap();

    // Verify Phase 1 fields are exported
    assert_eq!(exported["domain"].as_str(), Some("ecommerce"));
    assert_eq!(exported["dataProduct"].as_str(), Some("customer-analytics"));
    assert_eq!(exported["tenant"].as_str(), Some("acme-corp"));
    assert_eq!(exported["pricing"]["model"].as_str(), Some("subscription"));
    assert!(exported["team"].as_sequence().is_some());
    assert!(exported["roles"].as_mapping().is_some());
    assert!(exported["terms"].as_mapping().is_some());
}

#[test]
fn test_import_export_phase2_fields_roundtrip() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: phase2-contract
name: Phase 2 Contract
version: 1.0.0
servicelevels:
  availability:
    description: 99.9% uptime
    percentage: "99.9%"
  retention:
    description: Data retained for 2 years
    period: P2Y
links:
  githubRepo: https://github.com/example/repo
  documentation: https://docs.example.com
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    // Import
    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Verify Phase 2 fields are parsed
    assert!(table.odcl_metadata.get("servicelevels").is_some());
    assert!(table.odcl_metadata.get("links").is_some());

    // Add to model
    let added_table = model_service.add_table(table).unwrap();

    // Export
    let exported_yaml = ODCSExporter::export_table(&added_table, "odcs");
    let exported: serde_yaml::Value = serde_yaml::from_str(&exported_yaml).unwrap();

    // Verify Phase 2 fields are exported
    assert_eq!(exported["servicelevels"]["availability"]["description"].as_str(), Some("99.9% uptime"));
    assert_eq!(exported["links"]["githubRepo"].as_str(), Some("https://github.com/example/repo"));
}

#[test]
fn test_import_export_phase3_fields_roundtrip() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: phase3-contract
name: Phase 3 Contract
version: 1.0.0
infrastructure:
  cluster: production-cluster
  region: us-east-1
servers:
  - name: prod-db
    type: postgres
    url: postgresql://localhost:5432/db
    environment: production
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    // Import
    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Verify Phase 3 fields are parsed
    assert!(table.odcl_metadata.get("infrastructure").is_some());
    assert!(table.odcl_metadata.get("servers").is_some());

    // Add to model
    let added_table = model_service.add_table(table).unwrap();

    // Export
    let exported_yaml = ODCSExporter::export_table(&added_table, "odcs");
    let exported: serde_yaml::Value = serde_yaml::from_str(&exported_yaml).unwrap();

    // Verify Phase 3 fields are exported
    assert_eq!(exported["infrastructure"]["cluster"].as_str(), Some("production-cluster"));
    assert_eq!(exported["servers"][0]["name"].as_str(), Some("prod-db"));
    assert_eq!(exported["servers"][0]["type"].as_str(), Some("postgres"));
}

#[test]
fn test_import_export_all_phases_roundtrip() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: comprehensive-contract
name: Comprehensive Contract
version: 2.0.0
status: active
domain: ecommerce
dataProduct: customer-analytics
tenant: acme-corp
pricing:
  model: subscription
  currency: USD
team:
  - name: John Doe
    email: john@example.com
roles:
  viewer:
    description: Can view data
terms:
  usage: Internal use only
servicelevels:
  availability:
    description: 99.9% uptime
    percentage: "99.9%"
links:
  githubRepo: https://github.com/example/repo
infrastructure:
  cluster: production-cluster
servers:
  - name: prod-db
    type: postgres
    url: postgresql://localhost:5432/db
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
        required: true
"#;

    // Import
    let mut parser = ODCSParser::new();
    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Verify all phases are parsed
    assert!(table.odcl_metadata.get("domain").is_some());
    assert!(table.odcl_metadata.get("pricing").is_some());
    assert!(table.odcl_metadata.get("servicelevels").is_some());
    assert!(table.odcl_metadata.get("links").is_some());
    assert!(table.odcl_metadata.get("infrastructure").is_some());
    assert!(table.odcl_metadata.get("servers").is_some());

    // Add to model
    let added_table = model_service.add_table(table).unwrap();

    // Export
    let exported_yaml = ODCSExporter::export_table(&added_table, "odcs");
    let exported: serde_yaml::Value = serde_yaml::from_str(&exported_yaml).unwrap();

    // Verify all phases are exported
    assert_eq!(exported["apiVersion"].as_str(), Some("v3.1.0"));
    assert_eq!(exported["kind"].as_str(), Some("DataContract"));
    assert_eq!(exported["domain"].as_str(), Some("ecommerce"));
    assert!(exported["pricing"].as_mapping().is_some());
    assert!(exported["servicelevels"].as_mapping().is_some());
    assert!(exported["links"].as_mapping().is_some());
    assert!(exported["infrastructure"].as_mapping().is_some());
    assert!(exported["servers"].as_sequence().is_some());

    // Verify schema is exported correctly
    assert!(exported["schema"].as_sequence().is_some());
    assert_eq!(exported["schema"][0]["name"].as_str(), Some("Customer"));
    assert_eq!(exported["schema"][0]["properties"]["id"]["type"].as_str(), Some("INTEGER"));
}

#[test]
fn test_update_table_with_phase_fields() {
    let (mut model_service, _temp_dir) = setup_test_model();

    // Create initial table
    let initial_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: update-test
name: Update Test
version: 1.0.0
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let mut parser = ODCSParser::new();
    let (table, _) = parser.parse(initial_yaml).unwrap();
    let added_table = model_service.add_table(table).unwrap();

    // Update table with phase fields
    let mut updated_table = model_service.get_table(added_table.id).unwrap();
    updated_table.odcl_metadata.insert("domain".to_string(), serde_json::json!("ecommerce"));
    updated_table.odcl_metadata.insert("servicelevels".to_string(), serde_json::json!({
        "availability": {
            "description": "99.9% uptime"
        }
    }));
    updated_table.odcl_metadata.insert("infrastructure".to_string(), serde_json::json!({
        "cluster": "production-cluster"
    }));

    let updated = model_service.update_table(updated_table.id, updated_table).unwrap();

    // Verify updates are persisted
    assert_eq!(updated.odcl_metadata.get("domain").and_then(|v| v.as_str()), Some("ecommerce"));
    assert!(updated.odcl_metadata.get("servicelevels").is_some());
    assert!(updated.odcl_metadata.get("infrastructure").is_some());

    // Export and verify
    let exported_yaml = ODCSExporter::export_table(&updated, "odcs");
    let exported: serde_yaml::Value = serde_yaml::from_str(&exported_yaml).unwrap();

    assert_eq!(exported["domain"].as_str(), Some("ecommerce"));
    assert!(exported["servicelevels"].as_mapping().is_some());
    assert!(exported["infrastructure"].as_mapping().is_some());
}
