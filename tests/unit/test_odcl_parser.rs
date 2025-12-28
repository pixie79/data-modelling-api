//! Unit tests for ODCL parser service.

use data_modelling_api::api::services::odcs_parser::ODCSParser;
use data_modelling_api::api::models::enums::{
    DatabaseType, MedallionLayer, SCDPattern, DataVaultClassification,
};

#[test]
fn test_parse_simple_odcl_table() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
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

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.name, "users");
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.columns[0].name, "id");
    assert_eq!(table.database_type, Some(DatabaseType::Postgres));
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_metadata() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
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

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.medallion_layers.len(), 1);
    assert_eq!(table.medallion_layers[0], MedallionLayer::Gold);
    assert_eq!(table.scd_pattern, Some(SCDPattern::Type2));
    if let Some(serde_json::Value::String(desc)) = table.odcl_metadata.get("description") {
        assert_eq!(desc, "User table");
    }
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_data_vault() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: hub_customer
columns:
  - name: customer_key
    data_type: VARCHAR(50)
data_vault_classification: Hub
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.data_vault_classification, Some(DataVaultClassification::Hub));
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_invalid_odcl() {
    let mut parser = ODCLParser::new();
    let invalid_yaml = "not: valid: yaml: structure:";

    // Should fail to parse YAML
    assert!(parser.parse(invalid_yaml).is_err());
}

#[test]
fn test_parse_odcl_missing_required_fields() {
    let mut parser = ODCLParser::new();
    let non_conformant = r#"
name: users
# Missing required columns field
"#;

    // Should fail with missing columns
    assert!(parser.parse(non_conformant).is_err());
}

#[test]
fn test_parse_odcl_with_foreign_key() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
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

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.columns.len(), 2);
    let user_id_col = table.columns.iter().find(|c| c.name == "user_id").unwrap();
    assert!(user_id_col.foreign_key.is_some());
    assert_eq!(user_id_col.foreign_key.as_ref().unwrap().table_id, "users");
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_constraints() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: products
columns:
  - name: id
    data_type: INT
    primary_key: true
  - name: name
    data_type: VARCHAR(255)
    nullable: false
    constraints:
      - UNIQUE
      - NOT NULL
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.columns.len(), 2);
    let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable);
    assert!(name_col.constraints.contains(&"UNIQUE".to_string()));
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_medallion_layers_plural() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
medallion_layers:
  - bronze
  - silver
  - gold
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.medallion_layers.len(), 3);
    assert!(table.medallion_layers.contains(&MedallionLayer::Bronze));
    assert!(table.medallion_layers.contains(&MedallionLayer::Silver));
    assert!(table.medallion_layers.contains(&MedallionLayer::Gold));
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_description() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
    description: "Primary key identifier"
  - name: name
    data_type: VARCHAR(255)
    description: "User's full name"
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(id_col.description, "Primary key identifier");
    let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
    assert_eq!(name_col.description, "User's full name");
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_with_complex_data_types() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
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

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.columns.len(), 4);
    let tags_col = table.columns.iter().find(|c| c.name == "tags").unwrap();
    assert!(tags_col.data_type.starts_with("ARRAY"));
    let metadata_col = table.columns.iter().find(|c| c.name == "metadata").unwrap();
    assert!(metadata_col.data_type.starts_with("STRUCT"));
    let coords_col = table.columns.iter().find(|c| c.name == "coordinates").unwrap();
    assert!(coords_col.data_type.starts_with("MAP"));
    assert_eq!(errors.len(), 0);
}

#[test]
fn test_parse_odcl_scd_and_data_vault_mutually_exclusive() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
scd_pattern: TYPE_2
data_vault_classification: Hub
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    // Should have an error about mutual exclusivity
    assert!(errors.len() > 0);
    assert!(errors.iter().any(|e| e.message.contains("mutually exclusive")));
}

#[test]
fn test_parse_odcl_various_database_types() {
    let mut parser = ODCLParser::new();
    let test_cases = vec![
        ("Postgres", DatabaseType::Postgres),
        ("MySQL", DatabaseType::Mysql),
        ("SQL_SERVER", DatabaseType::SqlServer),
        ("Databricks", DatabaseType::DatabricksDelta),
        ("AWS_GLUE", DatabaseType::AwsGlue),
    ];

    for (db_type_str, expected) in test_cases {
        let odcl_yaml = format!(
            r#"
name: test_table
columns:
  - name: id
    data_type: INT
database_type: {}
"#,
            db_type_str
        );

        let (table, _) = parser.parse(&odcl_yaml).unwrap();
        assert_eq!(table.database_type, Some(expected), "Failed for {}", db_type_str);
    }
}

