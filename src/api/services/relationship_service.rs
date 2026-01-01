//! Relationship service for managing table relationships.

use crate::models::enums::{Cardinality, RelationshipType};
use crate::models::relationship::{ETLJobMetadata, ForeignKeyDetails};
use crate::models::{DataModel, Relationship};
use anyhow::Result;
use petgraph::algo::is_cyclic_directed;
use petgraph::graphmap::DiGraphMap;
use tracing::info;
use uuid::Uuid;

/// Service for managing relationships between tables.
pub struct RelationshipService {
    /// Data model containing tables and relationships
    model: Option<DataModel>,
}

impl RelationshipService {
    /// Create a new relationship service instance.
    #[allow(dead_code)]
    pub fn new(model: Option<DataModel>) -> Self {
        Self { model }
    }

    /// Set the model for this service.
    #[allow(dead_code)]
    pub fn set_model(&mut self, model: DataModel) {
        self.model = Some(model);
    }

    /// Get the model (mutable).
    #[allow(dead_code)]
    pub fn get_model_mut(&mut self) -> Option<&mut DataModel> {
        self.model.as_mut()
    }

    /// Create a new relationship.
    pub fn create_relationship(
        &mut self,
        source_table_id: Uuid,
        target_table_id: Uuid,
        cardinality: Option<Cardinality>,
        foreign_key_details: Option<ForeignKeyDetails>,
        etl_job_metadata: Option<ETLJobMetadata>,
        relationship_type: Option<RelationshipType>,
    ) -> Result<Relationship> {
        let model = self
            .model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model loaded"))?;

        // Validate tables exist (extract names before mutable borrow)
        let source_table_name = model
            .get_table_by_id(source_table_id)
            .map(|t| t.name.clone())
            .ok_or_else(|| anyhow::anyhow!("Source table {} not found", source_table_id))?;

        let target_table_name = model
            .get_table_by_id(target_table_id)
            .map(|t| t.name.clone())
            .ok_or_else(|| anyhow::anyhow!("Target table {} not found", target_table_id))?;

        // Check for self-reference
        if source_table_id == target_table_id {
            return Err(anyhow::anyhow!(
                "Cannot create relationship from table to itself"
            ));
        }

        // Check for circular dependency (clone model to avoid borrow conflict)
        let model_clone = model.clone();
        let temp_service = RelationshipService::new(Some(model_clone));
        let (is_circular, cycle_path) =
            temp_service.check_circular_dependency(source_table_id, target_table_id)?;
        if is_circular {
            let cycle_msg = if let Some(path) = cycle_path {
                format!(
                    "Cycle detected: {}",
                    path.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                )
            } else {
                "Cycle detected".to_string()
            };
            return Err(anyhow::anyhow!("Cannot create relationship: {}", cycle_msg));
        }

        // Create relationship
        let relationship = Relationship {
            id: Uuid::new_v4(),
            source_table_id,
            target_table_id,
            cardinality,
            source_optional: None,
            target_optional: None,
            foreign_key_details,
            etl_job_metadata,
            relationship_type,
            notes: None,
            visual_metadata: None,
            drawio_edge_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        model.relationships.push(relationship.clone());
        info!(
            "Created relationship: {} -> {}",
            source_table_name, target_table_name
        );

        Ok(relationship)
    }

    /// Get a relationship by ID.
    pub fn get_relationship(&self, relationship_id: Uuid) -> Option<&Relationship> {
        self.model
            .as_ref()?
            .relationships
            .iter()
            .find(|r| r.id == relationship_id)
    }

    /// Get all relationships involving a table.
    #[allow(dead_code)]
    pub fn get_relationships_for_table(&self, table_id: Uuid) -> Vec<&Relationship> {
        self.model
            .as_ref()
            .map(|m| {
                m.relationships
                    .iter()
                    .filter(|r| r.source_table_id == table_id || r.target_table_id == table_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Update a relationship.
    ///
    /// Note: cardinality is `Option<Option<Cardinality>>` where:
    /// - None = field not provided, don't update
    /// - Some(None) = clear the cardinality
    /// - Some(Some(c)) = set the cardinality
    #[allow(clippy::too_many_arguments)]
    pub fn update_relationship(
        &mut self,
        relationship_id: Uuid,
        cardinality: Option<Option<Cardinality>>,
        source_optional: Option<bool>,
        target_optional: Option<bool>,
        foreign_key_details: Option<ForeignKeyDetails>,
        etl_job_metadata: Option<ETLJobMetadata>,
        relationship_type: Option<RelationshipType>,
        notes: Option<String>,
    ) -> Result<Option<Relationship>> {
        let model = self
            .model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model loaded"))?;

        let relationship = model
            .relationships
            .iter_mut()
            .find(|r| r.id == relationship_id)
            .ok_or_else(|| anyhow::anyhow!("Relationship not found"))?;

        // Update cardinality - only if provided (Some(...))
        // cardinality is Option<Option<Cardinality>>:
        // - None = field not provided, don't update
        // - Some(None) = clear the cardinality
        // - Some(Some(c)) = set the cardinality
        if let Some(card) = cardinality {
            relationship.cardinality = card;
            tracing::debug!(
                "Updated relationship {} cardinality to: {:?}",
                relationship_id,
                relationship.cardinality
            );
        }
        // Update optional/mandatory flags
        if source_optional.is_some() {
            relationship.source_optional = source_optional;
        }
        if target_optional.is_some() {
            relationship.target_optional = target_optional;
        }
        if let Some(fk) = foreign_key_details {
            relationship.foreign_key_details = Some(fk);
        }
        if let Some(etl) = etl_job_metadata {
            relationship.etl_job_metadata = Some(etl);
        }
        if let Some(rt) = relationship_type {
            relationship.relationship_type = Some(rt);
        }
        if notes.is_some() {
            relationship.notes = notes;
        }

        relationship.updated_at = chrono::Utc::now();
        info!("Updated relationship: {}", relationship_id);
        Ok(Some(relationship.clone()))
    }

    /// Delete a relationship.
    pub fn delete_relationship(&mut self, relationship_id: Uuid) -> Result<bool> {
        let model = self
            .model
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No model loaded"))?;

        let initial_len = model.relationships.len();
        model.relationships.retain(|r| r.id != relationship_id);

        if model.relationships.len() < initial_len {
            info!("Deleted relationship: {}", relationship_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if adding a relationship would create a circular dependency.
    pub fn check_circular_dependency(
        &self,
        source_table_id: Uuid,
        target_table_id: Uuid,
    ) -> Result<(bool, Option<Vec<Uuid>>)> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No model loaded"))?;

        // Build graph from existing relationships
        let mut graph = DiGraphMap::<Uuid, ()>::new();

        // Add all existing relationships
        for rel in &model.relationships {
            graph.add_edge(rel.source_table_id, rel.target_table_id, ());
        }

        // Add the proposed relationship temporarily
        graph.add_edge(source_table_id, target_table_id, ());

        // Check for cycles
        let is_circular = is_cyclic_directed(&graph);

        if is_circular {
            // Try to find the cycle path
            let cycle_path = self.find_cycle_path(&graph, source_table_id, target_table_id);
            Ok((true, cycle_path))
        } else {
            Ok((false, None))
        }
    }

    /// Find cycle path in the graph (simplified - returns path if found).
    fn find_cycle_path(
        &self,
        graph: &DiGraphMap<Uuid, ()>,
        start: Uuid,
        end: Uuid,
    ) -> Option<Vec<Uuid>> {
        // Simple DFS to find path from end back to start
        use std::collections::HashSet;

        let mut visited = HashSet::new();
        let mut path = Vec::new();

        fn dfs(
            graph: &DiGraphMap<Uuid, ()>,
            current: Uuid,
            target: Uuid,
            visited: &mut HashSet<Uuid>,
            path: &mut Vec<Uuid>,
        ) -> bool {
            if current == target && !path.is_empty() {
                path.push(current);
                return true;
            }

            if visited.contains(&current) {
                return false;
            }

            visited.insert(current);
            path.push(current);

            for edge in graph.edges(current) {
                let neighbor = edge.1;
                if dfs(graph, neighbor, target, visited, path) {
                    return true;
                }
            }

            path.pop();
            false
        }

        if dfs(graph, end, start, &mut visited, &mut path) {
            Some(path)
        } else {
            None
        }
    }
}
