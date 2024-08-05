use crate::process::Process;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use crate::input::{input_panic, read_csv};

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Debug, Deserialize, PartialEq)]
struct AssetRaw {
    process_id: String,
    region_id: String,
    agent_id: String,
    capacity: f64,
    commission_year: u32,
}

#[derive(Debug)]
pub struct Asset {
    pub process: Rc<Process>,
    pub region_id: String,
    pub agent_id: String,
    pub capacity: f64,
    pub commission_year: u32,
}

fn read_assets_from_iter<I>(
    iter: I,
    file_path: &Path,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Vec<Asset>
where
    I: Iterator<Item = AssetRaw>,
{
    iter.map(|record| {
        let process = processes
            .get(record.process_id.as_str())
            .unwrap_or_else(|| {
                input_panic(
                    file_path,
                    &format!("Invalid process ID: {}", record.process_id),
                )
            });
        if !region_ids.contains(record.region_id.as_str()) {
            input_panic(
                file_path,
                &format!("Invalid region ID: {}", record.region_id),
            );
        }

        Asset {
            process: Rc::clone(process),
            region_id: record.region_id,
            agent_id: record.agent_id,
            capacity: record.capacity,
            commission_year: record.commission_year,
        }
    })
    .collect()
}

pub fn read_assets(
    model_dir: &Path,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Vec<Asset> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), &file_path, processes, region_ids)
}
