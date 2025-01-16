//! Code for reading process-related information from CSV files.
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::{Process, ProcessAvailability, ProcessFlow, ProcessParameter};
use crate::region::RegionSelection;
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

/// A map of process-related data structures, grouped by process ID
type GroupedMap<T> = HashMap<Rc<str>, Vec<T>>;

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
    let descriptions = read_csv_id_file::<ProcessDescription>(&file_path)?;
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let availabilities = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let pacs = read_process_pacs(model_dir, &process_ids, commodities, &flows)?;
    let parameters = read_process_parameters(model_dir, &process_ids, year_range)?;
    let regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    create_process_map(
        descriptions.into_values(),
        availabilities,
        flows,
        pacs,
        parameters,
        regions,
    )
}

fn create_process_map<I>(
    descriptions: I,
    availabilities: GroupedMap<ProcessAvailability>,
    flows: GroupedMap<ProcessFlow>,
    pacs: GroupedMap<Rc<Commodity>>,
    parameters: HashMap<Rc<str>, ProcessParameter>,
    regions: HashMap<Rc<str>, RegionSelection>,
) -> Result<HashMap<Rc<str>, Rc<Process>>>
where
    I: Iterator<Item = ProcessDescription>,
{
    // Need to be mutable as we remove elements as we go along
    let mut availabilities = availabilities;
    let mut flows = flows;
    let mut pacs = pacs;
    let mut parameters = parameters;
    let mut regions = regions;

    descriptions
        .map(|description| {
            let id = &description.id;
            let flows = flows
                .remove(id)
                .with_context(|| format!("No commodity flows defined for process {id}"))?;
            let pacs = pacs
                .remove(id)
                .with_context(|| format!("No PACs defined for process {id}"))?;

            // We've already checked that these exist for each process
            let parameter = parameters.remove(id).unwrap();
            let regions = regions.remove(id).unwrap();
            let availabilities = availabilities.remove(id).unwrap();

            let process = Process {
                id: Rc::clone(id),
                description: description.description,
                availabilities,
                flows,
                pacs,
                parameter,
                regions,
            };

            Ok((description.id, process.into()))
        })
        .process_results(|iter| iter.collect())
}
