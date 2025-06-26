//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Dimensionless, Flow};
use indexmap::IndexMap;
use itertools::Itertools;
use log::info;
use std::collections::HashMap;
use std::ops::Range;

type DemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;

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

    for (commodity_id, commodity) in model.commodities.iter() {
        if commodity.kind != CommodityType::ServiceDemand {
            // We only consider SVD commodities first
            continue;
        }

        for region_id in model.iter_regions() {
            let (load_map, peak) =
                calculate_load(&model.time_slice_info, commodity_id, region_id, &demand);
            let tranches = get_tranches(peak, model.num_demand_tranches);
            info!("{}: {:?}", commodity_id, tranches);

            for (i, tranche) in tranches.iter().enumerate() {
                let tranche_load = calculate_load_in_tranche(&load_map, tranche);
                info!("{:?}", tranche_load.values());
                let load_factor = calculate_load_factor(&model.time_slice_info, &tranche_load);
                info!("Tranche {i}: LF: {load_factor}");
            }
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

/// NB: USING INDEXMAP FOR EASE OF DEBUGGING
fn calculate_load(
    time_slice_info: &TimeSliceInfo,
    commodity_id: &CommodityID,
    region_id: &RegionID,
    demand: &DemandMap,
) -> (IndexMap<TimeSliceID, Flow>, Flow) {
    let mut load = IndexMap::new();
    let mut peak_load = Flow(0.0);

    for (time_slice, ts_length) in time_slice_info.iter() {
        // NB: This **should** be in units of FlowPerYear
        let demand = demand
            .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
            .unwrap();
        let power = *demand / ts_length;
        load.insert(time_slice.clone(), power);

        peak_load = peak_load.max(power);
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

/// NB: USING INDEXMAP FOR EASE OF DEBUGGING
fn calculate_load_in_tranche(
    load: &IndexMap<TimeSliceID, Flow>,
    tranche: &Range<Flow>,
) -> IndexMap<TimeSliceID, Flow> {
    load.iter()
        .map(|(time_slice, &power)| {
            // SHOULD BOUNDS BE RANGEINCLUSIVE?!
            let load_in_tranche = if power >= tranche.start {
                power.min(tranche.end) - tranche.start
            } else {
                Flow(0.0)
            };

            (time_slice.clone(), load_in_tranche)
        })
        .collect()
}

fn calculate_load_factor(
    time_slice_info: &TimeSliceInfo,
    load: &IndexMap<TimeSliceID, Flow>,
) -> Dimensionless {
    time_slice_info
        .time_slices
        .iter()
        .map(|(ts, &ts_length)| {
            if *load.get(ts).unwrap() > Flow(0.0) {
                ts_length
            } else {
                Dimensionless(0.0)
            }
        })
        .sum()

    // TODO: CLAMP RETURN VALUE???
}
