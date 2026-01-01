//! DrawIO XML document structure.

use chrono::{DateTime, Utc};
use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use super::models::{DrawIOCell, DrawIOEdge};

/// Represents the root mxfile element in DrawIO XML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIODocument {
    /// Host application name
    pub host: String,

    /// Last modified timestamp
    pub modified: DateTime<Utc>,

    /// DrawIO version
    pub version: String,

    /// Device type
    #[serde(rename = "type")]
    pub device_type: String,

    /// Diagram content
    pub diagram: DrawIODiagram,
}

/// Represents a diagram within the mxfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIODiagram {
    /// Diagram ID
    pub id: String,

    /// Diagram name
    pub name: String,

    /// Graph model
    pub graph_model: DrawIOGraphModel,
}

/// Represents the mxGraphModel element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOGraphModel {
    /// Model dimensions and settings
    pub dx: i32,
    pub dy: i32,
    pub grid: i32,
    pub grid_size: i32,
    pub guides: i32,
    pub tooltips: i32,
    pub connect: i32,
    pub arrows: i32,
    pub fold: i32,
    pub page: i32,
    pub page_scale: i32,
    pub page_width: i32,
    pub page_height: i32,
    pub math: i32,
    pub shadow: i32,

    /// Root element containing cells
    pub root: DrawIORoot,
}

/// Represents the root element containing all cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIORoot {
    /// Root cell (id="0")
    pub root_cell: DrawIORootCell,

    /// Layer cell (id="1")
    pub layer_cell: DrawIORootCell,

    /// Table cells
    #[serde(default)]
    pub table_cells: Vec<DrawIOCell>,

    /// Relationship edges
    #[serde(default)]
    pub relationship_edges: Vec<DrawIOEdge>,
}

