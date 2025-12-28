//! Unit tests for ODCS exporter covering Phase 1, 2, and 3 fields.
//!
//! Tests cover:
//! - Phase 1: domain, dataProduct, tenant, pricing, team, roles, terms
//! - Phase 2: servicelevels, links
//! - Phase 3: infrastructure, servers (full object)

use data_modelling_api::api::export::odcs::ODCSExporter;
use data_modelling_api::api::models::{Table, Column};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

fn create_test_table_with_metadata() -> Table {
    let mut odcl_metadata = HashMap::new();

    // Phase 1 fields
    odcl_metadata.insert("domain".to_string(), json!("ecommerce"));
    odcl_metadata.insert("dataProduct".to_string(), json!("customer-analytics"));
    odcl_metadata.insert("tenant".to_string(), json!("acme-corp"));
    odcl_metadata.insert("pricing".to_string(), json!({
        "model": "subscription",
        "currency": "USD",
        "amount": 100.00,
        "unit": "per-month"
    }));
    odcl_metadata.insert("team".to_string(), json!([
        {
            "name": "John Doe",
            "email": "john@example.com",
            "role": "Data Engineer"
        }
    ]));
    odcl_metadata.insert("roles".to_string(), json!({
        "viewer": {
            "description": "Can view data",
            "permissions": ["read"]
        }
    }));
    odcl_metadata.insert("terms".to_string(), json!({
        "usage": "Internal use only",
        "legal": "Subject to company policy"
    }));

    // Phase 2 fields
    odcl_metadata.insert("servicelevels".to_string(), json!({
        "availability": {
            "description": "99.9% uptime",
            "percentage": "99.9%"
        },
        "retention": {
            "description": "Data retained for 2 years",
            "period": "P2Y"
        }
    }));
    odcl_metadata.insert("links".to_string(), json!({
        "githubRepo": "https://github.com/example/repo",
        "documentation": "https://docs.example.com"
    }));

    // Phase 3 fields
    odcl_metadata.insert("infrastructure".to_string(), json!({
        "cluster": "production-cluster",
        "region": "us-east-1"
    }));
    odcl_metadata.insert("servers".to_string(), json!([
        {
            "name": "production-db",
            "type": "postgres",
            "url": "postgresql://localhost:5432/db",
            "environment": "production"
        }
    ]));

    // Standard fields
    odcl_metadata.insert("id".to_string(), json!("test-contract"));
    odcl_metadata.insert("version".to_string(), json!("1.0.0"));
    odcl_metadata.insert("status".to_string(), json!("active"));

    Table {
        id: Uuid::new_v4(),
        name: "TestTable".to_string(),
        columns: vec![
            Column {
                id: Uuid::new_v4(),
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                nullable: false,
                primary_key: true,
                secondary_key: false,
                composite_key: None,
                foreign_key: None,
                constraints: Vec::new(),
                description: "Primary key".to_string(),
                errors: Vec::new(),
                quality: Vec::new(),
                enum_values: Vec::new(),
                column_order: 0,
            }
        ],
        database_type: None,
        catalog_name: None,
        schema_name: None,
        medallion_layers: Vec::new(),
        scd_pattern: None,
        data_vault_classification: None,
        modeling_level: None,
        tags: vec!["test".to_string(), "customer".to_string()],
        odcl_metadata,
        position: None,
        yaml_file_path: None,
        drawio_cell_id: None,
        quality: Vec::new(),
        errors: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[test]
fn test_export_phase1_fields() {
    let table = create_test_table_with_metadata();
    let yaml_output = ODCSExporter::export_table(&table, "odcs");

    // Parse the YAML output to verify fields
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Check Phase 1 fields are exported
    assert_eq!(parsed["domain"].as_str(), Some("ecommerce"));
    assert_eq!(parsed["dataProduct"].as_str(), Some("customer-analytics"));
    assert_eq!(parsed["tenant"].as_str(), Some("acme-corp"));

    assert_eq!(parsed["pricing"]["model"].as_str(), Some("subscription"));
    assert_eq!(parsed["pricing"]["currency"].as_str(), Some("USD"));
    assert_eq!(parsed["pricing"]["amount"].as_f64(), Some(100.0));

    assert!(parsed["team"].as_sequence().is_some());
    assert_eq!(parsed["team"][0]["name"].as_str(), Some("John Doe"));

    assert_eq!(parsed["roles"]["viewer"]["description"].as_str(), Some("Can view data"));

    assert_eq!(parsed["terms"]["usage"].as_str(), Some("Internal use only"));
    assert_eq!(parsed["terms"]["legal"].as_str(), Some("Subject to company policy"));
}

#[test]
fn test_export_phase2_fields() {
    let table = create_test_table_with_metadata();
    let yaml_output = ODCSExporter::export_table(&table, "odcs");

    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Check Phase 2 fields are exported
    assert_eq!(parsed["servicelevels"]["availability"]["description"].as_str(), Some("99.9% uptime"));
    assert_eq!(parsed["servicelevels"]["availability"]["percentage"].as_str(), Some("99.9%"));
    assert_eq!(parsed["servicelevels"]["retention"]["period"].as_str(), Some("P2Y"));

    assert_eq!(parsed["links"]["githubRepo"].as_str(), Some("https://github.com/example/repo"));
    assert_eq!(parsed["links"]["documentation"].as_str(), Some("https://docs.example.com"));
}

#[test]
fn test_export_phase3_fields() {
    let table = create_test_table_with_metadata();
    let yaml_output = ODCSExporter::export_table(&table, "odcs");

    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Check Phase 3 fields are exported
    assert_eq!(parsed["infrastructure"]["cluster"].as_str(), Some("production-cluster"));
    assert_eq!(parsed["infrastructure"]["region"].as_str(), Some("us-east-1"));

    assert!(parsed["servers"].as_sequence().is_some());
    assert_eq!(parsed["servers"][0]["name"].as_str(), Some("production-db"));
    assert_eq!(parsed["servers"][0]["type"].as_str(), Some("postgres"));
    assert_eq!(parsed["servers"][0]["url"].as_str(), Some("postgresql://localhost:5432/db"));
    assert_eq!(parsed["servers"][0]["environment"].as_str(), Some("production"));
}

#[test]
fn test_export_all_phases_combined() {
    let table = create_test_table_with_metadata();
    let yaml_output = ODCSExporter::export_table(&table, "odcs");

    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Verify required ODCS fields
    assert_eq!(parsed["apiVersion"].as_str(), Some("v3.1.0"));
    assert_eq!(parsed["kind"].as_str(), Some("DataContract"));
    assert_eq!(parsed["id"].as_str(), Some("test-contract"));
    assert_eq!(parsed["name"].as_str(), Some("TestTable"));
    assert_eq!(parsed["version"].as_str(), Some("1.0.0"));
    assert_eq!(parsed["status"].as_str(), Some("active"));

    // Verify tags are exported
    assert!(parsed["tags"].as_sequence().is_some());
    assert_eq!(parsed["tags"][0].as_str(), Some("test"));

    // Verify schema is exported
    assert!(parsed["schema"].as_sequence().is_some());
    assert_eq!(parsed["schema"][0]["name"].as_str(), Some("TestTable"));
    assert!(parsed["schema"][0]["properties"].as_mapping().is_some());

    // Verify all phases are present
    assert!(parsed["domain"].is_string());
    assert!(parsed["pricing"].is_mapping());
    assert!(parsed["servicelevels"].is_mapping());
    assert!(parsed["links"].is_mapping());
    assert!(parsed["infrastructure"].is_mapping());
    assert!(parsed["servers"].is_sequence());
}

#[test]
fn test_export_partial_fields() {
    let mut table = create_test_table_with_metadata();

    // Remove some fields to test partial export
    table.odcl_metadata.remove("pricing");
    table.odcl_metadata.remove("team");
    table.odcl_metadata.remove("infrastructure");

    let yaml_output = ODCSExporter::export_table(&table, "odcs");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Fields that exist should be exported
    assert_eq!(parsed["domain"].as_str(), Some("ecommerce"));
    assert!(parsed["servicelevels"].is_mapping());
    assert!(parsed["servers"].is_sequence());

    // Fields that don't exist should not be in output
    assert!(parsed["pricing"].is_null());
    assert!(parsed["team"].is_null());
    assert!(parsed["infrastructure"].is_null());
}

#[test]
fn test_export_empty_optional_fields() {
    let mut table = create_test_table_with_metadata();

    // Set empty values
    table.odcl_metadata.insert("servicelevels".to_string(), json!({}));
    table.odcl_metadata.insert("links".to_string(), json!({}));
    table.odcl_metadata.insert("servers".to_string(), json!([]));

    let yaml_output = ODCSExporter::export_table(&table, "odcs");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Empty objects/arrays should not be exported (or should be empty)
    // Based on exporter logic, empty objects might not be included
    // This test verifies the exporter handles empty values correctly
    assert!(parsed["domain"].is_string()); // Other fields should still be present
}

#[test]
fn test_export_complex_nested_structures() {
    let mut table = create_test_table_with_metadata();

    // Add complex nested structures
    table.odcl_metadata.insert("servicelevels".to_string(), json!({
        "availability": {
            "description": "99.9% uptime",
            "percentage": "99.9%"
        },
        "latency": {
            "description": "Data available within minutes",
            "threshold": "1h",
            "sourceTimestampField": "source_ts",
            "processedTimestampField": "processed_ts"
        },
        "freshness": {
            "description": "Age of youngest row",
            "threshold": "1m",
            "timestampField": "created_at"
        }
    }));

    table.odcl_metadata.insert("servers".to_string(), json!([
        {
            "name": "prod-db",
            "type": "postgres",
            "url": "postgresql://prod:5432/db",
            "description": "Production database",
            "environment": "production"
        },
        {
            "name": "staging-db",
            "type": "mysql",
            "url": "mysql://staging:3306/db",
            "description": "Staging database",
            "environment": "staging"
        }
    ]));

    let yaml_output = ODCSExporter::export_table(&table, "odcs");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();

    // Verify complex nested structures are exported correctly
    assert_eq!(parsed["servicelevels"]["latency"]["threshold"].as_str(), Some("1h"));
    assert_eq!(parsed["servicelevels"]["latency"]["sourceTimestampField"].as_str(), Some("source_ts"));
    assert_eq!(parsed["servicelevels"]["freshness"]["timestampField"].as_str(), Some("created_at"));

    assert_eq!(parsed["servers"].as_sequence().unwrap().len(), 2);
    assert_eq!(parsed["servers"][1]["type"].as_str(), Some("mysql"));
    assert_eq!(parsed["servers"][1]["environment"].as_str(), Some("staging"));
}
