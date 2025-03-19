//! Code for reading in agent-related data from CSV files.
use super::*;
use crate::agent::{Agent, AgentMap, DecisionRule, SearchSpace};
use crate::commodity::CommodityMap;
use crate::process::ProcessMap;
use crate::region::RegionSelection;
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

pub mod objective;
use objective::read_agent_objectives;
pub mod region;
use region::read_agent_regions;

const AGENT_FILE_NAME: &str = "agents.csv";

/// An agent in the simulation
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct AgentRaw {
    /// A unique identifier for the agent.
    id: Rc<str>,
    /// A text description of the agent.
    description: String,
    /// The commodity that the agent produces (could be a service demand too).
    commodity_id: String,
    /// The proportion of the commodity production that the agent is responsible for.
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    commodity_portion: f64,
    /// The processes that the agent will consider investing in. Expressed as process IDs separated
    /// by semicolons or `None`, meaning all processes.
    search_space: Option<String>,
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
    let mut agents = read_agents_file(model_dir, commodities, &process_ids)?;
    let agent_ids = agents.keys().cloned().collect();

    let mut agent_regions = read_agent_regions(model_dir, &agent_ids, region_ids)?;
    let mut objectives = read_agent_objectives(model_dir, &agents, milestone_years)?;

    for (id, agent) in agents.iter_mut() {
        agent.regions = agent_regions.remove(id).unwrap();
        agent.objectives = objectives.remove(id).unwrap();
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
pub fn read_agents_file(
    model_dir: &Path,
    commodities: &CommodityMap,
    process_ids: &HashSet<Rc<str>>,
) -> Result<AgentMap> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    let agents_csv = read_csv(&file_path)?;
    read_agents_file_from_iter(agents_csv, commodities, process_ids)
        .with_context(|| input_err_msg(&file_path))
}

/// Read agents info from an iterator.
fn read_agents_file_from_iter<I>(
    iter: I,
    commodities: &CommodityMap,
    process_ids: &HashSet<Rc<str>>,
) -> Result<AgentMap>
where
    I: Iterator<Item = AgentRaw>,
{
    let mut agents = AgentMap::new();
    for agent_raw in iter {
        let commodity = commodities
            .get(agent_raw.commodity_id.as_str())
            .context("Invalid commodity ID")?;

        // Parse search space string
        let search_space = match agent_raw.search_space {
            None => SearchSpace::AllProcesses,
            Some(processes) => {
                let mut set = HashSet::new();
                for id in processes.split(';') {
                    set.insert(process_ids.get_id(id)?);
                }

                SearchSpace::Some(set)
            }
        };

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
            id: Rc::clone(&agent_raw.id),
            description: agent_raw.description,
            commodity: Rc::clone(commodity),
            commodity_portion: agent_raw.commodity_portion,
            search_space,
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
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

    #[test]
    fn test_read_agents_file_from_iter() {
        let process_ids = ["A".into(), "B".into(), "C".into()].into_iter().collect();
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let commodities = iter::once(("commodity1".into(), Rc::clone(&commodity))).collect();

        // Valid case
        let search_space = HashSet::from_iter(["A".into(), "B".into()]);
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "commodity1".into(),
            commodity_portion: 1.0,
            search_space: Some("A;B".into()),
            decision_rule: "single".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        let agent_out = Agent {
            id: "agent".into(),
            description: "".into(),
            commodity,
            commodity_portion: 1.0,
            search_space: SearchSpace::Some(search_space),
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        };
        let expected = AgentMap::from_iter(iter::once(("agent".into(), agent_out)));
        let actual =
            read_agents_file_from_iter(iter::once(agent), &commodities, &process_ids).unwrap();
        assert_eq!(actual, expected);

        // Invalid commodity ID
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "made_up_commodity".into(),
            commodity_portion: 1.0,
            search_space: None,
            decision_rule: "single".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        assert!(read_agents_file_from_iter(iter::once(agent), &commodities, &process_ids).is_err());

        // Invalid process ID
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "commodity1".into(),
            commodity_portion: 1.0,
            search_space: Some("A;D".into()),
            decision_rule: "single".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        assert!(read_agents_file_from_iter(iter::once(agent), &commodities, &process_ids).is_err());

        // Duplicate agent ID
        let agents = [
            AgentRaw {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "commodity1".into(),
                commodity_portion: 1.0,
                search_space: None,
                decision_rule: "single".into(),
                capex_limit: None,
                annual_cost_limit: None,
                decision_lexico_tolerance: None,
            },
            AgentRaw {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "commodity1".into(),
                commodity_portion: 1.0,
                search_space: None,
                decision_rule: "single".into(),
                capex_limit: None,
                annual_cost_limit: None,
                decision_lexico_tolerance: None,
            },
        ];
        assert!(
            read_agents_file_from_iter(agents.into_iter(), &commodities, &process_ids).is_err()
        );

        // Lexico tolerance missing for lexico decision rule
        let agent = AgentRaw {
            id: "agent".into(),
            description: "".into(),
            commodity_id: "commodity1".into(),
            commodity_portion: 1.0,
            search_space: None,
            decision_rule: "lexico".into(),
            capex_limit: None,
            annual_cost_limit: None,
            decision_lexico_tolerance: None,
        };
        assert!(read_agents_file_from_iter(iter::once(agent), &commodities, &process_ids).is_err());
    }
}
