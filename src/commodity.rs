use crate::input::*;
use crate::time_slice::TimeSliceLevel;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;
use std::rc::Rc;

const COMMODITY_FILE_NAME: &str = "commodities.csv";
const COMMODITY_COSTS_FILE_NAME: &str = "commodity_costs.csv";

#[derive(PartialEq, Debug, Deserialize)]
pub struct Commodity {
    pub id: Rc<str>,
    pub description: String,
    #[serde(rename = "type")] // NB: we can't name a field type as it's a reserved keyword
    pub commodity_type: CommodityType,
    pub time_slice_level: TimeSliceLevel,

    #[serde(skip)]
    pub costs: Vec<CommodityCost>,
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
pub struct CommodityCost {
    pub commodity_id: String,
    pub region_id: String,
    pub balance_type: BalanceType,
    pub year: u32,
    pub time_slice: String,
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

fn check_commodity_costs<'a, I>(
    costs: I,
    file_path: &Path,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) where
    I: Iterator<Item = &'a CommodityCost>,
{
    for cost in costs {
        // Check region ID is valid
        if !region_ids.contains(cost.region_id.as_str()) {
            input_panic(
                file_path,
                &format!("Region ID {} is invalid", cost.region_id),
            )
        }

        // Check year is in range
        if !year_range.contains(&cost.year) {
            input_panic(file_path, &format!("Year {} is out of range", cost.year));
        }
    }
}

fn read_commodity_costs(
    model_dir: &Path,
    commodity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, Vec<CommodityCost>> {
    let file_path = model_dir.join(COMMODITY_COSTS_FILE_NAME);
    let costs = read_csv_grouped_by_id::<CommodityCost>(&file_path, commodity_ids);
    check_commodity_costs(costs.values().flatten(), &file_path, region_ids, year_range);

    costs
}

/// Read commodity data from the specified model directory.
pub fn read_commodities(
    model_dir: &Path,
    region_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, Rc<Commodity>> {
    let commodities = read_csv_id_file::<Commodity>(&model_dir.join(COMMODITY_FILE_NAME));
    let commodity_ids = commodities.keys().cloned().collect();
    let mut costs = read_commodity_costs(model_dir, &commodity_ids, region_ids, year_range);

    // Populate Vecs for each Commodity
    commodities
        .into_iter()
        .map(|(id, mut commodity)| {
            if let Some(costs) = costs.remove(&id) {
                commodity.costs = costs;
            }

            (id, commodity.into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    macro_rules! assert_panics {
        ($e:expr) => {
            assert!(std::panic::catch_unwind(|| $e).is_err())
        };
    }

    #[test]
    fn test_check_commodity_costs() {
        let p = PathBuf::new();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();
        let year_range = 2010..=2020;

        // Valid
        let cost = CommodityCost {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 5.0,
        };
        check_commodity_costs([cost].iter(), &p, &region_ids, &year_range);

        // Bad region
        let cost = CommodityCost {
            commodity_id: "commodity".into(),
            region_id: "USA".into(),
            balance_type: BalanceType::Consumption,
            year: 2010,
            time_slice: "winter.day".into(),
            value: 5.0,
        };
        assert_panics!(check_commodity_costs(
            [cost].iter(),
            &p,
            &region_ids,
            &year_range
        ));

        // Bad year
        let cost = CommodityCost {
            commodity_id: "commodity".into(),
            region_id: "GBR".into(),
            balance_type: BalanceType::Consumption,
            year: 1999,
            time_slice: "winter.day".into(),
            value: 5.0,
        };
        assert_panics!(check_commodity_costs(
            [cost].iter(),
            &p,
            &region_ids,
            &year_range
        ));
    }
}