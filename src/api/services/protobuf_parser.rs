//! Protobuf parser service.
//!
//! Parses Protobuf schema files.
//! Uses SDK ProtobufImporter to avoid code duplication.

use crate::models::{Column, Table};
use chrono::Utc;
use data_modelling_sdk::import::ProtobufImporter;
use std::collections::HashMap;
use uuid::Uuid;

/// Protobuf parser service wrapper around SDK
pub struct ProtobufParser;

impl ProtobufParser {
    /// Create a new Protobuf parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a Protobuf schema file
    pub async fn parse(
        &self,
        content: &str,
    ) -> Result<(Vec<crate::models::Table>, Vec<String>), Box<dyn std::error::Error>> {
        let importer = ProtobufImporter::new();
        // SDK ProtobufImporter returns Result<DataModel, ImportError>
        match importer.import(content) {
            Ok(model) => {
                // Convert SDK TableData to API Table
                let tables: Vec<Table> = model
                    .tables
                    .into_iter()
                    .map(Self::convert_sdk_table_to_api_table)
                    .collect();
                Ok((tables, Vec::new()))
            }
            Err(e) => Err(format!("Protobuf import error: {}", e).into()),
        }
    }

    /// Convert SDK TableData to API Table
    fn convert_sdk_table_to_api_table(sdk_table: data_modelling_sdk::import::TableData) -> Table {
        let now = Utc::now();
        let columns: Vec<Column> = sdk_table
            .columns
            .into_iter()
            .map(|sdk_col| {
                Column {
                    name: sdk_col.name,
                    data_type: sdk_col.data_type,
                    nullable: sdk_col.nullable,
                    primary_key: sdk_col.primary_key,
                    secondary_key: false,
                    composite_key: None,
                    foreign_key: None,
                    constraints: Vec::new(),
                    description: String::new(), // SDK ColumnData doesn't have description field
                    errors: Vec::new(),
                    quality: Vec::new(),
                    enum_values: Vec::new(),
                    column_order: 0,
                }
            })
            .collect();

        Table {
            id: Uuid::new_v4(),
            name: sdk_table
                .name
                .unwrap_or_else(|| "unnamed_table".to_string()),
            columns,
            database_type: None,
            catalog_name: None,
            schema_name: None,
            medallion_layers: Vec::new(),
            scd_pattern: None,
            data_vault_classification: None,
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata: HashMap::new(),
            position: None,
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

impl Default for ProtobufParser {
    fn default() -> Self {
        Self::new()
    }
}
