//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{Commodity, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceLevel};
use crate::units::Flow;
use log::info;
use std::collections::HashMap;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
/// * `year` - The current milestone year
pub fn perform_agent_investment(
    model: &Model,
    _flow_map: &FlowMap,
    _prices: &CommodityPrices,
    assets: &mut AssetPool,
    year: u32,
) {
    info!("Performing agent investment...");

    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We look at SVD commodities first
            continue;
        }

        // Calculate demand per time slice
        let _demand = calculate_svd_demand(model, commodity, year);
    }

    let mut new_pool = Vec::new();
    for asset in assets.iter() {
        // **TODO**: Implement agent investment. For now, just keep all assets.
        new_pool.push(asset.clone().into());
    }

    assets.replace_active_pool(new_pool);
}

/// Get demand per time slice for the given commodity.
///
/// For commodities with a time slice level of "daynight", this information is provided directly in
/// `demand_slices.csv`. If the time slice level is seasonal or annual, we assume the user-specified
/// demand is uniformly distributed for either each season or the whole year.
pub fn calculate_svd_demand(
    model: &Model,
    commodity: &Commodity,
    year: u32,
) -> HashMap<(RegionID, TimeSliceID), Flow> {
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

            // Assume demand is uniformly distributed between time slices in `ts_selection`
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
