//! Agents drive the economy of the MUSE 2.0 simulation, through relative investment in different
//! assets.
use crate::commodity::{Commodity, CommodityID};
use crate::id::{define_id_getter, define_id_type};
use crate::process::ProcessID;
use crate::region::RegionID;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

define_id_type! {AgentID}

/// A map of [`Agent`]s, keyed by agent ID
pub type AgentMap = IndexMap<AgentID, Agent>;

/// A map of cost limits for an agent, keyed by year
pub type AgentCostLimitsMap = HashMap<u32, AgentCostLimits>;

/// A map of commodity portions for an agent, keyed by commodity and year
pub type AgentCommodityPortionsMap = HashMap<(CommodityID, u32), f64>;

/// An agent in the simulation
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    /// A unique identifier for the agent.
    pub id: AgentID,
    /// A text description of the agent.
    pub description: String,
    /// The proportion of the commodity production that the agent is responsible for.
    pub commodity_portions: AgentCommodityPortionsMap,
    /// The processes that the agent will consider investing in.
    pub search_space: Vec<AgentSearchSpace>,
    /// The decision rule that the agent uses to decide investment.
    pub decision_rule: DecisionRule,
    /// Cost limits (e.g. capital cost, annual operating cost)
    pub cost_limits: AgentCostLimitsMap,
    /// The regions in which this agent operates.
    pub regions: HashSet<RegionID>,
    /// The agent's objectives.
    pub objectives: Vec<AgentObjective>,
}
define_id_getter! {Agent, AgentID}

/// The cost limits for an agent in a particular year
#[derive(Debug, Clone, PartialEq)]
pub struct AgentCostLimits {
    /// The maximum capital cost the agent will pay.
    pub capex_limit: Option<f64>,
    /// The maximum annual operating cost (fuel plus var_opex etc) that the agent will pay.
    pub annual_cost_limit: Option<f64>,
}

/// Search space for an agent
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSearchSpace {
    /// The year the objective is relevant for
    pub year: u32,
    /// The commodity to apply the search space to
    pub commodity: Rc<Commodity>,
    /// The agent's search space
    pub search_space: Vec<ProcessID>,
}

/// The decision rule for a particular objective
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionRule {
    /// Used when there is only a single objective
    Single,
    /// A simple weighting of objectives
    Weighted,
    /// Objectives are considered in a specific order
    Lexicographical {
        /// The tolerance around the main objective to consider secondary objectives. This is an absolute value of maximum deviation in the units of the main objective.
        tolerance: f64,
    },
}

/// An objective for an agent with associated parameters
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentObjective {
    /// Unique agent id identifying the agent this objective belongs to
    pub agent_id: AgentID,
    /// The year the objective is relevant for
    pub year: u32,
    /// Acronym identifying the objective (e.g. LCOX)
    pub objective_type: ObjectiveType,
    /// For the weighted sum decision rule, the set of weights to apply to each objective.
    pub decision_weight: Option<f64>,
    /// For the lexico decision rule, the order in which to consider objectives.
    pub decision_lexico_order: Option<u32>,
}

/// The type of objective for the agent
#[derive(Debug, Clone, Copy, PartialEq, DeserializeLabeledStringEnum)]
pub enum ObjectiveType {
    /// Average cost of one unit of output commodity over its lifetime
    #[string = "lcox"]
    LevelisedCostOfX,
}
