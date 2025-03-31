//! Code for reading the agent commodities CSV file.
use super::super::*;
use crate::agent::{AgentCommodity, AgentMap};
use crate::commodity::CommodityMap;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const AGENT_COMMODITIES_FILE_NAME: &str = "agent_commodities.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct AgentCommodityRaw {
    /// Unique agent id identifying the agent.
    pub agent_id: String,
    /// The commodity that the agent is responsible for.
    pub commodity_id: String,
    /// The year the commodity portion applies to.
    pub year: u32,
    /// The proportion of the commodity production that the agent is responsible for.
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    pub commodity_portion: f64,
}

impl AgentCommodityRaw {
    pub fn to_agent_commodity(
        &self,
        commodities: &CommodityMap,
        milestone_years: &[u32],
    ) -> Result<AgentCommodity> {
        // Get commodity
        let commodity = commodities
            .get(self.commodity_id.as_str())
            .context("Invalid commodity ID")?;

        // Check that the year is a valid milestone year
        ensure!(
            milestone_years.binary_search(&self.year).is_ok(),
            "Invalid milestone year {}",
            self.year
        );

        // Create AgentCommodity
        Ok(AgentCommodity {
            agent_id: self.agent_id.clone(),
            year: self.year,
            commodity: Rc::clone(commodity),
            commodity_portion: self.commodity_portion,
        })
    }
}

/// Read agent objective info from the agent_commodities.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agent_commodities(
    model_dir: &Path,
    agents: &AgentMap,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentCommodity>>> {
    let file_path = model_dir.join(AGENT_COMMODITIES_FILE_NAME);
    let agent_commodities_csv = read_csv(&file_path)?;
    read_agent_commodities_from_iter(agent_commodities_csv, agents, commodities, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_commodities_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentCommodity>>>
where
    I: Iterator<Item = AgentCommodityRaw>,
{
    let mut agent_commodities = HashMap::new();
    for agent_commodity in iter {
        let agent_commodity = agent_commodity.to_agent_commodity(commodities, milestone_years)?;

        let (id, _agent) = agents
            .get_key_value(agent_commodity.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Append to Vec with the corresponding key or create
        agent_commodities
            .entry(Rc::clone(id))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(agent_commodity);
    }

    Ok(agent_commodities)
}
