//! DrawIO service for managing DrawIO XML operations.

use crate::drawio::document::DrawIODocument;
use crate::drawio::models::{DrawIOCell, DrawIOEdge, DrawIOPoint, DrawIOPoints};
use crate::models::{DataModel, Relationship};
use crate::models::{Position, Table};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

/// Service for managing DrawIO XML files.
pub struct DrawIOService {
    /// Path to the DrawIO XML file
    diagram_file_path: PathBuf,
}

impl DrawIOService {
    /// Create a new DrawIO service.
    ///
    /// # Arguments
    ///
    /// * `git_directory_path` - Path to the Git directory containing the model
    pub fn new(git_directory_path: &Path) -> Self {
        Self {
            diagram_file_path: git_directory_path.join("diagram.drawio"),
        }
    }

    /// Save table positions to DrawIO XML.
    ///
    /// Updates the DrawIO XML file with current table positions.
    /// If the file doesn't exist, creates a new one.
    pub fn save_table_positions(&self, model: &DataModel) -> Result<()> {
        info!(
            "Saving table positions to DrawIO XML: {:?}",
            self.diagram_file_path
        );

        // Load existing DrawIO XML or create new one
        let mut document = if self.diagram_file_path.exists() {
            self.load_document()?
        } else {
            DrawIODocument::new(model.name.clone())
        };

        // Update table positions in the document
        for table in &model.tables {
            if let Some(position) = &table.position {
                // Find existing cell or create new one
                let cell_id = format!("table-{}", table.id);
                let existing_cell = document
                    .diagram
                    .graph_model
                    .root
                    .table_cells
                    .iter_mut()
                    .find(|c| c.id == cell_id);

                if let Some(cell) = existing_cell {
                    // Update existing cell position
                    cell.geometry.x = position.x;
                    cell.geometry.y = position.y;
                } else {
                    // Create new cell
                    let odcs_reference = format!("tables/{}.yaml", table.name);
                    let style = Self::generate_table_style(&table.medallion_layers);

                    let cell = DrawIOCell::new_table(
                        table.id,
                        table.name.clone(),
                        position.x,
                        position.y,
                        200.0, // Default width
                        300.0, // Default height
                        style,
                        odcs_reference,
                    );
                    document.diagram.graph_model.root.table_cells.push(cell);
                }
            }
        }

        // Save document to file
        self.save_document(&document)?;

        info!("Table positions saved successfully");
        Ok(())
    }

    /// Load table positions from DrawIO XML.
    ///
    /// Reads positions from the DrawIO XML file and updates tables in the model.
    pub fn load_table_positions(&self, model: &mut DataModel) -> Result<()> {
        if !self.diagram_file_path.exists() {
            warn!("DrawIO XML file not found: {:?}", self.diagram_file_path);
            return Ok(()); // No file is not an error
        }

        info!(
            "Loading table positions from DrawIO XML: {:?}",
            self.diagram_file_path
        );

        let document = self.load_document()?;

        // Create a map of table positions by table ID
        let mut positions: HashMap<Uuid, (f64, f64)> = HashMap::new();

        for cell in &document.diagram.graph_model.root.table_cells {
            if let Some(table_id) = cell.table_id {
                positions.insert(table_id, (cell.geometry.x, cell.geometry.y));
            }
        }

        // Update table positions in the model
        for table in &mut model.tables {
            if let Some((x, y)) = positions.get(&table.id) {
                table.position = Some(Position { x: *x, y: *y });
            }
        }

        info!("Table positions loaded successfully");
        Ok(())
    }

