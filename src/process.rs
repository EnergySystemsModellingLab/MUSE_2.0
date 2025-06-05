//! Processes are used for converting between different commodities. The data structures in this
//! module are used to represent these conversions along with the associated costs.
use crate::commodity::{Commodity, CommodityID};
use crate::id::define_id_type;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use indexmap::IndexMap;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::rc::Rc;

define_id_type! {ProcessID}

/// A map of [`Process`]es, keyed by process ID
pub type ProcessMap = IndexMap<ProcessID, Rc<Process>>;

/// A map indicating relative PAC energy limits for a [`Process`] throughout the year.
///
/// The value is calculated as availability multiplied by time slice length. Note that it is a
/// **fraction** of energy for the year; to calculate **actual** energy limits for a given time
/// slice you need to know the maximum activity (energy per year) for the specific instance of a
/// [`Process`] in use.
///
/// The limits are given as ranges, depending on the user-specified limit type and value for
/// availability.
pub type ProcessEnergyLimitsMap = HashMap<(RegionID, u32, TimeSliceID), RangeInclusive<f64>>;

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
    /// Limits on PAC energy consumption/production for each time slice (as a fraction of maximum)
    pub energy_limits: ProcessEnergyLimitsMap,
    /// Maximum annual commodity flows for this process
    pub flows: ProcessFlowsMap,
    /// Additional parameters for this process
    pub parameters: ProcessParameterMap,
    /// The regions in which this process can operate
    pub regions: HashSet<RegionID>,
}

impl Process {
    /// Whether the process contains a flow for a given commodity
    pub fn contains_commodity_flow(
        &self,
        commodity_id: &CommodityID,
        region_id: &RegionID,
        year: u32,
    ) -> bool {
        self.flows
            .get(&(region_id.clone(), year))
            .unwrap() // all regions and years are covered
            .contains_key(commodity_id)
    }

    /// Iterate over this process's Primary Activity Commodity flows
    pub fn iter_pacs(&self, region_id: &RegionID, year: u32) -> impl Iterator<Item = &ProcessFlow> {
        self.flows
            .get(&(region_id.clone(), year))
            .unwrap()
            .values()
            .take_while(|flow| flow.is_pac)
    }
}

/// Represents a maximum annual commodity flow for a given process
#[derive(PartialEq, Debug, Clone)]
pub struct ProcessFlow {
    /// The commodity produced or consumed by this flow
    pub commodity: Rc<Commodity>,
    /// Maximum annual commodity flow quantity relative to other commodity flows.
    ///
    /// Positive value indicates flow out and negative value indicates flow in.
    pub flow: f64,
    /// Identifies if a flow is fixed or flexible.
    pub flow_type: FlowType,
    /// Cost per unit flow.
    ///
    /// For example, cost per unit of natural gas produced. The user can apply it to any specified
    /// flow, in contrast to [`ProcessParameter::variable_operating_cost`], which applies only to
    /// PAC flows.
    pub flow_cost: f64,
    /// Whether this flow represents a Primary Activity Commodity
    pub is_pac: bool,
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
    pub capital_cost: f64,
    /// Annual operating cost per unit capacity
    pub fixed_operating_cost: f64,
    /// Annual variable operating cost per unit activity, for PACs **only**
    pub variable_operating_cost: f64,
    /// Lifetime in years of an asset created from this process
    pub lifetime: u32,
    /// Process-specific discount rate
    pub discount_rate: f64,
    /// Factor for calculating the maximum PAC consumption/production over a year.
    ///
    /// Used for converting one unit of capacity to maximum energy of the PAC(s) per year. For
    /// example, if capacity is measured in GW and energy is measured in PJ, the
    /// capacity_to_activity for the process is 31.536 because 1 GW of capacity can produce 31.536
    /// PJ energy output in a year.
    pub capacity_to_activity: f64,
}
