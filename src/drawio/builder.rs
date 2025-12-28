//! DrawIO XML builder for constructing DrawIO XML from Table and Relationship models.

// Note: When building the binary, models are at crate::models
// The library build uses a different structure (handled via lib.rs)
use crate::models::enums::{MedallionLayer, RelationshipType};
use crate::models::{Relationship, Table};

use super::document::DrawIODocument;
use super::models::{DrawIOCell, DrawIOEdge};

/// Builder for constructing DrawIO XML documents from data models.
pub struct DrawIOBuilder {
    document: DrawIODocument,
}

impl DrawIOBuilder {
    /// Create a new DrawIO builder.
    pub fn new(diagram_name: String) -> Self {
        Self {
            document: DrawIODocument::new(diagram_name),
        }
    }

    /// Add a table to the diagram with ODCS reference.
    ///
    /// # Arguments
    ///
    /// * `table` - The table to add
    /// * `x` - X position
    /// * `y` - Y position
    /// * `width` - Table width (default: 200)
    /// * `height` - Table height (default: 300)
    #[allow(dead_code)]
    pub fn add_table(
        &mut self,
        table: &Table,
        x: f64,
        y: f64,
        width: Option<f64>,
        height: Option<f64>,
    ) {
        self.add_table_with_level(table, x, y, width, height, None)
    }

    /// Add a table to the diagram with column details based on modeling level.
    ///
    /// # Arguments
    ///
    /// * `table` - The table to add
    /// * `x` - X position
    /// * `y` - Y position
    /// * `width` - Table width
    /// * `height` - Table height
    /// * `modeling_level` - Optional modeling level to determine column details
    pub fn add_table_with_level(
        &mut self,
        table: &Table,
        x: f64,
        y: f64,
        width: Option<f64>,
        height: Option<f64>,
        modeling_level: Option<crate::models::enums::ModelingLevel>,
    ) {
        let width = width.unwrap_or(200.0);
        let height = height.unwrap_or(300.0);

        // Generate ODCS reference path (format: tables/{table_name}.yaml)
        let odcs_reference = format!("tables/{}.yaml", table.name);

        // Generate style based on medallion layer
        let style = Self::generate_table_style(&table.medallion_layers);

        // Generate table value (name + columns) based on modeling level
        let table_value = Self::generate_table_value(table, modeling_level);

        let cell = DrawIOCell::new_table_with_value(
            table.id,
            table_value,
            x,
            y,
            width,
            height,
            style,
            odcs_reference,
        );

        self.document.add_table_cell(cell);
    }

