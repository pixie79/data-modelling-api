//! Git service for managing model storage in Git directories.

use crate::models::{DataModel, Relationship, Table};
use crate::services::odcs_parser::ODCSParser;
use anyhow::{Context, Result};
use git2::{Repository, RepositoryInitOptions};
use serde_yaml;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

/// Service for Git-based model storage.
pub struct GitService {
    /// Git repository instance
    repo: Option<Repository>,
    /// Git directory path
    git_directory: Option<PathBuf>,
}

impl GitService {
    /// Create a new Git service instance.
    pub fn new() -> Self {
        Self {
            repo: None,
            git_directory: None,
        }
    }

    /// Set git directory path without loading the model (for saving only).
    /// This avoids reparsing tables when we just need to save relationships.
    pub fn set_git_directory_path(&mut self, git_directory_path: &Path) -> Result<()> {
        // Validate directory exists
        if !git_directory_path.exists() {
            return Err(anyhow::anyhow!(
                "Directory does not exist: {:?}",
                git_directory_path
            ));
        }

        // Initialize or open Git repository (but don't load model)
        let repo = match Repository::open(git_directory_path) {
            Ok(repo) => repo,
            Err(_) => {
                // Initialize new Git repository if it doesn't exist
                let mut opts = RepositoryInitOptions::new();
                opts.bare(false);
                let repo = Repository::init_opts(git_directory_path, &opts).with_context(|| {
                    format!(
                        "Failed to initialize Git repository at {:?}",
                        git_directory_path
                    )
                })?;
                info!("Initialized new Git repository at {:?}", git_directory_path);
                repo
            }
        };

        self.repo = Some(repo);
        self.git_directory = Some(git_directory_path.to_path_buf());
        Ok(())
    }

    /// Map a Git directory and load existing model.
    ///
    /// Supports both root and subfolder paths for focus areas.
    /// Returns the model and a list of orphaned relationships (relationships referencing non-existent tables).
    pub fn map_git_directory(&mut self, git_directory_path: &Path) -> Result<(DataModel, Vec<Relationship>)> {
        // Validate directory exists
        if !git_directory_path.exists() {
            return Err(anyhow::anyhow!(
                "Directory does not exist: {:?}",
                git_directory_path
            ));
        }

        // Initialize or open Git repository
        let repo = match Repository::open(git_directory_path) {
            Ok(repo) => repo,
            Err(_) => {
                // Initialize new Git repository if it doesn't exist
                let mut opts = RepositoryInitOptions::new();
                opts.bare(false);
                let repo = Repository::init_opts(git_directory_path, &opts).with_context(|| {
                    format!(
                        "Failed to initialize Git repository at {:?}",
                        git_directory_path
                    )
                })?;
                info!("Initialized new Git repository at {:?}", git_directory_path);
                repo
            }
        };

        self.repo = Some(repo);
        self.git_directory = Some(git_directory_path.to_path_buf());

        // Load existing model from YAML files
        let (model, orphaned_relationships) = self.load_model_from_yaml()?;

        info!("Mapped Git directory: {:?}", git_directory_path);
        Ok((model, orphaned_relationships))
    }

