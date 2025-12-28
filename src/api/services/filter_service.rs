//! Filter service for managing table filtering and modeling levels.

use crate::models::enums::{MedallionLayer, ModelingLevel};
use crate::models::{DataModel, Table};
use std::collections::{HashMap, HashSet};
use tracing::info;
use uuid::Uuid;

/// Service for filtering tables by various criteria.
pub struct FilterService {
    /// Data model containing tables
    model: Option<DataModel>,
}

impl FilterService {
    /// Create a new filter service instance.
    pub fn new(model: Option<DataModel>) -> Self {
        Self { model }
    }

    /// Set the model for this service.
    pub fn set_model(&mut self, model: DataModel) {
        self.model = Some(model);
    }

    /// Filter tables based on criteria.
    ///
    /// Optimized for large models (100+ tables) using early filtering.
    pub fn filter_tables(
        &self,
        table_ids: Option<&[String]>,
        modeling_level: Option<ModelingLevel>,
        medallion_layers: Option<&[MedallionLayer]>,
        database_types: Option<&[String]>,
        scd_patterns: Option<&[String]>,
        data_vault_classifications: Option<&[String]>,
    ) -> Vec<Table> {
        let model = match &self.model {
            Some(m) => m,
            None => return Vec::new(),
        };

        let mut filtered: Vec<&Table> = model.tables.iter().collect();

        // Filter by table IDs
        if let Some(ids) = table_ids {
            let id_set: HashSet<Uuid> = ids
                .iter()
                .filter_map(|id| Uuid::parse_str(id).ok())
                .collect();
            filtered.retain(|t| id_set.contains(&t.id));
        }

        // Filter by modeling level
        if let Some(level) = modeling_level {
            filtered.retain(|t| t.modeling_level == Some(level));
        }

        // Filter by medallion layers
        if let Some(layers) = medallion_layers {
            let layer_set: HashSet<MedallionLayer> = layers.iter().copied().collect();
            filtered.retain(|t| {
                t.medallion_layers
                    .iter()
                    .any(|layer| layer_set.contains(layer))
            });
        }

        // Filter by database types
        if let Some(db_types) = database_types {
            let db_type_set: HashSet<&str> = db_types.iter().map(|s| s.as_str()).collect();
            filtered.retain(|t| {
                t.database_type
                    .as_ref()
                    .map(|dt| {
                        // Convert enum to string for comparison
                        format!("{:?}", dt)
                    })
                    .map(|s| db_type_set.contains(s.as_str()))
                    .unwrap_or(false)
            });
        }

        // Filter by SCD patterns
        if let Some(patterns) = scd_patterns {
            let pattern_set: HashSet<&str> = patterns.iter().map(|s| s.as_str()).collect();
            filtered.retain(|t| {
                t.scd_pattern
                    .as_ref()
                    .map(|p| format!("{:?}", p))
                    .map(|s| pattern_set.contains(s.as_str()))
                    .unwrap_or(false)
            });
        }

        // Filter by Data Vault classifications
        if let Some(classifications) = data_vault_classifications {
            let class_set: HashSet<&str> = classifications.iter().map(|s| s.as_str()).collect();
            filtered.retain(|t| {
                t.data_vault_classification
                    .as_ref()
                    .map(|c| format!("{:?}", c))
                    .map(|s| class_set.contains(s.as_str()))
                    .unwrap_or(false)
            });
        }

        let result: Vec<Table> = filtered.into_iter().cloned().collect();
        info!(
            "Filtered {} tables to {} tables",
            model.tables.len(),
            result.len()
        );
        result
    }

    /// Get list of modeling levels present in the model.
    pub fn get_available_modeling_levels(&self) -> Vec<String> {
        let model = match &self.model {
            Some(m) => m,
            None => return Vec::new(),
        };

        let mut levels = HashSet::new();
        for table in &model.tables {
            if let Some(level) = &table.modeling_level {
                levels.insert(format!("{:?}", level));
            }
        }

        let mut result: Vec<String> = levels.into_iter().collect();
        result.sort();
        result
    }

    /// Get list of medallion layers present in the model.
    pub fn get_available_medallion_layers(&self) -> Vec<String> {
        let model = match &self.model {
            Some(m) => m,
            None => return Vec::new(),
        };

        let mut layers = HashSet::new();
        for table in &model.tables {
            for layer in &table.medallion_layers {
                layers.insert(format!("{:?}", layer));
            }
        }

        let mut result: Vec<String> = layers.into_iter().collect();
        result.sort();
        result
    }

    /// Get count of tables by modeling level.
    pub fn get_table_count_by_level(&self) -> HashMap<String, usize> {
        let model = match &self.model {
            Some(m) => m,
            None => return HashMap::new(),
        };

        let mut counts = HashMap::new();
        for table in &model.tables {
            let level = table
                .modeling_level
                .as_ref()
                .map(|l| format!("{:?}", l))
                .unwrap_or_else(|| "none".to_string());
            *counts.entry(level).or_insert(0) += 1;
        }

        counts
    }

    /// Get count of tables by medallion layer.
    pub fn get_table_count_by_layer(&self) -> HashMap<String, usize> {
        let model = match &self.model {
            Some(m) => m,
            None => return HashMap::new(),
        };

        let mut counts = HashMap::new();
        for table in &model.tables {
            if table.medallion_layers.is_empty() {
                *counts.entry("none".to_string()).or_insert(0) += 1;
            } else {
                for layer in &table.medallion_layers {
                    let layer_name = format!("{:?}", layer);
                    *counts.entry(layer_name).or_insert(0) += 1;
                }
            }
        }

        counts
    }
}
