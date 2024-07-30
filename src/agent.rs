use crate::input::*;
use crate::region::*;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

const AGENT_FILE_NAME: &str = "agents.csv";
const AGENT_REGIONS_FILE_NAME: &str = "agent_regions.csv";
const AGENT_OBJECTIVES_FILE_NAME: &str = "agent_objectives.csv";

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq, DeserializeLabeledStringEnum)]
pub enum DecisionRule {
    #[string = "single"]
    Single,
    #[string = "weighted"]
    Weighted,
    #[string = "lexico"]
    Lexicographical,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Agent {
    pub id: Rc<str>,
    pub description: String,
    pub commodity_id: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    pub commodity_portion: f64,
    pub search_space: SearchSpace,
    pub decision_rule: DecisionRule,
    pub capex_limit: Option<f64>,
    pub annual_cost_limit: Option<f64>,

    #[serde(skip)]
    pub regions: RegionSelection,
    #[serde(skip)]
    pub objectives: Vec<AgentObjective>,
}

macro_rules! define_agent_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.agent_id
            }
        }
    };
}

#[derive(Debug, Deserialize, PartialEq)]
struct AgentRegion {
    agent_id: String,
    region_id: String,
}
define_agent_id_getter! {AgentRegion}
define_region_id_getter! {AgentRegion}

/// **TODO** Add more objective types
#[derive(Debug, PartialEq, DeserializeLabeledStringEnum)]
pub enum ObjectiveType {
    #[string = "lcox"]
    LevellisedCostOfX,
    #[string = "eac"]
    EquivalentAnnualCost,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AgentObjective {
    agent_id: String,
    objective_type: ObjectiveType,
    decision_weight: Option<f64>,
    decision_lexico_tolerance: Option<f64>,
}
define_agent_id_getter! {AgentObjective}

/// Check that required parameters are present and others are absent
fn check_objective_parameter(objective: &AgentObjective, agent: &Agent) -> Result<(), String> {
    // Check that the user hasn't supplied a value for a field we're not using
    macro_rules! check_field_none {
        ($field:ident) => {
            if objective.$field.is_some() {
                Err(format!(
                    "Field {} should be empty for this decision rule",
                    stringify!($field)
                ))?;
            }
        };
    }

    // Check that required fields are present
    macro_rules! check_field_some {
        ($field:ident) => {
            if objective.$field.is_none() {
                Err(format!("Required field {} is empty", stringify!($field)))?;
            }
        };
    }

    match &agent.decision_rule {
        DecisionRule::Single => {
            check_field_none!(decision_weight);
            check_field_none!(decision_lexico_tolerance);
        }
        DecisionRule::Weighted => {
            check_field_none!(decision_lexico_tolerance);
            check_field_some!(decision_weight);
        }
        DecisionRule::Lexicographical => {
            check_field_none!(decision_weight);
            check_field_some!(decision_lexico_tolerance);
        }
    };

    Ok(())
}

fn read_agent_objectives_from_iter<I>(
    iter: I,
    agents: &HashMap<Rc<str>, Agent>,
    agent_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Vec<AgentObjective>>, Box<dyn Error>>
where
    I: Iterator<Item = AgentObjective>,
{
    let objectives = iter.into_id_map(agent_ids)?;
    for objective in objectives.values().flatten() {
        // We've already checked that agent IDs are valid
        let agent = agents.get(objective.agent_id.as_str()).unwrap();

        // Check that required parameters are present and others are absent
        check_objective_parameter(objective, agent)?;
    }

    if objectives.len() < agent_ids.len() {
        Err("All agents must have at least one objective")?;
    }

    Ok(objectives)
}

fn read_agent_objectives(
    model_dir: &Path,
    agents: &HashMap<Rc<str>, Agent>,
    agent_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Vec<AgentObjective>> {
    let file_path = model_dir.join(AGENT_OBJECTIVES_FILE_NAME);
    read_agent_objectives_from_iter(read_csv(&file_path), agents, agent_ids)
        .unwrap_input_err(&file_path)
}

/// Read agents info from a CSV file.
pub fn read_agents_file(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Agent> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    let mut agents = HashMap::new();
    for agent in read_csv::<Agent>(&file_path) {
        if let SearchSpace::Some(ref search_space) = agent.search_space {
            // Check process IDs are all valid
            if !search_space
                .iter()
                .all(|id| process_ids.contains(id.as_str()))
            {
                input_panic(&file_path, "Invalid process ID");
            }
        }

        if agents.insert(Rc::clone(&agent.id), agent).is_some() {
            input_panic(&file_path, "Duplicate agent ID");
        }
    }

    agents
}

/// Read agents info from CSV files.
pub fn read_agents(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Agent> {
    let mut agents = read_agents_file(model_dir, process_ids);
    let agent_ids = agents.keys().cloned().collect();

    let file_path = model_dir.join(AGENT_REGIONS_FILE_NAME);
    let mut agent_regions =
        read_regions_for_entity::<AgentRegion>(&file_path, &agent_ids, region_ids);
    let mut objectives = read_agent_objectives(model_dir, &agents, &agent_ids);

    for (id, agent) in agents.iter_mut() {
        agent.regions = agent_regions.remove(id).unwrap();
        agent.objectives = objectives.remove(id).unwrap();
    }

    agents
}
