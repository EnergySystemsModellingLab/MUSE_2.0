//! Code for reading in agent-related data from CSV files.
use super::*;
use crate::agent::{Agent, AgentID, AgentMap, DecisionRule};
use crate::commodity::CommodityMap;
use crate::process::ProcessMap;
use crate::region::RegionSelection;
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

mod objective;
use objective::read_agent_objectives;
mod region;
use region::read_agent_regions;
mod search_space;
use search_space::read_agent_search_space;
mod commodity;
use commodity::read_agent_commodities;

const AGENT_FILE_NAME: &str = "agents.csv";

/// An agent in the simulation
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct AgentRaw {
    /// A unique identifier for the agent.
    id: Rc<str>,
    /// A text description of the agent.
    description: String,
    /// The decision rule that the agent uses to decide investment.
    decision_rule: String,
    /// The maximum capital cost the agent will pay.
    capex_limit: Option<f64>,
    /// The maximum annual operating cost (fuel plus var_opex etc) that the agent will pay.
    annual_cost_limit: Option<f64>,
    /// The tolerance around the main objective to consider secondary objectives.
    decision_lexico_tolerance: Option<f64>,
}

/// Read agents info from various CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodities` - Commodities for the model
/// * `process_ids` - The possible valid process IDs
/// * `region_ids` - The possible valid region IDs
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agents(
    model_dir: &Path,
    commodities: &CommodityMap,
    processes: &ProcessMap,
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> Result<AgentMap> {
    let process_ids = processes.keys().cloned().collect();
    let mut agents = read_agents_file(model_dir)?;
    let agent_ids = agents.keys().cloned().collect();

    let mut agent_regions = read_agent_regions(model_dir, &agent_ids, region_ids)?;
    let mut objectives = read_agent_objectives(model_dir, &agents, milestone_years)?;
    let mut search_spaces = read_agent_search_space(
        model_dir,
        &agents,
        &process_ids,
        commodities,
        milestone_years,
    )?;
    let mut agent_commodities =
        read_agent_commodities(model_dir, &agents, commodities, region_ids, milestone_years)?;

    for (id, agent) in agents.iter_mut() {
        agent.regions = agent_regions.remove(id).unwrap();
        agent.objectives = objectives.remove(id).unwrap();
        if let Some(search_space) = search_spaces.remove(id) {
            agent.search_space = search_space;
        }
        agent.commodities = agent_commodities.remove(id).unwrap();
    }

    Ok(agents)
}

/// Read agents info from the agents.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodities` - Commodities for the model
/// * `process_ids` - The possible valid process IDs
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
fn read_agents_file(model_dir: &Path) -> Result<AgentMap> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    let agents_csv = read_csv(&file_path)?;
    read_agents_file_from_iter(agents_csv).with_context(|| input_err_msg(&file_path))
}

/// Read agents info from an iterator.
fn read_agents_file_from_iter<I>(iter: I) -> Result<AgentMap>
where
    I: Iterator<Item = AgentRaw>,
{
    let mut agents = AgentMap::new();
    for agent_raw in iter {
        // Parse decision rule
        let decision_rule = match agent_raw.decision_rule.to_ascii_lowercase().as_str() {
            "single" => DecisionRule::Single,
            "weighted" => DecisionRule::Weighted,
            "lexico" => {
                let tolerance = agent_raw
                    .decision_lexico_tolerance
                    .with_context(|| "Missing tolerance for lexico decision rule")?;
                ensure!(
                    tolerance >= 0.0,
                    "Lexico tolerance must be non-negative, got {}",
                    tolerance
                );
                DecisionRule::Lexicographical { tolerance }
            }
            invalid_rule => bail!("Invalid decision rule: {}", invalid_rule),
        };

        let agent = Agent {
            id: AgentID(agent_raw.id.clone()),
            description: agent_raw.description,
            commodities: Vec::new(),
            search_space: Vec::new(),
            decision_rule,
            capex_limit: agent_raw.capex_limit,
            annual_cost_limit: agent_raw.annual_cost_limit,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        };

        ensure!(
            agents.insert(agent_raw.id, agent).is_none(),
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
    use std::iter;

    #[test]
    fn test_read_agents_file_from_iter() {
        // Valid case
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            decision_rule: "single".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        let agent_out = Agent {
            id: "agent".into(),
            description: "".into(),
            commodities: Vec::new(),
            search_space: Vec::new(),
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        };
        let expected = AgentMap::from_iter(iter::once(("agent".into(), agent_out)));
        let actual = read_agents_file_from_iter(iter::once(agent)).unwrap();
        assert_eq!(actual, expected);

        // Duplicate agent ID
        let agents = [
            AgentRaw {
                id: "agent".into(),
                description: "".into(),
                decision_rule: "single".into(),
                capex_limit: None,
                annual_cost_limit: None,
                decision_lexico_tolerance: None,
            },
            AgentRaw {
                id: "agent".into(),
                description: "".into(),
                decision_rule: "single".into(),
                capex_limit: None,
                annual_cost_limit: None,
                decision_lexico_tolerance: None,
            },
        ];
        assert!(read_agents_file_from_iter(agents.into_iter()).is_err());

        // Lexico tolerance missing for lexico decision rule
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            decision_rule: "lexico".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        assert!(read_agents_file_from_iter(iter::once(agent)).is_err());
    }
}
