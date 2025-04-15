//! Code for working with demand for a given commodity. Demand can vary by region, year and time
//! slice.
use super::super::*;
use super::demand_slicing::{read_demand_slices, DemandSliceMap, DemandSliceMapKey};
use crate::commodity::{CommodityID, DemandMap};
use crate::id::IDCollection;
use crate::time_slice::TimeSliceInfo;
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const DEMAND_FILE_NAME: &str = "demand.csv";

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

/// A map relating commodity, region and year to annual demand
pub type AnnualDemandMap = HashMap<AnnualDemandMapKey, f64>;

/// A key for an [`AnnualDemandMap`]
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct AnnualDemandMapKey {
    /// The commodity to which this demand applies
    commodity_id: CommodityID,
    /// The region to which this demand applies
    region_id: Rc<str>,
    /// The simulation year to which this demand applies
    year: u32,
}

/// A set of commodity + region pairs
pub type CommodityRegionPairs = HashSet<(CommodityID, Rc<str>)>;

/// Reads demand data from CSV files.
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
/// This function returns [`DemandMap`]s grouped by commodity ID.
pub fn read_demand(
    model_dir: &Path,
    commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<CommodityID, DemandMap>> {
    let (demand, commodity_regions) =
        read_demand_file(model_dir, commodity_ids, region_ids, milestone_years)?;
    let slices = read_demand_slices(
        model_dir,
        commodity_ids,
        region_ids,
        &commodity_regions,
        time_slice_info,
    )?;

    Ok(compute_demand_maps(&demand, &slices, time_slice_info))
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
/// Annual demand data, grouped by commodity, region and milestone year.
fn read_demand_file(
    model_dir: &Path,
    commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> Result<(AnnualDemandMap, CommodityRegionPairs)> {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    let iter = read_csv(&file_path)?;
    read_demand_from_iter(iter, commodity_ids, region_ids, milestone_years)
}

/// Read the demand data from an iterator.
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
/// The demand for each combination of commodity, region and year along with a [`HashSet`] of all
/// commodity + region pairs included in the file.
fn read_demand_from_iter<I>(
    iter: I,
    commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> Result<(AnnualDemandMap, CommodityRegionPairs)>
where
    I: Iterator<Item = Demand>,
{
    let mut map = AnnualDemandMap::new();

    // Keep track of all commodity + region pairs so we can check that every milestone year is
    // covered
    let mut commodity_regions = HashSet::new();

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

        let key = AnnualDemandMapKey {
            commodity_id: commodity_id.clone(),
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

        commodity_regions.insert((commodity_id, region_id));
    }

    // If a commodity + region combination is represented, it must include entries for every
    // milestone year
    for (commodity_id, region_id) in commodity_regions.iter() {
        for year in milestone_years.iter().copied() {
            let key = AnnualDemandMapKey {
                commodity_id: commodity_id.clone(),
                region_id: Rc::clone(region_id),
                year,
            };
            ensure!(
                map.contains_key(&key),
                "Missing milestone year {year} for commodity {commodity_id} in region {region_id}"
            );
        }
    }

    Ok((map, commodity_regions))
}

/// Calculate the demand for each combination of commodity, region, year and time slice.
///
/// # Arguments
///
/// * `demand` - Total annual demand for combinations of commodity, region and year
/// * `slices` - How annual demand is shared between time slices
/// * `time_slice_info` - Information about time slices
///
/// # Returns
///
/// [`DemandMap`]s for combinations of region, year and time slice, grouped by the commodity to
/// which the demand applies.
fn compute_demand_maps(
    demand: &AnnualDemandMap,
    slices: &DemandSliceMap,
    time_slice_info: &TimeSliceInfo,
) -> HashMap<CommodityID, DemandMap> {
    let mut map = HashMap::new();
    for (demand_key, annual_demand) in demand.iter() {
        let commodity_id = &demand_key.commodity_id;
        let region_id = &demand_key.region_id;
        for time_slice in time_slice_info.iter_ids() {
            let slice_key = DemandSliceMapKey {
                commodity_id: commodity_id.clone(),
                region_id: Rc::clone(region_id),
                time_slice: time_slice.clone(),
            };

            // NB: This has already been checked, so shouldn't fail
            let demand_fraction = slices.get(&slice_key).unwrap();

            // Get or create entry
            let map = map
                .entry(commodity_id.clone())
                .or_insert_with(DemandMap::new);

            // Add a new demand entry
            map.insert(
                Rc::clone(region_id),
                demand_key.year,
                time_slice.clone(),
                annual_demand * demand_fraction,
            );
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::iproduct;
    use std::fs::File;
    use std::io::Write;
    use std::iter;
    use std::path::Path;
    use tempfile::tempdir;

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
        let milestone_years = [2020];

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

        // Missing entry for a milestone year
        let demand = Demand {
            year: 2020,
            region_id: "North".to_string(),
            commodity_id: "COM1".to_string(),
            demand: 10.0,
        };
        assert!(read_demand_from_iter(
            iter::once(demand),
            &commodity_ids,
            &region_ids,
            &[2020, 2030]
        )
        .is_err());
    }

    #[test]
    fn test_read_demand_file() {
        let dir = tempdir().unwrap();
        create_demand_file(dir.path());
        let commodity_ids = HashSet::from_iter(iter::once("COM1".into()));
        let region_ids =
            HashSet::from_iter(["North".into(), "South".into(), "East".into(), "West".into()]);
        let milestone_years = [2020];
        let expected = AnnualDemandMap::from_iter([
            (
                AnnualDemandMapKey {
                    commodity_id: "COM1".into(),
                    region_id: "North".into(),
                    year: 2020,
                },
                10.0,
            ),
            (
                AnnualDemandMapKey {
                    commodity_id: "COM1".into(),
                    region_id: "South".into(),
                    year: 2020,
                },
                11.0,
            ),
            (
                AnnualDemandMapKey {
                    commodity_id: "COM1".into(),
                    region_id: "East".into(),
                    year: 2020,
                },
                12.0,
            ),
            (
                AnnualDemandMapKey {
                    commodity_id: "COM1".into(),
                    region_id: "West".into(),
                    year: 2020,
                },
                13.0,
            ),
        ]);
        let (demand, commodity_regions) =
            read_demand_file(dir.path(), &commodity_ids, &region_ids, &milestone_years).unwrap();
        let commodity_regions_expected =
            iproduct!(commodity_ids.iter().cloned(), region_ids.iter().cloned()).collect();
        assert_eq!(demand, expected);
        assert_eq!(commodity_regions, commodity_regions_expected);
    }
}
