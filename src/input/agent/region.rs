//! Code for loading the agent regions CSV file.
use super::super::region::read_regions_for_entity;
use crate::agent::AgentID;
use crate::id::{define_region_id_getter, HasID};
use crate::region::{RegionID, RegionSelection};
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const AGENT_REGIONS_FILE_NAME: &str = "agent_regions.csv";

#[derive(Debug, Deserialize, PartialEq)]
struct AgentRegion {
    agent_id: AgentID,
    /// The region to which an agent belongs.
    region_id: RegionID,
}
define_region_id_getter!(AgentRegion);

impl HasID<AgentID> for AgentRegion {
    fn get_id(&self) -> &AgentID {
        &self.agent_id
    }
}

/// Read the agent regions file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `agent_ids` - The possible valid agent IDs
/// * `region_ids` - The possible valid region IDs
///
/// # Returns
///
/// A map of [`RegionSelection`]s, with the agent ID as the key.
pub fn read_agent_regions(
    model_dir: &Path,
    agent_ids: &HashSet<AgentID>,
    region_ids: &HashSet<RegionID>,
) -> Result<HashMap<AgentID, RegionSelection>> {
    let file_path = model_dir.join(AGENT_REGIONS_FILE_NAME);
    read_regions_for_entity::<AgentRegion, AgentID>(&file_path, agent_ids, region_ids)
}
