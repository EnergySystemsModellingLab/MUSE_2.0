//! Demand slicing determines how annual demand is distributed across the year.
use super::demand::*;
use crate::input::*;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const DEMAND_SLICING_FILE_NAME: &str = "demand_slicing.csv";

#[derive(Clone, Deserialize)]
struct DemandSlice {
    commodity_id: String,
    region_id: String,
    time_slice: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    fraction: f64,
}

/// A map relating commodity, region and time slice to the fraction of annual demand
pub type DemandSliceMap = HashMap<DemandSliceMapKey, f64>;

/// A key for a [`DemandSliceMap`]
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct DemandSliceMapKey {
    /// The commodity to which this demand applies
    pub commodity_id: Rc<str>,
    /// The region to which this demand applies
    pub region_id: Rc<str>,
    /// The time slice to which this demand applies
    pub time_slice: TimeSliceID,
}

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
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    commodity_regions: &CommodityRegionPairs,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap> {
    let file_path = model_dir.join(DEMAND_SLICING_FILE_NAME);
    let demand_slices_csv = read_csv(&file_path)?;
    read_demand_slices_from_iter(
        demand_slices_csv,
        commodity_ids,
        region_ids,
        commodity_regions,
        time_slice_info,
    )
    .with_context(|| input_err_msg(file_path))
}

/// Read demand slices from an iterator
fn read_demand_slices_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    commodity_regions: &CommodityRegionPairs,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap>
where
    I: Iterator<Item = DemandSlice>,
{
    let mut demand_slices = DemandSliceMap::new();

    for slice in iter {
        let commodity_id = commodity_ids.get_id(&slice.commodity_id)?;
        let region_id = region_ids.get_id(&slice.region_id)?;
        ensure!(
            commodity_regions.contains(&(Rc::clone(&commodity_id), Rc::clone(&region_id))),
            "Demand slicing provided for commodity {commodity_id} in region {region_id} \
            without a corresponding entry in demand CSV file"
        );

        // We need to know how many time slices are covered by the current demand slice entry and
        // how long they are relative to one another so that we can divide up the demand for this
        // entry appropriately
        let ts_selection = time_slice_info.get_selection(&slice.time_slice)?;
        for (ts, demand_fraction) in time_slice_info.calculate_share(&ts_selection, slice.fraction)
        {
            let key = DemandSliceMapKey {
                commodity_id: Rc::clone(&commodity_id),
                region_id: Rc::clone(&region_id),
                time_slice: ts.clone(),
            };

            // Share demand between the time slices in proportion to duration
            ensure!(demand_slices.insert(key, demand_fraction).is_none(),
                "Duplicate demand slicing entry (or same time slice covered by more than one entry) \
                (commodity: {commodity_id}, region: {region_id}, time slice: {ts})"
            );
        }
    }

    validate_demand_slices(commodity_regions, &demand_slices, time_slice_info)?;

    Ok(demand_slices)
}

