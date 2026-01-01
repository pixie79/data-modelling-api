//! API endpoint tests matching frontend test coverage.
//!
//! These tests verify the API endpoints that the frontend tests are using:
//! - POST /import/sql/text
//! - POST /import/odcl/text
//! - POST /tables
//! - GET /tables
//! - GET /tables/{id}
//! - PUT /tables/{id}
//! - DELETE /tables/{id}

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
async fn test_import_sql_text_endpoint() {
    let server = create_test_server();

    let request_body = json!({
        "content": "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(255) NOT NULL, email VARCHAR(255));",
        "use_ai": false,
        "filename": "test.sql"
    });

    let response = server.post("/import/sql/text").json(&request_body).await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert!(body.get("tables").is_some());
    let tables = body.get("tables").unwrap().as_array().unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0]["name"], "users");
    assert!(tables[0].get("columns").is_some());
    let columns = tables[0]["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0]["name"], "id");
    assert_eq!(columns[0]["data_type"], "INTEGER");
    assert_eq!(columns[0]["primary_key"], true);
}

#[tokio::test]
async fn test_import_odcl_text_endpoint() {
    let server = create_test_server();

    let request_body = json!({
        "content": r#"
name: users
columns:
  - name: id
    data_type: INT
    nullable: false
    primary_key: true
  - name: name
    data_type: VARCHAR(255)
    nullable: false
"#,
        "use_ai": false,
        "filename": "test.yaml"
    });

    let response = server.post("/import/odcl/text").json(&request_body).await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert!(body.get("tables").is_some());
    let tables = body.get("tables").unwrap().as_array().unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0]["name"], "users");
    assert!(tables[0].get("columns").is_some());
    let columns = tables[0]["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0]["name"], "id");
    assert_eq!(columns[0]["primary_key"], true);
}

#[tokio::test]
async fn test_create_table_endpoint() {
    let server = create_test_server();

    let table_data = json!({
        "name": "test_table",
        "columns": [
            {
                "name": "id",
                "data_type": "INTEGER",
                "nullable": false,
                "primary_key": true,
                "column_order": 0
            },
            {
                "name": "name",
                "data_type": "VARCHAR(255)",
                "nullable": true,
                "primary_key": false,
                "column_order": 1
            }
        ]
    });

    let response = server.post("/tables").json(&table_data).await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["name"], "test_table");
    assert!(body.get("id").is_some());
    assert!(body.get("columns").is_some());
    let columns = body["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 2);
}

#[tokio::test]
async fn test_get_tables_endpoint() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "test_table",
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
    assert_eq!(create_response.status_code(), StatusCode::OK);

    // Then get all tables
    let response = server.get("/tables").await;
    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert!(body.is_array());
    let tables = body.as_array().unwrap();
    assert!(tables.len() >= 1);
    assert!(tables.iter().any(|t| t["name"] == "test_table"));
}

#[tokio::test]
async fn test_get_table_by_id_endpoint() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "test_table",
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
    assert_eq!(create_response.status_code(), StatusCode::OK);

    let created_table: Value = create_response.json();
    let table_id = created_table["id"].as_str().unwrap();

    // Then get the table by ID
    let response = server.get(&format!("/tables/{}", table_id)).await;
    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["id"], table_id);
    assert_eq!(body["name"], "test_table");
}

#[tokio::test]
async fn test_get_table_not_found() {
    let server = create_test_server();

    let response = server
        .get("/tables/00000000-0000-0000-0000-000000000000")
        .await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_table_endpoint() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "test_table",
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
    assert_eq!(create_response.status_code(), StatusCode::OK);

    let created_table: Value = create_response.json();
    let table_id = created_table["id"].as_str().unwrap();

    // Then update the table
    let update_data = json!({
        "name": "updated_table_name"
    });

    let response = server
        .put(&format!("/tables/{}", table_id))
        .json(&update_data)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["id"], table_id);
    assert_eq!(body["name"], "updated_table_name");
}

#[tokio::test]
async fn test_delete_table_endpoint() {
    let server = create_test_server();

    // First create a table
    let table_data = json!({
        "name": "test_table",
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
    assert_eq!(create_response.status_code(), StatusCode::OK);

    let created_table: Value = create_response.json();
    let table_id = created_table["id"].as_str().unwrap();

    // Then delete the table
    let response = server.delete(&format!("/tables/{}", table_id)).await;
    assert_eq!(response.status_code(), StatusCode::OK);

    // Verify it's deleted
    let get_response = server.get(&format!("/tables/{}", table_id)).await;
    assert_eq!(get_response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_table_not_found() {
    let server = create_test_server();

    // Use a valid UUID format but non-existent table ID
    let response = server
        .delete("/tables/550e8400-e29b-41d4-a716-446655440000")
        .await;
    // May return 400 (bad request) for invalid UUID parsing or 404 (not found) for valid UUID but missing table
    assert!(
        response.status_code() == StatusCode::NOT_FOUND
            || response.status_code() == StatusCode::BAD_REQUEST
    );
}
