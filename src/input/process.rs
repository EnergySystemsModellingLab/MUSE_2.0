//! Code for reading process-related information from CSV files.
use crate::commodity::{Commodity, CommodityType};
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
    let parameters = read_process_parameters(model_dir, &process_ids, year_range)?;
    let regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    // Validate commodities after the flows have been read
    validate_commodities(commodities, &flows)?;

    create_process_map(
        descriptions.into_values(),
        availabilities,
        flows,
        parameters,
        regions,
    )
}

/// Perform consistency checks for commodity flows.
fn validate_commodities(
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<()> {
    for (commodity_id, commodity) in commodities {
        if commodity.kind == CommodityType::SupplyEqualsDemand {
            validate_sed_commodity(commodity_id, commodity, flows)?;
        }
    }
    Ok(())
}

fn validate_sed_commodity(
    commodity_id: &Rc<str>,
    commodity: &Rc<Commodity>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<()> {
    let mut has_producer = false;
    let mut has_consumer = false;

    for flow in flows.values().flatten() {
        if Rc::ptr_eq(&flow.commodity, commodity) {
            if flow.flow > 0.0 {
                has_producer = true;
            } else if flow.flow < 0.0 {
                has_consumer = true;
            }
        }
    }

    ensure!(
        has_producer && has_consumer,
        "Commodity {} of 'SED' type must have both producer and consumer processes",
        commodity_id
    );

    Ok(())
}

fn create_process_map<I>(
    descriptions: I,
    availabilities: GroupedMap<ProcessAvailability>,
    flows: GroupedMap<ProcessFlow>,
    parameters: HashMap<Rc<str>, ProcessParameter>,
    regions: HashMap<Rc<str>, RegionSelection>,
) -> Result<HashMap<Rc<str>, Rc<Process>>>
where
    I: Iterator<Item = ProcessDescription>,
{
    // Need to be mutable as we remove elements as we go along
    let mut availabilities = availabilities;
    let mut flows = flows;
    let mut parameters = parameters;
    let mut regions = regions;

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
    use crate::commodity::{CommodityCostMap, DemandMap};
    use crate::process::FlowType;
    use crate::time_slice::TimeSliceLevel;

    use super::*;

    struct ProcessData {
        descriptions: Vec<ProcessDescription>,
        availabilities: GroupedMap<ProcessAvailability>,
        flows: GroupedMap<ProcessFlow>,
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
            .map(|id| (id.into(), vec![]))
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

    #[test]
    fn test_validate_commodities() {
        // Create mock commodities
        let commodity_sed = Rc::new(Commodity {
            id: "commodity_sed".into(),
            description: "SED commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });

        let commodity_non_sed = Rc::new(Commodity {
            id: "commodity_non_sed".into(),
            description: "Non-SED commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });

        let commodities: HashMap<Rc<str>, Rc<Commodity>> = [
            (Rc::clone(&commodity_sed.id), Rc::clone(&commodity_sed)),
            (
                Rc::clone(&commodity_non_sed.id),
                Rc::clone(&commodity_non_sed),
            ),
        ]
        .into_iter()
        .collect();

        // Create mock flows
        let process_flows: HashMap<Rc<str>, Vec<ProcessFlow>> = [
            (
                "process1".into(),
                vec![
                    ProcessFlow {
                        process_id: "process1".into(),
                        commodity: Rc::clone(&commodity_sed),
                        flow: 10.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                        is_pac: false,
                    },
                    ProcessFlow {
                        process_id: "process1".into(),
                        commodity: Rc::clone(&commodity_non_sed),
                        flow: -5.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                        is_pac: false,
                    },
                ],
            ),
            (
                "process2".into(),
                vec![ProcessFlow {
                    process_id: "process2".into(),
                    commodity: Rc::clone(&commodity_sed),
                    flow: -10.0,
                    flow_type: FlowType::Fixed,
                    flow_cost: 1.0,
                    is_pac: false,
                }],
            ),
        ]
        .into_iter()
        .collect();

        // Validate commodities
        assert!(validate_commodities(&commodities, &process_flows).is_ok());

        // Modify flows to make the validation fail
        let process_flows_invalid: HashMap<Rc<str>, Vec<ProcessFlow>> = [(
            "process1".into(),
            vec![ProcessFlow {
                process_id: "process1".into(),
                commodity: Rc::clone(&commodity_sed),
                flow: 10.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
                is_pac: false,
            }],
        )]
        .into_iter()
        .collect();

        // Validate commodities should fail
        assert!(validate_commodities(&commodities, &process_flows_invalid).is_err());
    }
}
