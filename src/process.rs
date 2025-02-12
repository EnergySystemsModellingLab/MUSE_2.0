#![allow(missing_docs)]
use crate::commodity::Commodity;
use crate::region::RegionSelection;
use crate::time_slice::TimeSliceID;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::rc::Rc;

#[derive(PartialEq, Debug)]
pub struct Process {
    pub id: Rc<str>,
    pub description: String,
    pub capacity_fractions: ProcessCapacityMap,
    pub flows: Vec<ProcessFlow>,
    pub parameter: ProcessParameter,
    pub regions: RegionSelection,
}

impl Process {
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

#[derive(PartialEq, Debug, Deserialize, Clone)]
pub struct ProcessFlow {
    /// A unique identifier for the process
    pub process_id: String,
    /// Identifies the commodity for the specified flow
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

#[derive(PartialEq, Clone, Debug, Deserialize)]
pub struct ProcessParameter {
    pub process_id: String,
    pub years: RangeInclusive<u32>,
    pub capital_cost: f64,
    pub fixed_operating_cost: f64,
    pub variable_operating_cost: f64,
    pub lifetime: u32,
    pub discount_rate: f64,
    pub cap2act: f64,
}
