//! Module for creating and analysing commodity graphs
use crate::commodity::CommodityID;
use crate::process::{ProcessID, ProcessMap};
use crate::region::RegionID;
use anyhow::{anyhow, Result};
use petgraph::algo::toposort;
use petgraph::graph::Graph;
use petgraph::Directed;
use std::collections::HashMap;

/// A graph of commodity flows for a given region and year
type CommoditiesGraph = Graph<CommodityID, ProcessID, Directed>;

/// Creates a graph of commodity flows for a given region and year
pub fn create_commodities_graph_for_region_year(
    processes: &ProcessMap,
    region_id: &RegionID,
    year: u32,
) -> CommoditiesGraph {
    let mut graph = Graph::new();
    let mut commodity_to_node_index = HashMap::new();

    let key = (region_id.clone(), year);
    for process in processes.values() {
        if let Some(flows) = process.flows.get(&key) {
            // Collect primary outputs and inputs for the process
            let primary_outputs: Vec<_> = flows
                .values()
                .filter(|flow| flow.is_primary_output)
                .map(|flow| flow.commodity.id.clone())
                .collect();
            let inputs: Vec<_> = flows
                .values()
                .filter(|flow| flow.is_input())
                .map(|flow| flow.commodity.id.clone())
                .collect();

            // Create edges from inputs to primary outputs
            // We also create nodes for commodities the first time they are encountered
            for input in inputs {
                let source_node = *commodity_to_node_index
                    .entry(input.clone())
                    .or_insert_with(|| graph.add_node(input.clone()));
                for primary_output in &primary_outputs {
                    let target_node = *commodity_to_node_index
                        .entry(primary_output.clone())
                        .or_insert_with(|| graph.add_node(primary_output.clone()));
                    graph.add_edge(source_node, target_node, process.id.clone());
                }
            }
        }
    }

    graph
}

/// Performs topological sort on the commodity graph
pub fn topo_sort_commodities(graph: &CommoditiesGraph) -> Result<Vec<CommodityID>> {
    // Perform a topological sort on the graph
    let order = toposort(graph, None).map_err(|cycle| {
        let cycle_commodity = graph.node_weight(cycle.node_id()).unwrap().clone();
        anyhow!(
            "Cycle detected in commodity graph for commodity: {}",
            cycle_commodity
        )
    })?;

    // We return the order in reverse so that leaf-node commodities are solved first
    let order = order
        .iter()
        .rev()
        .map(|node| graph.node_weight(*node).unwrap().clone())
        .collect();
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::Graph;

    #[test]
    fn test_topo_sort_linear_graph() {
        // Create a simple linear graph: A -> B -> C
        let mut graph = Graph::new();

        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));
        let node_c = graph.add_node(CommodityID::from("C"));

        // Add edges: A -> B -> C
        graph.add_edge(node_a, node_b, ProcessID::from("process1"));
        graph.add_edge(node_b, node_c, ProcessID::from("process2"));

        let result = topo_sort_commodities(&graph).unwrap();

        // Expected order: C, B, A (leaf nodes first)
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], CommodityID::from("C"));
        assert_eq!(result[1], CommodityID::from("B"));
        assert_eq!(result[2], CommodityID::from("A"));
    }

    #[test]
    fn test_topo_sort_cyclic_graph() {
        // Create a simple cyclic graph: A -> B -> A
        let mut graph = Graph::new();

        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));

        // Add edges creating a cycle: A -> B -> A
        graph.add_edge(node_a, node_b, ProcessID::from("process1"));
        graph.add_edge(node_b, node_a, ProcessID::from("process2"));

        // This should return an error due to the cycle
        let result = topo_sort_commodities(&graph);
        assert!(result.is_err());

        // The error message should flag either commodity A or B since both are involved in the cycle
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.starts_with("Cycle detected in commodity graph for commodity: "));
        assert!(error_msg.ends_with("A") || error_msg.ends_with("B"));
    }
}
