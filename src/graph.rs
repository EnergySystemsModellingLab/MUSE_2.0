//! WIP
use crate::commodity::CommodityID;
use crate::process::{ProcessID, ProcessMap};
use crate::units::FlowPerActivity;
use petgraph::algo::toposort;
use petgraph::graph::Graph;
use petgraph::Directed;
use std::collections::HashMap;

/// WIP
pub fn create_flows_graph<'a>(
    commodity_ids: &'a [CommodityID],
    processes: &'a ProcessMap,
) -> Graph<&'a CommodityID, &'a ProcessID, Directed> {
    // Create directed graph
    let mut graph: Graph<&CommodityID, &ProcessID, Directed> = Graph::new();

    // Create nodes for commodities
    let mut commodity_to_node_index = HashMap::new();
    for commodity_id in commodity_ids {
        let node_index = graph.add_node(commodity_id);
        commodity_to_node_index.insert(commodity_id.clone(), node_index);
    }

    // Create edges from process flows
    for process in processes.values() {
        for ((_region, _year), flows) in process.flows.iter() {
            // Get primary outputs
            let mut primary_outputs = Vec::new();
            for flow in flows.values() {
                if flow.is_primary_output {
                    primary_outputs.push(flow.commodity.id.clone());
                }
            }

            // Get inputs
            let mut inputs = Vec::new();
            for flow in flows.values() {
                if flow.coeff < FlowPerActivity(0.0) {
                    inputs.push(flow.commodity.id.clone());
                }
            }

            // Create edges from inputs to primary outputs
            // TODO: need to create separate graphs for each region and year
            for input in inputs {
                for primary_output in primary_outputs.clone() {
                    graph.add_edge(
                        commodity_to_node_index[&input],
                        commodity_to_node_index[&primary_output],
                        &process.id,
                    );
                }
            }
        }
    }

    graph
}

/// WIP
pub fn topo_sort_commodities(
    graph: &Graph<&CommodityID, &ProcessID, Directed>,
) -> Vec<CommodityID> {
    // Will panic if there are cycles
    let order = toposort(graph, None).unwrap();
    order
        .iter()
        .map(|node| (*graph.node_weight(*node).unwrap()).clone())
        .collect::<Vec<_>>()
}
