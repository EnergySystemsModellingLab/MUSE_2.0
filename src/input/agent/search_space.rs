//! Code for reading the agent search space CSV file.
use super::super::*;
use crate::agent::{AgentID, AgentMap, AgentSearchSpaceMap};
use crate::commodity::CommodityID;
use crate::id::IDCollection;
use crate::process::ProcessID;
use crate::year::parse_year_str;
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
    /// The year(s) to apply the search space to.
    years: String,
    /// The processes that the agent will consider investing in. Expressed as process IDs separated
    /// by semicolons or `None`, meaning all processes.
    search_space: String,
}

/// Search space for an agent
#[derive(Debug)]
struct AgentSearchSpace {
    /// The agent to which this search space applies
    agent_id: AgentID,
    /// The commodity to apply the search space to
    commodity_id: CommodityID,
    /// The year(s) the objective is relevant for
    years: Vec<u32>,
    /// The agent's search space
    search_space: Rc<Vec<ProcessID>>,
}

impl AgentSearchSpaceRaw {
    fn into_agent_search_space(
        self,
        agents: &AgentMap,
        process_ids: &IndexSet<ProcessID>,
        commodity_ids: &HashSet<CommodityID>,
        milestone_years: &[u32],
    ) -> Result<AgentSearchSpace> {
        // Parse search_space string
        let search_space = Rc::new(parse_search_space_str(&self.search_space, process_ids)?);

        // Get commodity
        let commodity_id = commodity_ids.get_id_by_str(&self.commodity_id)?;

        // Check that the year is a valid milestone year
        let year = parse_year_str(&self.years, milestone_years)?;

        let (agent_id, _) = agents
            .get_key_value(self.agent_id.as_str())
            .context("Invalid agent ID")?;

        Ok(AgentSearchSpace {
            agent_id: agent_id.clone(),
            commodity_id,
            years: year,
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
    commodity_ids: &HashSet<CommodityID>,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, AgentSearchSpaceMap>> {
    let file_path = model_dir.join(AGENT_SEARCH_SPACE_FILE_NAME);
    let iter = read_csv_optional::<AgentSearchSpaceRaw>(&file_path)?;
    read_agent_search_space_from_iter(iter, agents, process_ids, commodity_ids, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_search_space_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    process_ids: &IndexSet<ProcessID>,
    commodity_ids: &HashSet<CommodityID>,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, AgentSearchSpaceMap>>
where
    I: Iterator<Item = AgentSearchSpaceRaw>,
{
    let mut search_spaces = HashMap::new();
    for search_space_raw in iter {
        let search_space = search_space_raw.into_agent_search_space(
            agents,
            process_ids,
            commodity_ids,
            milestone_years,
        )?;

        // Get or create search space map
        let map = search_spaces
            .entry(search_space.agent_id)
            .or_insert_with(AgentSearchSpaceMap::new);

        // Store process IDs
        for year in search_space.years {
            try_insert(
                map,
                (search_space.commodity_id.clone(), year),
                search_space.search_space.clone(),
            )?;
        }
    }

    Ok(search_spaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{agents, assert_error};
    use rstest::{fixture, rstest};
    use std::iter;

    #[fixture]
    fn process_ids() -> IndexSet<ProcessID> {
        ["A".into(), "B".into(), "C".into()].into_iter().collect()
    }

    #[fixture]
    fn commodity_ids() -> HashSet<CommodityID> {
        iter::once("commodity1".into()).collect()
    }

    #[rstest]
    fn test_search_space_raw_into_search_space_valid(
        agents: AgentMap,
        process_ids: IndexSet<ProcessID>,
        commodity_ids: HashSet<CommodityID>,
    ) {
        // Valid search space
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            years: "2020".into(),
            search_space: "A;B".into(),
        };
        assert!(raw
            .into_agent_search_space(&agents, &process_ids, &commodity_ids, &[2020])
            .is_ok());
    }

    #[rstest]
    fn test_search_space_raw_into_search_space_invalid_commodity_id(
        agents: AgentMap,
        process_ids: IndexSet<ProcessID>,
        commodity_ids: HashSet<CommodityID>,
    ) {
        // Invalid commodity ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "invalid_commodity".into(),
            years: "2020".into(),
            search_space: "A;B".into(),
        };
        assert_error!(
            raw.into_agent_search_space(&agents, &process_ids, &commodity_ids, &[2020]),
            "Unknown ID invalid_commodity found"
        );
    }

    #[rstest]
    fn test_search_space_raw_into_search_space_invalid_process_id(
        agents: AgentMap,
        process_ids: IndexSet<ProcessID>,
        commodity_ids: HashSet<CommodityID>,
    ) {
        // Invalid process ID
        let raw = AgentSearchSpaceRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            years: "2020".into(),
            search_space: "A;D".into(),
        };
        assert_error!(
            raw.into_agent_search_space(&agents, &process_ids, &commodity_ids, &[2020]),
            "Unknown ID D found"
        );
    }
}
