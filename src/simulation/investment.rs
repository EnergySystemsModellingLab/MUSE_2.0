//! Code for performing agent investment.
use std::collections::HashMap;

use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{Commodity, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceLevel};
use log::info;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `year` - The current milestone year
pub fn perform_agent_investment(
    model: &Model,
    assets: &mut AssetPool,
    _flow_map: &FlowMap,
    _prices: &CommodityPrices,
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
