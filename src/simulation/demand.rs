//! Calculations related to demand, including demand profile and tranching
use super::optimisation::FlowMap;
use crate::commodity::{CommodityID, CommodityMap, CommodityType};
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Dimensionless, Flow, FlowPerYear};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::ops::RangeInclusive;

type DemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;

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

/// NB: USING INDEXMAP FOR EASE OF DEBUGGING
pub fn calculate_load(
    time_slice_info: &TimeSliceInfo,
    commodity_id: &CommodityID,
    region_id: &RegionID,
    demand: &DemandMap,
) -> (IndexMap<TimeSliceID, FlowPerYear>, FlowPerYear) {
    let mut load = IndexMap::new();
    let mut peak_load = FlowPerYear(0.0);

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

pub fn get_tranches(
    peak: FlowPerYear,
    num_tranches: u32,
) -> impl Iterator<Item = RangeInclusive<FlowPerYear>> {
    let tranche_width = peak / Dimensionless(num_tranches as f64);

    (0..num_tranches).map(move |i| {
        let lower = Dimensionless(i as f64) * tranche_width;
        lower..=lower + tranche_width
    })
}

/// NB: USING INDEXMAP FOR EASE OF DEBUGGING
pub fn calculate_load_in_tranche(
    load: &IndexMap<TimeSliceID, FlowPerYear>,
    tranche: &RangeInclusive<FlowPerYear>,
) -> IndexMap<TimeSliceID, FlowPerYear> {
    load.iter()
        .map(|(time_slice, &power)| {
            let load_capped = power.min(*tranche.end());
            let load_in_tranche = (load_capped - *tranche.start()).max(FlowPerYear(0.0));

            (time_slice.clone(), load_in_tranche)
        })
        .collect()
}

pub fn calculate_load_factor<I>(loads: I, peak_load: FlowPerYear) -> Dimensionless
where
    I: ExactSizeIterator<Item = FlowPerYear>,
{
    let len = loads.len();
    let mean_load = loads.sum::<FlowPerYear>() / Dimensionless(len as f64);
    dbg!(mean_load);
    mean_load / peak_load
}

pub fn load_to_demand<'a>(
    time_slice_info: &'a TimeSliceInfo,
    load: &'a IndexMap<TimeSliceID, FlowPerYear>,
) -> impl Iterator<Item = (TimeSliceID, Flow)> + 'a {
    time_slice_info.iter().map(|(time_slice, ts_length)| {
        let load = *load.get(time_slice).unwrap();

        (time_slice.clone(), load * ts_length)
    })
}