#[test]
fn test_parse_odcl_empty_columns() {
    let mut parser = ODCLParser::new();
    let odcl_yaml = r#"
name: empty_table
columns: []
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(table.name, "empty_table");
    assert_eq!(table.columns.len(), 0);
    assert_eq!(errors.len(), 0);
}

    #[test]
    fn test_parse_odcl_column_with_default_nullable() {
        let mut parser = ODCSParser::new();
        let odcl_yaml = r#"
name: users
columns:
  - name: id
    data_type: INT
    primary_key: true
  - name: email
    data_type: VARCHAR(255)
    # nullable not specified, should default to true
"#;

        let (table, errors) = parser.parse(odcl_yaml).unwrap();
        let email_col = table.columns.iter().find(|c| c.name == "email").unwrap();
        assert!(email_col.nullable); // Should default to true
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_parse_data_contract_format() {
        let mut parser = ODCSParser::new();
        let data_contract_yaml = r#"
dataContractSpecification: "1.2.1"
id: "test-contract-001"
info:
  title: "Test Data Contract"
  version: "1.0.0"
  description: "Test contract for users"
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
        description: "User name"
      email:
        type: "VARCHAR(255)"
        required: false
"#;

        let (table, errors) = parser.parse(data_contract_yaml).unwrap();
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.columns[0].name, "id");
        assert_eq!(table.columns[0].data_type, "INTEGER");
        assert!(!table.columns[0].nullable); // required: true
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_parse_data_contract_with_array() {
        let mut parser = ODCSParser::new();
        let data_contract_yaml = r#"
dataContractSpecification: "1.2.1"
id: "test-contract-002"
models:
  products:
    fields:
      id:
        type: "INTEGER"
        required: true
      tags:
        type: "ARRAY"
        items: "STRING"
        required: false
"#;

        let (table, errors) = parser.parse(data_contract_yaml).unwrap();
        assert_eq!(table.name, "products");
        let tags_col = table.columns.iter().find(|c| c.name == "tags").unwrap();
        assert!(tags_col.data_type.starts_with("ARRAY"));
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_parse_data_contract_with_ref() {
        let mut parser = ODCSParser::new();
        let data_contract_yaml = r#"
dataContractSpecification: "1.2.1"
id: "test-contract-003"
models:
  orders:
    fields:
      id:
        type: "INTEGER"
        required: true
      customer:
        $ref: "#/definitions/customer"
definitions:
  customer:
    type: "OBJECT"
    fields:
      name:
        type: "VARCHAR(255)"
      email:
        type: "VARCHAR(255)"
"#;

        let (table, errors) = parser.parse(data_contract_yaml).unwrap();
        assert_eq!(table.name, "orders");
        // Should have customer column (and potentially nested columns if STRUCT expansion is implemented)
        let customer_col = table.columns.iter().find(|c| c.name == "customer");
        assert!(customer_col.is_some());
        // Note: Full STRUCT expansion would add customer.name and customer.email columns
    }

    #[test]
    fn test_parse_odcs_v3_format() {
        let mut parser = ODCSParser::new();
        let odcs_v3_yaml = r#"
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

        let (table, errors) = parser.parse(odcs_v3_yaml).unwrap();
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].name, "id");
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_parse_odcs_v3_with_custom_properties() {
        let mut parser = ODCSParser::new();
        let odcs_v3_yaml = r#"
apiVersion: "v3.0.2"
kind: "DataContract"
id: "test-odcs-002"
version: "1.0.0"
name: "users"
schema:
  - name: "users"
    properties:
      id:
        type: "INTEGER"
customProperties:
  - property: "medallionLayers"
    value: ["bronze", "silver", "gold"]
  - property: "scdPattern"
    value: "TYPE_2"
  - property: "tags"
    value: ["user-data", "production"]
"#;

        let (table, errors) = parser.parse(odcs_v3_yaml).unwrap();
        assert_eq!(table.medallion_layers.len(), 3);
        assert_eq!(table.scd_pattern, Some(SCDPattern::Type2));
        assert_eq!(table.tags.len(), 2);
        assert!(table.tags.contains(&"user-data".to_string()));
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_parse_odcs_v3_with_servers() {
        let mut parser = ODCSParser::new();
        let odcs_v3_yaml = r#"
apiVersion: "v3.0.2"
kind: "DataContract"
id: "test-odcs-003"
version: "1.0.0"
name: "users"
schema:
  - name: "users"
    properties:
      id:
        type: "INTEGER"
servers:
  - server: "databricks-cluster"
    type: "databricks"
"#;

        let (table, errors) = parser.parse(odcs_v3_yaml).unwrap();
        assert_eq!(table.database_type, Some(DatabaseType::DatabricksDelta));
        assert_eq!(errors.len(), 0);
    }
