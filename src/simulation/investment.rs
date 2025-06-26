//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
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
pub fn perform_agent_investment(
    model: &Model,
    flow_map: &FlowMap,
    _prices: &CommodityPrices,
    _assets: &mut AssetPool,
) {
    info!("Performing agent investment...");

    let _demand = calculate_svd_demand_profile(&model.commodities, flow_map);

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}

/// Get demand per time slice for SVD commodities
pub fn calculate_svd_demand_profile(
    commodities: &CommodityMap,
    flow_map: &FlowMap,
) -> HashMap<(CommodityID, RegionID, TimeSliceID), Flow> {
    let mut map = HashMap::new();
    for ((asset, commodity_id, time_slice), &flow) in flow_map.iter() {
        let commodity = commodities.get(commodity_id).unwrap();
        if commodity.kind != CommodityType::ServiceDemand {
            continue;
        }

        map.entry((
            commodity_id.clone(),
            asset.region_id.clone(),
            time_slice.clone(),
        ))
        .and_modify(|value| *value += flow)
        .or_insert(flow);
    }

    map
}
