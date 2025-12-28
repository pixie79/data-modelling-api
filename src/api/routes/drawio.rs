//! DrawIO export routes.

use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::tables::AppState;
use crate::models::enums::ModelingLevel;
use crate::services::drawio_service::DrawIOService;
use std::path::Path;

/// Create the DrawIO export router
pub fn drawio_router() -> Router<AppState> {
    Router::new().route("/drawio", get(export_drawio).post(import_drawio))
}

/// Query parameters for DrawIO export
#[derive(Debug, Deserialize)]
struct DrawIOExportQuery {
    modeling_level: Option<String>,
}

/// GET /export/drawio - Export model to DrawIO XML format
async fn export_drawio(
    State(state): State<AppState>,
    Query(params): Query<DrawIOExportQuery>,
) -> Result<Response<Body>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Parse modeling level from query parameter
    let modeling_level = params.modeling_level.as_deref().and_then(|level| {
        match level.to_lowercase().as_str() {
            "conceptual" => Some(ModelingLevel::Conceptual),
            "logical" => Some(ModelingLevel::Logical),
            "physical" => Some(ModelingLevel::Physical),
            _ => None,
        }
    });

    // Create DrawIO service
    let git_path = Path::new(&model.git_directory_path);
    let drawio_service = DrawIOService::new(git_path);

    // Export to DrawIO XML with modeling level
    let xml_content = drawio_service
        .export_to_drawio_with_level(model, modeling_level)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate filename with modeling level if specified
    let level_suffix = modeling_level
        .map(|l| format!("_{:?}", l).to_lowercase())
        .unwrap_or_default();
    let filename = format!("{}{}.drawio", model.name.replace(" ", "_"), level_suffix);

    // Create response with proper headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .body(Body::from(xml_content))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}

/// POST /import/drawio - Import DrawIO XML file to restore layout
async fn import_drawio(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Extract DrawIO XML from multipart form
    let mut xml_content: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let field_name = field.name().unwrap_or("");

        if field_name == "file" || field_name == "drawio" {
            let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            xml_content = Some(String::from_utf8_lossy(&data).to_string());
            break;
        }
    }

    let xml_content = xml_content.ok_or(StatusCode::BAD_REQUEST)?;

    // Validate XML structure
    let _drawio_service = DrawIOService::new(Path::new(&model.git_directory_path));
    if let Err(e) = DrawIOService::validate_drawio_xml(&xml_content) {
        return Ok(Json(json!({
            "success": false,
            "error": format!("Invalid DrawIO XML: {}", e)
        })));
    }

    // Parse DrawIO XML
    let document = match DrawIOService::parse_drawio_xml(&xml_content) {
        Ok(doc) => doc,
        Err(e) => {
            return Ok(Json(json!({
                "success": false,
                "error": format!("Failed to parse DrawIO XML: {}", e)
            })));
        }
    };

    // Extract table positions
    let positions = DrawIOService::extract_table_positions(&document);
    for table in &mut model.tables {
        if let Some((x, y)) = positions.get(&table.id) {
            use crate::models::Position;
            table.position = Some(Position { x: *x, y: *y });
        }
    }

    // Extract relationship routing
    let routing = DrawIOService::extract_relationship_routing(&document);
    for relationship in &mut model.relationships {
        if let Some(waypoints) = routing.get(&relationship.id) {
            use crate::models::relationship::{ConnectionPoint, VisualMetadata};

            let routing_waypoints: Vec<ConnectionPoint> = waypoints
                .iter()
                .map(|(x, y)| ConnectionPoint { x: *x, y: *y })
                .collect();

            relationship.visual_metadata = Some(VisualMetadata {
                source_connection_point: relationship
                    .visual_metadata
                    .as_ref()
                    .and_then(|v| v.source_connection_point.clone()),
                target_connection_point: relationship
                    .visual_metadata
                    .as_ref()
                    .and_then(|v| v.target_connection_point.clone()),
                routing_waypoints,
                label_position: relationship
                    .visual_metadata
                    .as_ref()
                    .and_then(|v| v.label_position.clone()),
            });
        }
    }

    // Resolve ODCS references and check for missing ones
    let git_path = Path::new(&model.git_directory_path);
    let missing_refs = DrawIOService::handle_missing_odcs_references(&document, git_path);

    let warnings = if !missing_refs.is_empty() {
        Some(missing_refs)
    } else {
        None
    };

    Ok(Json(json!({
        "success": true,
        "tables_updated": positions.len(),
        "relationships_updated": routing.len(),
        "warnings": warnings
    })))
}