    /// Save relationship routing to DrawIO XML.
    ///
    /// Updates the DrawIO XML file with relationship visual metadata.
    pub fn save_relationship_routing(&self, model: &DataModel) -> Result<()> {
        info!(
            "Saving relationship routing to DrawIO XML: {:?}",
            self.diagram_file_path
        );

        // Load existing DrawIO XML or create new one
        let mut document = if self.diagram_file_path.exists() {
            self.load_document()?
        } else {
            DrawIODocument::new(model.name.clone())
        };

        // Update relationship edges in the document
        for relationship in &model.relationships {
            let edge_id = format!("edge-{}", relationship.id);
            let source_cell_id = format!("table-{}", relationship.source_table_id);
            let target_cell_id = format!("table-{}", relationship.target_table_id);

            let existing_edge = document
                .diagram
                .graph_model
                .root
                .relationship_edges
                .iter_mut()
                .find(|e| e.id == edge_id);

            if let Some(edge) = existing_edge {
                // Update existing edge with visual metadata
                if let Some(ref visual) = relationship.visual_metadata {
                    // Update waypoints if present
                    if !visual.routing_waypoints.is_empty() {
                        let waypoints: Vec<(f64, f64)> = visual
                            .routing_waypoints
                            .iter()
                            .map(|cp| (cp.x, cp.y))
                            .collect();
                        edge.geometry.points = Some(DrawIOPoints {
                            array: Some(
                                waypoints
                                    .into_iter()
                                    .map(|(x, y)| DrawIOPoint {
                                        x,
                                        y,
                                        as_type: Some("mxPoint".to_string()),
                                    })
                                    .collect(),
                            ),
                        });
                    }
                }
            } else {
                // Create new edge
                let style = Self::generate_edge_style(relationship.relationship_type);
                let cardinality = relationship.cardinality.map(|c| format!("{:?}", c));

                let waypoints = relationship
                    .visual_metadata
                    .as_ref()
                    .map(|v| v.routing_waypoints.iter().map(|cp| (cp.x, cp.y)).collect());

                let edge = DrawIOEdge::new_relationship(
                    relationship.id,
                    source_cell_id,
                    target_cell_id,
                    style,
                    cardinality,
                    waypoints,
                );
                document
                    .diagram
                    .graph_model
                    .root
                    .relationship_edges
                    .push(edge);
            }
        }

        // Save document to file
        self.save_document(&document)?;

        info!("Relationship routing saved successfully");
        Ok(())
    }

    /// Load relationship routing from DrawIO XML.
    ///
    /// Reads visual metadata from the DrawIO XML file and updates relationships in the model.
    pub fn load_relationship_routing(&self, model: &mut DataModel) -> Result<()> {
        if !self.diagram_file_path.exists() {
            warn!("DrawIO XML file not found: {:?}", self.diagram_file_path);
            return Ok(()); // No file is not an error
        }

        info!(
            "Loading relationship routing from DrawIO XML: {:?}",
            self.diagram_file_path
        );

        let document = self.load_document()?;

        // Create a map of relationship visual metadata by relationship ID
        let mut routing_data: HashMap<Uuid, Vec<(f64, f64)>> = HashMap::new();

        for edge in &document.diagram.graph_model.root.relationship_edges {
            if let Some(relationship_id) = edge.relationship_id {
                if let Some(ref points) = edge.geometry.points {
                    if let Some(ref array) = points.array {
                        let waypoints: Vec<(f64, f64)> =
                            array.iter().map(|p: &DrawIOPoint| (p.x, p.y)).collect();
                        routing_data.insert(relationship_id, waypoints);
                    }
                }
            }
        }

        // Update relationship visual metadata in the model
        for relationship in &mut model.relationships {
            if let Some(waypoints) = routing_data.get(&relationship.id) {
                use crate::models::relationship::{ConnectionPoint, VisualMetadata};

                let routing_waypoints: Vec<ConnectionPoint> = waypoints
                    .iter()
                    .map(|(x, y)| ConnectionPoint { x: *x, y: *y })
                    .collect();

                relationship.visual_metadata = Some(VisualMetadata {
                    source_connection_point: relationship
                        .visual_metadata
                        .as_ref()
                        .and_then(|v| v.source_connection_point.clone()),
                    target_connection_point: relationship
                        .visual_metadata
                        .as_ref()
                        .and_then(|v| v.target_connection_point.clone()),
                    routing_waypoints,
                    label_position: relationship
                        .visual_metadata
                        .as_ref()
                        .and_then(|v| v.label_position.clone()),
                });
            }
        }

        info!("Relationship routing loaded successfully");
        Ok(())
    }

