//! Code for reading in the commodity cost CSV file.
use super::super::*;
use crate::commodity::{BalanceType, CommodityCost, CommodityCostMap, CommodityID};
use crate::id::IDCollection;
use crate::region::RegionID;
use crate::time_slice::TimeSliceInfo;
use anyhow::{ensure, Context, Result};
use itertools::iproduct;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const COMMODITY_COSTS_FILE_NAME: &str = "commodity_costs.csv";

/// Cost parameters for each commodity
#[derive(PartialEq, Debug, Deserialize, Clone)]
struct CommodityCostRaw {
    /// Unique identifier for the commodity (e.g. "ELC")
    commodity_id: String,
    /// The region to which the commodity cost applies.
    region_id: String,
    /// Type of balance for application of cost.
    balance_type: BalanceType,
    /// The year to which the cost applies.
    year: u32,
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
    for cost in iter {
        // Get/check IDs
        let commodity_id = commodity_ids.get_id_by_str(&cost.commodity_id)?;
        let region_id = region_ids.get_id_by_str(&cost.region_id)?;
        let ts_selection = time_slice_info.get_selection(&cost.time_slice)?;
        ensure!(
            milestone_years.binary_search(&cost.year).is_ok(),
            "Year {} is not a milestone year. \
                Input of non-milestone years is currently not supported.",
            cost.year
        );

        // Get or create CommodityCostMap for this commodity
        let map = map
            .entry(commodity_id.clone())
            .or_insert_with(CommodityCostMap::new);

        // Insert cost for each timeslice in the selection
        for (time_slice, _) in time_slice_info.iter_selection(&ts_selection) {
            let value = CommodityCost {
                balance_type: cost.balance_type.clone(),
                value: cost.value,
            };
            if let Err(e) = map.insert((region_id.clone(), cost.year, time_slice.clone()), value) {
                return Err(e.context("Error for commodity {commodity_id}"));
            }
        }
    }

    // Check map completeness
    for (commodity_id, map) in &map {
        // Map should contain every combination of region + year + time slice for this commodity
        let time_slices: Vec<_> = time_slice_info.iter_ids().cloned().collect();
        for (region_id, year, time_slice) in iproduct!(region_ids, milestone_years, time_slices) {
            if !map.contains_key(&(region_id.clone(), *year, time_slice.clone())) {
                return Err(anyhow::anyhow!(
                    "Missing cost for commodity {commodity_id}, region {region_id}, year {year}, time slice {time_slice}"
                ));
            }
        }
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_slice::TimeSliceID;
    use std::iter;

    #[test]
    fn test_read_commodity_costs_iter() {
        let commodity_ids = ["commodity".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();
        let slices = [
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            TimeSliceID {
                season: "summer".into(),
                time_of_day: "night".into(),
            },
        ];
        let time_slice_info = TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            fractions: [(slices[0].clone(), 0.5), (slices[1].clone(), 0.5)]
                .into_iter()
                .collect(),
        };
        let time_slice = time_slice_info
            .get_time_slice_id_from_str("winter.day")
            .unwrap();
        let milestone_years = [2010];

        // Valid
        let cost1 = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        let cost2 = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "FRA".into(),
            balance_type: BalanceType::Production,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        let value1 = CommodityCost {
            balance_type: cost1.balance_type.clone(),
            value: cost1.value,
        };
        let value2 = CommodityCost {
            balance_type: cost2.balance_type.clone(),
            value: cost2.value,
        };
        let mut map = CommodityCostMap::new();
        map.insert(("GBR".into(), cost1.year, time_slice.clone()), value1);
        map.insert(("FRA".into(), cost2.year, time_slice.clone()), value2);
        let expected = HashMap::from_iter([("commodity".into(), map)]);
        assert_eq!(
            read_commodity_costs_iter(
                [cost1.clone(), cost2].into_iter(),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
                &milestone_years,
            )
            .unwrap(),
            expected
        );

        // Invalid: Overlapping time slices
        let cost2 = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Production,
            year: 2010,
            time_slice: "winter".into(), // NB: Covers all winter
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            [cost1.clone(), cost2].into_iter(),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());

        // Invalid: Bad commodity
        let cost = CommodityCostRaw {
            commodity_id: "commodity2".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Production,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            iter::once(cost),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());

        // Invalid: Bad region
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "USA".into(),
            balance_type: BalanceType::Production,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            iter::once(cost),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());

        // Invalid: Bad time slice selection
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Production,
            year: 2010,
            time_slice: "summer.evening".into(),
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            iter::once(cost),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());

        // Invalid: non-milestone year
        let cost2 = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2011, // NB: Non-milestone year
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            [cost1, cost2].into_iter(),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());

        // Invalid: Milestone year 2020 is not covered
        let milestone_years = [2010, 2020];
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 0.5,
        };
        assert!(read_commodity_costs_iter(
            iter::once(cost),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        )
        .is_err());
    }
}
