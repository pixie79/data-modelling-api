//! Export service for coordinating all export formats.

use crate::export::{
    AvroExporter, JSONSchemaExporter, ODCSExporter, PNGExporter, ProtobufExporter, SQLExporter,
};
use crate::models::DataModel;
use uuid::Uuid;

/// Service for coordinating exports to multiple formats.
pub struct ExportService;

impl ExportService {
    /// Export model to JSON Schema format.
    pub fn export_json_schema(model: &DataModel, table_ids: Option<&[Uuid]>) -> serde_json::Value {
        JSONSchemaExporter::export_model(model, table_ids)
    }

    /// Export model to AVRO format.
    pub fn export_avro(model: &DataModel, table_ids: Option<&[Uuid]>) -> serde_json::Value {
        AvroExporter::export_model(model, table_ids)
    }

    /// Export model to Protobuf format.
    pub fn export_protobuf(model: &DataModel, table_ids: Option<&[Uuid]>) -> String {
        ProtobufExporter::export_model(model, table_ids)
    }

    /// Export model to SQL format.
    pub fn export_sql(
        model: &DataModel,
        table_ids: Option<&[Uuid]>,
        dialect: Option<&str>,
    ) -> String {
        SQLExporter::export_model(model, table_ids, dialect)
    }

    /// Export model to ODCL format.
    pub fn export_odcl(
        model: &DataModel,
        table_ids: Option<&[Uuid]>,
        format: &str,
    ) -> std::collections::HashMap<String, String> {
        // Keep method name as export_odcl for backward compatibility, but uses ODCSExporter
        ODCSExporter::export_model(model, table_ids, format)
    }

    /// Export model to PNG format.
    pub fn export_png(
        model: &DataModel,
        width: u32,
        height: u32,
        table_ids: Option<&[Uuid]>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        PNGExporter::export_model(model, width, height, table_ids)
    }
}
