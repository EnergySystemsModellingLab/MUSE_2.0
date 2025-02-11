#![allow(missing_docs)]
use crate::input::*;
use crate::time_slice::{TimeSliceID, TimeSliceLevel};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::rc::Rc;

/// A commodity within the simulation. Represents a substance (e.g. CO2) or form of energy (e.g.
/// electricity) that can be produced and/or consumed by technologies in the model.
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

/// Represents a tax or other external cost on a commodity
#[derive(PartialEq, Clone, Debug)]
pub struct CommodityCost {
    /// Type of balance for application of cost.
    pub balance_type: BalanceType,
    /// Cost per unit commodity. For example, if a CO2 price is specified in input data, it can be applied to net CO2 via this value.
    pub value: f64,
}

/// Used for looking up [`CommodityCost`]s in a [`CommodityCostMap`]
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
struct CommodityCostKey {
    region_id: Rc<str>,
    year: u32,
    time_slice: TimeSliceID,
}

/// A data structure for easy lookup of [`CommodityCost`]s
#[derive(PartialEq, Debug, Default, Clone)]
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
        region_id: &Rc<str>,
        year: u32,
        time_slice: &TimeSliceID,
    ) -> Option<&CommodityCost> {
        let key = CommodityCostKey {
            region_id: Rc::clone(region_id),
            year,
            time_slice: time_slice.clone(),
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

/// A map relating region, year and time slice to demand (in real units, not a fraction).
///
/// This data type is exported as this is the way in we want to look up demand outside of this
/// module.
#[derive(PartialEq, Debug, Clone, Default)]
pub struct DemandMap(HashMap<DemandMapKey, f64>);

/// The key for a [`DemandMap`]
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
struct DemandMapKey {
    region_id: Rc<str>,
    year: u32,
    time_slice: TimeSliceID,
}

impl DemandMap {
    /// Create a new, empty [`DemandMap`]
    pub fn new() -> DemandMap {
        DemandMap::default()
    }

    /// Retrieve the demand for the specified region, year and time slice
    pub fn get(&self, region_id: &Rc<str>, year: u32, time_slice: &TimeSliceID) -> f64 {
        self.0
            .get(&DemandMapKey {
                region_id: region_id.clone(),
                year,
                time_slice: time_slice.clone(),
            })
            .copied()
            .expect("Missing demand entry")
    }

    /// Insert a new demand entry for the specified region, year and time slice
    pub fn insert(&mut self, region_id: Rc<str>, year: u32, time_slice: TimeSliceID, demand: f64) {
        self.0.insert(
            DemandMapKey {
                region_id,
                year,
                time_slice,
            },
            demand,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demand_map() {
        let time_slice = TimeSliceID {
            season: "all-year".into(),
            time_of_day: "all-day".into(),
        };
        let value = 0.25;
        let mut map = DemandMap::new();
        map.insert("North".into(), 2020, time_slice.clone(), value);

        assert_eq!(map.get(&"North".into(), 2020, &time_slice), value)
    }

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
        assert_eq!(map.get(&"GBR".into(), 2010, &ts).unwrap(), &value);
    }
}
