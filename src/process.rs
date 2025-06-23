//! Processes are used for converting between different commodities. The data structures in this
//! module are used to represent these conversions along with the associated costs.
use crate::commodity::{BalanceType, Commodity, CommodityID};
use crate::id::define_id_type;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::{
    Dimensionless, EnergyPerActivity, MoneyPerActivity, MoneyPerCapacity, MoneyPerCapacityPerYear,
    MoneyPerEnergy, PerCapacity,
};
use indexmap::IndexMap;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::rc::Rc;

define_id_type! {ProcessID}

/// A map of [`Process`]es, keyed by process ID
pub type ProcessMap = IndexMap<ProcessID, Rc<Process>>;

/// A map indicating activity limits for a [`Process`] throughout the year.
///
/// The value is calculated as availability multiplied by time slice length. The limits are given as
/// ranges, depending on the user-specified limit type and value for availability.
pub type ProcessActivityLimitsMap =
    HashMap<(RegionID, u32, TimeSliceID), RangeInclusive<Dimensionless>>;

/// A map of [`ProcessParameter`]s, keyed by region and year
pub type ProcessParameterMap = HashMap<(RegionID, u32), Rc<ProcessParameter>>;

/// A map of process flows, keyed by region and year.
///
/// The value is actually a map itself, keyed by commodity ID.
pub type ProcessFlowsMap = HashMap<(RegionID, u32), IndexMap<CommodityID, ProcessFlow>>;

/// Represents a process within the simulation
#[derive(PartialEq, Debug)]
pub struct Process {
    /// A unique identifier for the process (e.g. GASDRV)
    pub id: ProcessID,
    /// A human-readable description for the process (e.g. dry gas extraction)
    pub description: String,
    /// The years in which this process is available for investment
    pub years: Vec<u32>,
    /// Limits on activity for each time slice (as a fraction of maximum)
    pub activity_limits: ProcessActivityLimitsMap,
    /// Maximum annual commodity flows for this process
    pub flows: ProcessFlowsMap,
    /// Additional parameters for this process
    pub parameters: ProcessParameterMap,
    /// The regions in which this process can operate
    pub regions: HashSet<RegionID>,
}

/// Represents a maximum annual commodity coeff for a given process
#[derive(PartialEq, Debug, Clone)]
pub struct ProcessFlow {
    /// The commodity produced or consumed by this flow
    pub commodity: Rc<Commodity>,
    /// Maximum annual commodity flow quantity relative to other commodity flows.
    ///
    /// Positive value indicates flow out and negative value indicates flow in.
    pub coeff: EnergyPerActivity,
    /// Identifies if a flow is fixed or flexible.
    pub kind: FlowType,
    /// Cost per unit flow.
    ///
    /// For example, cost per unit of natural gas produced. The user can apply it to any specified
    /// flow.
    pub cost: MoneyPerEnergy,
}

impl ProcessFlow {
    /// Get the cost for this flow with the given parameters.
    ///
    /// This includes cost per unit flow and levies/incentives, if any.
    pub fn get_total_cost(
        &self,
        region_id: &RegionID,
        year: u32,
        time_slice: &TimeSliceID,
    ) -> MoneyPerActivity {
        let cost_per_unit = self.cost + self.get_levy(region_id, year, time_slice);

        self.coeff.abs() * cost_per_unit
    }

    /// Get the levy/incentive for this process flow with the given parameters, if any
    fn get_levy(
        &self,
        region_id: &RegionID,
        year: u32,
        time_slice: &TimeSliceID,
    ) -> MoneyPerEnergy {
        if self.commodity.levies.is_empty() {
            return MoneyPerEnergy(0.0);
        }

        let levy = self
            .commodity
            .levies
            .get(&(region_id.clone(), year, time_slice.clone()))
            .unwrap();
        let apply_levy = match levy.balance_type {
            BalanceType::Net => true,
            BalanceType::Consumption => self.coeff < EnergyPerActivity(0.0),
            BalanceType::Production => self.coeff > EnergyPerActivity(0.0),
        };

        if apply_levy {
            levy.value
        } else {
            MoneyPerEnergy(0.0)
        }
    }
}

/// Type of commodity flow (see [`ProcessFlow`])
#[derive(PartialEq, Default, Debug, Clone, DeserializeLabeledStringEnum)]
pub enum FlowType {
    /// The input to output flow ratio is fixed
    #[default]
    #[string = "fixed"]
    Fixed,
    /// The flow ratio can vary, subject to overall flow of a specified group of commodities whose
    /// input/output ratio must be as per user input data
    #[string = "flexible"]
    Flexible,
}