    /// Generate table value (HTML-formatted) with column details based on modeling level.
    fn generate_table_value(
        table: &Table,
        modeling_level: Option<crate::models::enums::ModelingLevel>,
    ) -> String {
        match modeling_level {
            Some(crate::models::enums::ModelingLevel::Conceptual) => {
                // Conceptual: just table name with colored header
                // Get header color based on medallion layer
                let (header_color, text_color) = if table.medallion_layers.is_empty() {
                    ("#F5F5F5", "#000000") // Light gray background with black text when no medallion layer
                } else {
                    let color = match table.medallion_layers[0] {
                        MedallionLayer::Bronze => "#CD7F32",
                        MedallionLayer::Silver => "#C0C0C0",
                        MedallionLayer::Gold => "#FFD700",
                        MedallionLayer::Operational => "#87CEEB",
                    };
                    (color, "#FFFFFF") // White text on colored background
                };
                
                // HTML table with centered header - exactly 4px padding on all sides
                // Header spans full width of box
                // Add extra padding to prevent clipping
                format!(
                    "<div style=\"padding:4px 4px 8px 4px;\"><table cellpadding=\"0\" cellspacing=\"0\" style=\"width:100%;border-collapse:collapse;margin:0;padding:0;table-layout:fixed;\"><tr><td style=\"background-color:{};color:{};padding:2px 0px;font-weight:bold;border-radius:0px;margin:0;line-height:1.3;text-align:center;width:100%;\">{}</td></tr></table></div>",
                    header_color, text_color, table.name
                )
            }
            Some(crate::models::enums::ModelingLevel::Logical) => {
                // Logical: table name + key columns (including nested key columns with dot notation)
                // Get header color based on medallion layer
                let (header_color, text_color) = if table.medallion_layers.is_empty() {
                    ("#F5F5F5", "#000000") // Light gray background with black text when no medallion layer
                } else {
                    let color = match table.medallion_layers[0] {
                        MedallionLayer::Bronze => "#CD7F32",
                        MedallionLayer::Silver => "#C0C0C0",
                        MedallionLayer::Gold => "#FFD700",
                        MedallionLayer::Operational => "#87CEEB",
                    };
                    (color, "#FFFFFF") // White text on colored background
                };
                
                // Build HTML with colored header and white body
                // Use minimal CSS padding (2px top/bottom, 4px left, 2px right)
                // Body content left-aligned for readability
                let mut html = format!(
                    "<div style=\"padding:8px 4px 8px 4px;\"><table cellpadding=\"0\" cellspacing=\"0\" style=\"width:100%;border-collapse:collapse;margin:0;padding:0;table-layout:fixed;\"><tr><td style=\"background-color:{};color:{};padding:2px 0px;font-weight:bold;border-radius:0px;margin:0;line-height:1.3;text-align:center;width:100%;\">{}</td></tr><tr><td style=\"background-color:#FFFFFF;padding:0px;margin:0;line-height:1.15;text-align:left;\">",
                    header_color, text_color, table.name
                );
                
                // Include ALL key columns including nested ones (they have dots in names)
                // Nested columns are stored with dot notation (e.g., "customer.id", "customer.name")
                let key_columns: Vec<&crate::models::Column> = table
                    .columns
                    .iter()
                    .filter(|c| c.primary_key || c.secondary_key || c.foreign_key.is_some())
                    .collect();

                if key_columns.is_empty() {
                    html.push_str("<i>No key columns</i>");
                } else {
                    for col in key_columns {
                        let key_type = if col.primary_key {
                            "PK"
                        } else if col.secondary_key {
                            "SK"
                        } else {
                            "FK"
                        };
                        html.push_str(&format!("{} <i>{}</i><br/>", key_type, col.name));
                    }
                }
                html.push_str("</td></tr></table></div>");
                html
            }
            Some(crate::models::enums::ModelingLevel::Physical) => {
                // Physical: table name + all columns with data types (including nested columns with dot notation)
                // Get header color based on medallion layer
                let (header_color, text_color) = if table.medallion_layers.is_empty() {
                    ("#F5F5F5", "#000000") // Light gray background with black text when no medallion layer
                } else {
                    let color = match table.medallion_layers[0] {
                        MedallionLayer::Bronze => "#CD7F32",
                        MedallionLayer::Silver => "#C0C0C0",
                        MedallionLayer::Gold => "#FFD700",
                        MedallionLayer::Operational => "#87CEEB",
                    };
                    (color, "#FFFFFF") // White text on colored background
                };
                
                // Build HTML with colored header and white body
                // Use minimal CSS padding (2px top/bottom, 4px left, 2px right)
                // Body content left-aligned for readability
                let mut html = format!(
                    "<div style=\"padding:8px 4px 8px 4px;\"><table cellpadding=\"0\" cellspacing=\"0\" style=\"width:100%;border-collapse:collapse;margin:0;padding:0;table-layout:fixed;\"><tr><td style=\"background-color:{};color:{};padding:2px 0px;font-weight:bold;border-radius:0px;margin:0;line-height:1.3;text-align:center;width:100%;\">{}</td></tr><tr><td style=\"background-color:#FFFFFF;padding:0px;margin:0;line-height:1.15;text-align:left;\">",
                    header_color, text_color, table.name
                );
                
                // Collect ALL columns including nested ones (they're stored with dot notation like "customer.name")
                // IMPORTANT: Nested columns are stored as separate Column objects with dot notation in their names
                // e.g., "customer.name", "customer.email", "cancellation.reason", etc.
                let mut all_columns: Vec<&crate::models::Column> = table.columns.iter().collect();
                
                // Debug: Log column count to verify nested columns are present
                let total_cols = all_columns.len();
                let nested_cols: Vec<&crate::models::Column> = all_columns.iter()
                    .filter(|c| c.name.contains('.'))
                    .copied()
                    .collect();
                
                // Sort: top-level columns first (no dots), then nested columns in dot notation order
                all_columns.sort_by(|a, b| {
                    let a_dots = a.name.matches('.').count();
                    let b_dots = b.name.matches('.').count();
                    if a_dots != b_dots {
                        a_dots.cmp(&b_dots)
                    } else {
                        a.name.cmp(&b.name)
                    }
                });
                
                // Log column counts to verify nested columns are present
                let columns_count = all_columns.len();
                tracing::info!(
                    "DrawIO export: Table '{}' has {} total columns, {} nested columns",
                    table.name, 
                    total_cols, 
                    nested_cols.len()
                );
                if !nested_cols.is_empty() {
                    tracing::info!(
                        "DrawIO export: Nested columns for '{}': {:?}",
                        table.name,
                        nested_cols.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()
                    );
                }
                
                // Display all columns including nested ones
                // Nested columns will appear with their full dot notation names (e.g., "customer.name: STRING")
                for col in &all_columns {
                    let nullable = if col.nullable { "" } else { " NOT NULL" };
                    let key_indicator = if col.primary_key || col.secondary_key {
                        " 🔑"
                    } else {
                        ""
                    };
                    html.push_str(&format!(
                        "{}: {} {}{}<br/>",
                        col.name, col.data_type, nullable, key_indicator
                    ));
                }
                
                tracing::info!(
                    "DrawIO export: Generated HTML for '{}' with {} columns (HTML length: {} chars)",
                    table.name,
                    columns_count,
                    html.len()
                );
                html.push_str("</td></tr></table></div>");
                html
            }
            None => {
                // Default: just table name (backward compatibility)
                table.name.clone()
            }
        }
    }

