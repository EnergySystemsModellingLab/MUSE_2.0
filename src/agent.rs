//! Agents drive the economy of the MUSE 2.0 simulation, through relative investment in different
//! assets.
use crate::commodity::Commodity;
use crate::process::Process;
use crate::region::RegionSelection;
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
    /// The processes that the agent will consider investing in.
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
}

/// Which processes apply to this agent
#[derive(Debug, Clone, PartialEq)]
pub enum SearchSpace {
    /// All processes are considered
    AllProcesses,
    /// Only these specific processes are considered
    Some(HashSet<Rc<str>>),
}

/// The decision rule for a particular objective
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum DecisionRule {
    /// Used when there is only a single objective
    #[string = "single"]
    Single,
    /// A simple weighting of objectives
    #[string = "weighted"]
    Weighted,
    /// Objectives are considered in a specific order
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
    /// Average cost of one unit of output commodity over its lifetime
    #[string = "lcox"]
    LevelisedCostOfX,
    /// Cost of serving agent's demand for a year, considering the asset's entire lifetime
    #[string = "eac"]
    EquivalentAnnualCost,
}

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// A unique identifier for the agent
    pub agent_id: Rc<str>,
    /// The [`Process`] that this asset corresponds to
    pub process: Rc<Process>,
    /// The region in which the asset is located
    pub region_id: Rc<str>,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
}

/// A pool of [`Asset`]s
pub type AssetPool = Vec<Asset>;
