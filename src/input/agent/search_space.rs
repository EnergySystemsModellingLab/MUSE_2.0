//! Code for reading the agent search space CSV file.
use super::super::*;
use crate::agent::{AgentID, AgentMap, AgentSearchSpace};
use crate::commodity::CommodityMap;
use crate::id::IDCollection;
use crate::process::ProcessID;
use anyhow::{Context, Result};
use indexmap::IndexSet;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const AGENT_SEARCH_SPACE_FILE_NAME: &str = "agent_search_space.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct AgentSearchSpaceRaw {
    /// The agent to apply the search space to.
    agent_id: String,
    /// The commodity to apply the search space to.
    commodity_id: String,
    /// The year to apply the search space to.
    year: u32,
    /// The processes that the agent will consider investing in. Expressed as process IDs separated
    /// by semicolons or `None`, meaning all processes.
    search_space: String,
}

impl AgentSearchSpaceRaw {
    fn to_agent_search_space(
        &self,
        process_ids: &IndexSet<ProcessID>,
        commodities: &CommodityMap,
        milestone_years: &[u32],
    ) -> Result<AgentSearchSpace> {
        // Parse search_space string
        let search_space = parse_search_space_str(&self.search_space, process_ids)?;

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
            year: self.year,
            commodity: Rc::clone(commodity),
            search_space,
        })
    }
}

/// Parse a string representing the processes the agent will invest in.
///
/// This string can either be:
///  * Empty, meaning all processes
///  * "all", meaning the same
///  * A list of process IDs separated by semicolons
fn parse_search_space_str(
    search_space: &str,
    process_ids: &IndexSet<ProcessID>,
) -> Result<Vec<ProcessID>> {
    let search_space = search_space.trim();
    if search_space.is_empty() || search_space.eq_ignore_ascii_case("all") {
        Ok(process_ids.iter().cloned().collect())
    } else {
        search_space
            .split(';')
            .map(|id| process_ids.get_id_by_str(id.trim()))
            .try_collect()
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
    process_ids: &IndexSet<ProcessID>,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, Vec<AgentSearchSpace>>> {
    let file_path = model_dir.join(AGENT_SEARCH_SPACE_FILE_NAME);
    let iter = read_csv_optional::<AgentSearchSpaceRaw>(&file_path)?;
    read_agent_search_space_from_iter(iter, agents, process_ids, commodities, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_search_space_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    process_ids: &IndexSet<ProcessID>,
    commodities: &CommodityMap,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, Vec<AgentSearchSpace>>>
where
    I: Iterator<Item = AgentSearchSpaceRaw>,
{
    let mut search_spaces = HashMap::new();
    for search_space_raw in iter {
        let search_space =
            search_space_raw.to_agent_search_space(process_ids, commodities, milestone_years)?;

        let (id, _agent) = agents
            .get_key_value(search_space_raw.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Append to Vec with the corresponding key or create
        search_spaces
            .entry(id.clone())
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
            search_space: "A;B".into(),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_ok());

        // Invalid commodity ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "invalid_commodity".into(),
            year: 2020,
            search_space: "A;B".into(),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_err());

        // Invalid process ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            year: 2020,
            search_space: "A;D".into(),
        };
        assert!(raw
            .to_agent_search_space(&process_ids, &commodities, &[2020])
            .is_err());
    }
}
