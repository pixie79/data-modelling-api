use super::column::Column;
use super::enums::{
    DataVaultClassification, DatabaseType, MedallionLayer, ModelingLevel, SCDPattern,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub id: Uuid,
    pub name: String,
    pub columns: Vec<Column>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_type: Option<DatabaseType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_name: Option<String>,
    #[serde(default)]
    pub medallion_layers: Vec<MedallionLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scd_pattern: Option<SCDPattern>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_vault_classification: Option<DataVaultClassification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modeling_level: Option<ModelingLevel>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub odcl_metadata: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaml_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawio_cell_id: Option<String>,
    #[serde(default)]
    pub quality: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub errors: Vec<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Table {
    #[allow(dead_code)]
    pub fn new(name: String, columns: Vec<Column>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
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

    pub fn get_unique_key(&self) -> (Option<String>, String, Option<String>, Option<String>) {
        (
            self.database_type.as_ref().map(|dt| format!("{:?}", dt)),
            self.name.clone(),
            self.catalog_name.clone(),
            self.schema_name.clone(),
        )
    }

    #[allow(dead_code)]
    pub fn validate_pattern_exclusivity(&self) -> Result<(), String> {
        if self.scd_pattern.is_some() && self.data_vault_classification.is_some() {
            return Err(
                "SCD pattern and Data Vault classification are mutually exclusive".to_string(),
            );
        }
        Ok(())
    }
}
