//! WIP
use crate::commodity::CommodityID;
use crate::process::{ProcessID, ProcessMap};
use crate::region::RegionID;
use crate::units::FlowPerActivity;
use petgraph::algo::toposort;
use petgraph::graph::Graph;
use petgraph::Directed;
use std::collections::HashMap;

/// WIP
pub fn create_flows_graph_for_region_year(
    commodity_ids: &[CommodityID],
    processes: &ProcessMap,
    region_id: &RegionID,
    year: u32,
) -> Graph<CommodityID, ProcessID, Directed> {
    let mut graph = Graph::new();
    let mut commodity_to_node_index = HashMap::new();

    // Create nodes for commodities
    for commodity_id in commodity_ids {
        let node_index = graph.add_node(commodity_id.clone());
        commodity_to_node_index.insert(commodity_id.clone(), node_index);
    }

    // Create edges from process flows
    let key = (region_id.clone(), year);

    for process in processes.values() {
        if let Some(flows) = process.flows.get(&key) {
            // Collect primary outputs and inputs
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
            for input in inputs {
                for primary_output in &primary_outputs {
                    graph.add_edge(
                        commodity_to_node_index[&input],
                        commodity_to_node_index[primary_output],
                        process.id.clone(),
                    );
                }
            }
        }
    }

    graph
}

/// Performs topological sort on the commodity graph
pub fn topo_sort_commodities(graph: &Graph<CommodityID, ProcessID, Directed>) -> Vec<CommodityID> {
    // Will panic if there are cycles
    let order = toposort(graph, None).unwrap();
    order
        .iter()
        .map(|node| graph.node_weight(*node).unwrap().clone())
        .collect()
}
