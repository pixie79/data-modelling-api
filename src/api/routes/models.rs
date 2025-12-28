//! Model export routes.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::tables::AppState;
use crate::services::drawio_service::DrawIOService;
use crate::services::export_service::ExportService;
use std::path::Path as StdPath;

#[derive(Deserialize)]
struct ExportQuery {
    table_ids: Option<Vec<String>>,
    dialect: Option<String>,     // For SQL export
    format: Option<String>, // For ODCS export (odcs_v3_1_0, odcl_v3_legacy, datacontract, simple)
    schema_type: Option<String>, // For schema export: json_schema, avro, protobuf
}

/// Create the models export router
pub fn models_router() -> Router<AppState> {
    Router::new()
        .route("/export/{format}", get(export_format))
        .route("/export/all", get(export_all))
}

/// GET /export/:format - Export model to specified format
async fn export_format(
    State(state): State<AppState>,
    Path(format): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Result<Response<Body>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Parse table IDs if provided
    let table_ids: Option<Vec<Uuid>> = query.table_ids.as_ref().map(|ids| {
        ids.iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect()
    });

    let table_ids_slice = table_ids.as_deref();

    // Export based on format
    let (content, content_type, filename) = match format.as_str() {
        "json_schema" => {
            let json = ExportService::export_json_schema(model, table_ids_slice);
            let content = serde_json::to_string_pretty(&json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            (content, "application/json", format!("{}.json", model.name))
        }
        "avro" => {
            let json = ExportService::export_avro(model, table_ids_slice);
            let content = serde_json::to_string_pretty(&json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            (content, "application/json", format!("{}.avsc", model.name))
        }
        "protobuf" => {
            let content = ExportService::export_protobuf(model, table_ids_slice);
            (
                content,
                "application/x-protobuf",
                format!("{}.proto", model.name),
            )
        }
        "sql" => {
            let content =
                ExportService::export_sql(model, table_ids_slice, query.dialect.as_deref());
            (content, "text/plain", format!("{}.sql", model.name))
        }
        "odcl" => {
            let format_type = query.format.as_deref().unwrap_or("odcs_v3_1_0");
            let exports = ExportService::export_odcl(model, table_ids_slice, format_type);

            // For single table, return YAML directly; for multiple, return JSON with all YAMLs
            if exports.len() == 1 {
                let (_, yaml) = exports.iter().next().unwrap();
                (
                    yaml.clone(),
                    "application/x-yaml",
                    format!("{}.yaml", model.name),
                )
            } else {
                let json = json!(exports);
                let content = serde_json::to_string_pretty(&json)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                (
                    content,
                    "application/json",
                    format!("{}.odcl.json", model.name),
                )
            }
        }
        "png" => {
            let png_data = ExportService::export_png(model, 1920, 1080, table_ids_slice)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
                .header(
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!("attachment; filename=\"{}.png\"", model.name))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                )
                .body(Body::from(png_data))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .body(Body::from(content))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /export/all - Export model to all formats as ZIP
async fn export_all(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response<Body>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Generate all export formats
    let mut zip_data = Vec::new();
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_data));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Export schemas (JSON Schema, AVRO, Protobuf) - individual files per table if schema_type is specified
        if let Some(ref schema_type) = query.schema_type {
            match schema_type.as_str() {
                "json_schema" => {
                    // Export individual JSON Schema files per table
                    use crate::export::json_schema::JSONSchemaExporter;
                    for table in &model.tables {
                        let table_schema = JSONSchemaExporter::export_table(table);
                        let schema_str = serde_json::to_string_pretty(&table_schema)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        zip.start_file(format!("schemas/{}.json", table.name), options)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        zip.write_all(schema_str.as_bytes())
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    }
                }
                "avro" => {
                    // Export individual AVRO schema files per table
                    use crate::export::avro::AvroExporter;
                    for table in &model.tables {
                        let table_schema = AvroExporter::export_table(table);
                        let schema_str = serde_json::to_string_pretty(&table_schema)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        zip.start_file(format!("schemas/{}.avsc", table.name), options)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        zip.write_all(schema_str.as_bytes())
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    }
                }
                "protobuf" => {
                    // Export individual Protobuf files per table
                    use crate::export::protobuf::ProtobufExporter;
                    for table in &model.tables {
                        let mut field_number = 0u32;
                        let mut proto = String::new();
                        proto.push_str("syntax = \"proto3\";\n\n");
                        proto.push_str("package com.datamodel;\n\n");
                        proto.push_str(&ProtobufExporter::export_table(table, &mut field_number));
                        zip.start_file(format!("schemas/{}.proto", table.name), options)
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                        zip.write_all(proto.as_bytes())
                            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    }
                }
                _ => {
                    // Unknown schema type - skip schema export
                }
            }
        } else {
            // Default: export all schema formats as combined files (backward compatibility)
            let json_schema = ExportService::export_json_schema(model, None);
            let json_schema_str = serde_json::to_string_pretty(&json_schema)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.start_file("model.json_schema.json", options)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.write_all(json_schema_str.as_bytes())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let avro = ExportService::export_avro(model, None);
            let avro_str = serde_json::to_string_pretty(&avro)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.start_file("model.avsc", options)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.write_all(avro_str.as_bytes())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let protobuf = ExportService::export_protobuf(model, None);
            zip.start_file("model.proto", options)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.write_all(protobuf.as_bytes())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Export SQL
        let sql = ExportService::export_sql(model, None, None);
        zip.start_file("model.sql", options)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        zip.write_all(sql.as_bytes())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Export ODCL
        let odcl_exports = ExportService::export_odcl(model, None, "odcs_v3_1_0");
        for (table_name, yaml) in odcl_exports {
            zip.start_file(format!("tables/{}.yaml", table_name), options)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.write_all(yaml.as_bytes())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Export DrawIO XML - three versions (conceptual, logical, physical)
        let drawio_service = DrawIOService::new(StdPath::new(&model.git_directory_path));

        use crate::models::enums::ModelingLevel;
        for level in [
            ModelingLevel::Conceptual,
            ModelingLevel::Logical,
            ModelingLevel::Physical,
        ] {
            let drawio_xml = drawio_service
                .export_to_drawio_with_level(model, Some(level))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let level_name = match level {
                ModelingLevel::Conceptual => "conceptual",
                ModelingLevel::Logical => "logical",
                ModelingLevel::Physical => "physical",
            };
            zip.start_file(format!("diagrams/diagram_{}.drawio", level_name), options)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            zip.write_all(drawio_xml.as_bytes())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Export PNG
        let png_data = ExportService::export_png(model, 1920, 1080, None)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        zip.start_file("diagram.png", options)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        zip.write_all(&png_data)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        zip.finish()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/zip"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}.zip\"", model.name))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        )
        .body(Body::from(zip_data))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
