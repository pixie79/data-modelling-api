//! Integration tests for TBLPROPERTIES parsing from SQL CREATE TABLE statements.

use data_modelling_api::api::models::enums::MedallionLayer;
use data_modelling_api::api::services::SQLParser;
use tempfile::TempDir;

fn setup_test_model() -> TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn test_parse_tblproperties_quality_bronze() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
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
    assert_eq!(table.quality.len(), 1);
    assert_eq!(table.quality[0].get("property").and_then(|v| v.as_str()), Some("quality"));
    assert_eq!(table.quality[0].get("value").and_then(|v| v.as_str()), Some("bronze"));
}

#[test]
fn test_parse_tblproperties_quality_silver() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY
        )
        TBLPROPERTIES ('quality' = 'silver');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Silver));
}

#[test]
fn test_parse_tblproperties_quality_gold() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY
        )
        TBLPROPERTIES ('quality' = 'gold');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Gold));
}

#[test]
fn test_parse_tblproperties_multiple_properties() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY
        )
        TBLPROPERTIES (
            'quality' = 'bronze',
            'delta.appendOnly' = 'true',
            'data_quality' = 'high'
        );
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));

    // Should have multiple quality rules
    assert!(table.quality.len() >= 1);

    // Check that quality property is extracted
    let quality_rule = table.quality.iter()
        .find(|r| r.get("property").and_then(|v| v.as_str()) == Some("quality"));
    assert!(quality_rule.is_some());
}

#[test]
fn test_parse_tblproperties_with_comment() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE test_table (
            id INTEGER PRIMARY KEY
        )
        COMMENT "Test table"
        TBLPROPERTIES ('quality' = 'bronze');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));
}

#[test]
fn test_parse_tblproperties_with_identifier() {
    let _temp_dir = setup_test_model();
    let parser = SQLParser::new();

    let sql = r#"
        CREATE TABLE IF NOT EXISTS IDENTIFIER(:catalog || '.bronze.test_table') (
            id STRING,
            name STRING
        )
        COMMENT "Test table"
        TBLPROPERTIES ('quality' = 'bronze', 'delta.appendOnly' = 'true');
    "#;

    let (tables, _) = parser.parse(sql).unwrap();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.medallion_layers.len(), 1);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));
    assert!(table.quality.len() >= 1);
}
