//! Export service.
//!
//! Provides multi-format export functionality.
//! Uses SDK exporters to avoid code duplication.

use crate::models::DataModel;
use crate::services::table_converter::{api_datamodel_to_sdk_datamodel, api_table_to_sdk_table};
use data_modelling_sdk::export::{AvroExporter, JSONSchemaExporter, ODCSExporter, SQLExporter};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Export service wrapper around local exporters
pub struct ExportService;

impl ExportService {
    /// Export model to JSON Schema format using SDK
    pub fn export_json_schema(model: &DataModel, table_ids: Option<&[Uuid]>) -> Value {
        use crate::services::table_converter::api_datamodel_to_sdk_datamodel;
        let sdk_model = api_datamodel_to_sdk_datamodel(model, table_ids);
        JSONSchemaExporter::export_model(&sdk_model, None)
    }

    /// Export model to Avro format using SDK
    pub fn export_avro(model: &DataModel, table_ids: Option<&[Uuid]>) -> Value {
        use crate::services::table_converter::api_datamodel_to_sdk_datamodel;
        let sdk_model = api_datamodel_to_sdk_datamodel(model, table_ids);
        AvroExporter::export_model(&sdk_model, None)
    }

    /// Export model to Protobuf format
    pub fn export_protobuf(model: &DataModel, table_ids: Option<&[Uuid]>) -> String {
        let mut proto = String::new();
        proto.push_str("syntax = \"proto3\";\n\n");
        proto.push_str("package com.datamodel;\n\n");

        let tables_to_export: Vec<&crate::models::Table> = if let Some(ids) = table_ids {
            model
                .tables
                .iter()
                .filter(|t| ids.contains(&t.id))
                .collect()
        } else {
            model.tables.iter().collect()
        };

        for table in tables_to_export {
            // ProtobufExporter from SDK - check actual method signature
            // For now, use a placeholder implementation
            proto.push_str(&format!("message {} {{\n", table.name));
            for (idx, col) in table.columns.iter().enumerate() {
                proto.push_str(&format!(
                    "  {} {} = {};\n",
                    Self::map_to_protobuf_type(&col.data_type),
                    col.name,
                    idx + 1
                ));
            }
            proto.push_str("}\n");
        }

        proto
    }

    /// Export model to SQL format using SDK
    pub fn export_sql(
        model: &DataModel,
        table_ids: Option<&[Uuid]>,
        dialect: Option<&str>,
    ) -> String {
        let sdk_model = api_datamodel_to_sdk_datamodel(model, table_ids);
        SQLExporter::export_model(&sdk_model, table_ids, dialect)
    }

    /// Export model to ODCL/ODCS format using SDK
    pub fn export_odcl(
        model: &DataModel,
        table_ids: Option<&[Uuid]>,
        format_type: &str,
    ) -> HashMap<String, String> {
        let tables_to_export: Vec<&crate::models::Table> = if let Some(ids) = table_ids {
            model
                .tables
                .iter()
                .filter(|t| ids.contains(&t.id))
                .collect()
        } else {
            model.tables.iter().collect()
        };

        let mut exports = HashMap::new();
        for table in tables_to_export {
            let sdk_table = api_table_to_sdk_table(table);
            let yaml = ODCSExporter::export_table(&sdk_table, format_type);
            exports.insert(table.name.clone(), yaml);
        }
        exports
    }

    /// Export model to PNG format (diagram)
    /// Note: PNG export requires DrawIO XML conversion via external tooling
    /// This returns DrawIO XML which can be converted to PNG using DrawIO desktop/web app
    pub fn export_png(
        _model: &DataModel,
        _width: u32,
        _height: u32,
        _table_ids: Option<&[Uuid]>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use crate::services::drawio_service::DrawIOService;
        use std::path::Path;

        // Generate DrawIO XML
        let drawio_service = DrawIOService::new(Path::new(&_model.git_directory_path));
        let xml = drawio_service.export_to_drawio(_model)?;

        // Return XML as bytes - actual PNG conversion requires DrawIO tooling
        // In production, this would call DrawIO CLI or API to convert XML to PNG
        Ok(xml.into_bytes())
    }

    /// Map data type to Protobuf type
    pub fn map_to_protobuf_type(data_type: &str) -> &str {
        match data_type.to_uppercase().as_str() {
            "INT" | "INTEGER" | "SMALLINT" | "TINYINT" => "int32",
            "BIGINT" => "int64",
            "FLOAT" | "REAL" => "float",
            "DOUBLE" | "DECIMAL" | "NUMERIC" => "double",
            "BOOLEAN" | "BOOL" => "bool",
            "BYTES" | "BINARY" | "VARBINARY" => "bytes",
            _ => "string",
        }
    }
}

impl Default for ExportService {
    fn default() -> Self {
        Self
    }
}
