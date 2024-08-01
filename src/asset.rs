use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use crate::input::{input_panic, read_csv};

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Debug, Deserialize, PartialEq)]
pub struct Asset {
    pub process_id: String,
    pub region_id: String,
    pub agent_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

fn read_assets_from_iter<I>(
    iter: I,
    file_path: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Vec<Asset>
where
    I: Iterator<Item = Asset>,
{
    iter.map(|asset| {
        if !process_ids.contains(asset.process_id.as_str()) {
            input_panic(
                file_path,
                &format!("Invalid process ID: {}", asset.process_id),
            );
        }
        if !region_ids.contains(asset.region_id.as_str()) {
            input_panic(
                file_path,
                &format!("Invalid region ID: {}", asset.region_id),
            );
        }

        asset
    })
    .collect()
}

pub fn read_assets(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> Vec<Asset> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), &file_path, process_ids, region_ids)
}
