//! Canvas Layout Service for managing canvas positions and routing in YAML format.

use crate::models::{DataModel, Position, VisualMetadata};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

/// Canvas layout file version
const CANVAS_LAYOUT_VERSION: &str = "1.0";

/// Canvas layout YAML structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasLayout {
    pub version: String,
    #[serde(default)]
    pub tables: Vec<TableLayout>,
    #[serde(default)]
    pub relationships: Vec<RelationshipLayout>,
}

/// Table layout information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableLayout {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// Relationship layout information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipLayout {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_metadata: Option<VisualMetadata>,
}

/// Service for managing canvas layout in YAML format.
pub struct CanvasLayoutService {
    /// Path to the canvas layout YAML file
    layout_file_path: PathBuf,
}

impl CanvasLayoutService {
    /// Create a new canvas layout service.
    ///
    /// # Arguments
    ///
    /// * `git_directory_path` - Path to the Git directory containing the model
    pub fn new(git_directory_path: &Path) -> Self {
        Self {
            layout_file_path: git_directory_path.join("canvas-layout.yaml"),
        }
    }

    /// Save canvas layout to YAML file.
    ///
    /// Saves all table positions and relationship visual metadata.
    pub fn save_canvas_layout(&self, model: &DataModel) -> Result<()> {
        info!("Saving canvas layout to YAML: {:?}", self.layout_file_path);

        // Build layout structure
        let mut layout = CanvasLayout {
            version: CANVAS_LAYOUT_VERSION.to_string(),
            tables: Vec::new(),
            relationships: Vec::new(),
        };

        // Add all tables with positions
        for table in &model.tables {
            layout.tables.push(TableLayout {
                id: table.id,
                position: table.position.clone(),
            });
        }

        // Add all relationships with visual metadata
        for relationship in &model.relationships {
            layout.relationships.push(RelationshipLayout {
                id: relationship.id,
                visual_metadata: relationship.visual_metadata.clone(),
            });
        }

        // Ensure parent directory exists
        if let Some(parent) = self.layout_file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Serialize to YAML and write to file
        let yaml_content = serde_yaml::to_string(&layout)
            .with_context(|| "Failed to serialize canvas layout to YAML")?;

        fs::write(&self.layout_file_path, yaml_content).with_context(|| {
            format!(
                "Failed to write canvas layout to {:?}",
                self.layout_file_path
            )
        })?;

        info!(
            "Canvas layout saved successfully: {} tables, {} relationships",
            layout.tables.len(),
            layout.relationships.len()
        );
        Ok(())
    }

    /// Load canvas layout from YAML file.
    ///
    /// Loads table positions and relationship visual metadata into the model.
    pub fn load_canvas_layout(&self, model: &mut DataModel) -> Result<()> {
        if !self.layout_file_path.exists() {
            warn!("Canvas layout file not found: {:?}", self.layout_file_path);
            return Ok(()); // No file is not an error
        }

        info!(
            "Loading canvas layout from YAML: {:?}",
            self.layout_file_path
        );

        // Read and parse YAML file
        let yaml_content = fs::read_to_string(&self.layout_file_path).with_context(|| {
            format!(
                "Failed to read canvas layout from {:?}",
                self.layout_file_path
            )
        })?;

        let layout: CanvasLayout = serde_yaml::from_str(&yaml_content)
            .with_context(|| "Failed to parse canvas layout YAML")?;

        // Create maps for quick lookup
        let mut positions: std::collections::HashMap<Uuid, Position> =
            std::collections::HashMap::new();
        for table_layout in &layout.tables {
            if let Some(ref position) = table_layout.position {
                positions.insert(table_layout.id, position.clone());
            }
        }

        let mut visual_metadata_map: std::collections::HashMap<Uuid, VisualMetadata> =
            std::collections::HashMap::new();
        for rel_layout in &layout.relationships {
            if let Some(ref visual_metadata) = rel_layout.visual_metadata {
                visual_metadata_map.insert(rel_layout.id, visual_metadata.clone());
            }
        }

        // Update table positions in the model
        for table in &mut model.tables {
            if let Some(position) = positions.get(&table.id) {
                table.position = Some(position.clone());
            }
        }

        // Update relationship visual metadata in the model
        for relationship in &mut model.relationships {
            if let Some(visual_metadata) = visual_metadata_map.get(&relationship.id) {
                relationship.visual_metadata = Some(visual_metadata.clone());
            }
        }

        info!(
            "Canvas layout loaded successfully: {} tables, {} relationships",
            layout.tables.len(),
            layout.relationships.len()
        );
        Ok(())
    }

