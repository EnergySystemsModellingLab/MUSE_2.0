//! Code for reading in commodity-related data from CSV files.
use super::*;
use crate::commodity::{Commodity, CommodityID, CommodityMap};
use crate::region::RegionID;
use crate::time_slice::TimeSliceInfo;
use anyhow::Result;
use indexmap::IndexSet;
use std::path::Path;

mod levy;
use levy::read_commodity_levies;
mod demand;
use demand::read_demand;
mod demand_slicing;

const COMMODITY_FILE_NAME: &str = "commodities.csv";

/// Read commodity data from the specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `region_ids` - All possible region IDs
/// * `time_slice_info` - Information about time slices
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// A map containing commodities, grouped by commodity ID or an error.
pub fn read_commodities(
    model_dir: &Path,
    region_ids: &IndexSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<CommodityMap> {
    let commodities =
        read_csv_id_file::<Commodity, CommodityID>(&model_dir.join(COMMODITY_FILE_NAME))?;
    let commodity_ids = commodities.keys().cloned().collect();
    let mut costs = read_commodity_levies(
        model_dir,
        &commodity_ids,
        region_ids,
        time_slice_info,
        milestone_years,
    )?;

    let mut demand = read_demand(
        model_dir,
        &commodities,
        region_ids,
        time_slice_info,
        milestone_years,
    )?;

    // Populate maps for each Commodity
    Ok(commodities
        .into_iter()
        .map(|(id, mut commodity)| {
            if let Some(costs) = costs.remove(&id) {
                commodity.levies = costs;
            }
            if let Some(demand) = demand.remove(&id) {
                commodity.demand = demand;
            }

            (id, commodity.into())
        })
        .collect())
}
