//! Code for working with demand for a given commodity. Demand can vary by region and year.
use crate::input::*;
use crate::time_slice::*;
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const DEMAND_FILE_NAME: &str = "demand.csv";
const DEMAND_SLICES_FILE_NAME: &str = "demand_slicing.csv";

/// Represents a single demand entry in the dataset.
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Demand {
    /// The commodity this demand entry refers to
    commodity_id: String,
    /// The region of the demand entry
    region_id: String,
    /// The year of the demand entry
    year: u32,
    /// Annual demand quantity
    demand: f64,
}

#[derive(Clone, Deserialize)]
struct DemandSliceRaw {
    commodity_id: String,
    region_id: String,
    time_slice: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    fraction: f64,
}

#[derive(PartialEq, Eq, Hash, Debug)]
struct DemandSliceMapKey {
    commodity_id: Rc<str>,
    region_id: Rc<str>,
    time_slice: TimeSliceID,
}

/// A map relating commodity, region and time slice to the fraction of annual demand
type DemandSliceMap = HashMap<DemandSliceMapKey, f64>;

/// How demand varies by time slice
#[derive(Debug, Clone, PartialEq)]
pub struct DemandSlice {
    /// Which time slice(s) this applies to
    pub time_slice: TimeSliceSelection,
    /// The fraction of total demand (between 0 and 1 inclusive)
    pub fraction: f64,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
struct DemandMapKey {
    region_id: Rc<str>,
    year: u32,
    time_slice: TimeSliceID,
}

/// A map relating region, year and time slice to actual demand (not a fraction).
///
/// This data type is exported as this is the way in we want to look up demand outside of this
/// module.
#[derive(PartialEq, Debug, Clone, Default)]
pub struct DemandMap(HashMap<DemandMapKey, f64>);

impl DemandMap {
    /// Create a new, empty [`DemandMap`]
    pub fn new() -> DemandMap {
        DemandMap::default()
    }

