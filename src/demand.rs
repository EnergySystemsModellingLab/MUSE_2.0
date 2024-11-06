use crate::input::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ops::RangeInclusive;
use std::path::Path;
use std::rc::Rc;

const DEMAND_FILE_NAME: &str = "demand.csv";
const DEMAND_SLICES_FILE_NAME: &str = "demand_slicing.csv";

/// Represents a single demand entry in the dataset.
#[derive(Debug, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DemandSlice {
    pub commodity_id: String,
    pub region_id: String,
    pub time_slice: String,
    pub fraction: f64,
}

fn read_demand_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, HashMap<Rc<str>, Demand>>, Box<dyn Error>>
where
    I: Iterator<Item = Demand>,
{
    let mut map_by_commodity = HashMap::new();

    for demand in iter {
        // **TODO**: add validation checks here? e.g. check not negative, apply interpolation and
        // extrapolation rules?
        let commodity_id = commodity_ids.get_id(&demand.commodity_id)?;
        let region_id = region_ids.get_id(&demand.region_id)?;

        if !year_range.contains(&demand.year) {
            Err(format!("Year {} is out of range", demand.year))?;
        }

        // Get entry for this commodity
        let map_by_region = map_by_commodity
            .entry(commodity_id)
            .or_insert_with(|| HashMap::with_capacity(1));

        if map_by_region.insert(region_id, demand).is_some() {
            Err("Multiple entries for same commodity and region found")?;
        }
    }

    Ok(map_by_commodity)
}

fn read_demand_file(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, HashMap<Rc<str>, Demand>> {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    read_demand_from_iter(read_csv(&file_path), commodity_ids, region_ids, year_range)
        .unwrap_input_err(&file_path)
}

fn try_get_demand<'a>(
    commodity_id: &str,
    region_id: &str,
    demand: &'a mut HashMap<Rc<str>, HashMap<Rc<str>, Demand>>,
) -> Option<&'a mut Demand> {
    demand.get_mut(commodity_id)?.get_mut(region_id)
}

fn read_demand_slices_from_iter<I>(
    iter: I,
    file_path: &Path,
    demand: &mut HashMap<Rc<str>, HashMap<Rc<str>, Demand>>,
) where
    I: Iterator<Item = DemandSlice>,
{
    for slice in iter {
        let demand =
            try_get_demand(&slice.commodity_id, &slice.region_id, demand).unwrap_or_else(|| {
                input_panic(
                    file_path,
                    &format!(
                        "No demand specified for commodity {} in region {}",
                        &slice.commodity_id, &slice.region_id
                    ),
                )
            });

        demand.demand_slices.push(slice);
    }

    // TODO: Check for demand entries without any demand slices specified?
}

fn read_demand_slices(model_dir: &Path, demand: &mut HashMap<Rc<str>, HashMap<Rc<str>, Demand>>) {
    let file_path = model_dir.join(DEMAND_SLICES_FILE_NAME);
    read_demand_slices_from_iter(read_csv(&file_path), &file_path, demand)
}

/// Reads demand data from a CSV file.
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
/// This function returns demand data grouped by commodity and then region.
pub fn read_demand(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, HashMap<Rc<str>, Demand>> {
    let mut demand = read_demand_file(model_dir, commodity_ids, region_ids, year_range);

    // Read in demand slices
    read_demand_slices(model_dir, &mut demand);

    demand
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
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
                    HashMap::from_iter([
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
                    ])
                )]
                .into_iter()
            )
        );
    }

    fn create_demand() -> HashMap<Rc<str>, HashMap<Rc<str>, Demand>> {
        let demand_by_region = [(
            "GBR".into(),
            Demand {
                commodity_id: "COM1".into(),
                region_id: "GBR".into(),
                year: 2020,
                demand: 1.0,
                demand_slices: Vec::new(),
            },
        )];

        [("COM1".into(), demand_by_region.into_iter().collect())]
            .into_iter()
            .collect()
    }

    #[test]
    fn test_read_demand_slices_from_iter_good() {
        let p = PathBuf::new();
        let mut demand = create_demand();
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        read_demand_slices_from_iter([demand_slice.clone()].into_iter(), &p, &mut demand);
        assert_eq!(
            try_get_demand("COM1", "GBR", &mut demand)
                .unwrap()
                .demand_slices,
            vec![demand_slice]
        );
    }

    /// Demand slice with invalid commodity
    #[test]
    #[should_panic]
    fn test_read_demand_slices_from_iter_bad_commodity() {
        let p = PathBuf::new();
        let mut demand = create_demand();
        let demand_slice = DemandSlice {
            commodity_id: "COM2".into(),
            region_id: "GBR".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        read_demand_slices_from_iter([demand_slice].into_iter(), &p, &mut demand);
    }

    /// Demand slice with invalid region
    #[test]
    #[should_panic]
    fn test_read_demand_slices_from_iter_bad_region() {
        let p = PathBuf::new();
        let mut demand = create_demand();
        let demand_slice = DemandSlice {
            commodity_id: "COM1".into(),
            region_id: "USA".into(),
            time_slice: "winter.day".into(),
            fraction: 1.0,
        };
        read_demand_slices_from_iter([demand_slice].into_iter(), &p, &mut demand);
    }
}
