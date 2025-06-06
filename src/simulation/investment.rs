//! Code for performing agent investment.
use super::marginal_cost::marginal_cost_for_asset;
use super::optimisation::FlowMap;
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::asset::{Asset, AssetID, AssetPool};
use crate::asset::{Asset, AssetPool};
use crate::commodity::{Commodity, CommodityType};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::simulation::optimisation::UtilisationMap;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::time_slice::{TimeSliceID, TimeSliceLevel};
use itertools::Itertools;
use log::info;
use std::collections::HashMap;
use std::collections::HashMap;
use std::rc::Rc;

pub mod utilisation;
use utilisation::calculate_potential_utilisation_for_assets;

/// Marginal costs for the specified region + time slice for a given agent
pub type MarginalCosts = HashMap<(RegionID, TimeSliceID), Vec<(Rc<Asset>, f64)>>;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `flow_map` - Map of commodity flows
/// * `utilisations` - Map of utilisations
/// * `prices` - Commodity prices
/// * `year` - The current milestone year
pub fn perform_agent_investment(
    model: &Model,
    assets: &mut AssetPool,
    _flow_map: &FlowMap,
    utilisations: &UtilisationMap,
    prices: &CommodityPrices,
    year: u32,
) {
    info!("Performing agent investment...");

    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We look at SVD commodities first
            continue;
        }

        // Calculate demand per time slice
        let _demand = get_or_estimate_demand_per_time_slice(model, commodity, year);

        for agent in model.agents.values() {
            let Some(&commodity_portion) =
                agent.commodity_portions.get(&(commodity.id.clone(), year))
            else {
                // The agent isn't responsible for any of the demand
                continue;
            };

            let marginal_costs = get_marginal_costs(
                &model.time_slice_info,
                assets.iter_for_agent(&agent.id).cloned(),
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

            for (asset, region_id, time_slice, utilisation) in
                calculate_potential_utilisation_for_assets(
                    agent,
                    commodity,
                    &model.time_slice_info,
                    &utilisations,
                    &marginal_costs,
                    get_demand,
                )
            {
                // **TODO:** Do something with these values (e.g. store them)

                // The asset is constrained on how much demand it can serve by capacity and
                // availability
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
    }

    let mut new_pool = Vec::new();
    for asset in assets.iter() {
        // **TODO**: Implement agent investment. For now, just keep all assets.
        new_pool.push(asset.clone().into());
    }

    assets.replace_active_pool(new_pool);
}

/// Get or estimate demand per time slice for the given commodity.
///
/// For commodities with a time slice level of time slice, this information is provided by the user.
/// If the time slice level is seasonal or annual, we assume the demand is uniformly distributed for
/// either each season or the whole year.
pub fn get_or_estimate_demand_per_time_slice(
    model: &Model,
    commodity: &Commodity,
    year: u32,
) -> HashMap<(RegionID, TimeSliceID), f64> {
    // Sanity check
    assert!(commodity.kind == CommodityType::ServiceDemand);

    let mut map = HashMap::new();
    for region_id in model.iter_regions() {
        for ts_selection in model
            .time_slice_info
            .iter_selections_at_level(commodity.time_slice_level)
        {
            let demand_for_selection = *commodity
                .demand
                .get(&(region_id.clone(), year, ts_selection.clone()))
                .unwrap();

            // Assume demand for `ts_selection` is uniformly distributed between time slices
            let demand_iter = model
                .time_slice_info
                .calculate_share(
                    &ts_selection,
                    TimeSliceLevel::DayNight,
                    demand_for_selection,
                )
                .unwrap();

            for (ts_selection, demand) in demand_iter {
                // Safe: we know this is a time slice
                let time_slice = ts_selection.try_into().unwrap();
                map.insert((region_id.clone(), time_slice), demand);
            }
        }
    }

    map
}

/// Get marginal costs for the specified assets and sort.
///
/// Assets which do not produce `commodity_of_interest` are not included.
fn get_marginal_costs<I>(
    time_slice_info: &TimeSliceInfo,
    assets: I,
    prices: &CommodityPrices,
    commodity_of_interest: &CommodityID,
    year: u32,
) -> MarginalCosts
where
    I: IntoIterator<Item = Rc<Asset>>,
{
    let mut costs = assets
        .into_iter()
        .filter(|asset| {
            // Ignore assets which don't produce commodity_of_interest
            if let Some(flow) = asset.get_flow(commodity_of_interest) {
                flow.flow > 0.0
            } else {
                false
            }
        })
        .flat_map(|asset| {
            time_slice_info.iter_ids().map(move |time_slice| {
                let key = (asset.region_id.clone(), time_slice.clone());
                let cost = marginal_cost_for_asset(
                    &asset,
                    commodity_of_interest,
                    year,
                    time_slice,
                    prices,
                );

                // Not sure why, but compiler is insisting on a clone here
                let value = (Rc::clone(&asset), cost);

                (key, value)
            })
        })
        .into_group_map();

    for costs in costs.values_mut() {
        costs.sort_by(|(_, cost1), (_, cost2)| cost1.partial_cmp(cost2).unwrap());
    }

    costs
}
