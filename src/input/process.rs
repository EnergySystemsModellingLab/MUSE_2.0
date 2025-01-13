//! Code for reading process-related information from CSV files.
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::Process;
use crate::time_slice::TimeSliceInfo;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;
use std::rc::Rc;
pub mod availability;
use availability::read_process_availabilities;
pub mod flow;
use flow::read_process_flows;
pub mod pac;
use pac::read_process_pacs;
pub mod parameter;
use parameter::read_process_parameters;
pub mod region;
use region::read_process_regions;

const PROCESSES_FILE_NAME: &str = "processes.csv";

macro_rules! define_process_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.process_id
            }
        }
    };
}
use define_process_id_getter;

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessDescription {
    id: Rc<str>,
    description: String,
}
define_id_getter! {ProcessDescription}

/// Read process information from the specified CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodities` - Commodities for the model
/// * `region_ids` - All possible region IDs
/// * `time_slice_info` - Information about seasons and times of day
/// * `year_range` - The possible range of milestone years
///
/// # Returns
///
/// This function returns a map of processes, with the IDs as keys.
pub fn read_processes(
    model_dir: &Path,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, Rc<Process>>> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let mut descriptions = read_csv_id_file::<ProcessDescription>(&file_path)?;
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let mut availabilities = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let mut flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let mut pacs = read_process_pacs(model_dir, &process_ids, commodities, &flows)?;
    let mut parameters = read_process_parameters(model_dir, &process_ids, year_range)?;
    let mut regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    process_ids
        .into_iter()
        .map(|id| {
            // We know entry is present
            let desc = descriptions.remove(&id).unwrap();

            let flows = flows
                .remove(&id)
                .with_context(|| format!("No commodity flows defined for process {id}"))?;
            let pacs = pacs
                .remove(&id)
                .with_context(|| format!("No PACs defined for process {id}"))?;

            // We've already checked that these exist for each process
            let parameter = parameters.remove(&id).unwrap();
            let regions = regions.remove(&id).unwrap();

            let process = Process {
                id: desc.id,
                description: desc.description,
                availabilities: availabilities.remove(&id).unwrap_or_default(),
                flows,
                pacs,
                parameter,
                regions,
            };

            Ok((id, process.into()))
        })
        .process_results(|iter| iter.collect())
}
