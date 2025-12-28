//! Unit tests for ODCS parser covering Phase 1, 2, and 3 fields.
//!
//! Tests cover:
//! - Phase 1: domain, dataProduct, tenant, pricing, team, roles, terms
//! - Phase 2: servicelevels, links
//! - Phase 3: infrastructure, servers (full object)

use data_modelling_api::api::services::odcs_parser::ODCSParser;
use serde_json::Value as JsonValue;

#[test]
fn test_parse_phase1_domain_dataproduct_tenant() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
status: active
domain: ecommerce
dataProduct: customer-analytics
tenant: acme-corp
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
        required: true
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check Phase 1 fields are parsed
    assert_eq!(table.odcl_metadata.get("domain").and_then(|v| v.as_str()), Some("ecommerce"));
    assert_eq!(table.odcl_metadata.get("dataProduct").and_then(|v| v.as_str()), Some("customer-analytics"));
    assert_eq!(table.odcl_metadata.get("tenant").and_then(|v| v.as_str()), Some("acme-corp"));
}

#[test]
fn test_parse_phase1_pricing() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
pricing:
  model: subscription
  currency: USD
  amount: 100.00
  unit: per-month
  description: Monthly subscription fee
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check pricing is parsed
    let pricing = table.odcl_metadata.get("pricing").and_then(|v| v.as_object());
    assert!(pricing.is_some());
    let pricing_obj = pricing.unwrap();
    assert_eq!(pricing_obj.get("model").and_then(|v| v.as_str()), Some("subscription"));
    assert_eq!(pricing_obj.get("currency").and_then(|v| v.as_str()), Some("USD"));
    assert_eq!(pricing_obj.get("amount").and_then(|v| v.as_f64()), Some(100.0));
}

#[test]
fn test_parse_phase1_team() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
team:
  - name: John Doe
    email: john@example.com
    role: Data Engineer
  - name: Jane Smith
    email: jane@example.com
    role: Data Product Owner
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check team is parsed
    let team = table.odcl_metadata.get("team").and_then(|v| v.as_array());
    assert!(team.is_some());
    let team_arr = team.unwrap();
    assert_eq!(team_arr.len(), 2);
    assert_eq!(team_arr[0].get("name").and_then(|v| v.as_str()), Some("John Doe"));
    assert_eq!(team_arr[0].get("email").and_then(|v| v.as_str()), Some("john@example.com"));
    assert_eq!(team_arr[1].get("name").and_then(|v| v.as_str()), Some("Jane Smith"));
}

#[test]
fn test_parse_phase1_roles() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
roles:
  viewer:
    description: Can view data
    permissions:
      - read
  editor:
    description: Can edit data
    permissions:
      - read
      - write
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check roles are parsed
    let roles = table.odcl_metadata.get("roles").and_then(|v| v.as_object());
    assert!(roles.is_some());
    let roles_obj = roles.unwrap();
    assert_eq!(roles_obj.get("viewer").and_then(|v| v.get("description").and_then(|d| d.as_str())), Some("Can view data"));
    assert_eq!(roles_obj.get("editor").and_then(|v| v.get("description").and_then(|d| d.as_str())), Some("Can edit data"));
}

#[test]
fn test_parse_phase1_terms() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
terms:
  usage: Data can only be used for internal analytics
  legal: Subject to company data policy
  expiration: "2025-12-31"
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check terms are parsed
    let terms = table.odcl_metadata.get("terms").and_then(|v| v.as_object());
    assert!(terms.is_some());
    let terms_obj = terms.unwrap();
    assert_eq!(terms_obj.get("usage").and_then(|v| v.as_str()), Some("Data can only be used for internal analytics"));
    assert_eq!(terms_obj.get("legal").and_then(|v| v.as_str()), Some("Subject to company data policy"));
    assert_eq!(terms_obj.get("expiration").and_then(|v| v.as_str()), Some("2025-12-31"));
}

#[test]
fn test_parse_phase2_servicelevels() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
servicelevels:
  availability:
    description: The server is available during support hours
    percentage: "99.9%"
  retention:
    description: Data is retained for two years
    period: P2Y
    unlimited: false
  latency:
    description: Data is available within a few minutes
    threshold: 1h
    sourceTimestampField: "#PO to add timestamp field"
    processedTimestampField: "#PO to add timestamp field"
  freshness:
    description: The age of the youngest row in a table
    threshold: 1m
    timestampField: "#PO to add timestamp field"
  frequency:
    description: Data is delivered via a kafka topic
    type: streaming
  support:
    description: The data feed is supported via Platform Services
    time: 24x7
    responseTime: 1h
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check servicelevels are parsed
    let servicelevels = table.odcl_metadata.get("servicelevels").and_then(|v| v.as_object());
    assert!(servicelevels.is_some());
    let sl_obj = servicelevels.unwrap();

    assert_eq!(sl_obj.get("availability").and_then(|v| v.get("description").and_then(|d| d.as_str())), Some("The server is available during support hours"));
    assert_eq!(sl_obj.get("availability").and_then(|v| v.get("percentage").and_then(|p| p.as_str())), Some("99.9%"));

    assert_eq!(sl_obj.get("retention").and_then(|v| v.get("period").and_then(|p| p.as_str())), Some("P2Y"));
    assert_eq!(sl_obj.get("retention").and_then(|v| v.get("unlimited").and_then(|u| u.as_bool())), Some(false));

    assert_eq!(sl_obj.get("latency").and_then(|v| v.get("threshold").and_then(|t| t.as_str())), Some("1h"));
    assert_eq!(sl_obj.get("freshness").and_then(|v| v.get("threshold").and_then(|t| t.as_str())), Some("1m"));
    assert_eq!(sl_obj.get("frequency").and_then(|v| v.get("type").and_then(|t| t.as_str())), Some("streaming"));
    assert_eq!(sl_obj.get("support").and_then(|v| v.get("time").and_then(|t| t.as_str())), Some("24x7"));
}

