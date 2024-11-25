//! Code for working with demand for a given commodity. Demand can vary by region and year.
use crate::input::*;
use crate::time_slice::{TimeSliceInfo, TimeSliceSelection};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const DEMAND_FILE_NAME: &str = "demand.csv";
const DEMAND_SLICES_FILE_NAME: &str = "demand_slicing.csv";

/// Represents a single demand entry in the dataset.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Demand {
    /// The commodity this demand entry refers to
    pub commodity_id: String,
    /// The region of the demand entry
    pub region_id: String,
    /// The year of the demand entry
    pub year: u32,
    /// Annual demand quantity
    pub demand: f64,

    /// How demand varies by time slice
    #[serde(skip)]
    pub demand_slices: Vec<DemandSlice>,
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

/// A map of [`Demand`], keyed by region
#[derive(PartialEq, Debug, Clone, Default)]
pub struct DemandMap(HashMap<Rc<str>, Demand>);

impl DemandMap {
    /// Create a new, empty [`DemandMap`]
    pub fn new() -> DemandMap {
        DemandMap::default()
    }

    /// Retrieve a [`Demand`] entry from the map
    pub fn get(&self, region_id: &str) -> Option<&Demand> {
        self.0.get(region_id)
    }
}

/// A [`HashMap`] of [`Demand`] grouped first by commodity, then region
type CommodityDemandMap = HashMap<Rc<str>, DemandMap>;

/// Read the demand data from an iterator
///
/// # Arguments
///
/// * `iter` - An iterator of `Demand`s
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
) -> Result<CommodityDemandMap>
where
    I: Iterator<Item = Demand>,
{
    let mut map = HashMap::new();

    for demand in iter {
        let commodity_id = commodity_ids.get_id(&demand.commodity_id)?;
        let region_id = region_ids.get_id(&demand.region_id)?;

        ensure!(
            milestone_years.binary_search(&demand.year).is_ok(),
            "Year {} is not a milestone year. \
            Input of non-milestone years is currently not supported.",
            demand.year
        );

        // Get entry for this commodity
        let map = map
            .entry(commodity_id)
            .or_insert_with(|| DemandMap(HashMap::with_capacity(1)));

        ensure!(
            map.0.insert(region_id, demand).is_none(),
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
) -> Result<CommodityDemandMap> {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    let iter = read_csv(&file_path)?;
    read_demand_from_iter(iter, commodity_ids, region_ids, milestone_years)
}

/// Try to get demand for the given commodity and region. Returns `None` if not found.
fn try_get_demand<'a>(
    commodity_id: &str,
    region_id: &str,
    demand: &'a mut CommodityDemandMap,
) -> Option<&'a mut Demand> {
    demand.get_mut(commodity_id)?.0.get_mut(region_id)
}

/// Read demand slices from an iterator and store them in `demand`.
fn read_demand_slices_from_iter<I>(
    iter: I,
    time_slice_info: &TimeSliceInfo,
    demand: &mut CommodityDemandMap,
) -> Result<()>
where
    I: Iterator<Item = DemandSliceRaw>,
{
    for slice in iter {
        let demand =
            try_get_demand(&slice.commodity_id, &slice.region_id, demand).with_context(|| {
                format!(
                    "No demand specified for commodity {} in region {}",
                    &slice.commodity_id, &slice.region_id
                )
            })?;

        let time_slice = time_slice_info.get_selection(&slice.time_slice)?;
        demand.demand_slices.push(DemandSlice {
            time_slice,
            fraction: slice.fraction,
        });
    }

    Ok(())
}

/// Read demand slices from specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `time_slice_info` - Information about seasons and times of day
/// * `demand` - Demand data grouped by commodity and region
fn read_demand_slices(
    model_dir: &Path,
    time_slice_info: &TimeSliceInfo,
    demand: &mut CommodityDemandMap,
) -> Result<()> {
    let file_path = model_dir.join(DEMAND_SLICES_FILE_NAME);
    let demand_slices_csv = read_csv(&file_path)?;
    read_demand_slices_from_iter(demand_slices_csv, time_slice_info, demand)
        .with_context(|| input_err_msg(file_path))
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
/// This function returns demand data grouped by commodity and then region.
pub fn read_demand(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<CommodityDemandMap> {
    let mut demand = read_demand_file(model_dir, commodity_ids, region_ids, milestone_years)?;

    // Read in demand slices
    read_demand_slices(model_dir, time_slice_info, &mut demand)?;

    Ok(demand)
}

#[cfg(test)]
mod tests {
    use crate::time_slice::TimeSliceID;

    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_demand_map_get() {
        let value = Demand {
            year: 2020,
            region_id: "North".to_string(),
            commodity_id: "COM1".to_string(),
            demand: 10.0,
            demand_slices: Vec::new(),
        };
        let map = DemandMap(HashMap::from_iter([("North".into(), value.clone())]));
        assert_eq!(map.get("North").unwrap(), &value)
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
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
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
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
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
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
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
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
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
            &milestone_years
        )
        .is_err());

        // Multiple entries for same commodity and region
        let demand = [
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
                region_id: "North".to_string(),
                commodity_id: "COM1".to_string(),
                demand: 10.0,
                demand_slices: Vec::new(),
            },
            Demand {
                year: 2020,
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
        let demand = read_demand_file(dir.path(), &commodity_ids, &region_ids, &milestone_years);
        assert_eq!(
            demand.unwrap(),
            HashMap::from_iter(
                [(
                    "COM1".into(),
                    DemandMap(HashMap::from_iter([
                        (
                            "North".into(),
                            Demand {
                                year: 2020,
                                region_id: "North".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 10.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "South".into(),
                            Demand {
                                year: 2020,
                                region_id: "South".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 11.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "East".into(),
                            Demand {
                                year: 2020,
                                region_id: "East".to_string(),
                                commodity_id: "COM1".to_string(),
                                demand: 12.0,
                                demand_slices: Vec::new()
                            }
                        ),
                        (
                            "West".into(),
                            Demand {
                                year: 2020,
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
            DemandMap(
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
                try_get_demand("COM1", "GBR", &mut demand)
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
    }
}
