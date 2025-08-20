//! Module for creating and analysing commodity graphs
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::process::{ProcessID, ProcessMap};
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::Dimensionless;
use anyhow::{anyhow, ensure, Context, Result};
use indexmap::IndexSet;
use itertools::{iproduct, Itertools};
use petgraph::algo::toposort;
use petgraph::graph::Graph;
use petgraph::Directed;
use std::collections::HashMap;

/// A graph of commodity flows for a given region and year
type CommoditiesGraph = Graph<CommodityID, ProcessID, Directed>;

/// Creates a graph of commodity flows for a given region and year
fn create_commodities_graph_for_region_year(
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

/// Creates a time-slice filtered version of an existing commodity graph
fn create_commodities_graph_for_region_year_timeslice(
    graph: &CommoditiesGraph,
    processes: &ProcessMap,
    region_id: &RegionID,
    year: u32,
    time_slice: &TimeSliceID,
) -> CommoditiesGraph {
    let mut filtered_graph = graph.clone();

    // Remove edges for processes with zero availability in this time slice
    let edges_to_remove: Vec<_> = filtered_graph
        .edge_indices()
        .filter(|&edge_idx| {
            let process_id = filtered_graph.edge_weight(edge_idx).unwrap();
            let process = processes.get(process_id).unwrap();
            let process_avail = process
                .activity_limits
                .get(&(region_id.clone(), year, time_slice.clone()))
                .unwrap();
            *process_avail.end() <= Dimensionless(0.0)
        })
        .collect();

    for edge_idx in edges_to_remove {
        filtered_graph.remove_edge(edge_idx);
    }

    filtered_graph
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
fn validate_commodities_graph(graph: &CommoditiesGraph, commodities: &CommodityMap) -> Result<()> {
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
fn topo_sort_commodities(
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

/// Builds and validates commodity graphs for all regions and years
///
/// This function creates commodity graphs for each region/year combination,
/// validates them, determines commodity ordering via topological sort,
/// and validates time-slice specific graphs.
///
/// # Arguments
///
/// * `processes` - Map of all processes
/// * `commodities` - Map of all commodities
/// * `region_ids` - Collection of region IDs
/// * `years` - Collection of years
/// * `time_slice_info` - Time slice information
///
/// # Returns
///
/// A tuple containing:
/// * `HashMap<(RegionID, u32), CommoditiesGraph>` - Commodity graphs for each region/year
/// * `HashMap<(RegionID, u32), Vec<CommodityID>>` - Commodity ordering for each region/year
pub fn build_and_validate_commodity_graphs_for_model(
    processes: &crate::process::ProcessMap,
    commodities: &crate::commodity::CommodityMap,
    region_ids: &IndexSet<RegionID>,
    years: &[u32],
    time_slice_info: &crate::time_slice::TimeSliceInfo,
) -> Result<HashMap<(RegionID, u32), Vec<CommodityID>>> {
    // Build commodity graphs for each region and year
    let commodity_graphs: HashMap<(RegionID, u32), CommoditiesGraph> =
        iproduct!(region_ids, years.iter())
            .map(|(region_id, year)| {
                let graph = create_commodities_graph_for_region_year(processes, region_id, *year);
                ((region_id.clone(), *year), graph)
            })
            .collect();

    // Validate graphs and determine commodity ordering for each region and year
    let commodity_order: HashMap<(RegionID, u32), Vec<CommodityID>> = commodity_graphs
        .iter()
        .map(|((region_id, year), graph)| -> Result<_> {
            validate_commodities_graph(graph, commodities).with_context(|| {
                format!("Error validating commodity graph for {region_id} in {year}")
            })?;
            let order = topo_sort_commodities(graph, commodities)
                .with_context(|| format!("Error with commodity graph for {region_id} in {year}"))?;
            Ok(((region_id.clone(), *year), order))
        })
        .try_collect()?;

    // Validate graphs in each time slice
    for ((region_id, year), base_graph) in &commodity_graphs {
        for time_slice in time_slice_info.iter_ids() {
            let time_slice_graph = create_commodities_graph_for_region_year_timeslice(
                base_graph, processes, region_id, *year, time_slice,
            );

            validate_commodities_graph(&time_slice_graph, commodities).with_context(|| {
                format!("Error validating commodity graph for {region_id} in {year} at time slice {time_slice}")
            })?;
        }
    }

    Ok(commodity_order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{assert_error, other_commodity, sed_commodity, svd_commodity};
    use petgraph::graph::Graph;
    use std::rc::Rc;

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

        // Create commodities map using fixtures
        let mut commodities = CommodityMap::new();
        commodities.insert(CommodityID::from("A"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("C"), Rc::new(svd_commodity()));

        let result = topo_sort_commodities(&graph, &commodities).unwrap();

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

        // Create commodities map using fixtures
        let mut commodities = CommodityMap::new();
        commodities.insert(CommodityID::from("A"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));

        // This should return an error due to the cycle
        // The error message should flag commodity B
        // Note: A is also involved in the cycle, but B is flagged as it is encountered first
        let result = topo_sort_commodities(&graph, &commodities);
        assert_error!(result, "Cycle detected in commodity graph for commodity B");
    }

    #[test]
    fn test_validate_commodities_graph() {
        let mut graph = Graph::new();
        let mut commodities = CommodityMap::new();

        // Add test commodities
        commodities.insert(CommodityID::from("A"), Rc::new(other_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("C"), Rc::new(svd_commodity()));

        // Test valid graph: A(OTH) -> B(SED) -> C(SVD)
        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));
        let node_c = graph.add_node(CommodityID::from("C"));
        graph.add_edge(node_a, node_b, ProcessID::from("process1"));
        graph.add_edge(node_b, node_c, ProcessID::from("process2"));

        let result = validate_commodities_graph(&graph, &commodities);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_commodities_graph_invalid_svd_consumed() {
        let mut graph = Graph::new();
        let mut commodities = CommodityMap::new();

        commodities.insert(CommodityID::from("A"), Rc::new(svd_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("C"), Rc::new(other_commodity()));

        // Test invalid graph: C(OTH) -> A(SVD) -> B(SED) - SVD cannot be consumed
        let node_c = graph.add_node(CommodityID::from("C"));
        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));
        graph.add_edge(node_c, node_a, ProcessID::from("process1"));
        graph.add_edge(node_a, node_b, ProcessID::from("process2"));

        let result = validate_commodities_graph(&graph, &commodities);
        assert_error!(result, "SVD commodity A cannot be consumed");
    }

    #[test]
    fn test_validate_commodities_graph_invalid_svd_not_produced() {
        let mut graph = Graph::new();
        let mut commodities = CommodityMap::new();

        commodities.insert(CommodityID::from("A"), Rc::new(svd_commodity()));

        // Test invalid graph: A(SVD) with no incoming edges - SVD must be produced
        let _node_a = graph.add_node(CommodityID::from("A"));

        let result = validate_commodities_graph(&graph, &commodities);
        assert_error!(result, "SVD commodity A must have at least one producer");
    }

    #[test]
    fn test_validate_commodities_graph_invalid_sed() {
        let mut graph = Graph::new();
        let mut commodities = CommodityMap::new();

        commodities.insert(CommodityID::from("A"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));

        // Test invalid graph: B(SED) -> A(SED)
        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));
        graph.add_edge(node_b, node_a, ProcessID::from("process1"));

        let result = validate_commodities_graph(&graph, &commodities);
        assert_error!(result, "SED commodity B is consumed but has no producers");
    }

    #[test]
    fn test_validate_commodities_graph_invalid_oth() {
        let mut graph = Graph::new();
        let mut commodities = CommodityMap::new();

        commodities.insert(CommodityID::from("A"), Rc::new(other_commodity()));
        commodities.insert(CommodityID::from("B"), Rc::new(sed_commodity()));
        commodities.insert(CommodityID::from("C"), Rc::new(sed_commodity()));

        // Test invalid graph: B(SED) -> A(OTH) -> C(SED)
        let node_a = graph.add_node(CommodityID::from("A"));
        let node_b = graph.add_node(CommodityID::from("B"));
        let node_c = graph.add_node(CommodityID::from("C"));
        graph.add_edge(node_b, node_a, ProcessID::from("process1"));
        graph.add_edge(node_a, node_c, ProcessID::from("process2"));

        let result = validate_commodities_graph(&graph, &commodities);
        assert_error!(
            result,
            "OTH commodity A cannot have both producers and consumers"
        );
    }
}
