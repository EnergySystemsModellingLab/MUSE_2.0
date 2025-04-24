//! Code for reading the agent cost limits CSV file.
use super::super::*;
use crate::agent::{AgentID, AgentMap, CostLimits, CostLimitsMap};
use crate::year::{deserialize_year, YearSelection};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

const AGENT_COST_LIMITS_FILE_NAME: &str = "agent_cost_limits.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct AgentCostLimitRaw {
    agent_id: String,
    capex_limit: Option<f64>,
    annual_cost_limit: Option<f64>,
    #[serde(deserialize_with = "deserialize_year")]
    year: YearSelection,
}

impl AgentCostLimitRaw {
    fn to_cost_limit(&self) -> CostLimits {
        CostLimits {
            capex_limit: self.capex_limit,
            annual_cost_limit: self.annual_cost_limit,
        }
    }
}

/// Read agent cost limits info from the agent_cost_limits.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agent_cost_limits(
    model_dir: &Path,
    agents: &AgentMap,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, CostLimitsMap>> {
    let file_path = model_dir.join(AGENT_COST_LIMITS_FILE_NAME);
    let agent_cost_limits_csv = read_csv(&file_path)?;
    read_agent_cost_limits_from_iter(agent_cost_limits_csv, agents, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_cost_limits_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, CostLimitsMap>>
where
    I: Iterator<Item = AgentCostLimitRaw>,
{
    let mut map: HashMap<AgentID, CostLimitsMap> = HashMap::new();
    for agent_cost_limits_raw in iter {
        let cost_limits = agent_cost_limits_raw.to_cost_limit();
        let year = agent_cost_limits_raw.year;

        // Get agent ID
        let (id, _agent) = agents
            .get_key_value(agent_cost_limits_raw.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Get or create entry in the map
        let entry = map.entry(id.clone()).or_default();

        // Insert cost limits for the specified year(s)
        match year {
            YearSelection::All => {
                for year in milestone_years {
                    entry.insert(*year, cost_limits.clone());
                }
            }
            YearSelection::Some(years) => {
                for year in years {
                    entry.insert(year, cost_limits.clone());
                }
            }
        }
    }

    // Validation: if cost limits are specified for an agent, they must be present for all years.
    for (id, cost_limits) in map.iter() {
        for year in milestone_years {
            if !cost_limits.contains_key(year) {
                return Err(anyhow::anyhow!(
                    "Agent {} is missing cost limits for year {}",
                    id,
                    year
                ));
            }
        }
    }

    Ok(map)
}