    /// Update a single table position.
    ///
    /// Loads the layout, updates the position, and saves it back.
    pub fn update_table_position(
        &self,
        _model: &DataModel,
        table_id: Uuid,
        position: Position,
    ) -> Result<()> {
        // Load existing layout or create new one
        let mut layout = if self.layout_file_path.exists() {
            let yaml_content = fs::read_to_string(&self.layout_file_path).with_context(|| {
                format!(
                    "Failed to read canvas layout from {:?}",
                    self.layout_file_path
                )
            })?;
            serde_yaml::from_str::<CanvasLayout>(&yaml_content)
                .with_context(|| "Failed to parse canvas layout YAML")?
        } else {
            CanvasLayout {
                version: CANVAS_LAYOUT_VERSION.to_string(),
                tables: Vec::new(),
                relationships: Vec::new(),
            }
        };

        // Find and update table position, or add new entry
        if let Some(table_layout) = layout.tables.iter_mut().find(|t| t.id == table_id) {
            table_layout.position = Some(position);
        } else {
            layout.tables.push(TableLayout {
                id: table_id,
                position: Some(position),
            });
        }

        // Save back to file
        self.save_layout_to_file(&layout)?;
        Ok(())
    }

    /// Update a single relationship visual metadata.
    ///
    /// Loads the layout, updates the visual metadata, and saves it back.
    pub fn update_relationship_routing(
        &self,
        _model: &DataModel,
        relationship_id: Uuid,
        visual_metadata: VisualMetadata,
    ) -> Result<()> {
        // Load existing layout or create new one
        let mut layout = if self.layout_file_path.exists() {
            let yaml_content = fs::read_to_string(&self.layout_file_path).with_context(|| {
                format!(
                    "Failed to read canvas layout from {:?}",
                    self.layout_file_path
                )
            })?;
            serde_yaml::from_str::<CanvasLayout>(&yaml_content)
                .with_context(|| "Failed to parse canvas layout YAML")?
        } else {
            CanvasLayout {
                version: CANVAS_LAYOUT_VERSION.to_string(),
                tables: Vec::new(),
                relationships: Vec::new(),
            }
        };

        // Find and update relationship visual metadata, or add new entry
        if let Some(rel_layout) = layout
            .relationships
            .iter_mut()
            .find(|r| r.id == relationship_id)
        {
            rel_layout.visual_metadata = Some(visual_metadata);
        } else {
            layout.relationships.push(RelationshipLayout {
                id: relationship_id,
                visual_metadata: Some(visual_metadata),
            });
        }

        // Save back to file
        self.save_layout_to_file(&layout)?;
        Ok(())
    }

    /// Helper method to save layout to file
    fn save_layout_to_file(&self, layout: &CanvasLayout) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.layout_file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Serialize to YAML and write to file
        let yaml_content = serde_yaml::to_string(layout)
            .with_context(|| "Failed to serialize canvas layout to YAML")?;

        fs::write(&self.layout_file_path, yaml_content).with_context(|| {
            format!(
                "Failed to write canvas layout to {:?}",
                self.layout_file_path
            )
        })?;

        Ok(())
    }

    /// Migrate from DrawIO XML to canvas-layout.yaml
    ///
    /// This is a one-time migration function that reads from diagram.drawio
    /// and converts it to canvas-layout.yaml format.
    pub fn migrate_from_drawio(
        &self,
        model: &mut DataModel,
        drawio_service: &crate::services::drawio_service::DrawIOService,
    ) -> Result<()> {
        // Try to load from DrawIO XML
        if let Err(e) = drawio_service.load_table_positions(model) {
            warn!("Failed to load table positions from DrawIO: {}", e);
        }
        if let Err(e) = drawio_service.load_relationship_routing(model) {
            warn!("Failed to load relationship routing from DrawIO: {}", e);
        }

        // Save to YAML format
        self.save_canvas_layout(model)?;

        info!("Migrated canvas layout from DrawIO XML to YAML format");
        Ok(())
    }
}
