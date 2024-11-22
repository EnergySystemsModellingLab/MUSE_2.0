#![allow(missing_docs)]
use crate::demand::{read_demand, Demand};
use crate::input::*;
use crate::time_slice::{TimeSliceInfo, TimeSliceLevel, TimeSliceSelection};
use anyhow::Result;
use itertools::Itertools;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const COMMODITY_FILE_NAME: &str = "commodities.csv";
const COMMODITY_COSTS_FILE_NAME: &str = "commodity_costs.csv";

/// A commodity within the simulation
#[derive(PartialEq, Debug, Deserialize)]
pub struct Commodity {
    /// Unique identifier for the commodity (e.g. "ELC")
    pub id: Rc<str>,
    /// Text description of commodity (e.g. "electricity")
    pub description: String,
    #[serde(rename = "type")] // NB: we can't name a field type as it's a reserved keyword
    /// Commodity balance type. Can be supply = demand (SED), service demand (SVD), non-balance commodity (NBC).
    pub kind: CommodityType,
    /// The time slice level for commodity balance. Can be annual, seasonal or at time slice level.
    pub time_slice_level: TimeSliceLevel,

    #[serde(skip)]
    pub costs: Vec<CommodityCost>,
    #[serde(skip)]
    pub demand_by_region: HashMap<Rc<str>, Demand>,
}
define_id_getter! {Commodity}

macro_rules! define_commodity_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.commodity_id
            }
        }
    };
}

pub(crate) use define_commodity_id_getter;

/// Type of balance for application of cost
#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum BalanceType {
    #[string = "net"]
    Net,
    #[string = "cons"]
    Consumption,
    #[string = "prod"]
    Production,
}

/// Cost parameters for each commodity
#[derive(PartialEq, Debug, Deserialize)]
struct CommodityCostRaw {
    /// Unique identifier for the commodity (e.g. "ELC")
    pub commodity_id: String,
    /// The region to which the commodity cost applies.
    pub region_id: String,
    /// Type of balance for application of cost.
    pub balance_type: BalanceType,
    /// The year to which the cost applies.
    pub year: u32,
    /// The time slice to which the cost applies.
    pub time_slice: String,
    /// Cost per unit commodity. For example, if a CO2 price is specified in input data, it can be applied to net CO2 via this value.
    pub value: f64,
}

impl CommodityCostRaw {
    /// Convert the raw record type into a validated `CommodityCost` type
    fn try_into_commodity_cost(
        self,
        commodity_ids: &HashSet<Rc<str>>,
        region_ids: &HashSet<Rc<str>>,
        time_slice_info: &TimeSliceInfo,
        milestone_years: &[u32],
    ) -> Result<CommodityCost> {
        let commodity_id = commodity_ids.get_id(&self.commodity_id)?;
        let region_id = region_ids.get_id(&self.region_id)?;
        let time_slice = time_slice_info.get_selection(&self.time_slice)?;

        if milestone_years.binary_search(&self.year).is_err() {
            todo!(
                "Year {} is not a milestone year. \
                Input of non-milestone years is currently not supported.",
                self.year
            );
        }

        Ok(CommodityCost {
            commodity_id,
            region_id,
            balance_type: self.balance_type,
            year: self.year,
            time_slice,
            value: self.value,
        })
    }
}

/// Cost parameters for each commodity
#[derive(PartialEq, Debug)]
pub struct CommodityCost {
    pub commodity_id: Rc<str>,
    pub region_id: Rc<str>,
    pub balance_type: BalanceType,
    pub year: u32,
    pub time_slice: TimeSliceSelection,
    pub value: f64,
}
define_commodity_id_getter! {CommodityCost}

/// Commodity balance type
#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum CommodityType {
    #[string = "sed"]
    SupplyEqualsDemand,
    #[string = "svd"]
    ServiceDemand,
    #[string = "inc"]
    InputCommodity,
    #[string = "ouc"]
    OutputCommodity,
}

fn read_commodity_costs_iter<I>(
    iter: I,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<CommodityCost>>>
where
    I: Iterator<Item = CommodityCostRaw>,
{
    iter.map(|cost| {
        cost.try_into_commodity_cost(commodity_ids, region_ids, time_slice_info, milestone_years)
    })
    // Commodity IDs have already been validated
    .process_results(|iter| iter.into_id_map(commodity_ids).unwrap())
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
fn read_commodity_costs(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> HashMap<Rc<str>, Vec<CommodityCost>> {
    let file_path = model_dir.join(COMMODITY_COSTS_FILE_NAME);
    read_commodity_costs_iter(
        read_csv::<CommodityCostRaw>(&file_path),
        commodity_ids,
        region_ids,
        time_slice_info,
        milestone_years,
    )
    .unwrap_input_err(&file_path)
}

/// Read commodity data from the specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `region_ids` - All possible region IDs
/// * `time_slice_info` - Information about time slices
/// * `milestone_years` - All milestone years
///
/// # Returns
///
/// A map containing commodities, grouped by commodity ID.
pub fn read_commodities(
    model_dir: &Path,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> HashMap<Rc<str>, Rc<Commodity>> {
    let commodities = read_csv_id_file::<Commodity>(&model_dir.join(COMMODITY_FILE_NAME));
    let commodity_ids = commodities.keys().cloned().collect();
    let mut costs = read_commodity_costs(
        model_dir,
        &commodity_ids,
        region_ids,
        time_slice_info,
        milestone_years,
    );

    let year_range = *milestone_years.first().unwrap()..=*milestone_years.last().unwrap();
    let mut demand = read_demand(
        model_dir,
        &commodity_ids,
        region_ids,
        time_slice_info,
        &year_range,
    );

    // Populate Vecs for each Commodity
    commodities
        .into_iter()
        .map(|(id, mut commodity)| {
            if let Some(costs) = costs.remove(&id) {
                commodity.costs = costs;
            }
            if let Some(demand) = demand.remove(&id) {
                commodity.demand_by_region = demand;
            }

            (id, commodity.into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_into_commodity_cost() {
        let commodity_ids = ["commodity".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();
        let time_slice_info = TimeSliceInfo::default();
        let milestone_years = vec![2010, 2020];

        // Valid
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "".into(),
            value: 5.0,
        };
        assert!(cost
            .try_into_commodity_cost(
                &commodity_ids,
                &region_ids,
                &time_slice_info,
                &milestone_years
            )
            .is_ok());

        // Bad commodity
        let cost = CommodityCostRaw {
            commodity_id: "commodity2".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "".into(),
            value: 5.0,
        };
        assert!(cost
            .try_into_commodity_cost(
                &commodity_ids,
                &region_ids,
                &time_slice_info,
                &milestone_years
            )
            .is_err());

        // Bad region
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "USA".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "".into(),
            value: 5.0,
        };
        assert!(cost
            .try_into_commodity_cost(
                &commodity_ids,
                &region_ids,
                &time_slice_info,
                &milestone_years
            )
            .is_err());

        // Bad time slice selection
        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "spring".into(),
            value: 5.0,
        };
        assert!(cost
            .try_into_commodity_cost(
                &commodity_ids,
                &region_ids,
                &time_slice_info,
                &milestone_years
            )
            .is_err());
    }
}