    /// Retrieve the demand for the specified parameters
    pub fn get(&self, region_id: Rc<str>, year: u32, time_slice: TimeSliceID) -> Option<f64> {
        self.0
            .get(&DemandMapKey {
                region_id,
                year,
                time_slice,
            })
            .copied()
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
struct DemandMapInternalKey {
    commodity_id: Rc<str>,
    region_id: Rc<str>,
    year: u32,
}

/// A map relating commodity, region and year to annual demand
type DemandMapInternal = HashMap<DemandMapInternalKey, f64>;

/// Read the demand data from an iterator
///
/// # Arguments
///
/// * `iter` - An iterator of [`Demand`]s
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// The demand data (except for the demand slice information), grouped by commodity and region.
fn read_demand_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> Result<DemandMapInternal>
where
    I: Iterator<Item = Demand>,
{
    let mut map = DemandMapInternal::new();

    for demand in iter {
        let commodity_id = commodity_ids.get_id(&demand.commodity_id)?;
        let region_id = region_ids.get_id(&demand.region_id)?;

        ensure!(
            milestone_years.binary_search(&demand.year).is_ok(),
            "Year {} is not a milestone year. \
            Input of non-milestone years is currently not supported.",
            demand.year
        );

        ensure!(
            demand.demand.is_normal() && demand.demand > 0.0,
            "Demand must be a valid number greater than zero"
        );

        let key = DemandMapInternalKey {
            commodity_id: Rc::clone(&commodity_id),
            region_id: Rc::clone(&region_id),
            year: demand.year,
        };
        ensure!(
            map.insert(key, demand.demand).is_none(),
            "Duplicate demand entries (commodity: {}, region: {}, year: {})",
            commodity_id,
            region_id,
            demand.year
        );
    }

    Ok(map)
}

/// Read the demand.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// The demand data except for the demand slice information, which resides in a separate CSV file.
/// The data is grouped by commodity and region.
fn read_demand_file(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> DemandMapInternal {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    read_demand_from_iter(
        read_csv(&file_path),
        commodity_ids,
        region_ids,
        milestone_years,
    )
    .unwrap_input_err(&file_path)
}

type GroupedSlices = HashMap<(Rc<str>, Rc<str>), Vec<(TimeSliceID, f64)>>;

/// Group demand slices by commodity and region.
///
/// The reason this is necessary is because we need to have read in all the time slices for each
/// commodity + region pair, in order to validate them.
fn group_demand_slices<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<GroupedSlices>
where
    I: Iterator<Item = DemandSliceRaw>,
{
    let mut map = HashMap::new();
    for slice in iter {
        let commodity_id = commodity_ids.get_id(&slice.commodity_id)?;
        let region_id = region_ids.get_id(&slice.region_id)?;
        let ts_selection = time_slice_info.get_selection(&slice.time_slice)?;

        let vec = map
            .entry((commodity_id, region_id))
            .or_insert_with(Vec::new);
        vec.extend(
            time_slice_info
                .iter_selection(&ts_selection)
                .map(|time_slice| (time_slice, slice.fraction)),
        );
    }

    ensure!(!map.is_empty(), "Empty demand slices file");

    Ok(map)
}

/// Validate the grouped demand slices and convert to a [`DemandSliceMap`]
fn process_demand_slices(
    slices: GroupedSlices,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap> {
    let mut map = DemandSliceMap::new();
    for ((commodity_id, region_id), vec) in slices.into_iter() {
        ensure!(
            vec.len() == time_slice_info.fractions.len(),
            "Some time slices were not covered for commodity {commodity_id} and region {region_id}",
        );

        check_fractions_sum_to_one(vec.iter().map(|(_, fraction)| fraction).copied())?;
        for (time_slice, fraction) in vec.into_iter() {
            let key = DemandSliceMapKey {
                commodity_id: Rc::clone(&commodity_id),
                region_id: Rc::clone(&region_id),
                time_slice: time_slice.clone(),
            };

            ensure!(map.insert(key, fraction).is_none(),
                "Duplicate demand slicing entry (or same time slice covered by more than one entry) \
                (commodity: {commodity_id}, region: {region_id}, time slice: {time_slice})"
            );
        }
    }

    Ok(map)
}

/// Read demand slices from an iterator
fn read_demand_slices_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<DemandSliceMap>
where
    I: Iterator<Item = DemandSliceRaw>,
{
    let slices = group_demand_slices(iter, commodity_ids, region_ids, time_slice_info)?;
    process_demand_slices(slices, time_slice_info)
}

/// Read demand slices from specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `time_slice_info` - Information about seasons and times of day
fn read_demand_slices(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> DemandSliceMap {
    let file_path = model_dir.join(DEMAND_SLICES_FILE_NAME);
    read_demand_slices_from_iter(
        read_csv(&file_path),
        commodity_ids,
        region_ids,
        time_slice_info,
    )
    .unwrap_input_err(&file_path)
}

/// Calculate the demand for each combination of commodity, region, year and time slice
fn compute_demand_map(
    demand: &DemandMapInternal,
    slices: &DemandSliceMap,
    time_slice_info: &TimeSliceInfo,
) -> HashMap<Rc<str>, DemandMap> {
    let mut map = HashMap::new();
    for (demand_key, annual_demand) in demand.iter() {
        let commodity_id = &demand_key.commodity_id;
        let region_id = &demand_key.region_id;
        for time_slice in time_slice_info.iter() {
            let slice_key = DemandSliceMapKey {
                commodity_id: Rc::clone(commodity_id),
                region_id: Rc::clone(region_id),
                time_slice: time_slice.clone(),
            };

            // This has already been checked, so shouldn't fail
            let demand_fraction = slices.get(&slice_key).unwrap_or_else(|| {
                panic!(
                    "Missing demand slice entry (commodity: {}, region: {}, time slice: {})",
                    commodity_id, region_id, time_slice
                )
            });

            let map = map
                .entry(Rc::clone(commodity_id))
                .or_insert_with(DemandMap::new);

            let key = DemandMapKey {
                region_id: Rc::clone(region_id),
                year: demand_key.year,
                time_slice: time_slice.clone(),
            };
            let demand = annual_demand * demand_fraction;

            // Again, this shouldn't fail as both `demand` and `slices` have already been checked
            // for duplicates, but let's check anyway for the sake of code hygiene
            assert!(
                map.0.insert(key, demand).is_none(),
                "Duplicate demand entry (region: {}, year: {}, time slice: {})",
                region_id,
                demand_key.year,
                time_slice
            );
        }
    }

    map
}

/// Reads demand data from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `time_slice_info` - Information about seasons and times of day
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// !!!!!!!!!!!!!!!!!!!!!!!!!!!!1
/// This function returns demand data grouped by commodity and then region.
pub fn read_demand(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> HashMap<Rc<str>, DemandMap> {
    let demand = read_demand_file(model_dir, commodity_ids, region_ids, milestone_years);
    let slices = read_demand_slices(model_dir, commodity_ids, region_ids, time_slice_info);

    compute_demand_map(&demand, &slices, time_slice_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_slice::TimeSliceID;

    use std::fs::File;
    use std::io::Write;
    use std::iter;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_demand_map_get() {
        let time_slice = TimeSliceID {
            season: "all-year".into(),
            time_of_day: "all-day".into(),
        };
        let key = DemandMapKey {
            region_id: "North".into(),
            year: 2020,
            time_slice: time_slice.clone(),
        };
        let value = 0.2;

        let map = DemandMap(HashMap::from_iter(iter::once((key, value))));
        assert_eq!(map.get("North".into(), 2020, time_slice).unwrap(), value)
    }

    /// Create an example demand file in dir_path
    fn create_demand_file(dir_path: &Path) {
        let file_path = dir_path.join(DEMAND_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "commodity_id,region_id,year,demand
COM1,North,2020,10
COM1,South,2020,11
COM1,East,2020,12
COM1,West,2020,13"
        )
        .unwrap();
    }

    #[test]
    fn test_read_demand_from_iter() {
        let commodity_ids = ["COM1".into()].into_iter().collect();
        let region_ids = ["North".into(), "South".into()].into_iter().collect();
        let milestone_years = [2020, 2030];

        // Valid
        let demand = [
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &milestone_years
        )
        .is_ok());

        // Bad commodity ID
        let demand = [
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM2".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &milestone_years
        )
        .is_err());

        // Bad region ID
        let demand = [
            Demand {
                year: 2020,
                region_id: "East".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &milestone_years
        )
        .is_err());

        // Bad year
        let demand = [
            Demand {
                year: 2010,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &milestone_years
        )
        .is_err());

        // Bad demand quantity
        macro_rules! test_quantity {
            ($quantity: expr) => {
                let demand = [Demand {
                    year: 2020,
                    region_id: "North".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: $quantity,
                }];
                assert!(read_demand_from_iter(
                    demand.into_iter(),
                    &commodity_ids,
                    &region_ids,
                    &milestone_years,
                )
                .is_err());
            };
        }
        test_quantity!(-1.0);
        test_quantity!(0.0);
        test_quantity!(f64::NAN);
        test_quantity!(f64::NEG_INFINITY);
        test_quantity!(f64::INFINITY);

        // Multiple entries for same commodity and region
        let demand = [
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &milestone_years
        )
        .is_err());
    }

    #[test]
    fn test_read_demand_file() {
        let dir = tempdir().unwrap();
        create_demand_file(dir.path());
        let commodity_ids = ["COM1".into()].into_iter().collect();
        let region_ids = ["North".into(), "South".into(), "East".into(), "West".into()]
            .into_iter()
            .collect();
        let milestone_years = [2020, 2030];
        let expected = DemandMapInternal::from_iter([
            (
                DemandMapInternalKey {
                    commodity_id: "COM1".into(),
                    region_id: "North".into(),
                    year: 2020,
                },
                10.0,
            ),
            (
                DemandMapInternalKey {
                    commodity_id: "COM1".into(),
                    region_id: "South".into(),
                    year: 2020,
                },
                11.0,
            ),
            (
                DemandMapInternalKey {
                    commodity_id: "COM1".into(),
                    region_id: "East".into(),
                    year: 2020,
                },
                12.0,
            ),
            (
                DemandMapInternalKey {
                    commodity_id: "COM1".into(),
                    region_id: "West".into(),
                    year: 2020,
                },
                13.0,
            ),
        ]);
        assert_eq!(
            read_demand_file(dir.path(), &commodity_ids, &region_ids, &milestone_years),
            expected
        );
    }

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
        let commodity_ids = iter::once("COM1".into()).collect();
        let region_ids = iter::once("GBR".into()).collect();

        // Valid
        {
            let demand_slice = DemandSliceRaw {
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
            &time_slice_info,
        )
        .is_err());

        // Bad commodity
        {
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM2".into(),
                region_id: "GBR".into(),
                time_slice: "winter.day".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .is_err());
        }

        // Bad region
        {
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "FRA".into(),
                time_slice: "winter.day".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .is_err());
        }

        // Bad time slice selection
        {
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "summer".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .is_err());
        }

        // Some time slices uncovered
        {
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
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "winter".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                iter::once(demand_slice.clone()),
                &commodity_ids,
                &region_ids,
                &time_slice_info,
            )
            .is_err());
        }
    }
}
