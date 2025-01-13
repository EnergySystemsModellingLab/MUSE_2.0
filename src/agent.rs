#![allow(missing_docs)]
use crate::commodity::Commodity;
use crate::process::Process;
use crate::region::RegionSelection;
use anyhow::Result;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashSet;
use std::rc::Rc;

/// An agent in the simulation
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    /// A unique identifier for the agent.
    pub id: Rc<str>,
    /// A text description of the agent.
    pub description: String,
    /// The commodity that the agent produces (could be a service demand too).
    pub commodity: Rc<Commodity>,
    /// The proportion of the commodity production that the agent is responsible for.
    pub commodity_portion: f64,
    /// The list of processes that the agent will consider investing in.
    pub search_space: SearchSpace,
    /// The decision rule that the agent uses to decide investment.
    pub decision_rule: DecisionRule,
    /// The maximum capital cost the agent will pay.
    pub capex_limit: Option<f64>,
    /// The maximum annual operating cost (fuel plus var_opex etc) that the agent will pay.
    pub annual_cost_limit: Option<f64>,
    /// The regions in which this agent operates.
    pub regions: RegionSelection,
    /// The agent's objectives.
    pub objectives: Vec<AgentObjective>,
    /// Assets controlled by this agent.
    pub assets: Vec<Asset>,
}

/// Which processes apply to this agent
#[derive(Debug, Clone, PartialEq)]
pub enum SearchSpace {
    AllProcesses,
    Some(HashSet<String>),
}

impl<'de> Deserialize<'de> for SearchSpace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Option::<&str>::deserialize(deserializer)?;
        match value {
            None => Ok(SearchSpace::AllProcesses),
            Some(processes_str) => {
                let processes = HashSet::from_iter(processes_str.split(';').map(String::from));
                Ok(SearchSpace::Some(processes))
            }
        }
    }
}

/// The decision rule for a particular objective
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum DecisionRule {
    #[string = "single"]
    Single,
    #[string = "weighted"]
    Weighted,
    #[string = "lexico"]
    Lexicographical,
}

/// An objective for an agent with associated parameters
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentObjective {
    /// Unique agent id identifying the agent this objective belongs to
    pub agent_id: String,
    /// Acronym identifying the objective (e.g. LCOX)
    pub objective_type: ObjectiveType,
    /// For the weighted sum objective, the set of weights to apply to each objective.
    pub decision_weight: Option<f64>,
    /// The tolerance around the main objective to consider secondary objectives. This is an absolute value of maximum deviation in the units of the main objective.
    pub decision_lexico_tolerance: Option<f64>,
}

/// The type of objective for the agent
///
/// **TODO** Add more objective types
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum ObjectiveType {
    #[string = "lcox"]
    LevelisedCostOfX,
    #[string = "eac"]
    EquivalentAnnualCost,
}

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// The [Process] that this asset corresponds to
    pub process: Rc<Process>,
    /// The region in which the asset is located
    pub region_id: Rc<str>,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
}
