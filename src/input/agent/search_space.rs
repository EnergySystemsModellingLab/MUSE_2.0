//! Code for reading the agent search space CSV file.
use super::super::*;
use crate::agent::{AgentMap, AgentSearchSpace, SearchSpace};
use crate::commodity::CommodityMap;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const AGENT_SEARCH_SPACE_FILE_NAME: &str = "agent_search_space.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct AgentSearchSpaceRaw {
    /// The agent to apply the search space to.
    pub agent_id: String,
    /// The commodity to apply the search space to.
    pub commodity_id: String,
    /// The year to apply the search space to.
    pub year: u32,
    /// The processes that the agent will consider investing in. Expressed as process IDs separated
    /// by semicolons or `None`, meaning all processes.
    pub search_space: Option<String>,
}

impl AgentSearchSpaceRaw {
    pub fn to_agent_search_space(
        &self,
        process_ids: &HashSet<Rc<str>>,
        commodities: &CommodityMap,
        milestone_years: &[u32],
    ) -> Result<AgentSearchSpace> {
        // Parse search_space string
        let search_space = match &self.search_space {
            None => SearchSpace::AllProcesses,
            Some(processes) => {
                let mut set = HashSet::new();
                for id in processes.split(';') {
                    set.insert(process_ids.get_id(id)?);
                }
                SearchSpace::Some(set)
            }
        };

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

        // Create AgentSearchSpace
        Ok(AgentSearchSpace {
            agent_id: self.agent_id.clone(),
            year: self.year,
            commodity: Rc::clone(commodity),
            search_space,
        })
    }
}

/// Read agent search space info from the agent_search_space.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agent_search_space(
    model_dir: &Path,
    agents: &AgentMap,
    process_ids: &HashSet<Rc<str>>,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentSearchSpace>>> {
    let file_path = model_dir.join(AGENT_SEARCH_SPACE_FILE_NAME);
    let iter = read_csv::<AgentSearchSpaceRaw>(&file_path)?;
    read_agent_search_space_from_iter(iter, agents, process_ids, commodities, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_search_space_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    process_ids: &HashSet<Rc<str>>,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentSearchSpace>>>
where
    I: Iterator<Item = AgentSearchSpaceRaw>,
{
    let mut search_spaces = HashMap::new();
    for search_space in iter {
        let search_space =
            search_space.to_agent_search_space(process_ids, commodities, milestone_years)?;

        let (id, _agent) = agents
            .get_key_value(search_space.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Append to Vec with the corresponding key or create
        search_spaces
            .entry(Rc::clone(id))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(search_space);
    }

    Ok(search_spaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

    #[test]
    fn test_search_space_raw_into_search_space() {
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

        // Valid search space
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            year: 2020,
            search_space: Some("A;B".into()),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_ok());

        // Invalid commodity ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "invalid_commodity".into(),
            year: 2020,
            search_space: Some("A;B".into()),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_err());

        // Invalid process ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            year: 2020,
            search_space: Some("A;D".into()),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_err());
    }
}