    /// Add a relationship to the diagram.
    ///
    /// # Arguments
    ///
    /// * `relationship` - The relationship to add
    /// * `source_table_id` - Source table UUID
    /// * `target_table_id` - Target table UUID
    /// * `waypoints` - Optional routing waypoints
    pub fn add_relationship(
        &mut self,
        relationship: &Relationship,
        waypoints: Option<Vec<(f64, f64)>>,
    ) {
        let source_cell_id = format!("table-{}", relationship.source_table_id);
        let target_cell_id = format!("table-{}", relationship.target_table_id);

        // Generate style based on relationship type and cardinality (includes Crow's Foot markers)
        let style = Self::generate_edge_style(
            relationship.relationship_type,
            relationship.cardinality.as_ref(),
            relationship.source_optional,
            relationship.target_optional,
        );

        // Get cardinality as string
        let cardinality = relationship.cardinality.map(|c| format!("{:?}", c));

        let edge = DrawIOEdge::new_relationship(
            relationship.id,
            source_cell_id,
            target_cell_id,
            style,
            cardinality,
            waypoints,
        );

        self.document.add_relationship_edge(edge);
    }

    /// Build the DrawIO XML document.
    pub fn build(self) -> DrawIODocument {
        self.document
    }

    /// Generate table style string based on medallion layers.
    ///
    /// Colors:
    /// - Bronze: #CD7F32
    /// - Silver: #C0C0C0
    /// - Gold: #FFD700
    /// - Operational: #87CEEB
    ///
    /// Style uses white background with colored header (matching canvas style)
    /// Note: medallion_layers parameter kept for API compatibility but color is applied via HTML in generate_table_value
    fn generate_table_style(_medallion_layers: &[MedallionLayer]) -> String {
        // Always use white background - color will be applied to header via HTML
        // Set all spacing to 0 - we handle padding in HTML CSS (spacingTop/Bottom don't work with html=1)
        // align=left for cell, but header text is centered via CSS text-align:center in HTML
        format!(
            "rounded=0;whiteSpace=wrap;html=1;fillColor=#FFFFFF;strokeColor=#000000;spacingLeft=0;spacingRight=0;spacingTop=0;spacingBottom=0;spacing=0;align=left;overflow=visible"
        )
    }

