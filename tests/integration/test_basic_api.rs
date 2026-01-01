//! Basic API integration tests

use axum::http::StatusCode;
use data_modelling_api::routes;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_health_check() {
    let app_state = routes::create_app_state();
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .with_state(app_state);

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/health")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_openapi_endpoint() {
    let app_state = routes::create_app_state();
    let app = routes::create_api_router(app_state);

    let response = axum_test::TestServer::new(app)
        .unwrap()
        .get("/api/v1/openapi.json")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}
