#![allow(missing_docs)]
use crate::demand::DemandMap;
use crate::input::*;
use crate::time_slice::{TimeSliceID, TimeSliceLevel};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::rc::Rc;

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
    pub demand: DemandMap,
}
define_id_getter! {Commodity}

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
#[derive(PartialEq, Debug, Default)]
pub struct CommodityCostMap(HashMap<CommodityCostKey, CommodityCost>);

impl CommodityCostMap {
    /// Create a new, empty [`CommodityCostMap`]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a [`CommodityCost`] into the map
    pub fn insert(
        &mut self,
        region_id: Rc<str>,
        year: u32,
        time_slice: TimeSliceID,
        value: CommodityCost,
    ) -> Option<CommodityCost> {
        let key = CommodityCostKey {
            region_id,
            year,
            time_slice,
        };
        self.0.insert(key, value)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commodity_cost_map() {
        let ts = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let value = CommodityCost {
            balance_type: BalanceType::Consumption,
            value: 0.5,
        };
        let mut map = CommodityCostMap::new();
        assert!(map
            .insert("GBR".into(), 2010, ts.clone(), value.clone())
            .is_none());
        assert_eq!(map.get("GBR".into(), 2010, ts).unwrap(), &value);
    }
}
