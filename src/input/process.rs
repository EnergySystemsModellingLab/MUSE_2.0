//! Code for reading process-related information from CSV files.
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::{Process, ProcessAvailabilityMap, ProcessFlow, ProcessParameter};
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
    let descriptions = read_csv_id_file::<ProcessDescription>(&file_path)?;
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let availabilities = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let parameters = read_process_parameters(model_dir, &process_ids, year_range)?;
    let regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    create_process_map(
        descriptions.into_values(),
        availabilities,
        flows,
        parameters,
        regions,
    )
}

fn create_process_map<I>(
    descriptions: I,
    mut availabilities: HashMap<Rc<str>, ProcessAvailabilityMap>,
    mut flows: HashMap<Rc<str>, Vec<ProcessFlow>>,
    mut parameters: HashMap<Rc<str>, ProcessParameter>,
    mut regions: HashMap<Rc<str>, RegionSelection>,
) -> Result<HashMap<Rc<str>, Rc<Process>>>
where
    I: Iterator<Item = ProcessDescription>,
{
    descriptions
        .map(|description| {
            let id = &description.id;
            let availabilities = availabilities
                .remove(id)
                .with_context(|| format!("No availabilities defined for process {id}"))?;
            let flows = flows
                .remove(id)
                .with_context(|| format!("No commodity flows defined for process {id}"))?;
            let parameter = parameters
                .remove(id)
                .with_context(|| format!("No parameters defined for process {id}"))?;

            // We've already checked that regions are defined for each process
            let regions = regions.remove(id).unwrap();

            let process = Process {
                id: Rc::clone(id),
                description: description.description,
                availabilities,
                flows,
                parameter,
                regions,
            };

            Ok((description.id, process.into()))
        })
        .process_results(|iter| iter.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ProcessData {
        descriptions: Vec<ProcessDescription>,
        availabilities: HashMap<Rc<str>, ProcessAvailabilityMap>,
        flows: HashMap<Rc<str>, Vec<ProcessFlow>>,
        parameters: HashMap<Rc<str>, ProcessParameter>,
        regions: HashMap<Rc<str>, RegionSelection>,
    }

    /// Returns example data (without errors) for processes
    fn get_process_data() -> ProcessData {
        let descriptions = vec![
            ProcessDescription {
                id: Rc::from("process1"),
                description: "Process 1".to_string(),
            },
            ProcessDescription {
                id: Rc::from("process2"),
                description: "Process 2".to_string(),
            },
        ];

        let availabilities = ["process1", "process2"]
            .into_iter()
            .map(|id| (id.into(), ProcessAvailabilityMap::new()))
            .collect();

        let flows = ["process1", "process2"]
            .into_iter()
            .map(|id| (id.into(), vec![]))
            .collect();

        let parameters = ["process1", "process2"]
            .into_iter()
            .map(|id| {
                let parameter = ProcessParameter {
                    process_id: id.to_string(),
                    years: 2010..=2020,
                    capital_cost: 0.0,
                    fixed_operating_cost: 0.0,
                    variable_operating_cost: 0.0,
                    lifetime: 1,
                    discount_rate: 1.0,
                    cap2act: 0.0,
                };

                (id.into(), parameter)
            })
            .collect();

        let regions = ["process1", "process2"]
            .into_iter()
            .map(|id| (id.into(), RegionSelection::All))
            .collect();

        ProcessData {
            descriptions,
            availabilities,
            flows,
            parameters,
            regions,
        }
    }

    #[test]
    fn test_create_process_map_success() {
        let data = get_process_data();
        let result = create_process_map(
            data.descriptions.into_iter(),
            data.availabilities,
            data.flows,
            data.parameters,
            data.regions,
        )
        .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("process1"));
        assert!(result.contains_key("process2"));
    }

    /// Generate code for a test with data missing for a given field
    macro_rules! test_missing {
        ($field:ident) => {
            let mut data = get_process_data();
            data.$field.remove("process1");

            let result = create_process_map(
                data.descriptions.into_iter(),
                data.availabilities,
                data.flows,
                data.parameters,
                data.regions,
            );
            assert!(result.is_err());
        };
    }

    #[test]
    fn test_create_process_map_missing_availabilities() {
        test_missing!(availabilities);
    }

    #[test]
    fn test_create_process_map_missing_flows() {
        test_missing!(flows);
    }

    #[test]
    fn test_create_process_map_missing_parameters() {
        test_missing!(parameters);
    }
}
