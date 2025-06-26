//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Dimensionless, Flow};
use log::info;
use std::collections::HashMap;

type DemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;
type LoadMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;
type PeakLoadMap = HashMap<(CommodityID, RegionID), Flow>;

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

    let demand = calculate_svd_demand_profile(&model.commodities, flow_map);
    let (load, peak_load) = calculate_load(&model.time_slice_info, &demand);

    for tranche_num in 0..model.num_demand_tranches {
        do_something_with_demand_tranches(model, &load, &peak_load, tranche_num);
    }

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}

/// Get demand per time slice for SVD commodities
pub fn calculate_svd_demand_profile(commodities: &CommodityMap, flow_map: &FlowMap) -> DemandMap {
    let mut map = DemandMap::new();
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

fn calculate_load(time_slice_info: &TimeSliceInfo, demand: &DemandMap) -> (LoadMap, PeakLoadMap) {
    let mut load = HashMap::new();
    let mut peak_load = HashMap::new();

    for ((commodity_id, region_id, time_slice), demand) in demand.iter() {
        // NB: This **should** be in units of FlowPerYear
        let power = *demand / *time_slice_info.time_slices.get(time_slice).unwrap();
        load.insert(
            (commodity_id.clone(), region_id.clone(), time_slice.clone()),
            power,
        );

        peak_load
            .entry((commodity_id.clone(), region_id.clone()))
            .and_modify(|value: &mut Flow| *value = value.max(power))
            .or_insert(power);
    }

    (load, peak_load)
}

fn do_something_with_demand_tranches(
    model: &Model,
    load: &LoadMap,
    peak_load: &PeakLoadMap,
    tranche_num: u32,
) {
    let num_tranches = model.num_demand_tranches;
    for ((commodity_id, region_id), &peak_load) in peak_load.iter() {
        let tranche_width = peak_load / Dimensionless(num_tranches as f64);
        let tranche_bottom = Dimensionless((num_tranches - tranche_num - 1) as f64) * tranche_width;

        let mut sum_load = Flow(0.0);
        let mut count = 0;
        for time_slice in model.time_slice_info.iter_ids() {
            let load = *load
                .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                .unwrap();
            if load >= tranche_bottom {
                sum_load += load;
                count += 1;
            }
        }
        let mean_load = sum_load / Dimensionless(count as f64);
        let load_factor = mean_load / peak_load;
        info!(
            "Tranche {}: LF for {}: {}",
            tranche_num, commodity_id, load_factor
        );
    }
}
