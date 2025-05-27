//! Code for performing agent investment.
use super::marginal_cost::marginal_cost_for_asset;
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::asset::{Asset, AssetID, AssetPool};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use itertools::Itertools;
use log::info;
use std::collections::HashMap;
use std::rc::Rc;

pub mod utilisation;
use utilisation::calculate_potential_utilisation;

/// Marginal costs for the specified region + time slice for a given agent
pub type MarginalCosts = HashMap<(RegionID, TimeSliceID), Vec<(AssetID, f64)>>;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `solution` - The solution to the dispatch optimisation
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
/// * `year` - The current year of the simulation
pub fn perform_agent_investment(
    model: &Model,
    solution: &Solution,
    prices: &CommodityPrices,
    assets: &mut AssetPool,
    year: u32,
) {
    info!("Performing agent investment...");

    let utilisations = solution.create_utilisation_map();

    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::ServiceDemand {
            continue;
        }

        for agent in model.agents.values() {
            let Some(&commodity_portion) =
                agent.commodity_portions.get(&(commodity.id.clone(), year))
            else {
                // The agent isn't responsible for any of the demand
                continue;
            };

            let marginal_costs = get_marginal_costs(
                &model.time_slice_info,
                assets.iter_for_agent(&agent.id),
                prices,
                &commodity.id,
                year,
            );

            let get_demand = |region_id: &RegionID, time_slice: &TimeSliceID| {
                commodity_portion
                    * commodity
                        .demand
                        .get(&(region_id.clone(), year, time_slice.clone()))
                        .unwrap()
            };

            for (asset_id, region_id, time_slice, utilisation) in calculate_potential_utilisation(
                agent,
                commodity,
                &model.time_slice_info,
                &utilisations,
                &marginal_costs,
                get_demand,
            ) {
                // **TODO:** Do something with these values (e.g. store them)

                // The asset is constrained on how much demand it can serve by capacity and
                // availability
                let asset = assets.get(asset_id).unwrap();
                let max_utilisation = asset.capacity
                    * asset
                        .process
                        .energy_limits
                        .get(&(region_id.clone(), year, time_slice.clone()))
                        .unwrap()
                        .end();

                let _utilisation = max_utilisation.min(utilisation);
            }
        }

        // **TODO:** Implement rest of agent investment
    }

    // **PLACEHOLDER:** Keep all assets
    let mut new_pool = Vec::new();
    for (asset_id, _commodity_id, _time_slice, _flow) in solution.iter_commodity_flows_for_assets()
    {
        let Some(asset) = assets.get(asset_id) else {
            // Asset has been decommissioned
            continue;
        };

        // **TODO**: Implement agent investment. For now, just keep all assets.
        new_pool.push(asset.clone());
    }

    assets.replace_active_pool(new_pool);
}

/// Get marginal costs for the specified assets and sort.
///
/// Assets which do not produce `commodity_of_interest` are not included.
fn get_marginal_costs<'a, I>(
    time_slice_info: &TimeSliceInfo,
    assets: I,
    prices: &CommodityPrices,
    commodity_of_interest: &CommodityID,
    year: u32,
) -> MarginalCosts
where
    I: Iterator<Item = &'a Rc<Asset>>,
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
        .flat_map(|asset| {
            time_slice_info.iter_ids().map(move |time_slice| {
                let key = (asset.region_id.clone(), time_slice.clone());
                let value = (
                    asset.id,
                    marginal_cost_for_asset(asset, commodity_of_interest, year, time_slice, prices),
                );

                (key, value)
            })
        })
        .into_group_map();

    for costs in costs.values_mut() {
        costs.sort_by(|(_, cost1), (_, cost2)| cost1.partial_cmp(cost2).unwrap());
    }

    costs
}
