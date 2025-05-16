//! Code for calculating potential utilisation for assets
use super::super::optimisation::UtilisationMap;
use super::CommodityPrices;
use crate::agent::{Agent, AgentID};
use crate::asset::{Asset, AssetID, AssetPool};
use crate::commodity::{Commodity, CommodityID, CommodityType};
use crate::model::Model;
use crate::simulation::marginal_cost::marginal_cost_for_asset;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use itertools::{iproduct, Itertools};
use std::collections::HashMap;

/// Potential utilisation
type PotentialUtilisationMap = HashMap<(AgentID, AssetID, CommodityID, TimeSliceID), f64>;

/// Calculate the potential utilisation for assets
pub fn calculate_potential_utilisation(
    model: &Model,
    utilisations: &UtilisationMap,
    assets: &AssetPool,
    prices: &CommodityPrices,
    year: u32,
) -> PotentialUtilisationMap {
    let mut potentials = PotentialUtilisationMap::new();
    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::ServiceDemand {
            continue;
        }

        for agent in model.agents.values() {
            calculate_potential_utilisation_for_agent(
                &mut potentials,
                agent,
                commodity,
                year,
                &model.time_slice_info,
                assets,
                prices,
                utilisations,
            );
        }
    }

    potentials
}

/// Calculate the potential utilisation for a single agent, appending results to `potentials`
#[allow(clippy::too_many_arguments)]
fn calculate_potential_utilisation_for_agent(
    potentials: &mut PotentialUtilisationMap,
    agent: &Agent,
    commodity: &Commodity,
    year: u32,
    time_slice_info: &TimeSliceInfo,
    assets: &AssetPool,
    prices: &CommodityPrices,
    utilisations: &UtilisationMap,
) {
    let Some(&commodity_portion) = agent.commodity_portions.get(&(commodity.id.clone(), year))
    else {
        // The agent isn't responsible for any of the demand
        return;
    };

    for (time_slice, region_id) in iproduct!(time_slice_info.iter_ids(), agent.regions.iter()) {
        let marginal_costs = get_marginal_costs_sorted(
            assets.iter_for_region_and_agent(region_id, &agent.id),
            prices,
            &commodity.id,
            year,
            time_slice,
        );

        // Calculate share of demand for this agent
        let demand = commodity_portion
            * commodity
                .demand
                .get(&(region_id.clone(), year, time_slice.clone()))
                .unwrap();

        let utilisations = utilisations
            .get(&(commodity.id.clone(), time_slice.clone()))
            .unwrap();

        for &(asset, marginal_cost) in marginal_costs.iter() {
            // The asset is constrained on how much demand it can serve by capacity and availability
            let max_utilisation = asset.capacity
                * asset
                    .process
                    .energy_limits
                    .get(&(region_id.clone(), year, time_slice.clone()))
                    .unwrap()
                    .end();

            let value = max_utilisation.min(calculate_potential_utilisation_for_asset(
                demand,
                marginal_cost,
                &marginal_costs,
                utilisations,
            ));

            potentials.insert(
                (
                    agent.id.clone(),
                    asset.id,
                    commodity.id.clone(),
                    time_slice.clone(),
                ),
                value,
            );
        }
    }
}

/// Get marginal costs for the specified assets and sort.
///
/// Assets which do not produce `commodity_of_interest` are not included.
fn get_marginal_costs_sorted<'a, I>(
    assets: I,
    prices: &CommodityPrices,
    commodity_of_interest: &CommodityID,
    year: u32,
    time_slice: &TimeSliceID,
) -> Vec<(&'a Asset, f64)>
where
    I: Iterator<Item = &'a Asset>,
{
    let mut costs = assets
        .filter(|asset| {
            // Ignore commodities which don't produce commodity_of_interest
            if let Some(flow) = asset.get_flow(commodity_of_interest) {
                flow.flow > 0.0
            } else {
                false
            }
        })
        .map(|asset| {
            (
                asset,
                marginal_cost_for_asset(asset, commodity_of_interest, year, time_slice, prices),
            )
        })
        .collect_vec();
    costs.sort_by(|(_, cost1), (_, cost2)| cost1.partial_cmp(cost2).unwrap());
    costs
}

/// Calculate potential utilisation for a single asset
fn calculate_potential_utilisation_for_asset(
    demand: f64,
    marginal_cost: f64,
    marginal_costs: &[(&Asset, f64)],
    utilisations: &HashMap<AssetID, f64>,
) -> f64 {
    let cheaper_assets = marginal_costs
        .iter()
        .take_while(|(_, cost)| *cost <= marginal_cost)
        .map(|(asset, _)| &asset.id);
    let cheaper_demand = cheaper_assets
        .map(|id| utilisations.get(id).unwrap())
        .sum::<f64>();
    let remaining_demand = demand - cheaper_demand;
    assert!(remaining_demand >= 0.0);

    remaining_demand
}
