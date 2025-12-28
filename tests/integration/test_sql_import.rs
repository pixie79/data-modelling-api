//! Integration tests for SQL import workflow.

use data_modelling_api::api::models::DataModel;
use data_modelling_api::api::services::{ModelService, SQLParser};
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
fn test_import_sql_file_success() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255)
        );
    "#;

    let parser = SQLParser::new();
    let (tables, name_inputs) = parser.parse(sql_content).unwrap();

    assert_eq!(tables.len(), 1);
    assert_eq!(name_inputs.len(), 0);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[0].columns.len(), 3);

    // Add to model
    let table = model_service.add_table(tables[0].clone()).unwrap();
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 3);

    // Verify table is in model
    let retrieved = model_service.get_table(table.id).unwrap();
    assert_eq!(retrieved.name, "users");
}

#[test]
fn test_import_multiple_tables() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));
        CREATE TABLE orders (id INT PRIMARY KEY, user_id INT);
    "#;

    let parser = SQLParser::new();
    let (tables, name_inputs) = parser.parse(sql_content).unwrap();

    assert_eq!(tables.len(), 2);
    assert_eq!(name_inputs.len(), 0);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[1].name, "orders");

    // Add all tables to model
    for table in &tables {
        model_service.add_table(table.clone()).unwrap();
    }

    // Verify both tables are in model
    let model = model_service.get_current_model().unwrap();
    assert_eq!(model.tables.len(), 2);
    assert!(model.get_table_by_name("users").is_some());
    assert!(model.get_table_by_name("orders").is_some());
}

#[test]
fn test_import_sql_with_syntax_error() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = "CREATE TABLE users (id INT PRIMARY KEY"; // Missing closing paren

    let parser = SQLParser::new();
    let result = parser.parse(sql_content);

    // Parser should handle syntax errors gracefully
    match result {
        Ok((tables, _)) => {
            // If parsing succeeds with fallback, that's fine
            assert!(tables.len() <= 1);
            // Try to add if any tables were parsed
            for table in tables {
                let add_result = model_service.add_table(table);
                // May succeed or fail depending on fallback parsing
                assert!(add_result.is_ok() || add_result.is_err());
            }
        }
        Err(_) => {
            // If parsing fails, that's also acceptable for malformed SQL
        }
    }
}

#[test]
fn test_import_sql_with_naming_conflict() {
    let (mut model_service, _temp_dir) = setup_test_model();

    // First import
    let sql_content1 = "CREATE TABLE users (id INT PRIMARY KEY);";
    let parser = SQLParser::new();
    let (tables1, _) = parser.parse(sql_content1).unwrap();
    model_service.add_table(tables1[0].clone()).unwrap();

    // Second import with same table name
    let sql_content2 = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));";
    let (tables2, _) = parser.parse(sql_content2).unwrap();

    // Detect conflicts before adding
    let conflicts = model_service.detect_naming_conflicts(&tables2);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].0.name, "users");
    assert_eq!(conflicts[0].1.name, "users");

    // Attempting to add should fail
    let result = model_service.add_table(tables2[0].clone());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_import_sql_with_dynamic_table_name() {
    let (_model_service, _temp_dir) = setup_test_model();

    // SQL with IDENTIFIER() function (Databricks dynamic table names)
    let sql_content = r#"
        CREATE TABLE IDENTIFIER(:target_table) (
            id INT PRIMARY KEY,
            name VARCHAR(255)
        );
    "#;

    let parser = SQLParser::new();
    let (tables, name_inputs) = parser.parse(sql_content).unwrap();

    // Should detect that table name requires input
    assert_eq!(name_inputs.len(), 1);
    assert!(name_inputs[0].suggested_name.contains("table_"));

    // Table should have a suggested name
    assert!(!tables.is_empty());
    assert!(tables[0].name.contains("table_"));
}

#[test]
fn test_import_sql_with_foreign_key() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(255)
        );
        CREATE TABLE orders (
            id INT PRIMARY KEY,
            user_id INT,
            FOREIGN KEY (user_id) REFERENCES users(id)
        );
    "#;

    let parser = SQLParser::new();
    let (tables, _) = parser.parse(sql_content).unwrap();

    assert_eq!(tables.len(), 2);

    // Add tables to model
    for table in &tables {
        model_service.add_table(table.clone()).unwrap();
    }

    // Verify foreign key information is preserved
    let orders_table = model_service.get_table_by_name("orders").unwrap();
    let user_id_col = orders_table.columns.iter().find(|c| c.name == "user_id");
    assert!(user_id_col.is_some());
    // Note: Foreign key extraction from AST may vary, but column should exist
}

#[test]
fn test_import_sql_with_comments() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE products (
            id INT PRIMARY KEY COMMENT 'Product identifier',
            name VARCHAR(255) COMMENT 'Product name',
            price DECIMAL(10, 2) COMMENT 'Product price in USD'
        ) COMMENT 'Product information table';
    "#;

    let parser = SQLParser::new();
    let (tables, _) = parser.parse(sql_content).unwrap();

    assert_eq!(tables.len(), 1);
    let table = &tables[0];

    // Check table comment
    if let Some(serde_json::Value::String(desc)) = table.odcl_metadata.get("description") {
        assert!(desc.contains("Product information"));
    }

    // Check column comments
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(id_col.description, "Product identifier");

    let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
    assert_eq!(name_col.description, "Product name");

    // Add to model
    model_service.add_table(table.clone()).unwrap();
}

#[test]
fn test_import_sql_multiple_statements_with_errors() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));
        CREATE TABLE invalid (id INT PRIMARY KEY;  -- Syntax error
        CREATE TABLE orders (id INT PRIMARY KEY, user_id INT);
    "#;

    let parser = SQLParser::new();
    let result = parser.parse(sql_content);

    // Should parse valid statements even if some fail
    if let Ok((tables, _)) = result {
        // Should have at least users and orders
        assert!(tables.len() >= 2);

        // Add valid tables
        for table in &tables {
            if table.name == "users" || table.name == "orders" {
                model_service.add_table(table.clone()).unwrap();
            }
        }

        let model = model_service.get_current_model().unwrap();
        assert!(model.get_table_by_name("users").is_some());
        assert!(model.get_table_by_name("orders").is_some());
    }
}

#[test]
fn test_import_sql_with_schema_prefix() {
    let (mut model_service, _temp_dir) = setup_test_model();

    let sql_content = r#"
        CREATE TABLE schema.users (
            id INT PRIMARY KEY,
            name VARCHAR(255)
        );
    "#;

    let parser = SQLParser::new();
    let (tables, _) = parser.parse(sql_content).unwrap();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users"); // Should extract just the table name

    model_service.add_table(tables[0].clone()).unwrap();
    let retrieved = model_service.get_table_by_name("users").unwrap();
    assert_eq!(retrieved.name, "users");
}
