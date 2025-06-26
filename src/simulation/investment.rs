//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Dimensionless, Flow};
use itertools::Itertools;
use log::info;
use std::collections::HashMap;
use std::ops::Range;

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
    let (_load, peak_load) = calculate_load(&model.time_slice_info, &demand);

    for (commodity_id, commodity) in model.commodities.iter() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We only consider SVD commodities first
            continue;
        }

        for region_id in model.iter_regions() {
            let peak = *peak_load
                .get(&(commodity_id.clone(), region_id.clone()))
                .unwrap();
            let tranches = get_tranches(peak, model.num_demand_tranches);
            info!("{}: {:?}", commodity_id, tranches);
        }
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

fn get_tranches(peak: Flow, num_tranches: u32) -> Vec<Range<Flow>> {
    let tranche_width = peak / Dimensionless(num_tranches as f64);
    let tranche_bottom = |i| Dimensionless(i as f64) * tranche_width;
    let mut tranches = (0..num_tranches - 1)
        .map(|i| {
            let lower = tranche_bottom(i);
            lower..lower + tranche_width
        })
        .collect_vec();

    // Set the upper bound of highest tranche to infinity so we include time slices where value is
    // *equal* to peak
    tranches.push(tranche_bottom(num_tranches - 1)..Flow(f64::INFINITY));

    tranches
}
