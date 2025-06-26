//! Calculations related to demand, including demand profile and tranching

use super::optimisation::FlowMap;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::Flow;
use std::collections::HashMap;

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
