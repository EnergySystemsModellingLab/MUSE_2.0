//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::CommodityType;
use crate::model::Model;
use log::info;

pub mod utilisation;
use utilisation::estimate_demand_per_time_slice;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
/// * `previous_year` - The last milestone year
/// * `current_year` - The current milestone year
pub fn perform_agent_investment(
    model: &Model,
    flow_map: &FlowMap,
    _prices: &CommodityPrices,
    assets: &mut AssetPool,
    previous_year: u32,
    current_year: u32,
) {
    info!("Performing agent investment...");

    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We look at SVD commodities first
            continue;
        }

        let _demand = estimate_demand_per_time_slice(
            model,
            assets,
            flow_map,
            commodity,
            previous_year,
            current_year,
        );

        for ((region_id, time_slice), demand) in _demand {
            println!("!!! {} {region_id} {time_slice} {demand}", &commodity.id);
        }

        // break;
    }

    let mut new_pool = Vec::new();
    for asset in assets.iter() {
        // **TODO**: Implement agent investment. For now, just keep all assets.
        new_pool.push(asset.clone().into());
    }

    assets.replace_active_pool(new_pool);
}
