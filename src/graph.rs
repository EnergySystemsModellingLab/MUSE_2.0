//! Module for creating and analysing commodity graphs
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::process::{ProcessID, ProcessMap};
use crate::region::RegionID;
use anyhow::{anyhow, ensure, Result};
use itertools::iproduct;
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

    // Create _SOURCE and _SINK commodity IDs
    // We use these as mock commodities for processes that have no inputs or outputs
    let source_id = CommodityID::from("_SOURCE");
    let sink_id = CommodityID::from("_SINK");

    let key = (region_id.clone(), year);
    for process in processes.values() {
        let Some(flows) = process.flows.get(&key) else {
            // Process doesn't operate in this region/year
            continue;
        };

        // Get output flows for the process
        let outputs: Vec<_> = flows
            .values()
            .filter(|flow| flow.is_output())
            .map(|flow| &flow.commodity.id)
            .collect();

        // Get input flows for the process
        let inputs: Vec<_> = flows
            .values()
            .filter(|flow| flow.is_input())
            .map(|flow| &flow.commodity.id)
            .collect();

        // Use _SOURCE if no inputs, _SINK if no outputs
        let inputs = if inputs.is_empty() {
            vec![&source_id]
        } else {
            inputs
        };
        let outputs = if outputs.is_empty() {
            vec![&sink_id]
        } else {
            outputs
        };

        // Create edges from all inputs to all outputs
        // We also create nodes for commodities the first time they are encountered
        for (input, output) in iproduct!(inputs, outputs) {
            let source_node = *commodity_to_node_index
                .entry(input.clone())
                .or_insert_with(|| graph.add_node(input.clone()));
            let target_node = *commodity_to_node_index
                .entry((*output).clone())
                .or_insert_with(|| graph.add_node((*output).clone()));
            graph.add_edge(source_node, target_node, process.id.clone());
        }
    }

    graph
}

/// Validates that the commodity graph follows the rules for different commodity types
///
/// # Arguments
///
/// * `graph` - The commodity flow graph to validate
/// * `commodities` - Map of commodities with their types
///
/// # Returns
///
/// `Ok(())` if validation passes, or an error describing the violation
///
/// # Rules
///
/// - **SVD type commodities**: Must have at least one incoming edge (produced) and no outgoing edges (not consumed)
/// - **SED type commodities**: If they have outgoing edges (consumed), they must also have incoming edges (produced)
/// - **OTH type commodities**: Can have incoming or outgoing edges, or neither, but not both
pub fn validate_commodities_graph(
    graph: &CommoditiesGraph,
    commodities: &CommodityMap,
) -> Result<()> {
    for node_idx in graph.node_indices() {
        let commodity_id = graph.node_weight(node_idx).unwrap();

        // Skip _SOURCE and _SINK commodities
        if commodity_id == &CommodityID::from("_SOURCE")
            || commodity_id == &CommodityID::from("_SINK")
        {
            continue;
        }

        let incoming = graph
            .edges_directed(node_idx, petgraph::Direction::Incoming)
            .count();
        let outgoing = graph
            .edges_directed(node_idx, petgraph::Direction::Outgoing)
            .count();

        // Match validation rules to commodity type
        let commodity = commodities.get(commodity_id).unwrap();
        match commodity.kind {
            CommodityType::ServiceDemand => {
                // SVD: must be produced (incoming edges) but not consumed (no outgoing edges)
                ensure!(
                    incoming > 0,
                    "SVD commodity {} must have at least one producer",
                    commodity_id
                );
                ensure!(
                    outgoing == 0,
                    "SVD commodity {} cannot be consumed",
                    commodity_id
                );
            }
            CommodityType::SupplyEqualsDemand => {
                // SED: if consumed (outgoing edges), must also be produced (incoming edges)
                ensure!(
                    !(outgoing > 0 && incoming == 0),
                    "SED commodity {} is consumed but has no producers",
                    commodity_id
                );
            }
            CommodityType::Other => {
                // OTH: cannot have both incoming and outgoing edges
                ensure!(
                    !(incoming > 0 && outgoing > 0),
                    "OTH commodity {} cannot have both producers and consumers",
                    commodity_id
                );
            }
        }
    }

    Ok(())
}

/// Performs topological sort on the commodity graph
pub fn topo_sort_commodities(
    graph: &CommoditiesGraph,
    commodities: &CommodityMap,
) -> Result<Vec<CommodityID>> {
    // Perform a topological sort on the graph
    let order = toposort(graph, None).map_err(|cycle| {
        let cycle_commodity = graph.node_weight(cycle.node_id()).unwrap().clone();
        anyhow!(
            "Cycle detected in commodity graph for commodity {}",
            cycle_commodity
        )
    })?;

    // We return the order in reverse so that leaf-node commodities are solved first
    // We also filter to only include SVD and SED commodities
    let order = order
        .iter()
        .rev()
        .filter_map(|node_idx| {
            let commodity_id = graph.node_weight(*node_idx)?;
            let commodity = commodities.get(commodity_id)?;
            if matches!(
                commodity.kind,
                CommodityType::ServiceDemand | CommodityType::SupplyEqualsDemand
            ) {
                Some(commodity_id.clone())
            } else {
                None
            }
        })
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

        // The error message should flag commodity B
        // Note: A is also involved in the cycle, but B is flagged as it is encountered first
        let error_msg = result.unwrap_err().to_string();
        assert_eq!(
            error_msg,
            "Cycle detected in commodity graph for commodity B"
        );
    }
}
