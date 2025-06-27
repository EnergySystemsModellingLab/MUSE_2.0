//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::optimisation::Solution;
use super::prices::{reduced_costs_for_candidates_without_scarcity, reduced_costs_for_existing};
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::CommodityType;
use crate::model::Model;
use crate::simulation::demand::{
    calculate_demand_in_tranche, calculate_load, calculate_svd_demand_profile, get_tranches,
};
use indexmap::IndexMap;
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
    flow_map: &FlowMap,
    adjusted_prices: &CommodityPrices,
    unadjusted_prices: &CommodityPrices,
    assets: &AssetPool,
    year: u32,
) {
    info!("Performing agent investment...");

    let mut _reduced_costs: HashMap<_, _> =
        reduced_costs_for_candidates_without_scarcity(solution, adjusted_prices, unadjusted_prices)
            .collect();
    _reduced_costs.extend(reduced_costs_for_existing(
        &model.time_slice_info,
        assets,
        adjusted_prices,
        year,
    ));

    let demand = calculate_svd_demand_profile(&model.commodities, flow_map);

    for (commodity_id, commodity) in model.commodities.iter() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We only consider SVD commodities first
            continue;
        }

        for region_id in model.iter_regions() {
            let (load_map, peak) =
                calculate_load(&model.time_slice_info, commodity_id, region_id, &demand);
            let tranches = get_tranches(peak, model.num_demand_tranches);

            // We want to consider the tranche with the highest load factor first, but in our case
            // that will always be the first
            for (i, tranche) in tranches.enumerate() {
                let tranche_demand: IndexMap<_, _> =
                    calculate_demand_in_tranche(&model.time_slice_info, &load_map, &tranche)
                        .collect();
                info!("Tranche {i}: Demand: {tranche_demand:?}");
            }
        }
    }

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}
