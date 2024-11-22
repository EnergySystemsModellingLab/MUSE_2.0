#![allow(missing_docs)]
use crate::demand::{read_demand, Demand};
use crate::input::*;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use anyhow::{ensure, Result};
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
    pub costs: CommodityCostMap,
    #[serde(skip)]
    pub demand_by_region: HashMap<Rc<str>, Demand>,
}
define_id_getter! {Commodity}

impl CommodityCostMap {
    /// Create a new, empty [`CommodityCostMap`]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Retrieve a [`CommodityCost`] from the map
    pub fn get(
        &self,
        region_id: Rc<str>,
        year: u32,
        time_slice: TimeSliceID,
    ) -> Option<&CommodityCost> {
        let key = CommodityCostKey {
            region_id,
            year,
            time_slice,
        };
        self.0.get(&key)
    }
}

impl Default for CommodityCostMap {
    /// Create a new, empty [`CommodityCostMap`]
    fn default() -> Self {
        Self::new()
    }
}

/// Type of balance for application of cost
#[derive(PartialEq, Clone, Debug, DeserializeLabeledStringEnum)]
pub enum BalanceType {
    #[string = "net"]
    Net,
    #[string = "cons"]
    Consumption,
    #[string = "prod"]
    Production,
}

/// Cost parameters for each commodity
#[derive(PartialEq, Debug, Deserialize, Clone)]
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

/// Cost parameters for each commodity
#[derive(PartialEq, Clone, Debug)]
pub struct CommodityCost {
    /// Type of balance for application of cost.
    pub balance_type: BalanceType,
    /// Cost per unit commodity. For example, if a CO2 price is specified in input data, it can be applied to net CO2 via this value.
    pub value: f64,
}

/// Used for looking up [`CommodityCost`]s in a [`CommodityCostMap`]
#[derive(PartialEq, Eq, Hash, Debug)]
struct CommodityCostKey {
    region_id: Rc<str>,
    year: u32,
    time_slice: TimeSliceID,
}

/// A data structure for easy lookup of [`CommodityCost`]s
#[derive(PartialEq, Debug)]
pub struct CommodityCostMap(HashMap<CommodityCostKey, CommodityCost>);

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
) -> Result<HashMap<Rc<str>, CommodityCostMap>>
where
    I: Iterator<Item = CommodityCostRaw>,
{
    let mut map = HashMap::new();

    for cost in iter {
        let commodity_id = commodity_ids.get_id(&cost.commodity_id)?;
        let region_id = region_ids.get_id(&cost.region_id)?;
        let ts_selection = time_slice_info.get_selection(&cost.time_slice)?;

        if milestone_years.binary_search(&cost.year).is_err() {
            todo!(
                "Year {} is not a milestone year. \
                Input of non-milestone years is currently not supported.",
                cost.year
            );
        }

        // Get or create CommodityCostMap for this commodity
        let map = map
            .entry(commodity_id)
            .or_insert_with(|| CommodityCostMap(HashMap::with_capacity(1)));

        for time_slice in time_slice_info.iter_selection(&ts_selection) {
            let key = CommodityCostKey {
                region_id: Rc::clone(&region_id),
                year: cost.year,
                time_slice: time_slice.clone(),
            };
            let value = CommodityCost {
                balance_type: cost.balance_type.clone(),
                value: cost.value,
            };

            ensure!(
                map.0.insert(key, value).is_none(),
                "Commodity cost entry covered by more than one time slice \
                (region: {}, year: {}, time slice: {})",
                region_id,
                cost.year,
                time_slice
            );
        }
    }

    Ok(map)
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
) -> HashMap<Rc<str>, CommodityCostMap> {
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
    use std::iter;

    #[test]
    fn test_commodity_cost_map_get() {
        let ts = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let key = CommodityCostKey {
            region_id: "GBR".into(),
            year: 2010,
            time_slice: ts.clone(),
        };
        let value = CommodityCost {
            balance_type: BalanceType::Consumption,
            value: 0.5,
        };
        let map = CommodityCostMap(HashMap::from_iter([(key, value.clone())]));
        assert_eq!(map.get("GBR".into(), 2010, ts).unwrap(), &value);
    }

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
        let key1 = CommodityCostKey {
            region_id: "GBR".into(),
            year: cost1.year,
            time_slice: time_slice.clone(),
        };
        let value1 = CommodityCost {
            balance_type: cost1.balance_type.clone(),
            value: cost1.value,
        };
        let key2 = CommodityCostKey {
            region_id: "FRA".into(),
            year: cost2.year,
            time_slice: time_slice.clone(),
        };
        let value2 = CommodityCost {
            balance_type: cost2.balance_type.clone(),
            value: cost2.value,
        };
        let map = CommodityCostMap(HashMap::from_iter([(key1, value1), (key2, value2)]));
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
    }

    #[test]
    #[should_panic]
    fn test_read_commodity_costs_iter_non_milestone_year() {
        let commodity_ids = ["commodity".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();
        let time_slice_info = TimeSliceInfo::default();
        let milestone_years = [2010, 2020];

        let cost = CommodityCostRaw {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2011, // NB: Non-milestone year
            time_slice: "all-year.all-day".into(),
            value: 0.5,
        };
        let _ = read_commodity_costs_iter(
            iter::once(cost),
            &commodity_ids,
            &region_ids,
            &time_slice_info,
            &milestone_years,
        );
    }
}
