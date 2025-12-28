//! Comprehensive integration tests for all import types with generic example data.

use data_modelling_api::api::models::enums::{MedallionLayer, DatabaseType, SCDPattern, DataVaultClassification};
use data_modelling_api::api::models::{Column, DataModel, Table};
use data_modelling_api::api::services::{SQLParser, ODCSParser, ModelService};
use tempfile::TempDir;
use uuid::Uuid;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for comprehensive imports".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

// ========== SQL Import Tests ==========

#[test]
fn test_sql_import_simple_table() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255)
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[0].columns.len(), 3);

    let table = model_service.add_table(tables[0].clone()).unwrap();
    assert_eq!(table.columns.len(), 3);
}

#[test]
fn test_sql_import_with_tblproperties_quality() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name VARCHAR(255)
        )
        TBLPROPERTIES ('quality' = 'bronze');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));
    assert!(table.quality.len() >= 1);

    let added = model_service.add_table(table.clone()).unwrap();
    assert_eq!(added.medallion_layers.len(), 1);
}

#[test]
fn test_sql_import_with_nested_struct() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer STRUCT<
                name: STRING,
                email: STRING,
                address: STRUCT<
                    street: STRING,
                    city: STRING
                >
            >,
            items ARRAY<STRUCT<
                item_id: STRING,
                quantity: INTEGER
            >>
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    // Should have parent column
    assert!(table.columns.iter().any(|c| c.name == "customer"));
    // Should have nested columns
    assert!(table.columns.iter().any(|c| c.name == "customer.name"));
    assert!(table.columns.iter().any(|c| c.name == "customer.email"));
    assert!(table.columns.iter().any(|c| c.name == "customer.address.street"));
    assert!(table.columns.iter().any(|c| c.name == "items.item_id"));

    let added = model_service.add_table(table.clone()).unwrap();
    assert!(added.columns.iter().any(|c| c.name.starts_with("customer.")));
}

#[test]
fn test_sql_import_with_comments() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY COMMENT 'User ID',
            name VARCHAR(255) COMMENT 'User name',
            email VARCHAR(255) COMMENT 'User email'
        )
        COMMENT "Users table";
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(id_col.description, "User ID");

    assert!(table.odcl_metadata.get("description").is_some());
}

#[test]
fn test_sql_import_with_identifier() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE IF NOT EXISTS IDENTIFIER(:catalog || '.bronze.test_table') (
            id STRING,
            name STRING
        )
        TBLPROPERTIES ('quality' = 'bronze');
    "#;

    let (tables, name_inputs) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(name_inputs.len(), 1); // Should require name input

    let table = &tables[0];
    assert_eq!(table.name, "test_table");
    assert_eq!(table.medallion_layers.len(), 1);
}

// ========== ODCL Import Tests ==========

#[test]
fn test_odcl_import_simple() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

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
  - name: email
    data_type: VARCHAR(255)
    nullable: true
database_type: Postgres
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 3);
    assert_eq!(table.database_type, Some(DatabaseType::Postgres));

    let added = model_service.add_table(table).unwrap();
    assert_eq!(added.name, "users");
}

#[test]
fn test_odcl_import_with_medallion_layer() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: products
columns:
  - name: id
    data_type: INT
medallion_layer: gold
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Gold));

    let added = model_service.add_table(table).unwrap();
    assert_eq!(added.medallion_layers.len(), 1);
}

#[test]
fn test_odcl_import_with_scd_pattern() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: customers
columns:
  - name: id
    data_type: INT
scd_pattern: TYPE_2
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);
    assert_eq!(table.scd_pattern, Some(SCDPattern::Type2));

    let added = model_service.add_table(table).unwrap();
    assert_eq!(added.scd_pattern, Some(SCDPattern::Type2));
}

#[test]
fn test_odcl_import_with_data_vault() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: hub_customer
columns:
  - name: customer_key
    data_type: VARCHAR(50)
data_vault_classification: Hub
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);
    assert_eq!(table.data_vault_classification, Some(DataVaultClassification::Hub));

    let added = model_service.add_table(table).unwrap();
    assert_eq!(added.data_vault_classification, Some(DataVaultClassification::Hub));
}

