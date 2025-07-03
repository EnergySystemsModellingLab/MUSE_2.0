//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::optimisation::Solution;
use super::prices::{
    reduced_costs_for_existing, remove_scarcity_influence_from_candidate_reduced_costs,
};
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::model::Model;
use log::info;
use std::collections::HashMap;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `solution` - The solution to the dispatch optimisation
/// * `flow_map` - Map of commodity flows
/// * `adjusted_prices` - Commodity prices adjusted for scarcity
/// * `unadjusted_prices` - Unadjusted commodity prices
/// * `assets` - The asset pool
/// * `year` - Current milestone year
pub fn perform_agent_investment(
    model: &Model,
    solution: &Solution,
    _flow_map: &FlowMap,
    adjusted_prices: &CommodityPrices,
    unadjusted_prices: &CommodityPrices,
    assets: &AssetPool,
    year: u32,
) {
    info!("Performing agent investment...");

    // Reduced costs for candidate assets
    let mut _reduced_costs: HashMap<_, _> = solution
        .iter_reduced_costs_for_candidates()
        .map(|(asset, time_slice, cost)| ((asset.clone(), time_slice.clone()), cost))
        .collect();
    remove_scarcity_influence_from_candidate_reduced_costs(
        &mut _reduced_costs,
        adjusted_prices,
        unadjusted_prices,
    );

    // Reduced costs for existing assets
    _reduced_costs.extend(reduced_costs_for_existing(
        &model.time_slice_info,
        assets,
        adjusted_prices,
        year,
    ));

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}
