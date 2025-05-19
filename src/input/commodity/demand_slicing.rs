//! Demand slicing determines how annual demand is distributed across the year.
use super::super::*;
use crate::commodity::CommodityID;
use crate::id::IDCollection;
use crate::region::{parse_region_str, RegionID};
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const DEMAND_SLICING_FILE_NAME: &str = "demand_slicing.csv";

#[derive(Clone, Deserialize)]
struct DemandSlice {
    commodity_id: String,
    regions: String,
    time_slice: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    fraction: f64,
}

/// A map relating commodity, region and time slice to the fraction of annual demand
pub type DemandSliceMap = HashMap<(CommodityID, RegionID, TimeSliceID), f64>;

/// Read demand slices from specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `commodity_regions` - Pairs of commodities + regions listed in demand CSV file
/// * `time_slice_info` - Information about seasons and times of day
pub fn read_demand_slices(
    model_dir: &Path,
    svd_commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap> {
    let file_path = model_dir.join(DEMAND_SLICING_FILE_NAME);
    let demand_slices_csv = read_csv(&file_path)?;
    read_demand_slices_from_iter(
        demand_slices_csv,
        svd_commodity_ids,
        region_ids,
        time_slice_info,
    )
    .with_context(|| input_err_msg(file_path))
}

/// Read demand slices from an iterator
fn read_demand_slices_from_iter<I>(
    iter: I,
    svd_commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap>
where
    I: Iterator<Item = DemandSlice>,
{
    let mut demand_slices = DemandSliceMap::new();

    for slice in iter {
        let commodity_id = svd_commodity_ids
            .get_id_by_str(&slice.commodity_id)
            .with_context(|| {
                format!(
                    "Can only provide demand slice data for SVD commodities. Found entry for '{}'",
                    slice.commodity_id
                )
            })?;
        let regions = parse_region_str(&slice.regions, region_ids)?;

        // We need to know how many time slices are covered by the current demand slice entry and
        // how long they are relative to one another so that we can divide up the demand for this
        // entry appropriately
        let ts_selection = time_slice_info.get_selection(&slice.time_slice)?;
        for (ts, demand_fraction) in time_slice_info.calculate_share(&ts_selection, slice.fraction)
        {
            // Share demand between the time slices in proportion to duration
            for region_id in regions.iter() {
                let key = (commodity_id.clone(), region_id.clone(), ts.clone());
                ensure!(demand_slices.insert(key, demand_fraction).is_none(),
                "Duplicate demand slicing entry (or same time slice covered by more than one entry) \
                (commodity: {commodity_id}, region: {region_id}, time slice: {ts})"
            );
            }
        }
    }

    validate_demand_slices(
        svd_commodity_ids,
        region_ids,
        &demand_slices,
        time_slice_info,
    )?;

    Ok(demand_slices)
}

/// Check that the [`DemandSliceMap`] is well formed.
///
/// Specifically, check:
///
/// * It is non-empty
/// * For every commodity + region pair, there must be entries covering every time slice
/// * The demand fractions for all entries related to a commodity + region pair sum to one
fn validate_demand_slices(
    svd_commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    demand_slices: &DemandSliceMap,
    time_slice_info: &TimeSliceInfo,
) -> Result<()> {
    let commodity_regions = svd_commodity_ids
        .iter()
        .cartesian_product(region_ids.iter())
        .collect::<HashSet<_>>();
    for (commodity_id, region_id) in commodity_regions {
        time_slice_info
            .iter_ids()
            .map(|time_slice| {
                demand_slices
                    .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                    .with_context(|| {
                        format!(
                            "Demand slice missing for time slice {} (commodity: {}, region {})",
                            time_slice, commodity_id, region_id
                        )
                    })
            })
            .process_results(|iter| {
                check_fractions_sum_to_one(iter.copied()).context("Invalid demand fractions")
            })??;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{assert_error, commodity_ids, time_slice_info};
    use rstest::{fixture, rstest};
    use std::iter;

    #[fixture]
    pub fn region_ids() -> HashSet<RegionID> {
        iter::once("GBR".into()).collect()
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_valid(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Valid
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 1.0,
        };
        let time_slice = time_slice_info
            .get_time_slice_id_from_str("winter.day")
            .unwrap();
        let key = ("commodity1".into(), "GBR".into(), time_slice);
        let expected = DemandSliceMap::from_iter(iter::once((key, 1.0)));
        assert_eq!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .unwrap(),
            expected
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_valid_multiple_time_slices(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Valid, multiple time slices
        let time_slice_info = TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            fractions: [
                (
                    TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "day".into(),
                    },
                    3.0 / 16.0,
                ),
                (
                    TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "night".into(),
                    },
                    5.0 / 16.0,
                ),
                (
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "day".into(),
                    },
                    3.0 / 16.0,
                ),
                (
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "night".into(),
                    },
                    5.0 / 16.0,
                ),
            ]
            .into_iter()
            .collect(),
        };
        let demand_slices = [
            DemandSlice {
                commodity_id: "commodity1".into(),
                regions: "GBR".into(),
                time_slice: "winter".into(),
                fraction: 0.5,
            },
            DemandSlice {
                commodity_id: "commodity1".into(),
                regions: "GBR".into(),
                time_slice: "summer".into(),
                fraction: 0.5,
            },
        ];
        let expected = DemandSliceMap::from_iter([
            (
                (
                    "commodity1".into(),
                    "GBR".into(),
                    TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "day".into(),
                    },
                ),
                3.0 / 16.0,
            ),
            (
                (
                    "commodity1".into(),
                    "GBR".into(),
                    TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "night".into(),
                    },
                ),
                5.0 / 16.0,
            ),
            (
                (
                    "commodity1".into(),
                    "GBR".into(),
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "day".into(),
                    },
                ),
                3.0 / 16.0,
            ),
            (
                (
                    "commodity1".into(),
                    "GBR".into(),
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "night".into(),
                    },
                ),
                5.0 / 16.0,
            ),
        ]);
        assert_eq!(
            read_demand_slices_from_iter(
                demand_slices.into_iter(),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .unwrap(),
            expected
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_empty_file(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Empty CSV file
        assert_error!(
            read_demand_slices_from_iter(
                iter::empty(),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Demand slice missing for time slice winter.day (commodity: commodity1, region GBR)"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_bad_commodity(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Bad commodity
        let demand_slice = DemandSlice {
            commodity_id: "commodity2".into(),
            regions: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Can only provide demand slice data for SVD commodities. Found entry for 'commodity2'"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_bad_region(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Bad region
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "FRA".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Unknown ID FRA found"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_bad_time_slice(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Bad time slice selection
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "summer".into(),
            fraction: 1.0,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "'summer' is not a valid season"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_missing_time_slices(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Some time slices uncovered
        let time_slice_info = TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: iter::once("day".into()).collect(),
            fractions: [
                (
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "day".into(),
                    },
                    0.5,
                ),
                (
                    TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "day".into(),
                    },
                    0.5,
                ),
            ]
            .into_iter()
            .collect(),
        };
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 1.0,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Demand slice missing for time slice summer.day (commodity: commodity1, region GBR)"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_duplicate_time_slice(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Same time slice twice
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 0.5,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::repeat_n(demand_slice.clone(), 2),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Duplicate demand slicing entry (or same time slice covered by more than one entry) \
            (commodity: commodity1, region: GBR, time slice: winter.day)"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_season_time_slice_conflict(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Whole season and single time slice conflicting
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 0.5,
        };
        let demand_slice_season = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 0.5,
        };
        assert_error!(
            read_demand_slices_from_iter(
                [demand_slice, demand_slice_season].into_iter(),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Duplicate demand slicing entry (or same time slice covered by more than one entry) \
            (commodity: commodity1, region: GBR, time slice: winter.day)"
        );
    }

    #[rstest]
    fn test_read_demand_slices_from_iter_invalid_bad_fractions(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        time_slice_info: TimeSliceInfo,
    ) {
        // Fractions don't sum to one
        let demand_slice = DemandSlice {
            commodity_id: "commodity1".into(),
            regions: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 0.5,
        };
        assert_error!(
            read_demand_slices_from_iter(
                iter::once(demand_slice),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            ),
            "Invalid demand fractions"
        );
    }
}
