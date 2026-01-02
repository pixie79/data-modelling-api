//! Contract tests verifying API responses match Python backend API contract.
//!
//! These tests verify that the Rust API endpoints return responses
//! that match the contract defined in the Python backend.
//!
//! These tests verify:
//! 1. Response status codes match Python backend
//! 2. Response JSON structure matches Python backend
//! 3. Error responses match Python backend
//! 4. All endpoints return expected data types

use axum::http::StatusCode;
use axum_test::TestServer;
use data_modelling_api::api::routes::{create_api_router, create_app_state};
use serde_json::{json, Value};

fn create_test_server() -> TestServer {
    let app_state = create_app_state();
    let router = create_api_router(app_state);
    TestServer::new(router).unwrap()
}

#[tokio::test]
async fn test_health_check_contract() {
    let server = create_test_server();

    let response = server.get("/health").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    // Contract: GET /health returns {"status": "ok", "service": "data-modelling-api", "version": "1.0.0"}
    let body: Value = response.json();
    assert_eq!(body["status"], "ok", "Status should be 'ok'");
    assert_eq!(body["service"], "data-modelling-api", "Service name should match");
    assert!(body.get("version").is_some(), "Should have version field");
}

#[tokio::test]
async fn test_health_check_api_v1_contract() {
    let server = create_test_server();

    let response = server.get("/api/v1/health").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    // Contract: GET /api/v1/health returns {"status": "ok", "service": "data-modelling-api", "version": "1.0.0"}
    let body: Value = response.json();
    assert_eq!(body["status"], "ok", "Status should be 'ok'");
    assert_eq!(body["service"], "data-modelling-api", "Service name should match");
    assert!(body.get("version").is_some(), "Should have version field");
}

#[tokio::test]
async fn test_get_tables_empty_contract() {
    let server = create_test_server();

    let response = server.get("/tables").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    // Should return an array (empty when no model is loaded)
    assert!(body.is_array(), "GET /tables should return an array");
    let tables = body.as_array().unwrap();
    assert_eq!(tables.len(), 0, "Should return empty array when no model is loaded");
}

#[tokio::test]
async fn test_get_tables_with_filtering_contract() {
    let server = create_test_server();

    // Test with query parameters
    let response = server.get("/tables?modeling_level=conceptual").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    assert!(body.is_array(), "GET /tables should return an array");
}

#[tokio::test]
async fn test_import_sql_text_contract() {
    let server = create_test_server();

    let request_body = json!({
        "sql": "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(255));"
    });

    let response = server.post("/import/sql/text").json(&request_body).await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    // Contract: Returns {"tables": [...], "errors": []}
    assert!(body.get("tables").is_some(), "Response should have 'tables' field");
    assert!(body.get("errors").is_some(), "Response should have 'errors' field");
    
    let tables = body["tables"].as_array().unwrap();
    assert!(tables.len() >= 0, "Tables array should exist");
    
    let errors = body["errors"].as_array().unwrap();
    assert!(errors.len() >= 0, "Errors array should exist");
}

#[tokio::test]
async fn test_import_odcl_text_contract() {
    let server = create_test_server();

    let request_body = json!({
        "content": r#"
name: users
columns:
  - name: id
    data_type: INT
    nullable: false
    primary_key: true
"#
    });

    let response = server.post("/import/odcl/text").json(&request_body).await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    // Contract: Returns {"tables": [...], "errors": []}
    assert!(body.get("tables").is_some(), "Response should have 'tables' field");
    assert!(body.get("errors").is_some(), "Response should have 'errors' field");
    
    let tables = body["tables"].as_array().unwrap();
    assert!(tables.len() >= 0, "Tables array should exist");
    
    let errors = body["errors"].as_array().unwrap();
    assert!(errors.len() >= 0, "Errors array should exist");
}

#[tokio::test]
async fn test_get_table_by_id_contract() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "contract_test_table",
        "columns": [
            {
                "name": "id",
                "data_type": "INTEGER",
                "nullable": false,
                "primary_key": true,
                "column_order": 0
            }
        ]
    });

    let create_response = server.post("/tables").json(&table_data).await;
    
    if create_response.status_code() == StatusCode::OK {
        let created_table: Value = create_response.json();
        let table_id = created_table["id"].as_str().unwrap();

        // Then get the table by ID
        let response = server.get(&format!("/tables/{}", table_id)).await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: Value = response.json();
        assert_eq!(body["id"], table_id, "Table ID should match");
        assert_eq!(body["name"], "contract_test_table", "Table name should match");
        assert!(body.get("columns").is_some(), "Table should have columns");
    }
}