    /// List subfolders in Git directory that represent focus areas.
    ///
    /// Each subfolder should contain tables/, relationships.yaml, and diagram.drawio.
    pub fn list_subfolders(&self) -> Result<Vec<String>> {
        let git_dir = self
            .git_directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Git directory not mapped"))?;

        let mut subfolders = Vec::new();

        if let Ok(entries) = fs::read_dir(git_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if it looks like a focus area (has tables/ directory)
                    let tables_dir = path.join("tables");
                    if tables_dir.exists() && tables_dir.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            subfolders.push(name.to_string());
                        }
                    }
                }
            }
        }

        Ok(subfolders)
    }

    /// Load model from YAML files in Git directory.
    /// Returns the model and a list of orphaned relationships (relationships referencing non-existent tables).
    fn load_model_from_yaml(&self) -> Result<(DataModel, Vec<Relationship>)> {
        let git_dir = self
            .git_directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Git directory not mapped"))?;

        let tables_dir = git_dir.join("tables");
        let control_file = git_dir.join("relationships.yaml");

        // Create tables directory if it doesn't exist
        if !tables_dir.exists() {
            fs::create_dir_all(&tables_dir)
                .with_context(|| format!("Failed to create tables directory: {:?}", tables_dir))?;
        }

        // Load tables from individual YAML files
        let mut tables = Vec::new();
        if tables_dir.exists() {
            if let Ok(entries) = fs::read_dir(&tables_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "yaml" || ext == "yml")
                        .unwrap_or(false)
                    {
                        match self.load_table_from_yaml(&path) {
                            Ok((mut table, uuid_was_generated)) => {
                                table.yaml_file_path = Some(
                                    path.strip_prefix(git_dir)
                                        .unwrap_or(&path)
                                        .to_string_lossy()
                                        .to_string(),
                                );
                                
                                // If a new UUID was generated, save it back to the YAML file
                                if uuid_was_generated {
                                    info!(
                                        "[GitService] Table '{}' got a new UUID: {}, saving back to YAML file",
                                        table.name,
                                        table.id
                                    );
                                    // Save the table back to update the YAML file with the new UUID
                                    if let Err(e) = self.save_table_to_yaml(&table) {
                                        warn!(
                                            "[GitService] Failed to save updated UUID for table '{}' to {:?}: {}",
                                            table.name, path, e
                                        );
                                    } else {
                                        info!(
                                            "[GitService] Successfully updated UUID in YAML file for table '{}'",
                                            table.name
                                        );
                                    }
                                }
                                
                                info!(
                                    "[GitService] Loaded table '{}' with UUID: {} from {:?}",
                                    table.name,
                                    table.id,
                                    path
                                );
                                tables.push(table);
                            }
                            Err(e) => {
                                warn!("Failed to load table from {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        // Build set of table IDs for validation
        let table_ids: std::collections::HashSet<Uuid> = tables.iter().map(|t| t.id).collect();
        
        info!(
            "[GitService] Loaded {} tables with IDs: {:?}",
            table_ids.len(),
            table_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
        );

        // Load relationships from control file
        let mut all_relationships = Vec::new();
        let mut relationship_table_names: HashMap<Uuid, (Option<String>, Option<String>)> = HashMap::new();
        
        if control_file.exists() {
            info!("[GitService] Reading relationships file: {:?}", control_file);
            
            // Read YAML file to extract both relationships and table names
            if let Ok(yaml_content) = fs::read_to_string(&control_file) {
                if let Ok(data) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
                    // Extract relationships array
                    let rels_array = data.get("relationships").and_then(|v| v.as_sequence())
                        .or_else(|| data.as_sequence());
                    
                    if let Some(rels_array) = rels_array {
                        for rel_data in rels_array {
                            // Extract table names before parsing (for UUID fixing)
                            let rel_id = rel_data.get("id")
                                .and_then(|v| v.as_str())
                                .and_then(|s| uuid::Uuid::parse_str(s).ok());
                            
                            if let Some(id) = rel_id {
                                let source_name = rel_data.get("source_table_name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let target_name = rel_data.get("target_table_name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                relationship_table_names.insert(id, (source_name, target_name));
                            }
                            
                            // Parse relationship
                            match self.parse_relationship(rel_data) {
                                Ok(rel) => {
                                    all_relationships.push(rel);
                                },
                                Err(e) => {
                                    warn!("[GitService] Failed to parse relationship: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            
            if !all_relationships.is_empty() {
                info!(
                    "[GitService] Loaded {} relationships from {:?}",
                    all_relationships.len(),
                    control_file
                );
                info!(
                    "[GitService] Relationship details: {:?}",
                    all_relationships.iter().map(|r| format!(
                        "id={}, source={}, target={}",
                        r.id, r.source_table_id, r.target_table_id
                    )).collect::<Vec<_>>()
                );
            }
        } else {
            info!("[GitService] Relationships file does not exist: {:?}", control_file);
        }

        // Build a map of table names to UUIDs for matching relationships by name
        let table_name_to_id: HashMap<String, Uuid> = tables
            .iter()
            .map(|t| (t.name.clone(), t.id))
            .collect();

        // Separate valid and orphaned relationships
        let mut valid_relationships = Vec::new();
        let mut orphaned_relationships = Vec::new();
        let total_relationships = all_relationships.len();

        for mut rel in all_relationships {
            let mut source_exists = table_ids.contains(&rel.source_table_id);
            let mut target_exists = table_ids.contains(&rel.target_table_id);
            
            // If UUIDs don't match, try to match by table name (from relationships.yaml metadata)
            // This handles the case where tables got new UUIDs but relationships still reference old ones
            if !source_exists || !target_exists {
                let mut updated = false;
                
                // Get table names for this relationship
                if let Some((source_name_opt, target_name_opt)) = relationship_table_names.get(&rel.id) {
                    // Try to fix source table ID by name
                    if !source_exists {
                        if let Some(source_name) = source_name_opt {
                            if let Some(&correct_source_id) = table_name_to_id.get(source_name) {
                                info!(
                                    "[GitService] Fixing relationship {}: updating source_table_id from {} to {} (matched by name: {})",
                                    rel.id, rel.source_table_id, correct_source_id, source_name
                                );
                                rel.source_table_id = correct_source_id;
                                source_exists = true;
                                updated = true;
                            }
                        }
                    }
                    
                    // Try to fix target table ID by name
                    if !target_exists {
                        if let Some(target_name) = target_name_opt {
                            if let Some(&correct_target_id) = table_name_to_id.get(target_name) {
                                info!(
                                    "[GitService] Fixing relationship {}: updating target_table_id from {} to {} (matched by name: {})",
                                    rel.id, rel.target_table_id, correct_target_id, target_name
                                );
                                rel.target_table_id = correct_target_id;
                                target_exists = true;
                                updated = true;
                            }
                        }
                    }
                }
                
                if updated {
                    info!(
                        "[GitService] Successfully fixed relationship {} UUIDs by matching table names",
                        rel.id
                    );
                }
            }

            if source_exists && target_exists {
                valid_relationships.push(rel);
            } else {
                warn!(
                    "[GitService] Found orphaned relationship {}: source_table_id={} (exists: {}), target_table_id={} (exists: {})",
                    rel.id,
                    rel.source_table_id,
                    source_exists,
                    rel.target_table_id,
                    target_exists
                );
                orphaned_relationships.push(rel);
            }
        }
        
        info!(
            "[GitService] Relationship validation: {} valid, {} orphaned out of {} total",
            valid_relationships.len(),
            orphaned_relationships.len(),
            total_relationships
        );

        if !orphaned_relationships.is_empty() {
            info!(
                "[GitService] Found {} orphaned relationships (referencing non-existent tables)",
                orphaned_relationships.len()
            );
        }

        // Create or load model metadata
        let model_name = git_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Data Model")
            .to_string();

        let diagram_file = git_dir.join("diagram.drawio");
        let model = DataModel {
            id: Uuid::new_v4(),
            name: model_name,
            description: None,
            git_directory_path: git_dir.to_string_lossy().to_string(),
            control_file_path: control_file.to_string_lossy().to_string(),
            diagram_file_path: Some(diagram_file.to_string_lossy().to_string()),
            is_subfolder: false, // Will be determined based on path analysis
            parent_git_directory: None,
            tables,
            relationships: valid_relationships,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        Ok((model, orphaned_relationships))
    }

    /// Load a table from YAML file.
    /// Returns (Table, uuid_was_generated) where uuid_was_generated is true if a new UUID was assigned.
    fn load_table_from_yaml(&self, yaml_path: &Path) -> Result<(Table, bool)> {
        let yaml_content = fs::read_to_string(yaml_path)
            .with_context(|| format!("Failed to read YAML file: {:?}", yaml_path))?;

        // Check if the YAML file has a valid UUID before parsing
        // Parse the YAML and check if the id field contains a valid UUID
        let had_valid_uuid = {
            if let Ok(data) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
                // Check top-level id field (ODCS v3.1.0 format)
                if let Some(serde_yaml::Value::String(id_str)) = data.get("id") {
                    uuid::Uuid::parse_str(id_str).is_ok()
                } else {
                    // Check legacy formats (customProperties or odcl_metadata)
                    // For simplicity, we'll assume no valid UUID if not in standard location
                    // The parser will handle legacy formats and generate UUID if needed
                    false
                }
            } else {
                false
            }
        };

        let mut parser = ODCSParser::new();
        let (table, _errors) = parser
            .parse(&yaml_content)
            .with_context(|| format!("Failed to parse YAML file: {:?}", yaml_path))?;

        // UUID was generated if the file didn't have a valid UUID
        let uuid_was_generated = !had_valid_uuid;

        Ok((table, uuid_was_generated))
    }

    /// Load relationships from YAML file.
    fn load_relationships_from_yaml(&self, yaml_path: &Path) -> Result<Vec<Relationship>> {
        let yaml_content = fs::read_to_string(yaml_path)
            .with_context(|| format!("Failed to read relationships file: {:?}", yaml_path))?;

        info!("[GitService] Read {} bytes from relationships file", yaml_content.len());

        let data: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
            .with_context(|| format!("Failed to parse relationships YAML: {:?}", yaml_path))?;

        let mut relationships = Vec::new();
        let mut parse_errors = 0;

        // Handle both formats: direct array or object with "relationships" key
        if let Some(rels_array) = data.get("relationships").and_then(|v| v.as_sequence()) {
            // Format: { relationships: [...] }
            info!("[GitService] Found relationships array with {} items", rels_array.len());
            for (idx, rel_data) in rels_array.iter().enumerate() {
                match self.parse_relationship(rel_data) {
                    Ok(rel) => {
                        relationships.push(rel);
                        info!("[GitService] Successfully parsed relationship {}: id={}", idx, relationships.last().unwrap().id);
                    },
                    Err(e) => {
                        parse_errors += 1;
                        warn!("[GitService] Failed to parse relationship {}: {}", idx, e);
                    }
                }
            }
        } else if let Some(rels_array) = data.as_sequence() {
            // Format: [...] (direct array)
            info!("[GitService] Found direct relationships array with {} items", rels_array.len());
            for (idx, rel_data) in rels_array.iter().enumerate() {
                match self.parse_relationship(rel_data) {
                    Ok(rel) => {
                        relationships.push(rel);
                        info!("[GitService] Successfully parsed relationship {}: id={}", idx, relationships.last().unwrap().id);
                    },
                    Err(e) => {
                        parse_errors += 1;
                        warn!("[GitService] Failed to parse relationship {}: {}", idx, e);
                    }
                }
            }
        } else {
            warn!("[GitService] YAML file does not contain a relationships array or object with 'relationships' key");
        }

        info!("[GitService] Parsed {} relationships from YAML ({} parse errors)", relationships.len(), parse_errors);
        Ok(relationships)
    }

    /// Parse a relationship from YAML value.
    fn parse_relationship(&self, data: &serde_yaml::Value) -> Result<Relationship> {
        use crate::models::enums::{Cardinality, RelationshipType};
        use crate::models::relationship::{ETLJobMetadata, ForeignKeyDetails};

        let id = data
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let source_table_id = data
            .get("source_table_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing source_table_id"))?;

        let target_table_id = data
            .get("target_table_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing target_table_id"))?;

        let cardinality = data
            .get("cardinality")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "OneToOne" => Some(Cardinality::OneToOne),
                "OneToMany" => Some(Cardinality::OneToMany),
                "ManyToOne" => Some(Cardinality::ManyToOne),
                "ManyToMany" => Some(Cardinality::ManyToMany),
                _ => None,
            });

        let relationship_type = data
            .get("relationship_type")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "DataFlow" => Some(RelationshipType::DataFlow),
                "Dependency" => Some(RelationshipType::Dependency),
                "ForeignKey" => Some(RelationshipType::ForeignKey),
                "EtlTransformation" => Some(RelationshipType::EtlTransformation),
                _ => None,
            });

        let foreign_key_details = data.get("foreign_key_details").and_then(|v| {
            Some(ForeignKeyDetails {
                source_column: v.get("source_column")?.as_str()?.to_string(),
                target_column: v.get("target_column")?.as_str()?.to_string(),
            })
        });

        let etl_job_metadata = data.get("etl_job_metadata").and_then(|v| {
            Some(ETLJobMetadata {
                job_name: v.get("job_name")?.as_str()?.to_string(),
                notes: v
                    .get("notes")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string()),
                frequency: v
                    .get("frequency")
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string()),
            })
        });

        // Load notes as top-level field (relationship comments)
        let notes = data
            .get("notes")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Load optional/mandatory flags (Crow's Foot notation)
        let source_optional = data
            .get("source_optional")
            .and_then(|v| v.as_bool());
        let target_optional = data
            .get("target_optional")
            .and_then(|v| v.as_bool());

        let created_at = data
            .get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        let updated_at = data
            .get("updated_at")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        Ok(Relationship {
            id,
            source_table_id,
            target_table_id,
            cardinality,
            source_optional,
            target_optional,
            foreign_key_details,
            etl_job_metadata,
            relationship_type,
            notes,
            visual_metadata: None, // Loaded separately from DrawIO XML
            drawio_edge_id: None,
            created_at,
            updated_at,
        })
    }

    /// Save a table to ODCS YAML file.
    pub fn save_table_to_yaml(&self, table: &Table) -> Result<PathBuf> {
        let git_dir = self
            .git_directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Git directory not mapped"))?;

        let tables_dir = git_dir.join("tables");
        fs::create_dir_all(&tables_dir)
            .with_context(|| format!("Failed to create tables directory: {:?}", tables_dir))?;

        let yaml_file = tables_dir.join(format!("{}.yaml", table.name));

        // Export table to ODCS YAML format (ODCS v3.1.0)
        use crate::export::ODCSExporter;
        let yaml_content = ODCSExporter::export_table(table, "odcs_v3_1_0");

        fs::write(&yaml_file, yaml_content)
            .with_context(|| format!("Failed to write YAML file: {:?}", yaml_file))?;

        info!("Saved table {} to {:?}", table.name, yaml_file);
        Ok(yaml_file)
    }

    /// Save relationships to control YAML file.
    /// Includes table names for human readability.
    pub fn save_relationships_to_yaml(
        &self,
        relationships: &[Relationship],
        tables: &[Table],
    ) -> Result<PathBuf> {
        let git_dir = self
            .git_directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Git directory not mapped"))?;

        let control_file = git_dir.join("relationships.yaml");

        // Create a lookup map for table names by ID
        let table_name_map: HashMap<Uuid, &str> = tables
            .iter()
            .map(|t| (t.id, t.name.as_str()))
            .collect();

        // Convert relationships to YAML format
        let mut rels_data = Vec::new();
        for rel in relationships {
            let mut rel_data = serde_json::Map::new();
            rel_data.insert(
                "id".to_string(),
                serde_json::Value::String(rel.id.to_string()),
            );
            rel_data.insert(
                "source_table_id".to_string(),
                serde_json::Value::String(rel.source_table_id.to_string()),
            );
            rel_data.insert(
                "target_table_id".to_string(),
                serde_json::Value::String(rel.target_table_id.to_string()),
            );

            // Add table names for human readability
            if let Some(source_name) = table_name_map.get(&rel.source_table_id) {
                rel_data.insert(
                    "source_table_name".to_string(),
                    serde_json::Value::String(source_name.to_string()),
                );
            }
            if let Some(target_name) = table_name_map.get(&rel.target_table_id) {
                rel_data.insert(
                    "target_table_name".to_string(),
                    serde_json::Value::String(target_name.to_string()),
                );
            }

            // Always save cardinality if it exists (direction of relationship)
            if let Some(card) = &rel.cardinality {
                // Use serde to serialize properly (will produce "OneToMany", "ManyToOne", etc.)
                let card_str = match card {
                    crate::models::enums::Cardinality::OneToOne => "OneToOne",
                    crate::models::enums::Cardinality::OneToMany => "OneToMany",
                    crate::models::enums::Cardinality::ManyToOne => "ManyToOne",
                    crate::models::enums::Cardinality::ManyToMany => "ManyToMany",
                };
                rel_data.insert(
                    "cardinality".to_string(),
                    serde_json::Value::String(card_str.to_string()),
                );
                info!("Saving relationship {} with cardinality: {}", rel.id, card_str);
            } else {
                warn!("Relationship {} has no cardinality - not saving cardinality field", rel.id);
            }

            // Save optional/mandatory flags (Crow's Foot notation)
            if let Some(source_opt) = rel.source_optional {
                rel_data.insert(
                    "source_optional".to_string(),
                    serde_json::Value::Bool(source_opt),
                );
            }
            if let Some(target_opt) = rel.target_optional {
                rel_data.insert(
                    "target_optional".to_string(),
                    serde_json::Value::Bool(target_opt),
                );
            }

            if let Some(ref fk) = rel.foreign_key_details {
                let mut fk_data = serde_json::Map::new();
                fk_data.insert(
                    "source_column".to_string(),
                    serde_json::Value::String(fk.source_column.clone()),
                );
                fk_data.insert(
                    "target_column".to_string(),
                    serde_json::Value::String(fk.target_column.clone()),
                );
                rel_data.insert(
                    "foreign_key_details".to_string(),
                    serde_json::Value::Object(fk_data),
                );
            }

            // Save notes/comments as top-level field
            if let Some(ref notes) = rel.notes {
                rel_data.insert(
                    "notes".to_string(),
                    serde_json::Value::String(notes.clone()),
                );
            }

            // Save etl_job_metadata (for ETL-specific fields)
            if let Some(ref etl) = rel.etl_job_metadata {
                let mut etl_data = serde_json::Map::new();
                
                // Include job_name if it's not empty
                if !etl.job_name.is_empty() {
                    etl_data.insert(
                        "job_name".to_string(),
                        serde_json::Value::String(etl.job_name.clone()),
                    );
                }
                
                // Save ETL-specific notes if they exist (separate from relationship notes)
                if let Some(ref notes) = etl.notes {
                    etl_data.insert(
                        "notes".to_string(),
                        serde_json::Value::String(notes.clone()),
                    );
                }
                
                if let Some(ref freq) = etl.frequency {
                    etl_data.insert(
                        "frequency".to_string(),
                        serde_json::Value::String(freq.clone()),
                    );
                }
                
                // Always include etl_job_metadata if it has any content
                if !etl_data.is_empty() {
                    rel_data.insert(
                        "etl_job_metadata".to_string(),
                        serde_json::Value::Object(etl_data),
                    );
                }
            }

            // Always save relationship_type if it exists
            if let Some(rt) = &rel.relationship_type {
                // Use serde to serialize properly (will produce "DataFlow", "ForeignKey", etc.)
                let rt_str = match rt {
                    crate::models::enums::RelationshipType::DataFlow => "DataFlow",
                    crate::models::enums::RelationshipType::Dependency => "Dependency",
                    crate::models::enums::RelationshipType::ForeignKey => "ForeignKey",
                    crate::models::enums::RelationshipType::EtlTransformation => "EtlTransformation",
                };
                rel_data.insert(
                    "relationship_type".to_string(),
                    serde_json::Value::String(rt_str.to_string()),
                );
            }

            rel_data.insert(
                "created_at".to_string(),
                serde_json::Value::String(rel.created_at.to_rfc3339()),
            );
            rel_data.insert(
                "updated_at".to_string(),
                serde_json::Value::String(rel.updated_at.to_rfc3339()),
            );

            rels_data.push(serde_json::Value::Object(rel_data));
        }

        let mut yaml_data = serde_json::Map::new();
        yaml_data.insert(
            "relationships".to_string(),
            serde_json::Value::Array(rels_data),
        );

        // Convert to YAML
        let yaml_content = serde_yaml::to_string(&yaml_data)
            .with_context(|| "Failed to serialize relationships to YAML")?;

        fs::write(&control_file, yaml_content)
            .with_context(|| format!("Failed to write relationships file: {:?}", control_file))?;

        info!(
            "Saved {} relationships to {:?}",
            relationships.len(),
            control_file
        );
        Ok(control_file)
    }

    /// Save DrawIO XML file.
    pub fn save_drawio_xml(&self, xml_content: &str) -> Result<PathBuf> {
        let git_dir = self
            .git_directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Git directory not mapped"))?;

        let diagram_file = git_dir.join("diagram.drawio");

        fs::write(&diagram_file, xml_content)
            .with_context(|| format!("Failed to write DrawIO XML file: {:?}", diagram_file))?;

        info!("Saved DrawIO XML to {:?}", diagram_file);
        Ok(diagram_file)
    }

    /// Auto-commit and push changes (optional, for future implementation).
    pub fn auto_commit_and_push(&self, _message: &str) -> Result<()> {
        // TODO: Implement Git commit and push
        // This would use git2 to stage files, commit, and optionally push
        warn!("Auto-commit and push not yet implemented");
        Ok(())
    }
}

impl Default for GitService {
    fn default() -> Self {
        Self::new()
    }
}