/// Additional parameters for a process
#[derive(PartialEq, Clone, Debug)]
pub struct ProcessParameter {
    /// Overnight capital cost per unit capacity
    pub capital_cost: MoneyPerCapacity,
    /// Annual operating cost per unit capacity
    pub fixed_operating_cost: MoneyPerCapacityPerYear,
    /// Annual variable operating cost per unit activity
    pub variable_operating_cost: MoneyPerActivity,
    /// Lifetime in years of an asset created from this process
    pub lifetime: u32,
    /// Process-specific discount rate
    pub discount_rate: Dimensionless,
    /// Factor for calculating the maximum consumption/production over a year.
    ///
    /// Used for converting one unit of capacity to maximum energy of asset per year. For example,
    /// if capacity is measured in GW and energy is measured in PJ, the capacity_to_activity for the
    /// process is 31.536 because 1 GW of capacity can produce 31.536 PJ energy output in a year.
    pub capacity_to_activity: PerCapacity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{
        BalanceType, CommodityLevy, CommodityLevyMap, CommodityType, DemandMap,
    };
    use crate::fixture::{region_id, time_slice};
    use crate::time_slice::TimeSliceLevel;
    use rstest::{fixture, rstest};
    use std::rc::Rc;

    #[fixture]
    fn commodity_with_levy(region_id: RegionID, time_slice: TimeSliceID) -> Rc<Commodity> {
        let mut levies = CommodityLevyMap::new();
        // Add levy for the default region and time slice
        levies.insert(
            (region_id.clone(), 2020, time_slice.clone()),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(10.0),
            },
        );
        // Add levy for a different region
        levies.insert(
            ("USA".into(), 2020, time_slice.clone()),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(5.0),
            },
        );
        // Add levy for a different year
        levies.insert(
            (region_id.clone(), 2030, time_slice.clone()),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(7.0),
            },
        );
        // Add levy for a different time slice
        levies.insert(
            (
                region_id.clone(),
                2020,
                TimeSliceID {
                    season: "summer".into(),
                    time_of_day: "day".into(),
                },
            ),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(3.0),
            },
        );

        Rc::new(Commodity {
            id: "test_commodity".into(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies,
            demand: DemandMap::new(),
        })
    }

    #[fixture]
    fn commodity_with_consumption_levy(
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) -> Rc<Commodity> {
        let mut levies = CommodityLevyMap::new();
        levies.insert(
            (region_id, 2020, time_slice),
            CommodityLevy {
                balance_type: BalanceType::Consumption,
                value: MoneyPerEnergy(10.0),
            },
        );

        Rc::new(Commodity {
            id: "test_commodity".into(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies,
            demand: DemandMap::new(),
        })
    }

    #[fixture]
    fn commodity_with_production_levy(
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) -> Rc<Commodity> {
        let mut levies = CommodityLevyMap::new();
        levies.insert(
            (region_id, 2020, time_slice),
            CommodityLevy {
                balance_type: BalanceType::Production,
                value: MoneyPerEnergy(10.0),
            },
        );

        Rc::new(Commodity {
            id: "test_commodity".into(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies,
            demand: DemandMap::new(),
        })
    }

    #[fixture]
    fn commodity_with_incentive(region_id: RegionID, time_slice: TimeSliceID) -> Rc<Commodity> {
        let mut levies = CommodityLevyMap::new();
        levies.insert(
            (region_id, 2020, time_slice),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(-5.0),
            },
        );

        Rc::new(Commodity {
            id: "test_commodity".into(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies,
            demand: DemandMap::new(),
        })
    }

    #[fixture]
    fn commodity_no_levies() -> Rc<Commodity> {
        Rc::new(Commodity {
            id: "test_commodity".into(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::new(),
            demand: DemandMap::new(),
        })
    }

    #[fixture]
    fn flow_with_cost() -> ProcessFlow {
        ProcessFlow {
            commodity: Rc::new(Commodity {
                id: "test_commodity".into(),
                description: "Test commodity".into(),
                kind: CommodityType::ServiceDemand,
                time_slice_level: TimeSliceLevel::Annual,
                levies: CommodityLevyMap::new(),
                demand: DemandMap::new(),
            }),
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(5.0),
        }
    }

    #[fixture]
    fn flow_with_cost_and_levy(region_id: RegionID, time_slice: TimeSliceID) -> ProcessFlow {
        let mut levies = CommodityLevyMap::new();
        levies.insert(
            (region_id, 2020, time_slice),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(10.0),
            },
        );

        ProcessFlow {
            commodity: Rc::new(Commodity {
                id: "test_commodity".into(),
                description: "Test commodity".into(),
                kind: CommodityType::ServiceDemand,
                time_slice_level: TimeSliceLevel::Annual,
                levies,
                demand: DemandMap::new(),
            }),
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(5.0),
        }
    }

    #[fixture]
    fn flow_with_cost_and_incentive(region_id: RegionID, time_slice: TimeSliceID) -> ProcessFlow {
        let mut levies = CommodityLevyMap::new();
        levies.insert(
            (region_id, 2020, time_slice),
            CommodityLevy {
                balance_type: BalanceType::Net,
                value: MoneyPerEnergy(-3.0),
            },
        );

        ProcessFlow {
            commodity: Rc::new(Commodity {
                id: "test_commodity".into(),
                description: "Test commodity".into(),
                kind: CommodityType::ServiceDemand,
                time_slice_level: TimeSliceLevel::Annual,
                levies,
                demand: DemandMap::new(),
            }),
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(5.0),
        }
    }

    #[rstest]
    fn test_get_levy_no_levies(
        commodity_no_levies: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_no_levies,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(0.0)
        );
    }

    #[rstest]
    fn test_get_levy_with_levy(
        commodity_with_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_levy,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(10.0)
        );
    }

    #[rstest]
    fn test_get_levy_with_incentive(
        commodity_with_incentive: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_incentive,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(-5.0)
        );
    }

    #[rstest]
    fn test_get_levy_different_region(commodity_with_levy: Rc<Commodity>, time_slice: TimeSliceID) {
        let flow = ProcessFlow {
            commodity: commodity_with_levy,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&"USA".into(), 2020, &time_slice),
            MoneyPerEnergy(5.0)
        );
    }

    #[rstest]
    fn test_get_levy_different_year(
        commodity_with_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_levy,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2030, &time_slice),
            MoneyPerEnergy(7.0)
        );
    }

    #[rstest]
    fn test_get_levy_different_time_slice(commodity_with_levy: Rc<Commodity>, region_id: RegionID) {
        let flow = ProcessFlow {
            commodity: commodity_with_levy,
            coeff: EnergyPerActivity(1.0),
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        let different_time_slice = TimeSliceID {
            season: "summer".into(),
            time_of_day: "day".into(),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &different_time_slice),
            MoneyPerEnergy(3.0)
        );
    }

    #[rstest]
    fn test_get_levy_consumption_positive_coeff(
        commodity_with_consumption_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_consumption_levy,
            coeff: EnergyPerActivity(1.0), // Positive coefficient means production
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(0.0)
        );
    }

    #[rstest]
    fn test_get_levy_consumption_negative_coeff(
        commodity_with_consumption_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_consumption_levy,
            coeff: EnergyPerActivity(-1.0), // Negative coefficient means consumption
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(10.0)
        );
    }

    #[rstest]
    fn test_get_levy_production_positive_coeff(
        commodity_with_production_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_production_levy,
            coeff: EnergyPerActivity(1.0), // Positive coefficient means production
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(10.0)
        );
    }

    #[rstest]
    fn test_get_levy_production_negative_coeff(
        commodity_with_production_levy: Rc<Commodity>,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            commodity: commodity_with_production_levy,
            coeff: EnergyPerActivity(-1.0), // Negative coefficient means consumption
            kind: FlowType::Fixed,
            cost: MoneyPerEnergy(0.0),
        };

        assert_eq!(
            flow.get_levy(&region_id, 2020, &time_slice),
            MoneyPerEnergy(0.0)
        );
    }

    #[rstest]
    fn test_get_total_cost_base_cost(
        flow_with_cost: ProcessFlow,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        assert_eq!(
            flow_with_cost.get_total_cost(&region_id, 2020, &time_slice),
            MoneyPerActivity(5.0)
        );
    }

    #[rstest]
    fn test_get_total_cost_with_levy(
        flow_with_cost_and_levy: ProcessFlow,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        assert_eq!(
            flow_with_cost_and_levy.get_total_cost(&region_id, 2020, &time_slice),
            MoneyPerActivity(15.0)
        );
    }

    #[rstest]
    fn test_get_total_cost_with_incentive(
        flow_with_cost_and_incentive: ProcessFlow,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        assert_eq!(
            flow_with_cost_and_incentive.get_total_cost(&region_id, 2020, &time_slice),
            MoneyPerActivity(2.0)
        );
    }

    #[rstest]
    fn test_get_total_cost_negative_coeff(
        flow_with_cost: ProcessFlow,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            coeff: EnergyPerActivity(-2.0),
            ..flow_with_cost
        };
        assert_eq!(
            flow.get_total_cost(&region_id, 2020, &time_slice),
            MoneyPerActivity(10.0)
        );
    }

    #[rstest]
    fn test_get_total_cost_zero_coeff(
        flow_with_cost: ProcessFlow,
        region_id: RegionID,
        time_slice: TimeSliceID,
    ) {
        let flow = ProcessFlow {
            coeff: EnergyPerActivity(0.0),
            ..flow_with_cost
        };
        assert_eq!(
            flow.get_total_cost(&region_id, 2020, &time_slice),
            MoneyPerActivity(0.0)
        );
    }
}