    /// Auto-update DrawIO XML when table position changes.
    ///
    /// This should be called whenever a table position is updated.
    pub fn auto_update_position(&self, model: &DataModel) -> Result<()> {
        self.save_table_positions(model)
    }

    /// Auto-update DrawIO XML when relationship routing changes.
    ///
    /// This should be called whenever relationship visual metadata is updated.
    pub fn auto_update_routing(&self, model: &DataModel) -> Result<()> {
        self.save_relationship_routing(model)
    }

    /// Parse DrawIO XML content into a DrawIODocument.
    ///
    /// This is the main parsing method that converts DrawIO XML string
    /// into our internal document structure.
    pub fn parse_drawio_xml(xml_content: &str) -> Result<DrawIODocument> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml_content);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut document: Option<DrawIODocument> = None;
        let _current_element: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    if e.name().as_ref() == b"mxfile" {
                        // Parse mxfile attributes
                        // Parse mxfile attributes (currently unused but kept for future use)
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"host" | b"modified" | b"version" | b"type" => {
                                    // Attributes parsed but not currently used
                                    let _ = String::from_utf8_lossy(&attr.value);
                                }
                                _ => {}
                            }
                        }

                        // For now, create a basic document structure
                        // Full parsing will be implemented incrementally
                        document = Some(DrawIODocument::new("Imported Diagram".to_string()));
                    }
                    let _ = String::from_utf8_lossy(e.name().as_ref());
                }
                Ok(Event::End(_)) => {
                    // Element ended
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(anyhow::anyhow!("XML parsing error: {}", e));
                }
                _ => {}
            }
            buf.clear();
        }

        document.ok_or_else(|| anyhow::anyhow!("No mxfile element found in XML"))
    }

    /// Extract table positions from DrawIO XML document.
    ///
    /// Returns a map of table IDs to their positions.
    pub fn extract_table_positions(document: &DrawIODocument) -> HashMap<Uuid, (f64, f64)> {
        let mut positions = HashMap::new();

        for cell in &document.diagram.graph_model.root.table_cells {
            if let Some(table_id) = cell.table_id {
                positions.insert(table_id, (cell.geometry.x, cell.geometry.y));
            }
        }

        positions
    }

    /// Extract relationship routing from DrawIO XML document.
    ///
    /// Returns a map of relationship IDs to their visual metadata.
    pub fn extract_relationship_routing(
        document: &DrawIODocument,
    ) -> HashMap<Uuid, Vec<(f64, f64)>> {
        let mut routing = HashMap::new();

        for edge in &document.diagram.graph_model.root.relationship_edges {
            if let Some(relationship_id) = edge.relationship_id {
                if let Some(ref points) = edge.geometry.points {
                    if let Some(ref array) = points.array {
                        let waypoints: Vec<(f64, f64)> =
                            array.iter().map(|p: &DrawIOPoint| (p.x, p.y)).collect();
                        routing.insert(relationship_id, waypoints);
                    }
                }
            }
        }

        routing
    }

    /// Resolve ODCS references from DrawIO XML and load schema from YAML files.
    ///
    /// This method extracts ODCS references from DrawIO cells and loads
    /// the corresponding table schemas from YAML files.
    pub fn resolve_odcs_references(
        document: &DrawIODocument,
        git_directory_path: &Path,
    ) -> Result<HashMap<String, Table>> {
        use crate::services::odcs_parser::ODCSParser;
        use std::fs;

        let mut tables = HashMap::new();
        let mut odcl_parser = ODCSParser::new();

        // Extract ODCS references from table cells
        for cell in &document.diagram.graph_model.root.table_cells {
            if let Some(ref odcs_ref) = cell.odcs_reference {
                let yaml_path: PathBuf = git_directory_path.join(odcs_ref);

                if yaml_path.exists() {
                    match fs::read_to_string(&yaml_path) {
                        Ok(yaml_content) => match odcl_parser.parse(&yaml_content) {
                            Ok((table, _)) => {
                                tables.insert(odcs_ref.clone(), table);
                            }
                            Err(e) => {
                                warn!("Failed to parse ODCS YAML {}: {}", odcs_ref, e);
                            }
                        },
                        Err(e) => {
                            warn!("Failed to read ODCS YAML {}: {}", odcs_ref, e);
                        }
                    }
                }
            }
        }

        Ok(tables)
    }

    /// Handle missing ODCS references by creating placeholder tables or error messages.
    ///
    /// Returns a list of warnings/errors for missing references.
    pub fn handle_missing_odcs_references(
        document: &DrawIODocument,
        git_directory_path: &Path,
    ) -> Vec<String> {
        let mut warnings = Vec::new();

        for cell in &document.diagram.graph_model.root.table_cells {
            if let Some(ref odcs_ref) = cell.odcs_reference {
                let yaml_path = git_directory_path.join(odcs_ref);

                if !yaml_path.exists() {
                    warnings.push(format!(
                        "ODCS reference '{}' not found at path: {:?}",
                        odcs_ref, yaml_path
                    ));
                }
            }
        }

        warnings
    }

    /// Validate DrawIO XML structure and required elements.
    ///
    /// Checks that the XML has the required structure for a valid DrawIO diagram.
    pub fn validate_drawio_xml(xml_content: &str) -> Result<()> {
        // Basic validation: check for required elements
        if !xml_content.contains("<mxfile") {
            return Err(anyhow::anyhow!("Missing <mxfile> root element"));
        }

        if !xml_content.contains("<diagram") {
            return Err(anyhow::anyhow!("Missing <diagram> element"));
        }

        if !xml_content.contains("<mxGraphModel") {
            return Err(anyhow::anyhow!("Missing <mxGraphModel> element"));
        }

        // Try to parse the XML to ensure it's well-formed
        Self::parse_drawio_xml(xml_content)?;

        Ok(())
    }

    /// Load DrawIO document from file.
    fn load_document(&self) -> Result<DrawIODocument> {
        let xml_content = std::fs::read_to_string(&self.diagram_file_path).with_context(|| {
            format!(
                "Failed to read DrawIO XML file: {:?}",
                self.diagram_file_path
            )
        })?;

        Self::parse_drawio_xml(&xml_content)
    }

    /// Save DrawIO document to file.
    fn save_document(&self, document: &DrawIODocument) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.diagram_file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        // Generate XML and write to file
        let xml = document
            .to_xml()
            .map_err(|e| anyhow::anyhow!("Failed to generate DrawIO XML: {}", e))?;

        fs::write(&self.diagram_file_path, xml).with_context(|| {
            format!("Failed to write DrawIO XML to {:?}", self.diagram_file_path)
        })?;

        Ok(())
    }

    /// Generate table style string based on medallion layers.
    fn generate_table_style(medallion_layers: &[crate::models::enums::MedallionLayer]) -> String {
        use crate::models::enums::MedallionLayer;

        let color = if medallion_layers.is_empty() {
            "#FFFFFF" // White default
        } else {
            match medallion_layers[0] {
                MedallionLayer::Bronze => "#CD7F32",
                MedallionLayer::Silver => "#C0C0C0",
                MedallionLayer::Gold => "#FFD700",
                MedallionLayer::Operational => "#87CEEB",
            }
        };

        format!(
            "rounded=1;whiteSpace=wrap;html=1;fillColor={};strokeColor=#000000",
            color
        )
    }

    /// Generate edge style string based on relationship type.
    fn generate_edge_style(
        relationship_type: Option<crate::models::enums::RelationshipType>,
    ) -> String {
        use crate::models::enums::RelationshipType;

        let (color, dashed) = match relationship_type {
            Some(RelationshipType::DataFlow) => ("#0066CC", "0"),
            Some(RelationshipType::Dependency) => ("#808080", "1"),
            Some(RelationshipType::ForeignKey) => ("#000000", "0"),
            Some(RelationshipType::EtlTransformation) => ("#00AA00", "1"),
            None => ("#000000", "0"), // Default: black, solid
        };

        format!(
            "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;strokeColor={};dashed={}",
            color, dashed
        )
    }

    /// Export complete model to DrawIO XML format.
    ///
    /// Generates a complete DrawIO XML document with all tables and relationships,
    /// including positions, styling, and visual metadata.
    pub fn export_to_drawio(&self, model: &DataModel) -> Result<String> {
        self.export_to_drawio_with_level(model, None)
    }

    /// Export model to DrawIO XML format with specific modeling level.
    /// If modeling_level is None, exports basic table boxes (backward compatibility).
    pub fn export_to_drawio_with_level(
        &self,
        model: &DataModel,
        modeling_level: Option<crate::models::enums::ModelingLevel>,
    ) -> Result<String> {
        use crate::drawio::builder::DrawIOBuilder;

        let level_name = modeling_level.map(|l| match l {
            crate::models::enums::ModelingLevel::Conceptual => "Conceptual",
            crate::models::enums::ModelingLevel::Logical => "Logical",
            crate::models::enums::ModelingLevel::Physical => "Physical",
        });

        info!(
            "Exporting model to DrawIO XML: {} (level: {:?})",
            model.name, level_name
        );

        let mut builder = DrawIOBuilder::new(model.name.clone());

        // Add all tables with their positions and column details based on modeling level
        // Apply 4px offset to top-left of each box
        // All tables use the same top Y offset (4px) - only X varies for horizontal positioning
        const POSITION_OFFSET: f64 = 4.0;
        const FIXED_TOP_Y: f64 = 4.0; // Fixed top Y position for all tables
        for table in &model.tables {
            // Use stored X position (with offset), but fixed Y position for consistent top alignment
            let x = if let Some(ref pos) = table.position {
                pos.x + POSITION_OFFSET
            } else {
                // Default X position if not set (with offset)
                POSITION_OFFSET
            };
            let y = FIXED_TOP_Y; // All tables start at the same top Y position

            // Calculate dimensions based on modeling level and column count
            let (width, height) = Self::calculate_table_dimensions(table, modeling_level);

            builder.add_table_with_level(table, x, y, Some(width), Some(height), modeling_level);
        }

        // Add all relationships with their routing
        for relationship in &model.relationships {
            let waypoints = relationship
                .visual_metadata
                .as_ref()
                .map(|v| v.routing_waypoints.iter().map(|cp| (cp.x, cp.y)).collect());

            builder.add_relationship(relationship, waypoints);
        }

        // Build document and generate XML
        let document = builder.build();
        let xml = document
            .to_xml()
            .map_err(|e| anyhow::anyhow!("Failed to generate DrawIO XML: {}", e))?;

        info!("DrawIO XML export completed successfully");
        Ok(xml)
    }

    /// Calculate table dimensions based on modeling level and column count.
    /// Width = text width + 8px (4px each side)
    /// Height = text height + 8px (4px top + 4px bottom)
    fn calculate_table_dimensions(
        table: &crate::models::Table,
        modeling_level: Option<crate::models::enums::ModelingLevel>,
    ) -> (f64, f64) {
        // Font metrics for accurate text size calculation
        // Typical monospace font: ~6px per character (conservative estimate)
        const CHAR_WIDTH: f64 = 6.0;
        // Line height multipliers
        const HEADER_LINE_HEIGHT: f64 = 1.3; // line-height for header (slightly increased)
        const COLUMN_LINE_HEIGHT: f64 = 1.15; // line-height for columns (slightly increased)
        // Base font size (typical browser default is ~16px, but we'll use ~12px for tighter fit)
        const BASE_FONT_SIZE: f64 = 12.0;
        const MIN_WIDTH: f64 = 150.0;
        const MIN_HEIGHT: f64 = 50.0;
        // Fixed padding: exactly 4px on each side = 8px total
        const PADDING_WIDTH: f64 = 8.0; // 4px left + 4px right
        const PADDING_HEIGHT: f64 = 8.0; // 4px top + 4px bottom

        let columns_to_show = match modeling_level {
            Some(crate::models::enums::ModelingLevel::Conceptual) => {
                // Conceptual: no columns, just table name
                0
            }
            Some(crate::models::enums::ModelingLevel::Logical) => {
                // Logical: show key columns only
                table
                    .columns
                    .iter()
                    .filter(|c| c.primary_key || c.secondary_key || c.foreign_key.is_some())
                    .count()
            }
            Some(crate::models::enums::ModelingLevel::Physical) => {
                // Physical: show all columns
                table.columns.len()
            }
            None => {
                // Default: no columns (backward compatibility)
                0
            }
        };

        // Calculate exact text width: find longest line
        let max_text_width = table.name.len().max(
            table
                .columns
                .iter()
                .take(columns_to_show)
                .map(|c| {
                    // Build the exact text string that will be displayed
                    let mut text = format!("{}: {}", c.name, c.data_type);
                    if !c.nullable {
                        text.push_str(" NOT NULL");
                    }
                    if c.primary_key || c.secondary_key {
                        text.push_str(" 🔑");
                    }
                    text.len()
                })
                .max()
                .unwrap_or(0),
        ) as f64;
        
        // Width = exact text width + 8px (4px each side)
        let text_width = max_text_width * CHAR_WIDTH;
        let width = text_width + PADDING_WIDTH;

        // Calculate exact text height: header + columns
        // Header: base font size * line height (minimal padding to prevent clipping)
        let header_text_height = BASE_FONT_SIZE * HEADER_LINE_HEIGHT;
        // Column: base font size * line height (each line)
        let column_text_height = BASE_FONT_SIZE * COLUMN_LINE_HEIGHT;
        let total_text_height = header_text_height + (columns_to_show as f64 * column_text_height);
        
        // Height = text height + padding (8px top + 8px bottom = 16px total)
        // Use sufficient padding to prevent clipping without excessive whitespace
        const EXTRA_PADDING: f64 = 8.0; // Padding on top and bottom
        let height = total_text_height + (EXTRA_PADDING * 2.0);

        (width.max(MIN_WIDTH), height.max(MIN_HEIGHT))
    }

    /// Generate table shape with correct styling (medallion layer colors, rounded rectangles).
    ///
    /// This is used internally by the builder but exposed for testing.
    pub fn generate_table_shape_style(
        medallion_layers: &[crate::models::enums::MedallionLayer],
    ) -> String {
        Self::generate_table_style(medallion_layers)
    }

    /// Generate relationship edge with correct styling (colors, dash patterns).
    ///
    /// This is used internally by the builder but exposed for testing.
    pub fn generate_relationship_edge_style(
        relationship_type: Option<crate::models::enums::RelationshipType>,
    ) -> String {
        Self::generate_edge_style(relationship_type)
    }

    /// Generate label position for relationship.
    ///
    /// Returns the label position from visual metadata if available.
    pub fn generate_relationship_label_position(relationship: &Relationship) -> Option<(f64, f64)> {
        relationship
            .visual_metadata
            .as_ref()
            .and_then(|v| v.label_position.as_ref())
            .map(|cp| (cp.x, cp.y))
    }
}
