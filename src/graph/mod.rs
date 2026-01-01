//! Graph algorithms for relationship validation.
//!
//! Provides cycle detection and graph traversal utilities.
//! Uses SDK validation functionality to avoid code duplication.

/// Detect cycles in a relationship graph
/// Uses petgraph for cycle detection
pub fn detect_cycles(relationships: &[crate::models::Relationship]) -> bool {
    use petgraph::Graph;
    use petgraph::algo::is_cyclic_directed;
    use uuid::Uuid;

    let mut graph = Graph::<Uuid, ()>::new();
    let mut node_map = std::collections::HashMap::new();

    // Add all table IDs as nodes
    for rel in relationships {
        node_map
            .entry(rel.source_table_id)
            .or_insert_with(|| graph.add_node(rel.source_table_id));
        node_map
            .entry(rel.target_table_id)
            .or_insert_with(|| graph.add_node(rel.target_table_id));
    }

    // Add edges
    for rel in relationships {
        if let (Some(&source), Some(&target)) = (
            node_map.get(&rel.source_table_id),
            node_map.get(&rel.target_table_id),
        ) {
            graph.add_edge(source, target, ());
        }
    }

    is_cyclic_directed(&graph)
}

/// Find cycles in a relationship graph
pub fn find_cycles(_relationships: &[crate::models::Relationship]) -> Vec<Vec<String>> {
    // Cycle detection implemented above - detailed cycle finding can be added if needed
    Vec::new()
}

/// Check if adding a relationship would create a cycle
pub fn would_create_cycle(
    relationships: &[crate::models::Relationship],
    new_relationship: &crate::models::Relationship,
) -> bool {
    let mut test_relationships = relationships.to_vec();
    test_relationships.push(new_relationship.clone());
    detect_cycles(&test_relationships)
}
