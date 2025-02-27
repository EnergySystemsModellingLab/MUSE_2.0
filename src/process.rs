//! Processes are used for converting between different commodities. The data structures in this
//! module are used to represent these conversions along with the associated costs.
use crate::commodity::Commodity;
use crate::region::RegionSelection;
use crate::time_slice::TimeSliceID;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::rc::Rc;

/// A map of [`Process`]es, keyed by process ID
pub type ProcessMap = IndexMap<Rc<str>, Rc<Process>>;

/// Represents a process within the simulation
#[derive(PartialEq, Debug)]
pub struct Process {
    /// A unique identifier for the process (e.g. GASDRV)
    pub id: Rc<str>,
    /// A human-readable description for the process (e.g. dry gas extraction)
    pub description: String,
    /// The capacity limits for each time slice (as a fraction of maximum)
    pub capacity_fractions: ProcessCapacityMap,
    /// Commodity flows for this process
    pub flows: Vec<ProcessFlow>,
    /// Additional parameters for this process
    pub parameter: ProcessParameter,
    /// The regions in which this process can operate
    pub regions: RegionSelection,
}

impl Process {
    /// Whether the process contains a flow for a given commodity
    pub fn contains_commodity_flow(&self, commodity: &Rc<Commodity>) -> bool {
        self.flows
            .iter()
            .any(|flow| Rc::ptr_eq(&flow.commodity, commodity))
    }

    /// Iterate over this process's Primary Activity Commodity flows
    pub fn iter_pacs(&self) -> impl Iterator<Item = &ProcessFlow> {
        self.flows.iter().filter(|flow| flow.is_pac)
    }
}

/// A map indicating capacity limits for a [`Process`] throughout the year.
///
/// The capacity value is calculated as availability multiplied by time slice length. Note that it
/// is a *fraction* of capacity for the year; to calculate *actual* capacity for a given time slice
/// you need to know the maximum capacity for the specific instance of a [`Process`] in use.
///
/// The capacity is given as a range, depending on the user-specified limit type and value for
/// availability.
pub type ProcessCapacityMap = HashMap<TimeSliceID, RangeInclusive<f64>>;

/// Represents a commodity flow for a given process
#[derive(PartialEq, Debug, Deserialize, Clone)]
pub struct ProcessFlow {
    /// A unique identifier for the process
    pub process_id: String,
    /// The commodity produced or consumed by this flow
    pub commodity: Rc<Commodity>,
    /// Commodity flow quantity relative to other commodity flows.
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
    #[default]
    #[string = "fixed"]
    /// The input to output flow ratio is fixed
    Fixed,
    #[string = "flexible"]
    /// The flow ratio can vary, subject to overall flow of a specified group of commodities whose
    /// input/output ratio must be as per user input data
    Flexible,
}

/// Additional parameters for a process
#[derive(PartialEq, Clone, Debug, Deserialize)]
pub struct ProcessParameter {
    /// A unique identifier for the process
    pub process_id: String,
    /// The years in which this process is available for investment
    pub years: RangeInclusive<u32>,
    /// Overnight capital cost per unit capacity
    pub capital_cost: f64,
    /// Annual operating cost per unit capacity
    pub fixed_operating_cost: f64,
    /// Variable operating cost per unit activity, for PACs **only**
    pub variable_operating_cost: f64,
    /// Lifetime in years of an asset created from this process
    pub lifetime: u32,
    /// Process-specific discount rate
    pub discount_rate: f64,
    /// Factor for calculating the maximum PAC output over a year ("capacity to activity").
    ///
    /// Used for converting one unit of capacity to maximum activity of the PAC per year. For
    /// example, if capacity is measured in GW and activity is measured in PJ, the cap2act for the
    /// process is 31.536 because 1 GW of capacity can produce 31.536 PJ energy output in a year.
    pub cap2act: f64,
}
