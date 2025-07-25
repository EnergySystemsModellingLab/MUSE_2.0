//! Module for creating and analysing commodity graphs
use crate::commodity::CommodityID;
use crate::process::{ProcessID, ProcessMap};
use crate::region::RegionID;
use crate::units::FlowPerActivity;
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
                .filter(|flow| flow.coeff < FlowPerActivity(0.0))
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
pub fn topo_sort_commodities(graph: &CommoditiesGraph) -> Vec<CommodityID> {
    // Will panic if there are cycles
    let order = toposort(graph, None).unwrap();

    // Return the commodities in the order of the topological sort
    // We return the order in reverse so that leaf-node commodities are solved first
    order
        .iter()
        .rev()
        .map(|node| graph.node_weight(*node).unwrap().clone())
        .collect()
}
