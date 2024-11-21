//! Code for working with demand for a given commodity. Demand can vary by region and year.
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceSelection};
use crate::{input::*, region, time_slice};
use anyhow::{ensure, Context, Result};
use float_cmp::approx_eq;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::ops::RangeInclusive;
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

/// How demand varies by time slice
#[derive(Debug, Clone, PartialEq)]
pub struct DemandSlice {
    /// Which time slice(s) this applies to
    pub time_slice: TimeSliceSelection,
    /// The fraction of total demand (between 0 and 1 inclusive)
    pub fraction: f64,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DemandHashMapKey {
    region_id: Rc<str>,
    milestone_year: u32,
    time_slice: TimeSliceID,
}

/// A [HashMap] of [Demand] grouped by region ID
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DemandHashMap(HashMap<DemandHashMapKey, Demand>);

impl DemandHashMap {
    /// Create a new empty [DemandHashMap]
    pub fn new() -> DemandHashMap {
        DemandHashMap(HashMap::new())
    }

    /// Get demand for a particular region.
    pub fn get(
        &self,
        region_id: Rc<str>,
        milestone_year: u32,
        time_slice: TimeSliceID,
    ) -> Option<&Demand> {
        self.0.get(&DemandHashMapKey {
            region_id,
            milestone_year,
            time_slice,
        })
    }
}

/// A [HashMap] of [Demand] grouped first by commodity, then region
type CommodityDemandHashMap = HashMap<Rc<str>, DemandHashMap>;

#[derive(PartialEq, Eq, Hash)]
struct DemandKey {
    commodity_id: Rc<str>,
    region_id: Rc<str>,
    year: u32,
}

/// Read the demand data from an iterator
///
/// # Arguments
///
/// * `iter` - An iterator of `Demand`s
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `year_range` - The year range for the simulation
///
/// # Returns
///
/// The demand data (except for the demand slice information), grouped by commodity and region.
fn read_demand_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<DemandKey, f64>>
where
    I: Iterator<Item = Demand>,
{
    let mut map = HashMap::new();

    for demand in iter {
        let commodity_id = commodity_ids.get_id(&demand.commodity_id)?;
        let region_id = region_ids.get_id(&demand.region_id)?;

        ensure!(
            year_range.contains(&demand.year),
            "Year {} is out of range",
            demand.year
        );

        ensure!(
            demand.demand.is_normal() && demand.demand > 0.0,
            "Demand must be a valid number greater than zero"
        );

        let key = DemandKey {
            commodity_id,
            region_id,
            year: demand.year,
        };

        ensure!(
            map.insert(key, demand.demand).is_none(),
            "Multiple entries for same commodity and region found"
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
/// * `year_range` - The year range for the simulation
///
/// # Returns
///
/// The demand data except for the demand slice information, which resides in a separate CSV file.
/// The data is grouped by commodity and region.
fn read_demand_file(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> CommodityDemandHashMap {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    read_demand_from_iter(read_csv(&file_path), commodity_ids, region_ids, year_range)
        .unwrap_input_err(&file_path)
}

/// Try to get demand for the given commodity and region. Returns `None` if not found.
fn get_demand_mut<'a>(
    demand: &'a mut CommodityDemandHashMap,
    commodity_id: &str,
    region_id: &Rc<str>,
    milestone_year: u32,
) -> Option<&'a mut Demand> {
    let key = DemandHashMapKey {
        region_id: Rc::clone(region_id),
        milestone_year,
    };
    demand.get_mut(commodity_id)?.0.get_mut(&key)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct DemandSliceFractionsKey {
    commodity_id: Rc<str>,
    region_id: Rc<str>,
    time_slice: TimeSliceID,
}

impl Display for DemandSliceFractionsKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "commodity: {}, region: {}, time slice: {}",
            self.commodity_id, self.region_id, self.time_slice
        )
    }
}

/// Read demand slices from an iterator and store them in `demand`.
fn read_demand_slices_from_iter<I>(
    iter: I,
    time_slice_info: &TimeSliceInfo,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<DemandSliceFractionsKey, f64>>
where
    I: Iterator<Item = DemandSliceRaw>,
{
    let mut demand_slices = HashMap::new();

    for slice in iter {
        let commodity_id = commodity_ids.get_id(&slice.commodity_id)?;
        let region_id = region_ids.get_id(&slice.region_id)?;

        let ts_selection = time_slice_info.get_selection(&slice.time_slice)?;
        for time_slice in time_slice_info.iter_selection(&ts_selection) {
            let key = DemandSliceFractionsKey {
                commodity_id: Rc::clone(&commodity_id),
                region_id: Rc::clone(&region_id),
                time_slice,
            };

            ensure!(
                demand_slices.insert(key.clone(), slice.fraction).is_none(),
                "Demand slice covered by two or more entries ({key})"
            );
        }
    }

    for (commodity_id, region_id) in commodity_ids.iter().zip(region_ids.iter()) {
        let mut sum = 0.0;
        for time_slice in time_slice_info.iter() {
            let key = DemandSliceFractionsKey {
                commodity_id: Rc::clone(commodity_id),
                region_id: Rc::clone(region_id),
                time_slice,
            };
            let fraction = demand_slices
                .get(&key)
                .with_context(|| format!("Missing demand slice entry: {key}",))?;

            sum += fraction;
        }

        ensure!(
            approx_eq!(f64, sum, 1.0, epsilon = 1e-5),
            "Sum of demand slice fractions does not equal one \
            (actual: {sum}, commodity: {commodity_id}, region: {region_id})",
        );
    }

    Ok(demand_slices)
}

/// Read demand slices from specified model directory.
/// FIX ME!!!!!!!!!!1
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `time_slice_info` - Information about seasons and times of day
/// * `demand` - Demand data grouped by commodity and region
fn read_demand_slices(
    model_dir: &Path,
    time_slice_info: &TimeSliceInfo,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<DemandSliceFractionsKey, f64> {
    let file_path = model_dir.join(DEMAND_SLICES_FILE_NAME);
    read_demand_slices_from_iter(
        read_csv(&file_path),
        time_slice_info,
        commodity_ids,
        region_ids,
    )
    .unwrap_input_err(&file_path)
}

/// Reads demand data from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodity_ids` - All possible IDs of commodities
/// * `region_ids` - All possible IDs for regions
/// * `time_slice_info` - Information about seasons and times of day
/// * `year_range` - The year range for the simulation
///
/// # Returns
///
/// This function returns demand data grouped by commodity and then region.
pub fn read_demand(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    year_range: &RangeInclusive<u32>,
) -> CommodityDemandHashMap {
    let mut demand = read_demand_file(model_dir, commodity_ids, region_ids, year_range);

    // Read in demand slices
    let slices = read_demand_slices(model_dir, time_slice_info, &mut demand);

    demand
}

#[cfg(test)]
mod tests {
    use crate::time_slice::TimeSliceID;

    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    /// Create an example demand file in dir_path
    fn create_demand_file(dir_path: &Path) {
        let file_path = dir_path.join(DEMAND_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "commodity_id,region_id,year,demand
COM1,North,2023,10
COM1,South,2023,11
COM1,East,2023,12
COM1,West,2023,13"
        )
        .unwrap();
    }

    #[test]
    fn test_read_demand_from_iter() {
        let commodity_ids = ["COM1".into()].into_iter().collect();
        let region_ids = ["North".into(), "South".into()].into_iter().collect();
        let year_range = 2020..=2030;

        // Valid
        let demand = [
            Demand {
                year: 2023,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
                demand_slices: Vec::new(),
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &year_range
        )
        .is_ok());

        // Bad commodity ID
        let demand = [
            Demand {
                year: 2023,
                region_id: "North".to_string(),
                commodity_id: "COM2".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
                demand_slices: Vec::new(),
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &year_range
        )
        .is_err());

        // Bad region ID
        let demand = [
            Demand {
                year: 2023,
                region_id: "East".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
                demand_slices: Vec::new(),
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &year_range
        )
        .is_err());

        // Bad year
        let demand = [
            Demand {
                year: 2010,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
                demand_slices: Vec::new(),
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &year_range
        )
        .is_err());

        // Bad demand quantity
        macro_rules! test_quantity {
            ($quantity: expr) => {
                let demand = [Demand {
                    year: 2023,
                    region_id: "North".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: $quantity,
                    demand_slices: Vec::new(),
                }];
                assert!(read_demand_from_iter(
                    demand.into_iter(),
                    &commodity_ids,
                    &region_ids,
                    &year_range
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
                year: 2023,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2023,
                region_id: "South".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 11.0,
                demand_slices: Vec::new(),
            },
        ];
        assert!(read_demand_from_iter(
            demand.into_iter(),
            &commodity_ids,
            &region_ids,
            &year_range
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
        let year_range = 2020..=2030;
        let demand = read_demand_file(dir.path(), &commodity_ids, &region_ids, &year_range);
        assert_eq!(
            demand,
            HashMap::from_iter(
                [(
                    "COM1".into(),
                    DemandHashMap(HashMap::from_iter([
                        (
                            "North".into(),
                            Demand {
                                year: 2023,
                                region_id: "North".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 10.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "South".into(),
                            Demand {
                                year: 2023,
                                region_id: "South".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 11.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "East".into(),
                            Demand {
                                year: 2023,
                                region_id: "East".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 12.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "West".into(),
                            Demand {
                                year: 2023,
                                region_id: "West".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 13.0,
                                demand_slices: Vec::new()
                            }
                        )
                    ]))
                )]
                .into_iter()
            )
        );
    }

    #[test]
    fn test_read_demand_slices_from_iter() {
        let time_slice_info = TimeSliceInfo {
            seasons: ["winter".into()].into_iter().collect(),
            times_of_day: ["day".into()].into_iter().collect(),
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

        // Demand grouped by region
        let demand: HashMap<_, _> = [(
            "COM1".into(),
            DemandHashMap(
                [(
                    "GBR".into(),
                    Demand {
                        commodity_id: "COM1".into(),
                        region_id: "GBR".into(),
                        year: 2020,
                        demand: 1.0,
                        demand_slices: Vec::new(),
                    },
                )]
                .into_iter()
                .collect(),
            ),
        )]
        .into_iter()
        .collect();

        // Valid
        {
            let mut demand = demand.clone();
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "winter.day".into(),
                fraction: 1.0,
            };
            read_demand_slices_from_iter(
                [demand_slice.clone()].into_iter(),
                &time_slice_info,
                &mut demand,
            )
            .unwrap();
            let time_slice = time_slice_info.get_selection("winter.day").unwrap();
            assert_eq!(
                get_demand_mut("COM1", "GBR", &mut demand)
                    .unwrap()
                    .demand_slices,
                vec![DemandSlice {
                    time_slice,
                    fraction: 1.0
                }]
            );
        }

        // Bad commodity
        {
            let mut demand = demand.clone();
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM2".into(),
                region_id: "GBR".into(),
                time_slice: "winter.day".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                [demand_slice].into_iter(),
                &time_slice_info,
                &mut demand
            )
            .is_err());
        }

        // Bad region
        {
            let mut demand = demand.clone();
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "USA".into(),
                time_slice: "winter.day".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                [demand_slice].into_iter(),
                &time_slice_info,
                &mut demand
            )
            .is_err());
        }

        // Bad time slice
        {
            let mut demand = demand.clone();
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "summer.night".into(),
                fraction: 1.0,
            };
            assert!(read_demand_slices_from_iter(
                [demand_slice].into_iter(),
                &time_slice_info,
                &mut demand,
            )
            .is_err());
        }

        // Missing demand slicing
        {
            let mut demand = demand.clone();
            assert!(
                read_demand_slices_from_iter([].into_iter(), &time_slice_info, &mut demand)
                    .is_err()
            );
        }

        // Time slice fractions don't sum to one
        {
            let mut demand = demand.clone();
            let demand_slice = DemandSliceRaw {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                time_slice: "winter.day".into(),
                fraction: 0.5,
            };
            assert!(read_demand_slices_from_iter(
                [demand_slice].into_iter(),
                &time_slice_info,
                &mut demand,
            )
            .is_err());
        }
    }

    #[test]
    fn test_demand_hash_map_get() {
        let demand = Demand {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            year: 2020,
            demand: 1.0,
            demand_slices: Vec::new(),
        };
        let demand_hash_map = DemandHashMap([("GBR".into(), demand.clone())].into_iter().collect());
        assert!(*demand_hash_map.get("GBR").unwrap() == demand);
    }
}
