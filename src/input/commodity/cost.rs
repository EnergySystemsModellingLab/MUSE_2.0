//! Code for reading in the commodity cost CSV file.
use super::super::*;
use crate::commodity::{BalanceType, CommodityCost, CommodityCostMap, CommodityID};
use crate::id::IDCollection;
use crate::region::{parse_region_str, RegionID};
use crate::time_slice::TimeSliceInfo;
use crate::year::parse_year_str;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const COMMODITY_COSTS_FILE_NAME: &str = "commodity_costs.csv";

/// Cost parameters for each commodity
#[derive(PartialEq, Debug, Deserialize, Clone)]
struct CommodityCostRaw {
    /// Unique identifier for the commodity (e.g. "ELC")
    commodity_id: String,
    /// The region(s) to which the commodity cost applies.
    regions: String,
    /// Type of balance for application of cost.
    balance_type: BalanceType,
    /// The year(s) to which the cost applies.
    years: String,
    /// The time slice to which the cost applies.
    time_slice: String,
    /// Cost per unit commodity. For example, if a CO2 price is specified in input data, it can be applied to net CO2 via this value.
    value: f64,
}

/// Read costs associated with each commodity from commodity costs CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible commodity IDs
/// * `region_ids` - All possible region IDs
/// * `time_slice_info` - Information about time slices
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// A map containing commodity costs, grouped by commodity ID.
pub fn read_commodity_costs(
    model_dir: &Path,
    commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<CommodityID, CommodityCostMap>> {
    let file_path = model_dir.join(COMMODITY_COSTS_FILE_NAME);
    let commodity_costs_csv = read_csv::<CommodityCostRaw>(&file_path)?;
    read_commodity_costs_iter(
        commodity_costs_csv,
        commodity_ids,
        region_ids,
        time_slice_info,
        milestone_years,
    )
    .with_context(|| input_err_msg(&file_path))
}

fn read_commodity_costs_iter<I>(
    iter: I,
    commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<CommodityID, CommodityCostMap>>
where
    I: Iterator<Item = CommodityCostRaw>,
{
    let mut map = HashMap::new();

    // Keep track of commodity/region combinations specified. We will check that all years and
    // time slices are covered for each commodity/region combination.
    let mut commodity_regions: HashMap<CommodityID, HashSet<RegionID>> = HashMap::new();

    for cost in iter {
        let commodity_id = commodity_ids.get_id(&cost.commodity_id)?;
        let regions = parse_region_str(&cost.regions, region_ids)?;
        let years = parse_year_str(&cost.years, milestone_years)?;
        let ts_selection = time_slice_info.get_selection(&cost.time_slice)?;

        // Get or create CommodityCostMap for this commodity
        let map = map
            .entry(commodity_id.clone())
            .or_insert_with(CommodityCostMap::new);

        // Create CommodityCost
        let cost = CommodityCost {
            balance_type: cost.balance_type,
            value: cost.value,
        };

        // Insert cost into map for each region/year/time slice
        for region in regions.iter() {
            commodity_regions
                .entry(commodity_id.clone())
                .or_default()
                .insert(region.clone());
            for year in years.iter() {
                for (time_slice, _) in time_slice_info.iter_selection(&ts_selection) {
                    try_insert(
                        map,
                        (region.clone(), *year, time_slice.clone()),
                        cost.clone(),
                    )?;
                }
            }
        }
    }

    // Validate map
    for (commodity_id, regions) in commodity_regions.iter() {
        let map = map.get(commodity_id).unwrap();
        validate_commodity_cost_map(map, regions, milestone_years, time_slice_info)
            .with_context(|| format!("Missing costs for commodity {}", commodity_id))?;
    }
    Ok(map)
}

fn validate_commodity_cost_map(
    map: &CommodityCostMap,
    regions: &HashSet<RegionID>,
    milestone_years: &[u32],
    time_slice_info: &TimeSliceInfo,
) -> Result<()> {
    // Check that all regions, years and time slices are covered
    for region_id in regions.iter() {
        for year in milestone_years.iter() {
            for time_slice in time_slice_info.iter_ids() {
                ensure!(
                    map.contains_key(&(region_id.clone(), *year, time_slice.clone())),
                    "Missing cost for region {}, year {}, time slice {}",
                    region_id,
                    year,
                    time_slice
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_slice::TimeSliceID;

    #[test]
    fn test_validate_commodity_costs_map() {
        // Set up time slices
        let slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let ts_info = TimeSliceInfo {
            seasons: ["winter".into()].into(),
            times_of_day: ["day".into()].into(),
            fractions: [(slice.clone(), 1.0)].into(),
        };

        let regions = HashSet::from(["UK".into()]);
        let milestone_years = [2020];
        let cost = CommodityCost {
            balance_type: BalanceType::Net,
            value: 1.0,
        };

        // Valid map
        let mut map = CommodityCostMap::new();
        map.insert(("UK".into(), 2020, slice.clone()), cost.clone());
        assert!(validate_commodity_cost_map(&map, &regions, &milestone_years, &ts_info).is_ok());

        // Missing region
        let regions2 = HashSet::from(["UK".into(), "FR".into()]);
        assert!(validate_commodity_cost_map(&map, &regions2, &milestone_years, &ts_info).is_err());

        // Missing year
        assert!(validate_commodity_cost_map(&map, &regions, &[2020, 2030], &ts_info).is_err());

        // Missing time slice
        let slice2 = TimeSliceID {
            season: "winter".into(),
            time_of_day: "night".into(),
        };
        let ts_info2 = TimeSliceInfo {
            seasons: ["winter".into()].into(),
            times_of_day: ["day".into(), "night".into()].into(),
            fractions: [(slice.clone(), 0.5), (slice2.clone(), 0.5)].into(),
        };
        assert!(validate_commodity_cost_map(&map, &regions, &milestone_years, &ts_info2).is_err());
    }
}
