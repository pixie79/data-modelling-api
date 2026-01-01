//! OpenAPI specification endpoints.
//!
//! Provides endpoints to serve the OpenAPI spec as JSON.

use axum::{
    Router,
    response::{Html, Json},
    routing::get,
};
use utoipa::OpenApi;

use super::super::openapi::ApiDoc;
use super::app_state::AppState;

/// Create the OpenAPI router
pub fn openapi_router() -> Router<AppState> {
    Router::new()
        .route("/openapi.json", get(serve_openapi_json))
        .route("/swagger", get(serve_swagger_html))
}

/// GET /openapi.json - Serve the OpenAPI specification as JSON
#[utoipa::path(
    get,
    path = "/openapi.json",
    tag = "OpenAPI",
    responses(
        (status = 200, description = "OpenAPI specification", body = Object)
    )
)]
pub async fn serve_openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// GET /swagger - Serve a simple HTML page with link to OpenAPI spec
pub async fn serve_swagger_html() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Data Modelling API - OpenAPI Documentation</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
        }
        h1 { color: #333; }
        a {
            display: inline-block;
            margin-top: 20px;
            padding: 10px 20px;
            background-color: #007bff;
            color: white;
            text-decoration: none;
            border-radius: 5px;
        }
        a:hover { background-color: #0056b3; }
        .info {
            margin-top: 20px;
            padding: 15px;
            background-color: #f8f9fa;
            border-left: 4px solid #007bff;
        }
    </style>
</head>
<body>
    <h1>Data Modelling API Documentation</h1>
    <p>OpenAPI specification is available at:</p>
    <a href="/api/v1/openapi.json">Download openapi.json</a>
    <div class="info">
        <p><strong>Note:</strong> You can use this OpenAPI spec with external tools like:</p>
        <ul>
            <li><a href="https://editor.swagger.io" target="_blank">Swagger Editor</a></li>
            <li><a href="https://swagger.io/tools/swagger-ui/" target="_blank">Swagger UI</a></li>
            <li>Postman (import from URL)</li>
        </ul>
    </div>
</body>
</html>
"#,
    )
}