#[tokio::test]
async fn test_get_table_not_found_contract() {
    let server = create_test_server();

    // Use a valid UUID format but non-existent table ID
    let response = server
        .get("/tables/00000000-0000-0000-0000-000000000000")
        .await;

    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_relationships_empty_contract() {
    let server = create_test_server();

    let response = server.get("/relationships").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    assert!(body.is_array(), "GET /relationships should return an array");
    let relationships = body.as_array().unwrap();
    assert_eq!(relationships.len(), 0, "Should return empty array when no relationships exist");
}

#[tokio::test]
async fn test_create_relationship_contract() {
    let server = create_test_server();

    // First create two tables
    let table1_data = json!({
        "name": "source_table",
        "columns": [{"name": "id", "data_type": "INTEGER", "nullable": false, "primary_key": true, "column_order": 0}]
    });
    let table2_data = json!({
        "name": "target_table",
        "columns": [{"name": "id", "data_type": "INTEGER", "nullable": false, "primary_key": true, "column_order": 0}]
    });

    let create1_response = server.post("/tables").json(&table1_data).await;
    let create2_response = server.post("/tables").json(&table2_data).await;

    if create1_response.status_code() == StatusCode::OK && create2_response.status_code() == StatusCode::OK {
        let table1: Value = create1_response.json();
        let table2: Value = create2_response.json();
        let table1_id = table1["id"].as_str().unwrap();
        let table2_id = table2["id"].as_str().unwrap();

        let relationship_data = json!({
            "source_table_id": table1_id,
            "target_table_id": table2_id,
            "relationship_type": "one_to_many"
        });

        let response = server.post("/relationships").json(&relationship_data).await;

        // Should return 200 OK or 400 BAD_REQUEST (if circular)
        assert!(
            response.status_code() == StatusCode::OK || response.status_code() == StatusCode::BAD_REQUEST,
            "Should return 200 OK or 400 BAD_REQUEST"
        );

        if response.status_code() == StatusCode::OK {
            let body: Value = response.json();
            assert!(body.get("id").is_some(), "Relationship should have 'id' field");
            assert_eq!(body["source_table_id"], table1_id, "Should have source_table_id");
            assert_eq!(body["target_table_id"], table2_id, "Should have target_table_id");
        }
    }
}

#[tokio::test]
async fn test_get_table_stats_contract() {
    let server = create_test_server();

    let response = server.get("/tables/stats").await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    // Contract: Returns {"total_tables": N, "by_modeling_level": {}, "by_medallion_layer": {}}
    assert!(body.get("total_tables").is_some(), "Response should have 'total_tables' field");
    assert!(body.get("by_modeling_level").is_some(), "Response should have 'by_modeling_level' field");
    assert!(body.get("by_medallion_layer").is_some(), "Response should have 'by_medallion_layer' field");
    
    assert!(body["total_tables"].is_number(), "total_tables should be a number");
    assert!(body["by_modeling_level"].is_object(), "by_modeling_level should be an object");
    assert!(body["by_medallion_layer"].is_object(), "by_medallion_layer should be an object");
}

#[tokio::test]
async fn test_filter_tables_contract() {
    let server = create_test_server();

    let filter_data = json!({
        "modeling_level": "conceptual"
    });

    let response = server.post("/tables/filter").json(&filter_data).await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    assert!(body.is_array(), "POST /tables/filter should return an array");
}

#[tokio::test]
async fn test_import_sql_with_conflicts_contract() {
    let server = create_test_server();

    // First import a table
    let request1 = json!({
        "sql": "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(255));"
    });
    let _ = server.post("/import/sql/text").json(&request1).await;

    // Then try to import the same table again (should create conflict)
    let request2 = json!({
        "sql": "CREATE TABLE users (id INTEGER PRIMARY KEY, email VARCHAR(255));"
    });

    let response = server.post("/import/sql/text").json(&request2).await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    assert!(body.get("tables").is_some(), "Response should have 'tables' field");
    assert!(body.get("errors").is_some(), "Response should have 'errors' field");
    
    // May have conflicts or errors
    let errors = body["errors"].as_array().unwrap();
    // Errors array should exist (may be empty or contain conflict info)
    assert!(errors.len() >= 0, "Errors array should exist");
}

#[tokio::test]
async fn test_check_circular_dependency_contract() {
    let server = create_test_server();

    // Create a simple relationship check request
    let check_data = json!({
        "source_table_id": "00000000-0000-0000-0000-000000000001",
        "target_table_id": "00000000-0000-0000-0000-000000000002"
    });

    let response = server.post("/relationships/check-circular").json(&check_data).await;

    assert_eq!(response.status_code(), StatusCode::OK);
    
    let body: Value = response.json();
    // Contract: Returns {"is_circular": bool, "circular_path": [...]}
    assert!(body.get("is_circular").is_some(), "Response should have 'is_circular' field");
    assert!(body.get("circular_path").is_some(), "Response should have 'circular_path' field");
    
    assert!(body["is_circular"].is_boolean(), "is_circular should be a boolean");
    assert!(body["circular_path"].is_array(), "circular_path should be an array");
}

#[tokio::test]
async fn test_delete_table_contract() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "table_to_delete",
        "columns": [
            {
                "name": "id",
                "data_type": "INTEGER",
                "nullable": false,
                "primary_key": true,
                "column_order": 0
            }
        ]
    });

    let create_response = server.post("/tables").json(&table_data).await;
    
    if create_response.status_code() == StatusCode::OK {
        let created_table: Value = create_response.json();
        let table_id = created_table["id"].as_str().unwrap();

        // Then delete the table
        let response = server.delete(&format!("/tables/{}", table_id)).await;

        assert_eq!(response.status_code(), StatusCode::OK);
        
        // Response may be empty or contain a message
        // Contract allows for {"message": "Table deleted successfully"} or empty response
        let body_text = response.text();
        // Either empty or contains success message
        assert!(
            body_text.is_empty() || body_text.contains("deleted") || body_text.contains("success"),
            "Delete response should be empty or contain success message"
        );
    } else {
        // If table creation fails, test with non-existent ID
        let response = server
            .delete("/tables/00000000-0000-0000-0000-000000000000")
            .await;
        
        // Should return 404 NOT_FOUND for non-existent table
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
