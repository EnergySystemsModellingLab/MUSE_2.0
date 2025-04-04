//! Code for loading the agent regions CSV file.
use super::super::region::{define_region_id_getter, read_regions_for_entity};
use super::super::HasID;
use crate::region::RegionSelection;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

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
    agent_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, RegionSelection>> {
    let file_path = model_dir.join(AGENT_REGIONS_FILE_NAME);
    read_regions_for_entity::<AgentRegion>(&file_path, agent_ids, region_ids)
}
