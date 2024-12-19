//! Code for reading in agent-related data from CSV files.
use super::*;
use crate::agent::{Agent, SearchSpace};
use crate::asset::read_assets;
use crate::process::Process;
use anyhow::{bail, ensure, Context, Result};
use region::{define_region_id_getter, read_regions_for_entity};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

pub mod objective;
use objective::read_agent_objectives;

const AGENT_FILE_NAME: &str = "agents.csv";
const AGENT_REGIONS_FILE_NAME: &str = "agent_regions.csv";

#[derive(Debug, Deserialize, PartialEq)]
struct AgentRegion {
    agent_id: String,
    /// The region to which an agent belongs.
    region_id: String,
}
define_region_id_getter!(AgentRegion);

impl HasID for AgentRegion {
    fn get_id(&self) -> &str {
        &self.agent_id
    }
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
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Agent>> {
    let process_ids = processes.keys().cloned().collect();
    let mut agents = read_agents_file(model_dir, &process_ids)?;
    let agent_ids = agents.keys().cloned().collect();

    let file_path = model_dir.join(AGENT_REGIONS_FILE_NAME);
    let mut agent_regions =
        read_regions_for_entity::<AgentRegion>(&file_path, &agent_ids, region_ids)?;
    let mut objectives = read_agent_objectives(model_dir, &agents)?;
    let mut assets = read_assets(model_dir, &agent_ids, processes, region_ids)?;

    // Populate each Agent's Vecs
    for (id, agent) in agents.iter_mut() {
        agent.regions = agent_regions.remove(id).unwrap();
        agent.objectives = objectives.remove(id).unwrap();
        agent.assets = assets.remove(id).unwrap_or_default();
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
) -> Result<HashMap<Rc<str>, Agent>> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    let agents_csv = read_csv(&file_path)?;
    read_agents_file_from_iter(agents_csv, process_ids).with_context(|| input_err_msg(&file_path))
}

/// Read agents info from an iterator.
fn read_agents_file_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Agent>>
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
                bail!("Invalid process ID");
            }
        }

        ensure!(
            agents.insert(Rc::clone(&agent.id), agent).is_none(),
            "Duplicate agent ID"
        );
    }

    Ok(agents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::DecisionRule;
    use crate::region::RegionSelection;

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
            assets: Vec::new(),
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
            assets: Vec::new(),
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
                assets: Vec::new(),
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
                assets: Vec::new(),
            },
        ];
        assert!(read_agents_file_from_iter(agents.into_iter(), &process_ids).is_err());
    }
}
