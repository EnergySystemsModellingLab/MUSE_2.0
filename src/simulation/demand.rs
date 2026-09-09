//! Code for collecting and storing commodity demand data
use crate::commodity::{CommodityID, CommodityMap};
use crate::region::RegionID;
use crate::time_slice::TimeSliceSelection;
use crate::units::Flow;
use indexmap::IndexMap;

/// A map of demand across time-slice selections for a specific market
pub type DemandMap = IndexMap<TimeSliceSelection, Flow>;

/// Demand for a given combination of commodity, region and time-slice selection
pub type AllDemandMap = IndexMap<(CommodityID, RegionID, TimeSliceSelection), Flow>;

/// Collect the preset commodity demands for a given year into a map of commodity, region and
/// time slice selection to demand.
///
/// Demand for each commodity is stored at its natural time-slice selection level, matching the
/// balance level at which the investment appraisal operates.
pub fn collect_preset_demands_for_year(commodities: &CommodityMap, year: u32) -> AllDemandMap {
    let mut demand_map = AllDemandMap::new();
    for (commodity_id, commodity) in commodities {
        for ((region_id, data_year, time_slice_selection), demand) in &commodity.demand {
            if *data_year != year {
                continue;
            }
            demand_map.insert(
                (
                    commodity_id.clone(),
                    region_id.clone(),
                    time_slice_selection.clone(),
                ),
                *demand,
            );
        }
    }
    demand_map
}
