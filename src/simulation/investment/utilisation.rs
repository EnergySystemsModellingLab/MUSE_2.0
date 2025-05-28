//! Code for calculating potential utilisation for assets
use super::super::marginal_cost::marginal_cost_for_process;
use super::super::optimisation::UtilisationMap;
use super::MarginalCosts;
use crate::agent::Agent;
use crate::asset::AssetID;
use crate::commodity::Commodity;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use itertools::iproduct;
use std::collections::HashMap;

/// Calculate the potential utilisation for assets for a single agent
pub fn calculate_potential_utilisation_for_assets<'a, F>(
    agent: &'a Agent,
    commodity: &'a Commodity,
    time_slice_info: &'a TimeSliceInfo,
    utilisations: &'a UtilisationMap,
    marginal_costs: &'a MarginalCosts,
    get_demand: F,
) -> impl Iterator<Item = (AssetID, &'a RegionID, &'a TimeSliceID, f64)>
where
    F: Fn(&RegionID, &TimeSliceID) -> f64,
{
    calculate_potential_utilisation(
        agent,
        commodity,
        time_slice_info,
        utilisations,
        marginal_costs,
        get_demand,
        costs_to_utilisation_for_assets,
    )
}

/// Calculate the potential utilisation all processes in an agent's search space
pub fn calculate_potential_utilisation_for_processes<'a, F>(
    agent: &'a Agent,
    year: u32,
    commodity: &'a Commodity,
    time_slice_info: &'a TimeSliceInfo,
    utilisations: &'a UtilisationMap,
    marginal_costs: &'a MarginalCosts,
    get_demand: F,
) -> impl Iterator<Item = (AssetID, &'a RegionID, &'a TimeSliceID, f64)>
where
    F: Fn(&RegionID, &TimeSliceID) -> f64,
{
    let search_space = agent
        .search_space
        .get(&(commodity.id.clone(), year))
        .unwrap();
    let costs_to_utilisation = |region_id, time_slice, demand, marginal_costs, utilisation| {
        search_space.iter().map(|process| {
            let marginal_cost = marginal_cost_for_process(
                process,
                region_id,
                &commodity.id,
                year,
                year,
                time_slice,
                prices,
            );
            let utilisation =
                potential_utilisation(demand, marginal_cost, marginal_costs, utilisations);
        })
    };
    calculate_potential_utilisation(
        agent,
        commodity,
        time_slice_info,
        utilisations,
        marginal_costs,
        get_demand,
        costs_to_utilisation,
    )
}

/// Calculate the potential utilisation for a single agent
fn calculate_potential_utilisation<'a, F, G, I, T>(
    agent: &'a Agent,
    commodity: &'a Commodity,
    time_slice_info: &'a TimeSliceInfo,
    utilisations: &'a UtilisationMap,
    marginal_costs: &'a MarginalCosts,
    get_demand: F,
    costs_to_utilisations: G,
) -> impl Iterator<Item = (T, &'a RegionID, &'a TimeSliceID, f64)>
where
    F: Fn(&RegionID, &TimeSliceID) -> f64,
    G: Fn(&'a RegionID, &'a TimeSliceID, f64, &'a [(AssetID, f64)], &'a HashMap<AssetID, f64>) -> I,
    I: Iterator<Item = (T, f64)>,
{
    iproduct!(time_slice_info.iter_ids(), agent.regions.iter()).flat_map(
        move |(time_slice, region_id)| {
            let marginal_costs = marginal_costs
                .get(&(region_id.clone(), time_slice.clone()))
                .unwrap();

            // Calculate share of demand for this agent
            let demand = get_demand(region_id, time_slice);

            let utilisations = utilisations
                .get(&(commodity.id.clone(), time_slice.clone()))
                .unwrap();

            costs_to_utilisations(region_id, time_slice, demand, marginal_costs, utilisations)
                .map(move |(ret, utilisation)| (ret, region_id, time_slice, utilisation))
        },
    )
}

/// Calculate potential utilisation for a single asset/process
fn potential_utilisation(
    demand: f64,
    marginal_cost: f64,
    marginal_costs: &[(AssetID, f64)],
    utilisations: &HashMap<AssetID, f64>,
) -> f64 {
    let cheaper_assets = marginal_costs
        .iter()
        .take_while(|(_, cost)| *cost <= marginal_cost)
        .map(|(id, _)| id);
    let cheaper_demand = cheaper_assets
        .map(|id| utilisations.get(id).unwrap())
        .sum::<f64>();
    let remaining_demand = demand - cheaper_demand;
    assert!(remaining_demand >= 0.0);

    remaining_demand
}

fn costs_to_utilisation_for_assets<'a>(
    _region_id: &RegionID,
    _time_slice
    demand: f64,
    marginal_costs: &'a [(AssetID, f64)],
    utilisations: &'a HashMap<AssetID, f64>,
) -> impl Iterator<Item = (AssetID, f64)> + 'a {
    marginal_costs
        .iter()
        .copied()
        .map(move |(asset_id, marginal_cost)| {
            let utilisation =
                potential_utilisation(demand, marginal_cost, marginal_costs, utilisations);

            (asset_id, utilisation)
        })
}
