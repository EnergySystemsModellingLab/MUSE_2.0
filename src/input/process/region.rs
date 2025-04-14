//! Code for reading the process region CSV file
use super::super::region::read_regions_for_entity;
use crate::id::{define_region_id_getter, HasID};
use crate::region::RegionSelection;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_REGIONS_FILE_NAME: &str = "process_regions.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessRegion {
    process_id: String,
    region_id: String,
}
define_region_id_getter! {ProcessRegion}

impl HasID for ProcessRegion {
    fn get_id(&self) -> &str {
        &self.process_id
    }
}

/// Read the process regions file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - The possible valid process IDs
/// * `region_ids` - The possible valid region IDs
///
/// # Returns
///
/// A map of [`RegionSelection`]s, with the process ID as the key.
pub fn read_process_regions(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, RegionSelection>> {
    let file_path = model_dir.join(PROCESS_REGIONS_FILE_NAME);
    read_regions_for_entity::<ProcessRegion>(&file_path, process_ids, region_ids)
}
