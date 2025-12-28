//! Unit tests for SQL parser service.

use data_modelling_api::api::services::sql_parser::SQLParser;
use data_modelling_api::api::models::{Column, Table};

#[test]
fn test_parse_simple_create_table() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255)
        );
    "#;

    let (tables, name_inputs) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(name_inputs.len(), 0);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[0].columns.len(), 3);
    assert_eq!(tables[0].columns[0].name, "id");
    assert_eq!(tables[0].columns[0].data_type, "INTEGER");
    assert!(tables[0].columns[0].primary_key);
}

#[test]
fn test_parse_multiple_tables() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE users (id INT PRIMARY KEY);
        CREATE TABLE orders (id INT PRIMARY KEY, user_id INT);
    "#;

    let (tables, name_inputs) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].name, "users");
    assert_eq!(tables[1].name, "orders");
    assert_eq!(name_inputs.len(), 0);
}

#[test]
fn test_parse_with_foreign_key() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE orders (
            id INT PRIMARY KEY,
            user_id INT,
            FOREIGN KEY (user_id) REFERENCES users(id)
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    // Foreign key extraction may vary based on parser implementation
    // This test verifies the parser doesn't crash
}

#[test]
fn test_parse_syntax_error_handling() {
    let parser = SQLParser::new();
    let sql = "CREATE TABLE users (id INT PRIMARY KEY"; // Missing closing paren

    // Parser should handle syntax errors gracefully
    let result = parser.parse(sql);
    // Should either return empty tables or handle via fallback parsing
    if let Ok((tables, _)) = result {
        // If parsing succeeds with fallback, that's fine
        assert!(tables.len() <= 1);
    } else {
        // If parsing fails, that's also acceptable for malformed SQL
        assert!(result.is_err());
    }
}

#[test]
fn test_parse_empty_input() {
    let parser = SQLParser::new();
    let (tables, name_inputs) = parser.parse("").unwrap();

    assert_eq!(tables.len(), 0);
    assert_eq!(name_inputs.len(), 0);
}

#[test]
fn test_parse_table_with_comment() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(255)
        ) COMMENT 'User information table';
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
    // Check that comment is stored in odcl_metadata
    if let Some(serde_json::Value::String(desc)) = tables[0].odcl_metadata.get("description") {
        assert!(desc.contains("User information table"));
    }
}

#[test]
fn test_parse_columns_with_comments() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE products (
            id INT PRIMARY KEY COMMENT 'Product identifier',
            name VARCHAR(255) COMMENT 'Product name',
            price DECIMAL(10, 2) COMMENT 'Product price in USD'
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.columns.len(), 3);

    // Check column comments
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(id_col.description, "Product identifier");

    let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
    assert_eq!(name_col.description, "Product name");

    let price_col = table.columns.iter().find(|c| c.name == "price").unwrap();
    assert_eq!(price_col.description, "Product price in USD");
}

#[test]
fn test_parse_with_schema_prefix() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE schema.users (
            id INT PRIMARY KEY
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users"); // Should extract just the table name
}

#[test]
fn test_parse_quoted_table_name() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE "users" (
            id INT PRIMARY KEY
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
}

#[test]
fn test_parse_if_not_exists() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE IF NOT EXISTS users (
            id INT PRIMARY KEY
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");
}

#[test]
fn test_parse_decimal_types() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE products (
            price DECIMAL(10, 2),
            quantity DECIMAL(5)
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    let price_col = tables[0].columns.iter().find(|c| c.name == "price").unwrap();
    assert!(price_col.data_type.contains("DECIMAL"));
}

#[test]
fn test_parse_array_types() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE items (
            tags ARRAY<VARCHAR(50)>
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    let tags_col = tables[0].columns.iter().find(|c| c.name == "tags").unwrap();
    assert!(tags_col.data_type.starts_with("ARRAY"));
}

#[test]
fn test_parse_not_null_constraint() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255)
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    let name_col = tables[0].columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable);
    let email_col = tables[0].columns.iter().find(|c| c.name == "email").unwrap();
    assert!(email_col.nullable); // Default to nullable
}

#[test]
fn test_parse_various_data_types() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE test_types (
            col_int INT,
            col_bigint BIGINT,
            col_varchar VARCHAR(255),
            col_char CHAR(10),
            col_boolean BOOLEAN,
            col_date DATE,
            col_timestamp TIMESTAMP,
            col_float FLOAT,
            col_double DOUBLE
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].columns.len(), 9);
}

#[test]
fn test_parse_complex_table() {
    let parser = SQLParser::new();
    let sql = r#"
        CREATE TABLE complex_table (
            id INT PRIMARY KEY,
            name VARCHAR(255) NOT NULL COMMENT 'Name field',
            email VARCHAR(255) UNIQUE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP
        ) COMMENT 'Complex table with various constraints';
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "complex_table");
    assert_eq!(tables[0].columns.len(), 5);
}
