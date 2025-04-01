//! Code for reading the agent commodities CSV file.
use super::super::*;
use crate::agent::{AgentCommodity, AgentMap};
use crate::commodity::{CommodityMap, CommodityType};
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
    region_ids: &HashSet<Rc<str>>,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentCommodity>>> {
    let file_path = model_dir.join(AGENT_COMMODITIES_FILE_NAME);
    let agent_commodities_csv = read_csv(&file_path)?;
    read_agent_commodities_from_iter(
        agent_commodities_csv,
        agents,
        commodities,
        region_ids,
        milestone_years,
    )
    .with_context(|| input_err_msg(&file_path))
}

fn read_agent_commodities_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    commodities: &CommodityMap,
    region_ids: &HashSet<Rc<str>>,
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

    // CHECK 1: For each agent there must be at least one commodity for all years
    for (id, agent_commodities) in agent_commodities.iter() {
        let mut years = HashSet::new();
        for agent_commodity in agent_commodities {
            years.insert(agent_commodity.year);
        }
        for year in milestone_years {
            ensure!(
                years.contains(year),
                "Agent {} does not have a commodity for year {}",
                id,
                year
            );
        }
    }

    // CHECK 2: Total portions for each commodity/year/region must sum to 1
    // First step is to create a map with the key as (commodity_id, year, region_id), and the value
    // as the sum of the portions for that key across all agents
    let mut summed_portions = HashMap::new();
    for (id, agent_commodities) in agent_commodities.iter() {
        let agent = agents.get(id).context("Invalid agent ID")?;
        for agent_commodity in agent_commodities {
            let commodity_id = agent_commodity.commodity.get_id();
            let portion = agent_commodity.commodity_portion;
            for region in region_ids {
                if agent.regions.contains(region) {
                    let key = (commodity_id, agent_commodity.year, region);
                    summed_portions
                        .entry(key)
                        .and_modify(|v| *v += portion)
                        .or_insert(portion);
                }
            }
        }
    }

    // We then check the map to ensure values for each key are 1
    for (key, portion) in summed_portions.iter() {
        ensure!(
            (*portion - 1.0).abs() < f64::EPSILON,
            "Commodity {} in year {} and region {} does not sum to 1.0",
            key.0,
            key.1,
            key.2
        );
    }

    // CHECK 3: All commodities of SVD or SED type must be covered for all regions and years
    // This checks the same summed_portions map as above, just checking the keys
    // We first need to create a list of SVD and SED commodities to check against
    let svd_and_sed_commodities: Vec<Rc<str>> = commodities
        .iter()
        .filter(|(_, commodity)| {
            matches!(
                commodity.kind,
                CommodityType::SupplyEqualsDemand | CommodityType::ServiceDemand
            )
        })
        .map(|(id, _)| Rc::clone(id))
        .collect();

    // Check that summed_portions contains all SVD/SED commodities for all regions and milestone
    // years
    for commodity_id in svd_and_sed_commodities {
        for year in milestone_years {
            for region in region_ids {
                let key = (&*commodity_id, *year, region);
                ensure!(
                    summed_portions.contains_key(&key),
                    "Commodity {} in year {} and region {} is not covered",
                    commodity_id,
                    year,
                    region
                );
            }
        }
    }

    Ok(agent_commodities)
}
