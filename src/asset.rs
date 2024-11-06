use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use crate::input::*;
use crate::process::Process;

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Deserialize, PartialEq)]
pub struct AssetRaw {
    pub process_id: String,
    pub region_id: String,
    pub agent_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub process: Rc<Process>,
    pub region_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

/// Process assets from an iterator.
///
/// # Arguments
///
/// * `iter` - Iterator of `AssetRaw`s
/// * `model_dir` - Folder containing model configuration files
/// * `agent_ids` - All possible process IDs
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
fn read_assets_from_iter<I>(
    iter: I,
    agent_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Vec<Asset>>, Box<dyn Error>>
where
    I: Iterator<Item = AssetRaw>,
{
    let map: HashMap<Rc<str>, _> = iter
        .map(|asset| -> Result<_, Box<dyn Error>> {
            let process = processes
                .get(asset.process_id.as_str())
                .ok_or(format!("Invalid process ID: {}", &asset.process_id))?;

            Ok((
                asset.agent_id.into(),
                Asset {
                    process: Rc::clone(process),
                    region_id: asset.region_id,
                    capacity: asset.capacity,
                    commission_year: asset.commission_year,
                },
            ))
        })
        .process_results(|iter| iter.into_group_map())?;

    for agent_id in map.keys() {
        if !agent_ids.contains(agent_id) {
            Err(format!("Invalid agent ID: {}", agent_id))?;
        }
    }

    for asset in map.values().flatten() {
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
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
pub fn read_assets(
    model_dir: &Path,
    agent_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Vec<Asset>> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), agent_ids, processes, region_ids)
        .unwrap_input_err(&file_path)
}
