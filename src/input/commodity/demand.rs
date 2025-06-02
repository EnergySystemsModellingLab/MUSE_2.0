//! Code for working with demand for a given commodity. Demand can vary by region, year and time
//! slice.
use super::super::*;
use super::demand_slicing::{read_demand_slices, DemandSliceMap};
use crate::commodity::{Commodity, CommodityID, CommodityType, DemandMap};
use crate::id::IDCollection;
use crate::region::RegionID;
use crate::time_slice::TimeSliceInfo;
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
pub type AnnualDemandMap = HashMap<(CommodityID, RegionID, u32), f64>;

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
    commodities: &IndexMap<CommodityID, Commodity>,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<CommodityID, DemandMap>> {
    // Get set of SVD commodity IDs
    let svd_commodity_ids: HashSet<CommodityID> = commodities
        .iter()
        .filter(|(_, commodity)| commodity.kind == CommodityType::ServiceDemand)
        .map(|(id, _)| id.clone())
        .collect();

    let demand = read_demand_file(model_dir, &svd_commodity_ids, region_ids, milestone_years)?;
    let slices = read_demand_slices(model_dir, &svd_commodity_ids, region_ids, time_slice_info)?;

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
    svd_commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
) -> Result<AnnualDemandMap> {
    let file_path = model_dir.join(DEMAND_FILE_NAME);
    let iter = read_csv(&file_path)?;
    read_demand_from_iter(iter, svd_commodity_ids, region_ids, milestone_years)
        .with_context(|| input_err_msg(file_path))
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
    svd_commodity_ids: &HashSet<CommodityID>,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
) -> Result<AnnualDemandMap>
where
    I: Iterator<Item = Demand>,
{
    let mut map = AnnualDemandMap::new();
    for demand in iter {
        let commodity_id = svd_commodity_ids
            .get_id(&demand.commodity_id)
            .with_context(|| {
                format!(
                    "Can only provide demand data for SVD commodities. Found entry for '{}'",
                    demand.commodity_id
                )
            })?;
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

        ensure!(
            map.insert(
                (commodity_id.clone(), region_id.clone(), demand.year),
                demand.demand
            )
            .is_none(),
            "Duplicate demand entries (commodity: {}, region: {}, year: {})",
            commodity_id,
            region_id,
            demand.year
        );
    }

    // Check that demand data is specified for all combinations of commodity, region and year
    for commodity_id in svd_commodity_ids {
        let mut missing_keys = Vec::new();
        for region_id in region_ids {
            for year in milestone_years {
                if !map.contains_key(&(commodity_id.clone(), region_id.clone(), *year)) {
                    missing_keys.push((region_id.clone(), *year));
                }
            }
        }
        ensure!(
            missing_keys.is_empty(),
            "Commodity {} is missing demand data for {:?}",
            commodity_id,
            missing_keys
        );
    }

    Ok(map)
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
    for ((commodity_id, region_id, year), annual_demand) in demand.iter() {
        for time_slice in time_slice_info.iter_ids() {
            let slice_key = (commodity_id.clone(), region_id.clone(), time_slice.clone());

            // NB: This has already been checked, so shouldn't fail
            let demand_fraction = slices.get(&slice_key).unwrap();

            // Get or create entry
            let map = map
                .entry(commodity_id.clone())
                .or_insert_with(DemandMap::new);

            // Add a new demand entry
            map.insert(
                (region_id.clone(), *year, time_slice.clone()),
                annual_demand * demand_fraction,
            );
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{assert_error, commodity_ids, region_ids};
    use rstest::rstest;
    use std::fs::File;
    use std::io::Write;

    use std::path::Path;
    use tempfile::tempdir;

    #[rstest]
    fn test_read_demand_from_iter(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        let demand = [
            Demand {
                year: 2020,
                region_id: "GBR".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "USA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 11.0,
            },
        ];

        // Valid
        assert!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020]).is_ok()
        );
    }

    #[rstest]
    fn test_read_demand_from_iter_bad_commodity_id(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Bad commodity ID
        let demand = [
            Demand {
                year: 2020,
                region_id: "GBR".to_string(),
                commodity_id: "commodity2".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "USA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 11.0,
            },
        ];
        assert_error!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020]),
            "Can only provide demand data for SVD commodities. Found entry for 'commodity2'"
        );
    }

    #[rstest]
    fn test_read_demand_from_iter_bad_region_id(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Bad region ID
        let demand = [
            Demand {
                year: 2020,
                region_id: "FRA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "USA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 11.0,
            },
        ];
        assert_error!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020]),
            "Unknown ID FRA found"
        );
    }

    #[rstest]
    fn test_read_demand_from_iter_bad_year(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Bad year
        let demand = [
            Demand {
                year: 2010,
                region_id: "GBR".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "USA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 11.0,
            },
        ];
        assert_error!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020]),
            "Year 2010 is not a milestone year. \
            Input of non-milestone years is currently not supported."
        );
    }

    #[rstest]
    #[case(-1.0)]
    #[case(0.0)]
    #[case(f64::NAN)]
    #[case(f64::NEG_INFINITY)]
    #[case(f64::INFINITY)]
    fn test_read_demand_from_iter_bad_demand(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
        #[case] quantity: f64,
    ) {
        // Bad demand quantity
        let demand = [Demand {
            year: 2020,
            region_id: "GBR".to_string(),
            commodity_id: "commodity1".to_string(),
            demand: quantity,
        }];
        assert_error!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020],),
            "Demand must be a valid number greater than zero"
        );
    }

    #[rstest]
    fn test_read_demand_from_iter_multiple_entries(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Multiple entries for same commodity and region
        let demand = [
            Demand {
                year: 2020,
                region_id: "GBR".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "GBR".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 10.0,
            },
            Demand {
                year: 2020,
                region_id: "USA".to_string(),
                commodity_id: "commodity1".to_string(),
                demand: 11.0,
            },
        ];
        assert_error!(
            read_demand_from_iter(demand.into_iter(), &commodity_ids, &region_ids, &[2020]),
            "Duplicate demand entries (commodity: commodity1, region: GBR, year: 2020)"
        );
    }

    #[rstest]
    fn test_read_demand_from_iter_missing_year(
        commodity_ids: HashSet<CommodityID>,
        region_ids: HashSet<RegionID>,
    ) {
        // Missing entry for a milestone year
        let demand = Demand {
            year: 2020,
            region_id: "GBR".to_string(),
            commodity_id: "commodity1".to_string(),
            demand: 10.0,
        };
        assert!(read_demand_from_iter(
            std::iter::once(demand),
            &commodity_ids,
            &region_ids,
            &[2020, 2030]
        )
        .is_err());
    }

    /// Create an example demand file in dir_path
    fn create_demand_file(dir_path: &Path) {
        let file_path = dir_path.join(DEMAND_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "commodity_id,region_id,year,demand\n\
            commodity1,GBR,2020,10\n\
            commodity1,USA,2020,11\n"
        )
        .unwrap();
    }

    #[rstest]
    fn test_read_demand_file(commodity_ids: HashSet<CommodityID>, region_ids: HashSet<RegionID>) {
        let dir = tempdir().unwrap();
        create_demand_file(dir.path());
        let milestone_years = [2020];
        let expected = AnnualDemandMap::from_iter([
            (("commodity1".into(), "GBR".into(), 2020), 10.0),
            (("commodity1".into(), "USA".into(), 2020), 11.0),
        ]);
        let demand =
            read_demand_file(dir.path(), &commodity_ids, &region_ids, &milestone_years).unwrap();
        assert_eq!(demand, expected);
    }
}