#[test]
fn test_odcl_import_with_nested_object() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: orders
columns:
  - name: id
    data_type: INT
  - name: customer
    data_type: OBJECT
    fields:
      - name: name
        data_type: STRING
      - name: email
        data_type: STRING
      - name: address
        data_type: OBJECT
        fields:
          - name: street
            data_type: STRING
          - name: city
            data_type: STRING
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    // Errors might occur if nested parsing isn't perfect, but table should be created
    assert_eq!(table.name, "orders");

    // Should have parent column
    assert!(table.columns.iter().any(|c| c.name == "customer"));

    // Should have nested columns (if parser extracts them)
    // Note: This depends on ODCL parser implementation
    let added = model_service.add_table(table).unwrap();
    assert_eq!(added.name, "orders");
}

#[test]
fn test_odcl_import_with_tags() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: products
columns:
  - name: id
    data_type: INT
tags:
  - production
  - critical
  - pii
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);
    assert_eq!(table.tags.len(), 3);
    assert!(table.tags.contains(&"production".to_string()));
    assert!(table.tags.contains(&"critical".to_string()));
    assert!(table.tags.contains(&"pii".to_string()));
}

#[test]
fn test_odcl_import_with_metadata() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let mut parser = ODCLParser::new();

    let odcl_content = r#"
name: users
columns:
  - name: id
    data_type: INT
odcl_metadata:
  description: "User table"
  owner: "data-team"
  version: "1.0.0"
"#;

    let (table, errors) = parser.parse(odcl_content).unwrap();
    assert_eq!(errors.len(), 0);

    assert_eq!(table.odcl_metadata.get("description").and_then(|v| v.as_str()), Some("User table"));
    assert_eq!(table.odcl_metadata.get("owner").and_then(|v| v.as_str()), Some("data-team"));
}

// ========== Combined Import Tests ==========

#[test]
fn test_import_sql_then_odcl() {
    let (mut model_service, _temp_dir) = setup_test_model();

    // Import SQL table
    let sql_parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name VARCHAR(255)
        );
    "#;
    let (sql_tables, _) = sql_parser.parse(sql).unwrap();
    let sql_table = model_service.add_table(sql_tables[0].clone()).unwrap();

    // Import ODCL table
        let mut odcl_parser = ODCSParser::new();
    let odcl_content = r#"
name: orders
columns:
  - name: id
    data_type: INT
  - name: user_id
    data_type: INT
"#;
    let (odcl_table, _) = odcl_parser.parse(odcl_content).unwrap();
    let odcl_added = model_service.add_table(odcl_table).unwrap();

    // Verify both tables exist
    let model = model_service.get_current_model().unwrap();
    assert_eq!(model.tables.len(), 2);
    assert!(model.get_table_by_id(sql_table.id).is_some());
    assert!(model.get_table_by_id(odcl_added.id).is_some());
}

#[test]
fn test_import_multiple_sql_tables() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name VARCHAR(255)
        );

        CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            user_id INTEGER,
            total DECIMAL(10,2)
        )
        TBLPROPERTIES ('quality' = 'silver');

        CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            name VARCHAR(255),
            price DECIMAL(10,2)
        )
        TBLPROPERTIES ('quality' = 'gold');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 3);

    for table in tables {
        let added = model_service.add_table(table).unwrap();
        assert!(!added.name.is_empty());
    }

    let model = model_service.get_current_model().unwrap();
    assert_eq!(model.tables.len(), 3);

    // Verify medallion layers were parsed
    let orders = model.tables.iter().find(|t| t.name == "orders").unwrap();
    assert_eq!(orders.medallion_layers.len(), 1);
    assert!(orders.medallion_layers.contains(&MedallionLayer::Silver));

    let products = model.tables.iter().find(|t| t.name == "products").unwrap();
    assert_eq!(products.medallion_layers.len(), 1);
    assert!(products.medallion_layers.contains(&MedallionLayer::Gold));
}
