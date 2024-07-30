use crate::input::*;
use crate::time_slice::TimeSliceLevel;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
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

/// Read commodity data from the specified model directory.
pub fn read_commodities(model_dir: &Path) -> HashMap<Rc<str>, Commodity> {
    let mut commodities = read_csv_id_file::<Commodity>(&model_dir.join(COMMODITY_FILE_NAME));
    let commodity_ids = HashSet::from_iter(commodities.keys().cloned());
    let mut costs =
        read_csv_grouped_by_id(&model_dir.join(COMMODITY_COSTS_FILE_NAME), &commodity_ids);

    for (id, commodity) in commodities.iter_mut() {
        if let Some(costs) = costs.remove(id) {
            commodity.costs = costs;
        }
    }

    commodities
}
