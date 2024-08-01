use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use crate::input::*;

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Debug, Deserialize, PartialEq)]
pub struct Asset {
    pub process_id: String,
    pub region_id: String,
    pub agent_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

/// Process assets from an iterator.
///
/// # Arguments
///
/// * `iter` - Iterator of `AssetRaw`s
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - All possible process IDs
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `Vec` of assets.
fn read_assets_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<Vec<Asset>, Box<dyn Error>>
where
    I: Iterator<Item = Asset>,
{
    iter.map(|asset| {
        if !process_ids.contains(asset.process_id.as_str()) {
            Err(format!("Invalid process ID: {}", asset.process_id))?;
        }
        if !region_ids.contains(asset.region_id.as_str()) {
            Err(format!("Invalid region ID: {}", asset.region_id))?;
        }

        Ok(asset)
    })
    .process_results(|iter| iter.collect())
}

/// Read assets CSV file from model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - All possible process IDs
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `Vec` of assets.
pub fn read_assets(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Vec<Asset> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), process_ids, region_ids)
        .unwrap_input_err(&file_path)
}
