//! Code for performing agent investment.
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::CommodityType;
use crate::model::Model;
use log::info;
use std::collections::HashSet;

pub mod utilisation;
use utilisation::calculate_potential_utilisation_svd;

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
            let _potentials = calculate_potential_utilisation_svd(
                agent,
                commodity,
                year,
                &model.time_slice_info,
                assets,
                prices,
                &utilisations,
            );
        }

        // **TODO:** Implement rest of agent investment
    }

    // **PLACEHOLDER:** Keep all assets
    let mut assets_to_keep = HashSet::new();
    for (asset_id, _commodity_id, _time_slice, _flow) in solution.iter_commodity_flows_for_assets()
    {
        if assets.get(asset_id).is_none() {
            // Asset has been decommissioned
            continue;
        }

        assets_to_keep.insert(asset_id);
    }

    // Decommission non-selected assets
    assets.retain(&assets_to_keep);
}
