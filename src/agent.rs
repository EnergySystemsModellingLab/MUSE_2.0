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

/// An agent in the simulation
#[derive(Debug, Deserialize, PartialEq, Clone)]
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
define_id_getter! {Agent}

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

/// The type of objective for the agent
///
/// **TODO** Add more objective types
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum ObjectiveType {
    #[string = "lcox"]
    LevellisedCostOfX,
    #[string = "eac"]
    EquivalentAnnualCost,
}

/// An objective for an agent with associated parameters
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentObjective {
    agent_id: String,
    objective_type: ObjectiveType,
    decision_weight: Option<f64>,
    decision_lexico_tolerance: Option<f64>,
}
define_agent_id_getter! {AgentObjective}

/// Check that required parameters are present and others are absent
fn check_objective_parameter(
    objective: &AgentObjective,
    decision_rule: &DecisionRule,
) -> Result<(), String> {
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

    match decision_rule {
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
) -> Result<HashMap<Rc<str>, Vec<AgentObjective>>, Box<dyn Error>>
where
    I: Iterator<Item = AgentObjective>,
{
    let mut objectives = HashMap::new();
    for objective in iter {
        let (id, agent) = agents
            .get_key_value(objective.agent_id.as_str())
            .ok_or("Invalid agent ID")?;

        // Check that required parameters are present and others are absent
        check_objective_parameter(&objective, &agent.decision_rule)?;

        // Append to Vec with the corresponding key or create
        objectives
            .entry(Rc::clone(id))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(objective);
    }

    if objectives.len() < agents.len() {
        Err("All agents must have at least one objective")?;
    }

    Ok(objectives)
}

/// Read agent objective info from the agent_objectives.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
fn read_agent_objectives(
    model_dir: &Path,
    agents: &HashMap<Rc<str>, Agent>,
) -> HashMap<Rc<str>, Vec<AgentObjective>> {
    let file_path = model_dir.join(AGENT_OBJECTIVES_FILE_NAME);
    read_agent_objectives_from_iter(read_csv(&file_path), agents).unwrap_input_err(&file_path)
}

pub fn read_agents_file_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Agent>, &'static str>
where
    I: Iterator<Item = Agent>,
{
    let mut agents = HashMap::new();
    for agent in iter {
        if let SearchSpace::Some(ref search_space) = agent.search_space {
            // Check process IDs are all valid
            if !search_space
                .iter()
                .all(|id| process_ids.contains(id.as_str()))
            {
                Err("Invalid process ID")?;
            }
        }

        if agents.insert(Rc::clone(&agent.id), agent).is_some() {
            Err("Duplicate agent ID")?;
        }
    }

    Ok(agents)
}

/// Read agents info from the agents.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - The possible valid process IDs
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agents_file(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Agent> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    read_agents_file_from_iter(read_csv(&file_path), process_ids).unwrap_input_err(&file_path)
}

/// Read agents info from various CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - The possible valid process IDs
/// * `region_ids` - The possible valid region IDs
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
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
    let mut objectives = read_agent_objectives(model_dir, &agents);

    // Populate each Agent's Vecs
    for (id, agent) in agents.iter_mut() {
        agent.regions = agent_regions.remove(id).unwrap();
        agent.objectives = objectives.remove(id).unwrap();
    }

    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_agents_file_from_iter() {
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        // Valid case
        let search_space = ["A".into()].into_iter().collect();
        let agents = [Agent {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "".into(),
            commodity_portion: 1.0,
            search_space: SearchSpace::Some(search_space),
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::All,
            objectives: Vec::new(),
        }];
        let expected = HashMap::from_iter([("agent".into(), agents[0].clone())]);
        let actual = read_agents_file_from_iter(agents.into_iter(), &process_ids).unwrap();
        assert_eq!(actual, expected);

        // Invalid process ID
        let search_space = ["C".into()].into_iter().collect();
        let agents = [Agent {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "".into(),
            commodity_portion: 1.0,
            search_space: SearchSpace::Some(search_space),
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::All,
            objectives: Vec::new(),
        }];
        assert!(read_agents_file_from_iter(agents.into_iter(), &process_ids).is_err());

        // Duplicate agent ID
        let agents = [
            Agent {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "".into(),
                commodity_portion: 1.0,
                search_space: SearchSpace::AllProcesses,
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::All,
                objectives: Vec::new(),
            },
            Agent {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "".into(),
                commodity_portion: 1.0,
                search_space: SearchSpace::AllProcesses,
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::All,
                objectives: Vec::new(),
            },
        ];
        assert!(read_agents_file_from_iter(agents.into_iter(), &process_ids).is_err());
    }

    #[test]
    fn test_check_objective_parameter() {
        macro_rules! objective {
            ($decision_weight:expr, $decision_lexico_tolerance:expr) => {
                AgentObjective {
                    agent_id: "agent".into(),
                    objective_type: ObjectiveType::EquivalentAnnualCost,
                    decision_weight: $decision_weight,
                    decision_lexico_tolerance: $decision_lexico_tolerance,
                }
            };
        }

        // DecisionRule::Single
        let decision_rule = DecisionRule::Single;
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Weighted
        let decision_rule = DecisionRule::Weighted;
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Lexicographical
        let decision_rule = DecisionRule::Lexicographical;
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
    }

    #[test]
    fn test_read_agent_objectives_from_iter() {
        let agents: HashMap<_, _> = [(
            "agent".into(),
            Agent {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "".into(),
                commodity_portion: 1.0,
                search_space: SearchSpace::AllProcesses,
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::All,
                objectives: Vec::new(),
            },
        )]
        .into_iter()
        .collect();

        // Valid
        let objective = AgentObjective {
            agent_id: "agent".into(),
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: None,
            decision_lexico_tolerance: None,
        };
        let expected = [("agent".into(), vec![objective.clone()])]
            .into_iter()
            .collect();
        let actual = read_agent_objectives_from_iter([objective].into_iter(), &agents).unwrap();
        assert_eq!(actual, expected);

        // Missing objective for agent
        assert!(read_agent_objectives_from_iter([].into_iter(), &agents).is_err());

        // Bad parameter
        let objective = AgentObjective {
            agent_id: "agent".into(),
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: Some(1.0),
            decision_lexico_tolerance: None,
        };
        assert!(read_agent_objectives_from_iter([objective].into_iter(), &agents).is_err());
    }
}