//! DrawIO XML cell and edge models.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a DrawIO mxCell element for a table shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOCell {
    /// Cell ID (e.g., "table-{table_id}")
    pub id: String,

    /// Display value (table name)
    pub value: String,

    /// Style string (e.g., "rounded=1;whiteSpace=wrap;html=1;fillColor=#...")
    pub style: String,

    /// Whether this is a vertex (shape)
    #[serde(default)]
    pub vertex: bool,

    /// Parent cell ID (usually "1" for root)
    pub parent: String,

    /// Geometry information (position and size)
    pub geometry: DrawIOGeometry,

    /// Custom attribute: Reference to ODCS YAML file (e.g., "tables/{table_name}.yaml")
    #[serde(rename = "odcs_reference", skip_serializing_if = "Option::is_none")]
    pub odcs_reference: Option<String>,

    /// Custom attribute: Table UUID for linking
    #[serde(rename = "table_id", skip_serializing_if = "Option::is_none")]
    pub table_id: Option<Uuid>,
}

/// Represents a DrawIO mxCell element for a relationship edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOEdge {
    /// Edge ID (e.g., "edge-{relationship_id}")
    pub id: String,

    /// Display value (relationship label)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Style string (e.g., "edgeStyle=orthogonalEdgeStyle;rounded=0;...")
    pub style: String,

    /// Whether this is an edge
    #[serde(default)]
    pub edge: bool,

    /// Parent cell ID (usually "1" for root)
    pub parent: String,

    /// Source table cell ID
    pub source: String,

    /// Target table cell ID
    pub target: String,

    /// Geometry information (routing waypoints)
    pub geometry: DrawIOEdgeGeometry,

    /// Custom attribute: Relationship UUID for linking
    #[serde(rename = "relationship_id", skip_serializing_if = "Option::is_none")]
    pub relationship_id: Option<Uuid>,

    /// Custom attribute: Cardinality type
    #[serde(rename = "cardinality", skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<String>,
}

/// Geometry information for table cells (position and size).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOGeometry {
    /// X position
    pub x: f64,

    /// Y position
    pub y: f64,

    /// Width
    pub width: f64,

    /// Height
    pub height: f64,

    /// Geometry type (usually "geometry")
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_type: Option<String>,
}

/// Geometry information for relationship edges (routing waypoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOEdgeGeometry {
    /// Whether geometry is relative
    #[serde(default)]
    pub relative: bool,

    /// Geometry type (usually "geometry")
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_type: Option<String>,

    /// Routing waypoints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<DrawIOPoints>,
}

/// Array of waypoints for edge routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOPoints {
    /// Array of points
    #[serde(rename = "Array", skip_serializing_if = "Option::is_none")]
    pub array: Option<Vec<DrawIOPoint>>,
}

/// A single waypoint coordinate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawIOPoint {
    /// X coordinate
    pub x: f64,

    /// Y coordinate
    pub y: f64,

    /// Point type (usually "mxPoint")
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_type: Option<String>,
}

impl DrawIOCell {
    /// Create a new DrawIO cell for a table.
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn new_table(
        table_id: Uuid,
        table_name: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        style: String,
        odcs_reference: String,
    ) -> Self {
        Self::new_table_with_value(
            table_id,
            table_name,
            x,
            y,
            width,
            height,
            style,
            odcs_reference,
        )
    }

    /// Create a new DrawIO cell for a table with custom value (HTML content).
    #[allow(clippy::too_many_arguments)]
    pub fn new_table_with_value(
        table_id: Uuid,
        value: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        style: String,
        odcs_reference: String,
    ) -> Self {
        Self {
            id: format!("table-{}", table_id),
            value,
            style,
            vertex: true,
            parent: "1".to_string(),
            geometry: DrawIOGeometry {
                x,
                y,
                width,
                height,
                as_type: Some("geometry".to_string()),
            },
            odcs_reference: Some(odcs_reference),
            table_id: Some(table_id),
        }
    }
}

impl DrawIOEdge {
    /// Create a new DrawIO edge for a relationship.
    pub fn new_relationship(
        relationship_id: Uuid,
        source_cell_id: String,
        target_cell_id: String,
        style: String,
        cardinality: Option<String>,
        waypoints: Option<Vec<(f64, f64)>>,
    ) -> Self {
        let points = waypoints.map(|wps| DrawIOPoints {
            array: Some(
                wps.into_iter()
                    .map(|(x, y)| DrawIOPoint {
                        x,
                        y,
                        as_type: Some("mxPoint".to_string()),
                    })
                    .collect(),
            ),
        });

        Self {
            id: format!("edge-{}", relationship_id),
            value: None,
            style,
            edge: true,
            parent: "1".to_string(),
            source: source_cell_id,
            target: target_cell_id,
            geometry: DrawIOEdgeGeometry {
                relative: true,
                as_type: Some("geometry".to_string()),
                points,
            },
            relationship_id: Some(relationship_id),
            cardinality,
        }
    }
}
