//! Integration tests for OBJECTS/ARRAYS nested field extraction and entity data finding.

use data_modelling_api::api::models::{Column, DataModel, Table};
use data_modelling_api::api::services::{SQLParser, ModelService};
use tempfile::TempDir;
use uuid::Uuid;

fn setup_test_model() -> (ModelService, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut model_service = ModelService::new();

    model_service.create_model(
        "Test Model".to_string(),
        temp_dir.path().to_path_buf(),
        Some("Test model for OBJECTS/ARRAYS".to_string()),
    ).unwrap();

    (model_service, temp_dir)
}

#[test]
fn test_parse_nested_struct_fields() {
    let (_model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY,
            customer STRUCT<
                name: STRING,
                email: STRING,
                address: STRUCT<
                    street: STRING,
                    city: STRING,
                    zip: STRING
                >
            > COMMENT 'Customer information',
            items ARRAY<STRUCT<
                item_id: STRING,
                item_name: STRING,
                price: DECIMAL(10,2)
            >> COMMENT 'Array of items'
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];

    // Should have parent columns
    let customer_col = table.columns.iter().find(|c| c.name == "customer");
    assert!(customer_col.is_some());
    assert!(customer_col.unwrap().data_type.contains("STRUCT"));

    let items_col = table.columns.iter().find(|c| c.name == "items");
    assert!(items_col.is_some());
    assert!(items_col.unwrap().data_type.contains("ARRAY"));

    // Should have nested columns with dot notation
    let customer_name = table.columns.iter().find(|c| c.name == "customer.name");
    assert!(customer_name.is_some(), "Should have customer.name nested column");

    let customer_email = table.columns.iter().find(|c| c.name == "customer.email");
    assert!(customer_email.is_some(), "Should have customer.email nested column");

    let customer_address_street = table.columns.iter().find(|c| c.name == "customer.address.street");
    assert!(customer_address_street.is_some(), "Should have customer.address.street nested column");

    let items_item_id = table.columns.iter().find(|c| c.name == "items.item_id");
    assert!(items_item_id.is_some(), "Should have items.item_id nested column");
}

#[test]
fn test_parse_array_of_objects() {
    let (_model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY,
            tags ARRAY<STRING> COMMENT 'Array of tags',
            metadata ARRAY<STRUCT<
                key: STRING,
                value: STRING
            >> COMMENT 'Array of metadata objects'
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];

    // Should have parent columns
    let tags_col = table.columns.iter().find(|c| c.name == "tags");
    assert!(tags_col.is_some());
    assert!(tags_col.unwrap().data_type.contains("ARRAY"));

    let metadata_col = table.columns.iter().find(|c| c.name == "metadata");
    assert!(metadata_col.is_some());
    assert!(metadata_col.unwrap().data_type.contains("ARRAY"));

    // Should have nested columns for ARRAY<STRUCT>
    let metadata_key = table.columns.iter().find(|c| c.name == "metadata.key");
    assert!(metadata_key.is_some(), "Should have metadata.key nested column");

    let metadata_value = table.columns.iter().find(|c| c.name == "metadata.value");
    assert!(metadata_value.is_some(), "Should have metadata.value nested column");
}

#[test]
fn test_find_nested_columns_by_prefix() {
    let (mut model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY,
            customer STRUCT<
                name: STRING,
                email: STRING,
                address: STRUCT<
                    street: STRING,
                    city: STRING
                >
            >
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    let table = tables[0].clone();
    model_service.add_table(table.clone()).unwrap();

    // Test finding nested columns by prefix
    let customer_prefix = "customer.";
    let nested_customer_cols: Vec<&Column> = table.columns
        .iter()
        .filter(|c| c.name.starts_with(customer_prefix))
        .collect();

    assert!(nested_customer_cols.len() >= 2, "Should find customer.name and customer.email");

    // Test finding deeply nested columns
    let address_prefix = "customer.address.";
    let nested_address_cols: Vec<&Column> = table.columns
        .iter()
        .filter(|c| c.name.starts_with(address_prefix))
        .collect();

    assert!(nested_address_cols.len() >= 2, "Should find customer.address.street and customer.address.city");
}

#[test]
fn test_parse_complex_nested_structure() {
    let (_model_service, _temp_dir) = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY,
            order_info STRUCT<
                order_id: STRING,
                customer: STRUCT<
                    customer_id: STRING,
                    name: STRING,
                    contact: STRUCT<
                        email: STRING,
                        phone: STRING
                    >
                >,
                items: ARRAY<STRUCT<
                    item_id: STRING,
                    quantity: INTEGER,
                    price: DECIMAL(10,2)
                >>
            >
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];

    // Verify all nested levels are extracted
    assert!(table.columns.iter().any(|c| c.name == "order_info.order_id"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.customer.customer_id"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.customer.name"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.customer.contact.email"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.customer.contact.phone"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.items.item_id"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.items.quantity"));
    assert!(table.columns.iter().any(|c| c.name == "order_info.items.price"));
}