/// Check that the [`DemandSliceMap`] is well formed.
///
/// Specifically, check:
///
/// * It is non-empty
/// * If an entry is provided for any commodity + region pair, there must be entries covering every
///   time slice
/// * The demand fractions for all entries related to a commodity + region pair sum to one
fn validate_demand_slices(
    commodity_regions: &CommodityRegionPairs,
    demand_slices: &DemandSliceMap,
    time_slice_info: &TimeSliceInfo,
) -> Result<()> {
    for (commodity_id, region_id) in commodity_regions {
        time_slice_info
            .iter_ids()
            .map(|time_slice| {
                let key = DemandSliceMapKey {
                    commodity_id: Rc::clone(commodity_id),
                    region_id: Rc::clone(region_id),
                    time_slice: time_slice.clone(),
                };

                demand_slices.get(&key).with_context(|| {
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
    use crate::time_slice::TimeSliceID;
    use itertools::iproduct;
    use std::iter;

    #[test]
    fn test_read_demand_slices_from_iter() {
        let time_slice_info = TimeSliceInfo {
            seasons: iter::once("winter".into()).collect(),
            times_of_day: iter::once("day".into()).collect(),
            fractions: [(
                TimeSliceID {
                    season: "winter".into(),
                    time_of_day: "day".into(),
                },
                1.0,
            )]
            .into_iter()
            .collect(),
        };
        let commodity_ids = HashSet::from_iter(iter::once("COM1".into()));
        let region_ids = HashSet::from_iter(iter::once("GBR".into()));
        let commodity_regions =
            iproduct!(commodity_ids.iter().cloned(), region_ids.iter().cloned()).collect();

        // Valid
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 1.0,
        };
        let time_slice = time_slice_info
            .get_time_slice_id_from_str("winter.day")
            .unwrap();
        let key = DemandSliceMapKey {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice,
        };
        let expected = DemandSliceMap::from_iter(iter::once((key, 1.0)));
        assert_eq!(
            read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &commodity_regions,
                &time_slice_info,
            )
            .unwrap(),
            expected
        );

        // Valid, multiple time slices
        {
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
                    commodity_id: "COM1".into(),
                    region_id: "GBR".into(),
                    time_slice: "winter".into(),
                    fraction: 0.5,
                },
                DemandSlice {
                    commodity_id: "COM1".into(),
                    region_id: "GBR".into(),
                    time_slice: "summer".into(),
                    fraction: 0.5,
                },
            ];
            let expected = DemandSliceMap::from_iter([
                (
                    DemandSliceMapKey {
                        commodity_id: "COM1".into(),
                        region_id: "GBR".into(),
                        time_slice: TimeSliceID {
                            season: "summer".into(),
                            time_of_day: "day".into(),
                        },
                    },
                    3.0 / 16.0,
                ),
                (
                    DemandSliceMapKey {
                        commodity_id: "COM1".into(),
                        region_id: "GBR".into(),
                        time_slice: TimeSliceID {
                            season: "summer".into(),
                            time_of_day: "night".into(),
                        },
                    },
                    5.0 / 16.0,
                ),
                (
                    DemandSliceMapKey {
                        commodity_id: "COM1".into(),
                        region_id: "GBR".into(),
                        time_slice: TimeSliceID {
                            season: "winter".into(),
                            time_of_day: "day".into(),
                        },
                    },
                    3.0 / 16.0,
                ),
                (
                    DemandSliceMapKey {
                        commodity_id: "COM1".into(),
                        region_id: "GBR".into(),
                        time_slice: TimeSliceID {
                            season: "winter".into(),
                            time_of_day: "night".into(),
                        },
                    },
                    5.0 / 16.0,
                ),
            ]);
            assert_eq!(
                read_demand_slices_from_iter(
                    demand_slices.into_iter(),
                    &commodity_ids,
                    &region_ids,
                    &commodity_regions,
                    &time_slice_info,
                )
                .unwrap(),
                expected
            );
        }

        // Empty CSV file
        assert!(read_demand_slices_from_iter(
            iter::empty(),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // Bad commodity
        let demand_slice = DemandSlice {
            commodity_id: "COM2".into(),
            region_id: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        assert!(read_demand_slices_from_iter(
            iter::once(demand_slice.clone()),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // Bad region
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "FRA".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        assert!(read_demand_slices_from_iter(
            iter::once(demand_slice.clone()),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // Bad time slice selection
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "summer".into(),
            fraction: 1.0,
        };
        assert!(read_demand_slices_from_iter(
            iter::once(demand_slice.clone()),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        {
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
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "winter".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &commodity_regions,
                &time_slice_info,
            )
            .is_err());
        }

        // Same time slice twice
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 0.5,
        };
        assert!(read_demand_slices_from_iter(
            iter::repeat_n(demand_slice.clone(), 2),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // Whole season and single time slice conflicting
        let demand_slice_season = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 0.5,
        };
        assert!(read_demand_slices_from_iter(
            [demand_slice, demand_slice_season].into_iter(),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // Fractions don't sum to one
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 0.5,
        };
        assert!(read_demand_slices_from_iter(
            iter::once(demand_slice),
            &commodity_ids,
            &region_ids,
            &commodity_regions,
            &time_slice_info,
        )
        .is_err());

        // No corresponding entry for commodity + region in demand CSV file
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter".into(),
            fraction: 1.0,
        };
        assert!(read_demand_slices_from_iter(
            iter::once(demand_slice),
            &commodity_ids,
            &region_ids,
            &HashSet::new(),
            &time_slice_info,
        )
        .is_err());
    }
}
