//! Code for calculating the potential utilisation of assets/processes

use crate::asset::AssetPool;
use crate::commodity::{Commodity, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::simulation::optimisation::FlowMap;
use crate::time_slice::TimeSliceID;
use std::collections::HashMap;

/// Extract or estimate demand per time slice for the given commodity
pub fn estimate_demand_per_time_slice(
    model: &Model,
    assets: &AssetPool,
    flow_map: &FlowMap,
    commodity: &Commodity,
    previous_year: u32,
    current_year: u32,
) -> HashMap<(RegionID, TimeSliceID), f64> {
    assert!(commodity.kind == CommodityType::ServiceDemand);

    let mut map = HashMap::new();
    for region_id in model.iter_regions() {
        // Sum demand and store in map
        for (asset, _) in assets.iter_for_region_and_commodity(region_id, &commodity.id) {
            for time_slice in model.time_slice_info.iter_ids() {
                let demand = *flow_map
                    .get(&(asset.clone(), commodity.id.clone(), time_slice.clone()))
                    .unwrap();
                map.entry((region_id.clone(), time_slice.clone()))
                    .and_modify(|value| *value += demand)
                    .or_insert(demand);
            }
        }

        // Normalise stored demand
        let previous_annual_demand = *commodity
            .annual_demand
            .get(&(region_id.clone(), previous_year))
            .unwrap();
        let current_annual_demand = *commodity
            .annual_demand
            .get(&(region_id.clone(), current_year))
            .unwrap();
        let mut total = 0.0;
        for time_slice in model.time_slice_info.iter_ids() {
            let value = map
                .get_mut(&(region_id.clone(), time_slice.clone()))
                .unwrap();
            total += *value;
            *value *= current_annual_demand / previous_annual_demand;
        }

        println!("diff: {}", total - previous_annual_demand);
    }

    map
}
