use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use crate::input::*;

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Asset {
    pub process_id: String,
    pub region_id: String,
    pub agent_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

impl HasID for Asset {
    fn get_id(&self) -> &str {
        &self.agent_id
    }
}

/// Process assets from an iterator.
///
/// # Arguments
///
/// * `iter` - Iterator of `AssetRaw`s
/// * `model_dir` - Folder containing model configuration files
/// * `agent_ids` - All possible process IDs
/// * `process_ids` - All possible process IDs
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
fn read_assets_from_iter<I>(
    iter: I,
    agent_ids: &HashSet<Rc<str>>,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Vec<Asset>>, Box<dyn Error>>
where
    I: Iterator<Item = Asset>,
{
    let map = iter.into_id_map(agent_ids)?;

    for asset in map.values().flatten() {
        if !process_ids.contains(asset.process_id.as_str()) {
            Err(format!("Invalid process ID: {}", asset.process_id))?;
        }
        if !region_ids.contains(asset.region_id.as_str()) {
            Err(format!("Invalid region ID: {}", asset.region_id))?;
        }
    }

    Ok(map)
}

/// Read assets CSV file from model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `agent_ids` - All possible process IDs
/// * `process_ids` - All possible process IDs
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
pub fn read_assets(
    model_dir: &Path,
    agent_ids: &HashSet<Rc<str>>,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Vec<Asset>> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), agent_ids, process_ids, region_ids)
        .unwrap_input_err(&file_path)
}
