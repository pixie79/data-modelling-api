// Graph algorithms module for circular dependency detection
use petgraph::algo;
use petgraph::graph::DiGraph;
use std::collections::HashSet;

/// Detect cycles in a directed graph
pub fn detect_cycles(graph: &DiGraph<u32, ()>) -> bool {
    // Use DFS to detect cycles
    algo::is_cyclic_directed(graph)
}

/// Find all cycles in a directed graph
pub fn find_cycles(graph: &DiGraph<u32, ()>) -> Vec<Vec<u32>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();

    for node in graph.node_indices() {
        if !visited.contains(&node.index()) {
            find_cycles_dfs(
                graph,
                node.index() as u32,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut cycles,
            );
        }
    }

    cycles
}

fn find_cycles_dfs(
    graph: &DiGraph<u32, ()>,
    node: u32,
    visited: &mut HashSet<usize>,
    rec_stack: &mut HashSet<usize>,
    path: &mut Vec<u32>,
    cycles: &mut Vec<Vec<u32>>,
) {
    let node_idx = node as usize;
    visited.insert(node_idx);
    rec_stack.insert(node_idx);
    path.push(node);

    // Check all neighbors
    if let Some(node_index) = graph.node_indices().find(|&idx| idx.index() == node_idx) {
        for neighbor in graph.neighbors(node_index) {
            let neighbor_idx = neighbor.index();
            let neighbor_val = neighbor_idx as u32;

            if !visited.contains(&neighbor_idx) {
                find_cycles_dfs(graph, neighbor_val, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(&neighbor_idx) {
                // Found a cycle
                if let Some(cycle_start) = path.iter().position(|&x| x == neighbor_val) {
                    let cycle: Vec<u32> = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }
    }

    rec_stack.remove(&node_idx);
    path.pop();
}

/// Check if adding an edge would create a cycle
pub fn would_create_cycle(graph: &DiGraph<u32, ()>, source: u32, target: u32) -> bool {
    // Create a temporary graph with the new edge
    let mut temp_graph = graph.clone();

    // Find nodes by their value, not index
    let source_idx = temp_graph
        .node_indices()
        .find(|&idx| *temp_graph.node_weight(idx).unwrap() == source)
        .unwrap_or_else(|| temp_graph.add_node(source));
    let target_idx = temp_graph
        .node_indices()
        .find(|&idx| *temp_graph.node_weight(idx).unwrap() == target)
        .unwrap_or_else(|| temp_graph.add_node(target));

    // Check if edge already exists
    if temp_graph.find_edge(source_idx, target_idx).is_some() {
        return false; // Edge exists, no new cycle
    }

    // Add the edge
    temp_graph.add_edge(source_idx, target_idx, ());

    // Check for cycles
    detect_cycles(&temp_graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(1);
        let b = graph.add_node(2);
        let c = graph.add_node(3);

        graph.add_edge(a, b, ());
        graph.add_edge(b, c, ());

        assert!(!detect_cycles(&graph));
    }

    #[test]
    fn test_detect_cycles_with_cycle() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(1);
        let b = graph.add_node(2);
        let c = graph.add_node(3);

        graph.add_edge(a, b, ());
        graph.add_edge(b, c, ());
        graph.add_edge(c, a, ()); // Creates cycle

        assert!(detect_cycles(&graph));
    }

    #[test]
    fn test_would_create_cycle() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(1);
        let b = graph.add_node(2);
        let c = graph.add_node(3);

        graph.add_edge(a, b, ());
        graph.add_edge(b, c, ());

        // Adding c -> a would create a cycle
        assert!(would_create_cycle(&graph, 3, 1));

        // Adding a -> c would not create a cycle
        assert!(!would_create_cycle(&graph, 1, 3));
    }
}
