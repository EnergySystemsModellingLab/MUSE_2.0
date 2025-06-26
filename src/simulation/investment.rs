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
        do_something_with_demand_tranches(
            &model.time_slice_info,
            &load,
            &peak_load,
            tranche_num,
            model.num_demand_tranches,
        );
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
    time_slice_info: &TimeSliceInfo,
    load: &LoadMap,
    peak_load: &PeakLoadMap,
    tranche_num: u32,
    num_tranches: u32,
) {
    for ((commodity_id, region_id), &peak_load) in peak_load.iter() {
        let tranche_width = peak_load / Dimensionless(num_tranches as f64);
        // let tranche_bottom = (Dimensionless(tranche_num as f64)) * tranche_width;
        let tranche_top = Dimensionless((tranche_num + 1) as f64) * tranche_width;
        // let tranche_top = Flow(4.0);

        let mut sum_load = Flow(0.0);
        for time_slice in time_slice_info.iter_ids() {
            let load = load
                .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                .unwrap()
                .min(tranche_top);
            sum_load += load;
        }
        let mean_load = sum_load / Dimensionless(time_slice_info.time_slices.len() as f64);
        let load_factor = mean_load / tranche_top;

        info!(
            "Tranche {}: LF for {}: {}",
            tranche_num, commodity_id, load_factor
        );

        // return load_factor;
    }

    // unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{commodity_id, region_id};
    use crate::time_slice::{Season, TimeOfDay, TimeSliceInfo};
    // use float_cmp::assert_approx_eq;
    use indexmap::IndexSet;
    use rstest::rstest;
    use std::iter;

    #[rstest]
    fn test_tranches(commodity_id: CommodityID, region_id: RegionID) {
        let season: Season = "season1".into();
        let times_of_day: IndexSet<TimeOfDay> = (1..=3).map(|i| format!("ts{i}").into()).collect();
        let time_slices = times_of_day
            .iter()
            .zip([0.2, 0.3, 0.5])
            .map(|(tod, dur): (_, f64)| {
                (
                    TimeSliceID {
                        season: season.clone(),
                        time_of_day: tod.clone(),
                    },
                    Dimensionless(dur),
                )
            })
            .collect();
        let time_slice_info = TimeSliceInfo {
            times_of_day,
            seasons: iter::once((season, Dimensionless(1.0))).collect(),
            time_slices,
        };

        let demand = time_slice_info
            .iter_ids()
            .map(|time_slice| {
                (
                    (commodity_id.clone(), region_id.clone(), time_slice.clone()),
                    Flow(2.0),
                )
            })
            .collect();
        let (load, peak_load) = calculate_load(&time_slice_info, &demand);
        let _load_factor =
            do_something_with_demand_tranches(&time_slice_info, &load, &peak_load, 0, 2);

        // assert_approx_eq!(Dimensionless, load_factor, Dimensionless(1.0));
    }
}
