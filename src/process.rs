#![allow(missing_docs)]
use crate::commodity::Commodity;
use crate::region::RegionSelection;
use crate::time_slice::TimeSliceSelection;
use anyhow::Result;
use serde::{Deserialize, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::ops::RangeInclusive;
use std::rc::Rc;

#[derive(PartialEq, Debug)]
pub struct Process {
    pub id: Rc<str>,
    pub description: String,
    pub availabilities: Vec<ProcessAvailability>,
    pub flows: Vec<ProcessFlow>,
    pub pacs: Vec<Rc<Commodity>>,
    pub parameter: ProcessParameter,
    pub regions: RegionSelection,
}

#[derive(PartialEq, Debug, SerializeLabeledStringEnum, DeserializeLabeledStringEnum)]
pub enum LimitType {
    #[string = "lo"]
    LowerBound,
    #[string = "up"]
    UpperBound,
    #[string = "fx"]
    Equality,
}

/// The availabilities for a process over time slices
#[derive(PartialEq, Debug)]
pub struct ProcessAvailability {
    /// Unique identifier for the process (typically uses a structured naming convention).
    pub process_id: String,
    /// The limit type – lower bound, upper bound or equality.
    pub limit_type: LimitType,
    /// The time slice to which the availability applies.
    pub time_slice: TimeSliceSelection,
    /// The availability value, between 0 and 1 inclusive.
    pub value: f64,
}

#[derive(PartialEq, Debug, Deserialize, Clone)]
pub struct ProcessFlow {
    /// A unique identifier for the process (typically uses a structured naming convention).
    pub process_id: String,
    /// Identifies the commodity for the specified flow
    pub commodity_id: String,
    /// Commodity flow quantity relative to other commodity flows. +ve value indicates flow out, -ve value indicates flow in.
    pub flow: f64,
    #[serde(default)]
    /// Identifies if a flow is fixed or flexible.
    pub flow_type: FlowType,
    #[serde(deserialize_with = "deserialise_flow_cost")]
    /// Cost per unit flow. For example, cost per unit of natural gas produced. Differs from var_opex because the user can apply it to any specified flow, whereas var_opex applies to pac flow.
    pub flow_cost: f64,
}

#[derive(
    PartialEq, Default, Debug, Clone, SerializeLabeledStringEnum, DeserializeLabeledStringEnum,
)]
pub enum FlowType {
    #[default]
    #[string = "fixed"]
    /// The input to output flow ratio is fixed.
    Fixed,
    #[string = "flexible"]
    /// The flow ratio can vary, subject to overall flow of a specified group of commodities whose input/output ratio must be as per user input data.
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

/// Custom deserialiser for flow cost - treat empty fields as 0.0
fn deserialise_flow_cost<'de, D>(deserialiser: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<f64> = Deserialize::deserialize(deserialiser)?;
    match value {
        None => Ok(0.0),
        Some(value) => Ok(value),
    }
}
