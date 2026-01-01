//! Table conversion utilities for converting between API Table and SDK Table types.
//!
//! This module provides conversion functions to bridge between the API's internal
//! Table representation and the SDK's Table representation for export operations.

use crate::models::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, ModelingLevel, SCDPattern,
};
use crate::models::{Column, Table};
use data_modelling_sdk::Column as SdkColumn;
use data_modelling_sdk::Table as SdkTable;
use data_modelling_sdk::{
    DataVaultClassification as SdkDataVaultClassification, DatabaseType as SdkDatabaseType,
    MedallionLayer as SdkMedallionLayer, ModelingLevel as SdkModelingLevel,
    SCDPattern as SdkSCDPattern,
};
use uuid::Uuid;

/// Convert API Table to SDK Table for export operations
pub fn api_table_to_sdk_table(table: &Table) -> SdkTable {
    let columns: Vec<SdkColumn> = table
        .columns
        .iter()
        .map(|col| SdkColumn {
            name: col.name.clone(),
            data_type: col.data_type.clone(),
            nullable: col.nullable,
            primary_key: col.primary_key,
            description: col.description.clone(),
            column_order: col.column_order,
            composite_key: col.composite_key.clone(),
            constraints: col.constraints.clone(),
            secondary_key: col.secondary_key,
            foreign_key: col
                .foreign_key
                .as_ref()
                .map(|fk| data_modelling_sdk::ForeignKey {
                    table_id: fk.table_id.clone(),
                    column_name: fk.column_name.clone(),
                }),
            enum_values: col.enum_values.clone(),
            errors: col.errors.clone(),
            quality: col.quality.clone(),
        })
        .collect();

    SdkTable {
        id: table.id,
        name: table.name.clone(),
        columns,
        catalog_name: table.catalog_name.clone(),
        schema_name: table.schema_name.clone(),
        database_type: table.database_type.as_ref().and_then(|dt| match dt {
            DatabaseType::Postgres => Some(SdkDatabaseType::Postgres),
            DatabaseType::Mysql => Some(SdkDatabaseType::Mysql),
            DatabaseType::SqlServer => Some(SdkDatabaseType::SqlServer),
            DatabaseType::DatabricksDelta => Some(SdkDatabaseType::DatabricksDelta),
            DatabaseType::AwsGlue => Some(SdkDatabaseType::AwsGlue),
            _ => None,
        }),
        created_at: table.created_at,
        updated_at: table.updated_at,
        data_vault_classification: table.data_vault_classification.as_ref().map(|dv| match dv {
            DataVaultClassification::Hub => SdkDataVaultClassification::Hub,
            DataVaultClassification::Link => SdkDataVaultClassification::Link,
            DataVaultClassification::Satellite => SdkDataVaultClassification::Satellite,
        }),
        medallion_layers: table
            .medallion_layers
            .iter()
            .map(|ml| match ml {
                MedallionLayer::Bronze => SdkMedallionLayer::Bronze,
                MedallionLayer::Silver => SdkMedallionLayer::Silver,
                MedallionLayer::Gold => SdkMedallionLayer::Gold,
                MedallionLayer::Operational => SdkMedallionLayer::Operational,
            })
            .collect(),
        scd_pattern: table.scd_pattern.as_ref().map(|scd| match scd {
            SCDPattern::Type1 => SdkSCDPattern::Type1,
            SCDPattern::Type2 => SdkSCDPattern::Type2,
        }),
        modeling_level: table.modeling_level.as_ref().map(|ml| match ml {
            ModelingLevel::Conceptual => SdkModelingLevel::Conceptual,
            ModelingLevel::Logical => SdkModelingLevel::Logical,
            ModelingLevel::Physical => SdkModelingLevel::Physical,
        }),
        tags: table.tags.clone(),
        odcl_metadata: table.odcl_metadata.clone(),
        position: table
            .position
            .as_ref()
            .map(|p| data_modelling_sdk::models::Position { x: p.x, y: p.y }),
        yaml_file_path: table.yaml_file_path.clone(),
        drawio_cell_id: table.drawio_cell_id.clone(),
        quality: table.quality.clone(),
        errors: table.errors.clone(),
    }
}

/// Convert API DataModel to SDK DataModel for export operations
pub fn api_datamodel_to_sdk_datamodel(
    model: &crate::models::DataModel,
    table_ids: Option<&[Uuid]>,
) -> data_modelling_sdk::DataModel {
    let tables_to_export: Vec<&crate::models::Table> = if let Some(ids) = table_ids {
        model
            .tables
            .iter()
            .filter(|t| ids.contains(&t.id))
            .collect()
    } else {
        model.tables.iter().collect()
    };

    let sdk_tables: Vec<SdkTable> = tables_to_export
        .iter()
        .map(|t| api_table_to_sdk_table(t))
        .collect();

    data_modelling_sdk::DataModel {
        id: model.id,
        name: model.name.clone(),
        description: model.description.clone(),
        git_directory_path: model.git_directory_path.clone(),
        control_file_path: model.control_file_path.clone(),
        diagram_file_path: model.diagram_file_path.clone(),
        is_subfolder: model.is_subfolder,
        parent_git_directory: model.parent_git_directory.clone(),
        created_at: model.created_at,
        updated_at: model.updated_at,
        tables: sdk_tables,
        // Relationships are not currently exported via SDK DataModel
        // They are handled separately in the API layer
        relationships: Vec::new(),
    }
}

/// Convert SDK Table to API Table for import operations
#[allow(dead_code)] // Reserved for future import operations
pub fn sdk_table_to_api_table(sdk_table: data_modelling_sdk::import::TableData) -> Table {
    use chrono::Utc;
    use std::collections::HashMap;

    let columns: Vec<Column> = sdk_table
        .columns
        .into_iter()
        .map(|sdk_col| Column {
            name: sdk_col.name,
            data_type: sdk_col.data_type,
            nullable: sdk_col.nullable,
            primary_key: sdk_col.primary_key,
            secondary_key: false,
            composite_key: None,
            foreign_key: None,
            constraints: Vec::new(),
            description: String::new(),
            errors: Vec::new(),
            quality: Vec::new(),
            enum_values: Vec::new(),
            column_order: 0,
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
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