    /// Generate edge style string based on relationship type and cardinality.
    ///
    /// Colors and patterns:
    /// - DataFlow: Blue, solid
    /// - Dependency: Gray, dashed
    /// - ForeignKey: Black, solid
    /// - EtlTransformation: Green, dashed
    ///
    /// Cardinality markers (Crow's Foot notation):
    /// - ERone: Single line (one)
    /// - ERmany: Crowfoot (many)
    /// - ERoneToMany: One line + crowfoot
    /// - ERzeroToOne: Circle + line (optional one)
    /// - ERzeroToMany: Circle + crowfoot (optional many)
    fn generate_edge_style(
        relationship_type: Option<RelationshipType>,
        cardinality: Option<&crate::models::enums::Cardinality>,
        source_optional: Option<bool>,
        target_optional: Option<bool>,
    ) -> String {
        let (color, dashed) = match relationship_type {
            Some(RelationshipType::DataFlow) => ("#0066CC", "0"),
            Some(RelationshipType::Dependency) => ("#808080", "1"),
            Some(RelationshipType::ForeignKey) => ("#000000", "0"),
            Some(RelationshipType::EtlTransformation) => ("#00AA00", "1"),
            None => ("#000000", "0"), // Default: black, solid
        };

        // Determine arrow types based on cardinality (Crow's Foot notation)
        // DrawIO supports: ERone, ERmany, ERzeroToOne, ERzeroToMany, ERoneToMany, ERoneToOne
        let (start_arrow, end_arrow) = match cardinality {
            Some(crate::models::enums::Cardinality::OneToOne) => {
                // One-to-One: line at source, line at target
                let start = if source_optional.unwrap_or(false) {
                    "ERzeroToOne" // Circle + line for optional one
                } else {
                    "ERone" // Single line for mandatory one
                };
                let end = if target_optional.unwrap_or(false) {
                    "ERzeroToOne"
                } else {
                    "ERone"
                };
                (start, end)
            }
            Some(crate::models::enums::Cardinality::OneToMany) => {
                // One-to-Many: line at source, crowfoot at target
                let start = if source_optional.unwrap_or(false) {
                    "ERzeroToOne" // Circle + line for optional one
                } else {
                    "ERone" // Single line for mandatory one
                };
                let end = if target_optional.unwrap_or(false) {
                    "ERzeroToMany" // Circle + crowfoot for optional many
                } else {
                    "ERmany" // Crowfoot for mandatory many
                };
                (start, end)
            }
            Some(crate::models::enums::Cardinality::ManyToOne) => {
                // Many-to-One: crowfoot at source, line at target
                let start = if source_optional.unwrap_or(false) {
                    "ERzeroToMany" // Circle + crowfoot for optional many
                } else {
                    "ERmany" // Crowfoot for mandatory many
                };
                let end = if target_optional.unwrap_or(false) {
                    "ERzeroToOne" // Circle + line for optional one
                } else {
                    "ERone" // Single line for mandatory one
                };
                (start, end)
            }
            Some(crate::models::enums::Cardinality::ManyToMany) => {
                // Many-to-Many: crowfoot at both ends
                let start = if source_optional.unwrap_or(false) {
                    "ERzeroToMany" // Circle + crowfoot for optional many
                } else {
                    "ERmany" // Crowfoot for mandatory many
                };
                let end = if target_optional.unwrap_or(false) {
                    "ERzeroToMany" // Circle + crowfoot for optional many
                } else {
                    "ERmany" // Crowfoot for mandatory many
                };
                (start, end)
            }
            None => {
                // Default: arrow at target only
                ("none", "classic")
            }
        };

        format!(
            "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;strokeColor={};dashed={};startArrow={};endArrow={}",
            color, dashed, start_arrow, end_arrow
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Column;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_builder_creates_document() {
        let builder = DrawIOBuilder::new("Test Diagram".to_string());
        let document = builder.build();

        assert_eq!(document.diagram.name, "Test Diagram");
        assert_eq!(document.diagram.graph_model.root.table_cells.len(), 0);
        assert_eq!(
            document.diagram.graph_model.root.relationship_edges.len(),
            0
        );
    }

    #[test]
    fn test_add_table_with_odcs_reference() {
        let mut builder = DrawIOBuilder::new("Test".to_string());

        let table = Table {
            id: Uuid::new_v4(),
            name: "users".to_string(),
            columns: vec![Column::new("id".to_string(), "INT".to_string())],
            database_type: None,
            catalog_name: None,
            schema_name: None,
            medallion_layers: vec![MedallionLayer::Gold],
            scd_pattern: None,
            data_vault_classification: None,
            modeling_level: None,
            tags: Vec::new(),
            odcl_metadata: Default::default(),
            position: None,
            yaml_file_path: None,
            drawio_cell_id: None,
            quality: Vec::new(),
            errors: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        builder.add_table(&table, 100.0, 200.0, None, None);

        let document = builder.build();
        assert_eq!(document.diagram.graph_model.root.table_cells.len(), 1);

        let cell = &document.diagram.graph_model.root.table_cells[0];
        assert_eq!(cell.value, "users");
        assert_eq!(cell.odcs_reference, Some("tables/users.yaml".to_string()));
        assert_eq!(cell.table_id, Some(table.id));
        assert!(cell.style.contains("#FFD700")); // Gold color
    }

    #[test]
    fn test_add_relationship_with_custom_attributes() {
        let mut builder = DrawIOBuilder::new("Test".to_string());

        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let rel_id = Uuid::new_v4();

        let relationship = Relationship {
            id: rel_id,
            source_table_id: source_id,
            target_table_id: target_id,
            cardinality: Some(crate::models::enums::Cardinality::OneToMany),
            source_optional: None,
            target_optional: None,
            foreign_key_details: None,
            etl_job_metadata: None,
            relationship_type: Some(RelationshipType::DataFlow),
            visual_metadata: None,
            drawio_edge_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        builder.add_relationship(&relationship, None);

        let document = builder.build();
        assert_eq!(
            document.diagram.graph_model.root.relationship_edges.len(),
            1
        );

        let edge = &document.diagram.graph_model.root.relationship_edges[0];
        assert_eq!(edge.relationship_id, Some(rel_id));
        assert_eq!(edge.source, format!("table-{}", source_id));
        assert_eq!(edge.target, format!("table-{}", target_id));
        assert!(edge.style.contains("#0066CC")); // DataFlow blue color
    }
}
