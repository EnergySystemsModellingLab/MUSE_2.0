//! Calculations related to demand, including demand profile and tranching

use super::optimisation::FlowMap;
use crate::commodity::CommodityID;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::Flow;
use indexmap::IndexSet;
use std::collections::HashMap;

/// Get demand per time slice for specified commodities
pub fn get_demand_profile(
    commodities: &IndexSet<CommodityID>,
    flow_map: &FlowMap,
) -> HashMap<(CommodityID, RegionID, TimeSliceID), Flow> {
    let mut map = HashMap::new();
    for ((asset, commodity_id, time_slice), &flow) in flow_map.iter() {
        if commodities.contains(commodity_id) && flow > Flow(0.0) {
            map.entry((
                commodity_id.clone(),
                asset.region_id.clone(),
                time_slice.clone(),
            ))
            .and_modify(|value| *value += flow)
            .or_insert(flow);
        }
    }

    map
}
