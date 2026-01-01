//! Model service for managing data models and table operations.

use crate::models::{DataModel, Table};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

/// Service for managing data models.
pub struct ModelService {
    /// Current active model
    current_model: Option<DataModel>,
    // Git service for auto-saving (optional, will be added later)
    // git_service: Option<Box<dyn GitService>>,
}

impl ModelService {
    /// Create a new model service instance.
    pub fn new() -> Self {
        Self {
            current_model: None,
        }
    }

    /// Create a new data model.
    #[allow(dead_code)]
    pub fn create_model(
        &mut self,
        name: String,
        git_directory_path: PathBuf,
        description: Option<String>,
    ) -> Result<DataModel> {
        // Clear any existing tables directory to ensure clean start
        let tables_dir = git_directory_path.join("tables");
        if tables_dir.exists() {
            // Remove old YAML files to prevent loading stale data
            if let Ok(entries) = std::fs::read_dir(&tables_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "yaml" || ext == "yml")
                        .unwrap_or(false)
                        && let Err(e) = std::fs::remove_file(&path)
                    {
                        warn!("Failed to remove old table file {:?}: {}", path, e);
                    }
                }
            }
        }

        let control_file_path = git_directory_path.join("relationships.yaml");
        let model = DataModel {
            id: Uuid::new_v4(),
            name,
            description,
            git_directory_path: git_directory_path.to_string_lossy().to_string(),
            control_file_path: control_file_path.to_string_lossy().to_string(),
            tables: Vec::new(),
            relationships: Vec::new(),
            diagram_file_path: None,
            is_subfolder: false,
            parent_git_directory: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Load DrawIO XML if it exists
        let mut model = model;
        if let Err(e) = Self::load_canvas_layout(&mut model, &git_directory_path) {
            warn!("Failed to load DrawIO XML: {}", e);
        }

        self.current_model = Some(model.clone());
        info!("Created model: {} at {:?}", model.name, git_directory_path);
        Ok(model)
    }

    /// Load or create a model, loading existing tables from YAML files if they exist.
    /// Delegates YAML I/O to GitService to avoid code duplication.
    ///
    /// If `force_reload` is true, always reloads from disk even if model is already loaded.
    pub fn load_or_create_model(
        &mut self,
        name: String,
        git_directory_path: PathBuf,
        description: Option<String>,
    ) -> Result<DataModel> {
        self.load_or_create_model_with_reload(name, git_directory_path, description, false)
    }

    /// Load or create a model with option to force reload.
    pub fn load_or_create_model_with_reload(
        &mut self,
        name: String,
        git_directory_path: PathBuf,
        description: Option<String>,
        force_reload: bool,
    ) -> Result<DataModel> {
        // Check if we already have a model loaded for this path and don't need to force reload
        // Normalize paths for comparison (try canonicalize, fallback to string comparison)
        // Use both canonicalized and non-canonicalized paths for comparison to handle edge cases
        let normalized_path = git_directory_path
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| git_directory_path.to_string_lossy().to_string());
        let path_str = git_directory_path.to_string_lossy().to_string();

        if !force_reload && let Some(ref current_model) = self.current_model {
            // Normalize the stored path for comparison
            let stored_path_normalized = std::path::Path::new(&current_model.git_directory_path)
                .canonicalize()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| current_model.git_directory_path.clone());
            let stored_path_str = current_model.git_directory_path.clone();

            // Compare both canonicalized and non-canonicalized paths
            if stored_path_normalized == normalized_path || stored_path_str == path_str {
                info!(
                    "Model already loaded for path {:?}, skipping reload",
                    git_directory_path
                );
                return Ok(current_model.clone());
            }
        }

        use crate::services::git_service::GitService;

        // Use GitService to load model from YAML (handles all YAML I/O)
        let mut git_service = GitService::new();
        let model = match git_service.map_git_directory(&git_directory_path) {
            Ok((mut loaded_model, orphaned_relationships)) => {
                info!(
                    "[ModelService] Loaded model from Git directory: {} tables, {} relationships, {} orphaned",
                    loaded_model.tables.len(),
                    loaded_model.relationships.len(),
                    orphaned_relationships.len()
                );
                // Log orphaned relationships if any
                if !orphaned_relationships.is_empty() {
                    warn!(
                        "[ModelService] Found {} orphaned relationships during load (will be handled by frontend)",
                        orphaned_relationships.len()
                    );
                }
                // Update model name and description if provided
                loaded_model.name = name;
                loaded_model.description = description;
                loaded_model
            }
            Err(e) => {
                // If loading fails, create empty model
                warn!(
                    "Failed to load model from Git directory, creating empty model: {}",
                    e
                );
                let control_file_path = git_directory_path.join("relationships.yaml");
                let diagram_file_path = git_directory_path.join("diagram.drawio");
                DataModel {
                    id: Uuid::new_v4(),
                    name,
                    description,
                    git_directory_path: git_directory_path.to_string_lossy().to_string(),
                    control_file_path: control_file_path.to_string_lossy().to_string(),
                    tables: Vec::new(),
                    relationships: Vec::new(),
                    diagram_file_path: Some(diagram_file_path.to_string_lossy().to_string()),
                    is_subfolder: false,
                    parent_git_directory: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }
            }
        };

        // Load DrawIO XML if it exists (this will load table positions)
        let mut model = model;
        if let Err(e) = Self::load_canvas_layout(&mut model, &git_directory_path) {
            warn!("Failed to load DrawIO XML: {}", e);
        } else {
            info!(
                "DrawIO XML loaded successfully, {} tables have positions",
                model.tables.iter().filter(|t| t.position.is_some()).count()
            );
        }

        self.current_model = Some(model.clone());
        info!(
            "[ModelService] Stored model in current_model: {} at {:?} with {} tables and {} relationships",
            model.name,
            git_directory_path,
            model.tables.len(),
            model.relationships.len()
        );

        // Verify relationships are actually stored
        if let Some(stored_model) = &self.current_model {
            info!(
                "[ModelService] Verification: stored model has {} tables and {} relationships",
                stored_model.tables.len(),
                stored_model.relationships.len()
            );
        }

        Ok(model)
    }

    /// Add a table to the current model. Requires workspace to be created first.
    pub fn add_table(&mut self, table: Table) -> Result<Table> {
        if self.current_model.is_none() {
            // No workspace created - user must create workspace first via /workspace/create
            return Err(anyhow::anyhow!(
                "No workspace available. Please create a workspace first by providing your email address."
            ));
        }

        let model = self
            .current_model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model available"))?;

        // Check for uniqueness conflicts using unique key
        let database_type_str = table.database_type.as_ref().map(|dt| format!("{:?}", dt));
        info!(
            "[ModelService] Adding table '{}' with database_type: {:?}",
            table.name, database_type_str
        );

        if let Some(_existing) = model.get_table_by_unique_key(
            database_type_str.as_deref(),
            &table.name,
            table.catalog_name.as_deref(),
            table.schema_name.as_deref(),
        ) {
            let mut conflict_parts = vec![table.name.clone()];
            if let Some(ref dt) = table.database_type {
                conflict_parts.push(format!("database_type={:?}", dt));
            }
            if let Some(ref catalog) = table.catalog_name {
                conflict_parts.push(format!("catalog={}", catalog));
            }
            if let Some(ref schema) = table.schema_name {
                conflict_parts.push(format!("schema={}", schema));
            }
            return Err(anyhow::anyhow!(
                "Table with unique key ({}) already exists",
                conflict_parts.join(", ")
            ));
        }

        // Assign default position if table doesn't have one
        let mut table_with_position = table.clone();
        if table_with_position.position.is_none() {
            // Calculate default position based on existing tables to avoid overlap
            let existing_count = model.tables.len();
            let default_x = 100.0 + ((existing_count % 5) as f64 * 450.0);
            let default_y = 100.0 + ((existing_count / 5) as f64 * 400.0);
            table_with_position.position = Some(crate::models::Position {
                x: default_x,
                y: default_y,
            });
            info!(
                "Assigned default position ({}, {}) to table '{}'",
                default_x, default_y, table_with_position.name
            );
        }

        model.tables.push(table_with_position.clone());

        // Auto-save canvas layout when table is added (includes position)
        let git_path = PathBuf::from(&model.git_directory_path);
        if let Err(e) = Self::save_canvas_layout(model, &git_path) {
            warn!("Failed to auto-save canvas layout: {}", e);
        }

        // Auto-save table to YAML file
        if let Err(e) = Self::save_table_to_yaml(&table_with_position, &git_path) {
            warn!(
                "Failed to auto-save table {} to YAML: {}",
                table_with_position.name, e
            );
        }

        info!("Added table: {}", table_with_position.name);

        Ok(table_with_position)
    }

    /// Get a table by ID.
    pub fn get_table(&self, table_id: Uuid) -> Option<&Table> {
        self.current_model.as_ref()?.get_table_by_id(table_id)
    }

    /// Get a table by name (legacy method - use get_table_by_unique_key for proper uniqueness).
    #[allow(dead_code)]
    pub fn get_table_by_name(&self, name: &str) -> Option<&Table> {
        self.current_model.as_ref()?.get_table_by_name(name)
    }

    /// Get a table by unique key.
    #[allow(dead_code)]
    pub fn get_table_by_unique_key(
        &self,
        database_type: Option<&str>,
        name: &str,
        catalog_name: Option<&str>,
        schema_name: Option<&str>,
    ) -> Option<&Table> {
        self.current_model.as_ref()?.get_table_by_unique_key(
            database_type,
            name,
            catalog_name,
            schema_name,
        )
    }

    /// Update a table.
    pub fn update_table(
        &mut self,
        table_id: Uuid,
        updates: &serde_json::Value,
    ) -> Result<Option<Table>> {
        use crate::models::enums::{
            DataVaultClassification, DatabaseType, MedallionLayer, SCDPattern,
        };

        // Model must exist - workspace should be created first
        if self.current_model.is_none() {
            return Err(anyhow::anyhow!(
                "No workspace available. Please create a workspace first by providing your email address."
            ));
        }

        let model = self.current_model.as_mut().ok_or_else(|| {
            // Provide more helpful error message
            anyhow::anyhow!(
                "No model available. Please import tables or load a model first. \
                    The model may have been lost if the backend restarted. \
                    Try re-importing your SQL or loading the model from the git directory."
            )
        })?;

        // Clone git directory path before mutable borrow of table
        let git_directory_path = model.git_directory_path.clone();

        // Log model state for debugging
        let table_count = model.tables.len();
        let table_ids: Vec<String> = model.tables.iter().map(|t| t.id.to_string()).collect();
        info!(
            "[ModelService] Updating table {}, model has {} tables",
            table_id, table_count
        );
        if table_count == 0 {
            warn!("[ModelService] Model has no tables! Available table IDs: []");
        } else {
            info!("[ModelService] Available table IDs: {:?}", table_ids);
        }

        let table = model.get_table_by_id_mut(table_id).ok_or_else(|| {
            anyhow::anyhow!(
                "Table {} not found in model. Model has {} tables. Available IDs: {:?}",
                table_id,
                table_count,
                table_ids
            )
        })?;

        // Apply updates from JSON
        if let Some(obj) = updates.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "name" => {
                        if let Some(s) = value.as_str() {
                            table.name = s.trim().to_string();
                        }
                    }
                    "catalog_name" => {
                        if value.is_null() {
                            table.catalog_name = None;
                        } else if let Some(s) = value.as_str() {
                            table.catalog_name = if s.trim().is_empty() {
                                None
                            } else {
                                Some(s.trim().to_string())
                            };
                        }
                    }
                    "schema_name" => {
                        if value.is_null() {
                            table.schema_name = None;
                        } else if let Some(s) = value.as_str() {
                            table.schema_name = if s.trim().is_empty() {
                                None
                            } else {
                                Some(s.trim().to_string())
                            };
                        }
                    }
                    "database_type" => {
                        let old_db_type = table.database_type.map(|dt| format!("{:?}", dt));
                        if value.is_null() {
                            info!(
                                "[ModelService] Setting database_type to None for table '{}' (was: {:?})",
                                table.name, old_db_type
                            );
                            table.database_type = None;
                        } else if let Some(s) = value.as_str() {
                            let new_db_type = match s.to_uppercase().as_str() {
                                "POSTGRES" | "POSTGRESQL" => Some(DatabaseType::Postgres),
                                "MYSQL" => Some(DatabaseType::Mysql),
                                "SQL_SERVER" | "SQLSERVER" => Some(DatabaseType::SqlServer),
                                "DATABRICKS" | "DATABRICKS_DELTA" => {
                                    Some(DatabaseType::DatabricksDelta)
                                }
                                "AWS_GLUE" | "GLUE" => Some(DatabaseType::AwsGlue),
                                _ => {
                                    warn!(
                                        "[ModelService] Unknown database_type value '{}' for table '{}'",
                                        s, table.name
                                    );
                                    None
                                }
                            };
                            if let Some(ref new_type) = new_db_type {
                                info!(
                                    "[ModelService] Updated database_type for table '{}': {:?} -> {:?}",
                                    table.name,
                                    old_db_type,
                                    format!("{:?}", new_type)
                                );
                            }
                            table.database_type = new_db_type;
                        }
                    }
                    "medallion_layers" => {
                        if let Some(arr) = value.as_array() {
                            table.medallion_layers = arr
                                .iter()
                                .filter_map(|v| {
                                    v.as_str().and_then(|s| match s.to_lowercase().as_str() {
                                        "bronze" => Some(MedallionLayer::Bronze),
                                        "silver" => Some(MedallionLayer::Silver),
                                        "gold" => Some(MedallionLayer::Gold),
                                        "operational" => Some(MedallionLayer::Operational),
                                        _ => None,
                                    })
                                })
                                .collect();
                        }
                    }
                    "scd_pattern" => {
                        if value.is_null() {
                            table.scd_pattern = None;
                        } else if let Some(s) = value.as_str() {
                            table.scd_pattern = match s.to_uppercase().as_str() {
                                "TYPE_1" => Some(SCDPattern::Type1),
                                "TYPE_2" => Some(SCDPattern::Type2),
                                _ => None,
                            };
                        }
                    }
                    "data_vault_classification" => {
                        if value.is_null() {
                            table.data_vault_classification = None;
                        } else if let Some(s) = value.as_str() {
                            table.data_vault_classification = match s.to_uppercase().as_str() {
                                "HUB" => Some(DataVaultClassification::Hub),
                                "LINK" => Some(DataVaultClassification::Link),
                                "SATELLITE" => Some(DataVaultClassification::Satellite),
                                _ => None,
                            };
                        }
                    }
                    "tags" => {
                        if let Some(arr) = value.as_array() {
                            table.tags = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                        }
                    }
                    "columns" => {
                        if let Some(arr) = value.as_array() {
                            // Deserialize columns
                            let mut updated_columns = Vec::new();
                            let mut deserialization_errors = Vec::new();
                            for (idx, col_val) in arr.iter().enumerate() {
                                match serde_json::from_value::<crate::models::Column>(
                                    col_val.clone(),
                                ) {
                                    Ok(mut col) => {
                                        col.column_order = idx as i32;
                                        updated_columns.push(col);
                                    }
                                    Err(e) => {
                                        let col_name = col_val
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| format!("column[{}]", idx));
                                        warn!(
                                            "Failed to deserialize column '{}' (index {}): {}",
                                            col_name, idx, e
                                        );
                                        deserialization_errors
                                            .push(format!("Column '{}': {}", col_name, e));
                                    }
                                }
                            }
                            if !updated_columns.is_empty() {
                                if !deserialization_errors.is_empty() {
                                    warn!(
                                        "Some columns failed to deserialize: {:?}",
                                        deserialization_errors
                                    );
                                }
                                info!(
                                    "Updating {} columns ({} total in request)",
                                    updated_columns.len(),
                                    arr.len()
                                );
                                table.columns = updated_columns;
                            } else {
                                warn!(
                                    "No columns could be deserialized. Errors: {:?}",
                                    deserialization_errors
                                );
                            }
                        }
                    }
                    "quality" => {
                        if let Some(arr) = value.as_array() {
                            table.quality = arr
                                .iter()
                                .filter_map(|v| {
                                    if let Some(obj) = v.as_object() {
                                        let mut map = HashMap::new();
                                        for (k, val) in obj {
                                            map.insert(k.clone(), val.clone());
                                        }
                                        Some(map)
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                        }
                    }
                    "odcl_metadata" => {
                        if let Some(obj) = value.as_object() {
                            // Merge with existing metadata
                            for (k, v) in obj {
                                table.odcl_metadata.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    "position" => {
                        if let Some(pos_obj) = value.as_object() {
                            if let (Some(x), Some(y)) = (
                                pos_obj.get("x").and_then(|v| v.as_f64()),
                                pos_obj.get("y").and_then(|v| v.as_f64()),
                            ) {
                                table.position = Some(crate::models::Position { x, y });
                            }
                        } else if value.is_null() {
                            table.position = None;
                        }
                    }
                    _ => {
                        // Store unknown fields in odcl_metadata
                        table.odcl_metadata.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        table.updated_at = chrono::Utc::now();
        info!("Updated table: {}", table.name);

        // Clone table before releasing mutable borrow
        let table_clone = table.clone();

        // Release mutable borrow of model
        let _ = model; // Release mutable borrow

        // Auto-save DrawIO XML when table is updated (after mutable borrow is released)
        let git_path = std::path::PathBuf::from(&git_directory_path);
        if !git_directory_path.is_empty() {
            // Get immutable reference to model for saving
            if let Some(model_ref) = self.current_model.as_ref()
                && let Err(e) = Self::save_canvas_layout(model_ref, &git_path)
            {
                warn!("Failed to auto-save DrawIO XML: {}", e);
            }

            // Auto-save table to YAML file
            if let Err(e) = Self::save_table_to_yaml(&table_clone, &git_path) {
                warn!(
                    "Failed to auto-save table {} to YAML: {}",
                    table_clone.name, e
                );
            }
        }

        Ok(Some(table_clone))
    }

    /// Delete a table.
    /// Also deletes all relationships associated with the table (cascade delete).
    pub fn delete_table(&mut self, table_id: Uuid) -> Result<bool> {
        let model = self
            .current_model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model available"))?;

        // Extract table name before deletion
        let table_name = model
            .get_table_by_id(table_id)
            .map(|t| t.name.clone())
            .ok_or_else(|| anyhow::anyhow!("Table not found"))?;

        // Get relationships involving this table before deletion
        let relationships_to_delete: Vec<Uuid> = model
            .relationships
            .iter()
            .filter(|r| r.source_table_id == table_id || r.target_table_id == table_id)
            .map(|r| r.id)
            .collect();

        // Delete all relationships associated with this table (cascade delete)
        if !relationships_to_delete.is_empty() {
            let initial_len = model.relationships.len();
            model
                .relationships
                .retain(|r| r.source_table_id != table_id && r.target_table_id != table_id);
            let deleted_count = initial_len - model.relationships.len();
            info!(
                "Deleted {} relationship(s) associated with table '{}'",
                deleted_count, table_name
            );
        }

        // Delete the table
        model.tables.retain(|t| t.id != table_id);
        info!("Deleted table: {}", table_name);
        Ok(true)
    }

    /// Detect naming conflicts between new tables and existing tables using unique keys.
    pub fn detect_naming_conflicts(&self, new_tables: &[Table]) -> Vec<(Table, Table)> {
        let model = match &self.current_model {
            Some(m) => m,
            None => return Vec::new(),
        };

        let mut conflicts = Vec::new();
        for new_table in new_tables {
            let database_type_str = new_table
                .database_type
                .as_ref()
                .map(|dt| format!("{:?}", dt));

            if let Some(existing) = model.get_table_by_unique_key(
                database_type_str.as_deref(),
                &new_table.name,
                new_table.catalog_name.as_deref(),
                new_table.schema_name.as_deref(),
            ) {
                conflicts.push((new_table.clone(), existing.clone()));
            }
        }

        conflicts
    }

    /// Rename a table to resolve naming conflict.
    #[allow(dead_code)]
    pub fn resolve_naming_conflict(&mut self, table_id: Uuid, new_name: String) -> Result<Table> {
        let model = self
            .current_model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model available"))?;

        let table = model
            .get_table_by_id_mut(table_id)
            .ok_or_else(|| anyhow::anyhow!("Table not found"))?;

        table.name = new_name.clone();
        table.updated_at = chrono::Utc::now();
        info!("Renamed table to: {}", new_name);
        Ok(table.clone())
    }

    /// Get the current model.
    /// If no model exists, tries to reload from temp directories.
    pub fn get_current_model(&self) -> Option<&DataModel> {
        self.current_model.as_ref()
    }

    /// Ensure a model is available.
    /// Returns error if no workspace has been created - user must create workspace first.
    pub fn ensure_model_available(&mut self) -> Result<()> {
        if self.current_model.is_none() {
            Err(anyhow::anyhow!(
                "No workspace available. Please create a workspace first by providing your email address."
            ))
        } else {
            if let Some(model) = self.current_model.as_ref() {
                info!(
                    "[ModelService] Model already available with {} tables",
                    model.tables.len()
                );
            }
            Ok(())
        }
    }

    /// Get mutable reference to current model.
    pub fn get_current_model_mut(&mut self) -> Option<&mut DataModel> {
        self.current_model.as_mut()
    }

    /// Set the current model.
    #[allow(dead_code)]
    pub fn set_current_model(&mut self, model: DataModel) {
        self.current_model = Some(model);
    }

    /// Clear the current model (reset to empty state).
    #[allow(dead_code)]
    pub fn clear_model(&mut self) {
        self.current_model = None;
        info!("Model state cleared");
    }

    /// Load canvas layout from YAML (loads positions and routing).
    /// Also migrates from DrawIO XML if canvas-layout.yaml doesn't exist but diagram.drawio does.
    fn load_canvas_layout(model: &mut DataModel, git_directory_path: &PathBuf) -> Result<()> {
        use crate::services::canvas_layout_service::CanvasLayoutService;
        use crate::services::drawio_service::DrawIOService;
        use std::path::Path;

        let canvas_layout_service = CanvasLayoutService::new(Path::new(git_directory_path));
        let canvas_layout_path = git_directory_path.join("canvas-layout.yaml");
        let drawio_path = git_directory_path.join("diagram.drawio");

        // Check if we need to migrate from DrawIO XML
        if !canvas_layout_path.exists() && drawio_path.exists() {
            info!("Migrating canvas layout from DrawIO XML to YAML format");
            let drawio_service = DrawIOService::new(Path::new(git_directory_path));
            if let Err(e) = canvas_layout_service.migrate_from_drawio(model, &drawio_service) {
                warn!("Failed to migrate from DrawIO XML: {}", e);
                // Continue with loading from YAML (which will be empty)
            }
        }

        // Load from YAML (will be empty if file doesn't exist, which is OK)
        if let Err(e) = canvas_layout_service.load_canvas_layout(model) {
            warn!("Failed to load canvas layout from YAML: {}", e);
        }

        Ok(())
    }

    /// Save canvas layout to YAML (saves positions and routing).
    fn save_canvas_layout(model: &DataModel, git_directory_path: &PathBuf) -> Result<()> {
        use crate::services::canvas_layout_service::CanvasLayoutService;
        use std::path::Path;

        let canvas_layout_service = CanvasLayoutService::new(Path::new(git_directory_path));
        if let Err(e) = canvas_layout_service.save_canvas_layout(model) {
            warn!("Failed to save canvas layout to YAML: {}", e);
        }
        Ok(())
    }

    /// Save a table to YAML file in the git directory.
    fn save_table_to_yaml(table: &Table, git_directory_path: &Path) -> Result<()> {
        use crate::export::ODCSExporter;
        use std::fs;

        let tables_dir = git_directory_path.join("tables");
        fs::create_dir_all(&tables_dir)
            .with_context(|| format!("Failed to create tables directory: {:?}", tables_dir))?;

        let yaml_file = tables_dir.join(format!("{}.yaml", table.name));

        // Export table to ODCS YAML format (ODCS v3.1.0)
        let yaml_content = ODCSExporter::export_table(table, "odcs_v3_1_0");

        fs::write(&yaml_file, yaml_content)
            .with_context(|| format!("Failed to write YAML file: {:?}", yaml_file))?;

        info!("Saved table {} to {:?}", table.name, yaml_file);
        Ok(())
    }

    /// Add a table to the model even if it has errors (bypasses conflict checks).
    /// This is used when importing tables with errors that should still be saved.
    /// Requires workspace to be created first.
    pub fn add_table_with_errors(&mut self, mut table: Table) -> Result<Table> {
        if self.current_model.is_none() {
            // No workspace created - user must create workspace first
            return Err(anyhow::anyhow!(
                "No workspace available. Please create a workspace first by providing your email address."
            ));
        }

        let model = self
            .current_model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model available"))?;

        // Check for conflicts but don't fail - add error to table instead
        let database_type_str = table.database_type.as_ref().map(|dt| format!("{:?}", dt));
        if let Some(existing) = model.get_table_by_unique_key(
            database_type_str.as_deref(),
            &table.name,
            table.catalog_name.as_deref(),
            table.schema_name.as_deref(),
        ) {
            let mut conflict_parts = vec![table.name.clone()];
            if let Some(ref dt) = table.database_type {
                conflict_parts.push(format!("database_type={:?}", dt));
            }
            if let Some(ref catalog) = table.catalog_name {
                conflict_parts.push(format!("catalog={}", catalog));
            }
            if let Some(ref schema) = table.schema_name {
                conflict_parts.push(format!("schema={}", schema));
            }

            // Add conflict error to table.errors
            use std::collections::HashMap;
            let mut error_map = HashMap::new();
            error_map.insert(
                "type".to_string(),
                serde_json::Value::String("conflict".to_string()),
            );
            error_map.insert(
                "message".to_string(),
                serde_json::Value::String(format!(
                    "Table with unique key ({}) already exists",
                    conflict_parts.join(", ")
                )),
            );
            error_map.insert(
                "existing_table".to_string(),
                serde_json::Value::String(existing.name.clone()),
            );
            table.errors.push(error_map);

            warn!(
                "[ModelService] Table '{}' has conflicts but will be saved with errors",
                table.name
            );
        }

        model.tables.push(table.clone());

        // Auto-save DrawIO XML when table is added
        let git_path = PathBuf::from(&model.git_directory_path);
        if let Err(e) = Self::save_canvas_layout(model, &git_path) {
            warn!("Failed to auto-save DrawIO XML: {}", e);
        }

        // Auto-save table to YAML file
        if let Err(e) = Self::save_table_to_yaml(&table, &git_path) {
            warn!("Failed to auto-save table {} to YAML: {}", table.name, e);
        }

        info!(
            "Added table with errors: {} ({} errors)",
            table.name,
            table.errors.len()
        );

        Ok(table)
    }
}

impl Default for ModelService {
    fn default() -> Self {
        Self::new()
    }
}