#[test]
fn test_parse_phase2_links() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
links:
  githubRepo: https://github.com/Flutter-Global/gbsbom
  documentation: https://docs.example.com
  apiDocs: https://api.example.com/docs
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check links are parsed
    let links = table.odcl_metadata.get("links").and_then(|v| v.as_object());
    assert!(links.is_some());
    let links_obj = links.unwrap();
    assert_eq!(links_obj.get("githubRepo").and_then(|v| v.as_str()), Some("https://github.com/Flutter-Global/gbsbom"));
    assert_eq!(links_obj.get("documentation").and_then(|v| v.as_str()), Some("https://docs.example.com"));
    assert_eq!(links_obj.get("apiDocs").and_then(|v| v.as_str()), Some("https://api.example.com/docs"));
}

#[test]
fn test_parse_phase3_infrastructure() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
infrastructure:
  cluster: production-cluster
  region: us-east-1
  environment: production
  resources:
    cpu: "4 cores"
    memory: "16GB"
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check infrastructure is parsed
    let infrastructure = table.odcl_metadata.get("infrastructure").and_then(|v| v.as_object());
    assert!(infrastructure.is_some());
    let infra_obj = infrastructure.unwrap();
    assert_eq!(infra_obj.get("cluster").and_then(|v| v.as_str()), Some("production-cluster"));
    assert_eq!(infra_obj.get("region").and_then(|v| v.as_str()), Some("us-east-1"));
    assert_eq!(infra_obj.get("environment").and_then(|v| v.as_str()), Some("production"));
}

#[test]
fn test_parse_phase3_servers_full() {
    let mut parser = ODCSParser::new();
    let odcs_yaml = r#"
apiVersion: v3.1.0
kind: DataContract
id: test-contract
name: Test Contract
version: 1.0.0
servers:
  - name: production-db
    type: postgres
    url: postgresql://prod.example.com:5432/db
    description: Production PostgreSQL database
    environment: production
  - name: staging-db
    type: mysql
    url: mysql://staging.example.com:3306/db
    description: Staging MySQL database
    environment: staging
schema:
  - name: Customer
    properties:
      id:
        type: INTEGER
"#;

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check servers are parsed
    let servers = table.odcl_metadata.get("servers").and_then(|v| v.as_array());
    assert!(servers.is_some());
    let servers_arr = servers.unwrap();
    assert_eq!(servers_arr.len(), 2);

    assert_eq!(servers_arr[0].get("name").and_then(|v| v.as_str()), Some("production-db"));
    assert_eq!(servers_arr[0].get("type").and_then(|v| v.as_str()), Some("postgres"));
    assert_eq!(servers_arr[0].get("url").and_then(|v| v.as_str()), Some("postgresql://prod.example.com:5432/db"));
    assert_eq!(servers_arr[0].get("environment").and_then(|v| v.as_str()), Some("production"));

    assert_eq!(servers_arr[1].get("name").and_then(|v| v.as_str()), Some("staging-db"));
    assert_eq!(servers_arr[1].get("type").and_then(|v| v.as_str()), Some("mysql"));
}

#[test]
fn test_parse_all_phases_combined() {
    let mut parser = ODCSParser::new();
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

    let (table, errors) = parser.parse(odcs_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Verify all Phase 1 fields
    assert_eq!(table.odcl_metadata.get("domain").and_then(|v| v.as_str()), Some("ecommerce"));
    assert_eq!(table.odcl_metadata.get("dataProduct").and_then(|v| v.as_str()), Some("customer-analytics"));
    assert_eq!(table.odcl_metadata.get("tenant").and_then(|v| v.as_str()), Some("acme-corp"));
    assert!(table.odcl_metadata.get("pricing").is_some());
    assert!(table.odcl_metadata.get("team").is_some());
    assert!(table.odcl_metadata.get("roles").is_some());
    assert!(table.odcl_metadata.get("terms").is_some());

    // Verify Phase 2 fields
    assert!(table.odcl_metadata.get("servicelevels").is_some());
    assert!(table.odcl_metadata.get("links").is_some());

    // Verify Phase 3 fields
    assert!(table.odcl_metadata.get("infrastructure").is_some());
    assert!(table.odcl_metadata.get("servers").is_some());
}

#[test]
fn test_parse_odcl_data_contract_format_with_phases() {
    let mut parser = ODCSParser::new();
    let odcl_yaml = r#"
dataContractSpecification: 1.2.1
id: odcl-contract
info:
  title: ODCL Contract
  version: 1.0.0
  status: active
models:
  Customer:
    fields:
      id:
        type: integer
        required: true
servers:
  production:
    type: postgres
    url: postgresql://localhost:5432/db
tags:
  - customer
  - test
servicelevels:
  availability:
    description: 99.9% uptime
links:
  githubRepo: https://github.com/example/repo
"#;

    let (table, errors) = parser.parse(odcl_yaml).unwrap();
    assert_eq!(errors.len(), 0);

    // Check that ODCL format also parses Phase 2 and 3 fields
    assert!(table.odcl_metadata.get("servicelevels").is_some());
    assert!(table.odcl_metadata.get("links").is_some());
    assert!(table.odcl_metadata.get("servers").is_some());
    assert_eq!(table.tags.len(), 2);
}