/// Represents a root or layer cell (simple cell with just an ID).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIORootCell {
    /// Cell ID
    pub id: String,

    /// Parent ID (for layer cell, parent is "0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl DrawIODocument {
    /// Create a new DrawIO document.
    pub fn new(diagram_name: String) -> Self {
        let now = Utc::now();

        Self {
            host: "Data Modelling App".to_string(),
            modified: now,
            version: "1.0".to_string(),
            device_type: "device".to_string(),
            diagram: DrawIODiagram {
                id: "data-model-diagram".to_string(),
                name: diagram_name,
                graph_model: DrawIOGraphModel {
                    dx: 1422,
                    dy: 794,
                    grid: 1,
                    grid_size: 10,
                    guides: 1,
                    tooltips: 1,
                    connect: 1,
                    arrows: 1,
                    fold: 1,
                    page: 1,
                    page_scale: 1,
                    page_width: 1169,
                    page_height: 827,
                    math: 0,
                    shadow: 0,
                    root: DrawIORoot {
                        root_cell: DrawIORootCell {
                            id: "0".to_string(),
                            parent: None,
                        },
                        layer_cell: DrawIORootCell {
                            id: "1".to_string(),
                            parent: Some("0".to_string()),
                        },
                        table_cells: Vec::new(),
                        relationship_edges: Vec::new(),
                    },
                },
            },
        }
    }

    /// Add a table cell to the document.
    pub fn add_table_cell(&mut self, cell: DrawIOCell) {
        self.diagram.graph_model.root.table_cells.push(cell);
    }

    /// Add a relationship edge to the document.
    pub fn add_relationship_edge(&mut self, edge: DrawIOEdge) {
        self.diagram.graph_model.root.relationship_edges.push(edge);
    }

    /// Generate DrawIO XML string with custom namespace support.
    ///
    /// This method generates XML with custom attributes for ODCS references
    /// and table/relationship IDs. The custom namespace is handled via
    /// attribute prefixes in the XML output.
    pub fn to_xml(&self) -> Result<String, Box<dyn std::error::Error>> {
        // For now, use serde_xml_rs or quick-xml for proper XML generation
        // This is a simplified version - full implementation will use quick-xml
        // to properly handle custom namespaces and attributes

        let mut writer = Writer::new(Cursor::new(Vec::new()));

        // Write mxfile element
        let mut mxfile_elem = BytesStart::new("mxfile");
        mxfile_elem.push_attribute(("host", self.host.as_str()));
        let modified_str = self.modified.to_rfc3339();
        mxfile_elem.push_attribute(("modified", modified_str.as_str()));
        mxfile_elem.push_attribute(("version", self.version.as_str()));
        mxfile_elem.push_attribute(("type", self.device_type.as_str()));

        writer.write_event(Event::Start(mxfile_elem))?;

        // Write diagram element
        let mut diagram_elem = BytesStart::new("diagram");
        diagram_elem.push_attribute(("id", self.diagram.id.as_str()));
        diagram_elem.push_attribute(("name", self.diagram.name.as_str()));

        writer.write_event(Event::Start(diagram_elem))?;

        // Write mxGraphModel element
        let mut graph_elem = BytesStart::new("mxGraphModel");
        graph_elem.push_attribute(("dx", self.diagram.graph_model.dx.to_string().as_str()));
        graph_elem.push_attribute(("dy", self.diagram.graph_model.dy.to_string().as_str()));
        graph_elem.push_attribute(("grid", self.diagram.graph_model.grid.to_string().as_str()));
        graph_elem.push_attribute((
            "gridSize",
            self.diagram.graph_model.grid_size.to_string().as_str(),
        ));
        // ... add other attributes

        writer.write_event(Event::Start(graph_elem))?;

        // Write root element
        writer.write_event(Event::Start(BytesStart::new("root")))?;

        // Write root cell (id="0")
        let mut root_cell = BytesStart::new("mxCell");
        root_cell.push_attribute(("id", "0"));
        writer.write_event(Event::Empty(root_cell))?;

        // Write layer cell (id="1", parent="0")
        let mut layer_cell = BytesStart::new("mxCell");
        layer_cell.push_attribute(("id", "1"));
        layer_cell.push_attribute(("parent", "0"));
        writer.write_event(Event::Empty(layer_cell))?;

        // Write table cells with custom attributes
        for cell in &self.diagram.graph_model.root.table_cells {
            let mut cell_elem = BytesStart::new("mxCell");
            cell_elem.push_attribute(("id", cell.id.as_str()));
            cell_elem.push_attribute(("value", cell.value.as_str()));
            cell_elem.push_attribute(("style", cell.style.as_str()));
            if cell.vertex {
                cell_elem.push_attribute(("vertex", "1"));
            }
            cell_elem.push_attribute(("parent", cell.parent.as_str()));

            // Add custom attributes for ODCS reference and table ID
            if let Some(ref odcs_ref) = cell.odcs_reference {
                cell_elem.push_attribute(("odcs_reference", odcs_ref.as_str()));
            }
            if let Some(table_id) = cell.table_id {
                cell_elem.push_attribute(("table_id", table_id.to_string().as_str()));
            }

            writer.write_event(Event::Start(cell_elem))?;

            // Write geometry
            let mut geom_elem = BytesStart::new("mxGeometry");
            geom_elem.push_attribute(("x", cell.geometry.x.to_string().as_str()));
            geom_elem.push_attribute(("y", cell.geometry.y.to_string().as_str()));
            geom_elem.push_attribute(("width", cell.geometry.width.to_string().as_str()));
            geom_elem.push_attribute(("height", cell.geometry.height.to_string().as_str()));
            if let Some(ref as_type) = cell.geometry.as_type {
                geom_elem.push_attribute(("as", as_type.as_str()));
            }
            writer.write_event(Event::Empty(geom_elem))?;

            writer.write_event(Event::End(BytesEnd::new("mxCell")))?;
        }

        // Write relationship edges with custom attributes
        for edge in &self.diagram.graph_model.root.relationship_edges {
            let mut edge_elem = BytesStart::new("mxCell");
            edge_elem.push_attribute(("id", edge.id.as_str()));
            if let Some(ref value) = edge.value {
                edge_elem.push_attribute(("value", value.as_str()));
            }
            edge_elem.push_attribute(("style", edge.style.as_str()));
            if edge.edge {
                edge_elem.push_attribute(("edge", "1"));
            }
            edge_elem.push_attribute(("parent", edge.parent.as_str()));
            edge_elem.push_attribute(("source", edge.source.as_str()));
            edge_elem.push_attribute(("target", edge.target.as_str()));

            // Add custom attributes for relationship ID and cardinality
            if let Some(rel_id) = edge.relationship_id {
                edge_elem.push_attribute(("relationship_id", rel_id.to_string().as_str()));
            }
            if let Some(ref card) = edge.cardinality {
                edge_elem.push_attribute(("cardinality", card.as_str()));
            }

            writer.write_event(Event::Start(edge_elem))?;

            // Write geometry with waypoints
            let mut geom_elem = BytesStart::new("mxGeometry");
            if edge.geometry.relative {
                geom_elem.push_attribute(("relative", "1"));
            }
            if let Some(ref as_type) = edge.geometry.as_type {
                geom_elem.push_attribute(("as", as_type.as_str()));
            }
            writer.write_event(Event::Start(geom_elem))?;

            // Write waypoints if present
            if let Some(ref points) = edge.geometry.points
                && let Some(ref array) = points.array
            {
                let mut array_elem = BytesStart::new("Array");
                array_elem.push_attribute(("as", "points"));
                writer.write_event(Event::Start(array_elem))?;

                for point in array {
                    let mut point_elem = BytesStart::new("mxPoint");
                    point_elem.push_attribute(("x", point.x.to_string().as_str()));
                    point_elem.push_attribute(("y", point.y.to_string().as_str()));
                    writer.write_event(Event::Empty(point_elem))?;
                }

                writer.write_event(Event::End(BytesEnd::new("Array")))?;
            }

            writer.write_event(Event::End(BytesEnd::new("mxGeometry")))?;
            writer.write_event(Event::End(BytesEnd::new("mxCell")))?;
        }

        // Close root, graph model, diagram, and mxfile
        writer.write_event(Event::End(BytesStart::new("root").to_end()))?;
        writer.write_event(Event::End(BytesStart::new("mxGraphModel").to_end()))?;
        writer.write_event(Event::End(BytesStart::new("diagram").to_end()))?;
        writer.write_event(Event::End(BytesStart::new("mxfile").to_end()))?;

        let result = writer.into_inner().into_inner();
        Ok(String::from_utf8(result)?)
    }
}
