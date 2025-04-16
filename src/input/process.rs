//! Code for reading process-related information from CSV files.
use super::*;
use crate::commodity::{Commodity, CommodityID, CommodityMap, CommodityType};
use crate::process::{
    ActivityLimitsMap, Process, ProcessFlow, ProcessID, ProcessMap, ProcessParameter,
};
use crate::region::{RegionID, RegionSelection};
use crate::time_slice::TimeSliceInfo;
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

mod availability;
use availability::read_process_availabilities;
mod flow;
use flow::read_process_flows;
mod parameter;
use parameter::read_process_parameters;
mod region;
use crate::id::define_id_getter;
use region::read_process_regions;

const PROCESSES_FILE_NAME: &str = "processes.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessDescription {
    id: ProcessID,
    description: String,
}
define_id_getter! {ProcessDescription, ProcessID}

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
    commodities: &CommodityMap,
    region_ids: &HashSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<ProcessMap> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let descriptions = read_csv_id_file::<ProcessDescription, ProcessID>(&file_path)?;
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let availabilities = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let year_range = milestone_years[0]..=milestone_years[milestone_years.len() - 1];
    let parameters = read_process_parameters(model_dir, &process_ids, &year_range)?;
    let regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    // Validate commodities after the flows have been read
    validate_commodities(
        commodities,
        &flows,
        region_ids,
        milestone_years,
        time_slice_info,
        &parameters,
        &availabilities,
    )?;

    create_process_map(
        descriptions.into_values(),
        availabilities,
        flows,
        parameters,
        regions,
    )
}

struct ValidationParams<'a> {
    flows: &'a HashMap<ProcessID, Vec<ProcessFlow>>,
    region_ids: &'a HashSet<RegionID>,
    milestone_years: &'a [u32],
    time_slice_info: &'a TimeSliceInfo,
    parameters: &'a HashMap<ProcessID, ProcessParameter>,
    availabilities: &'a HashMap<ProcessID, ActivityLimitsMap>,
}

/// Perform consistency checks for commodity flows.
fn validate_commodities(
    commodities: &CommodityMap,
    flows: &HashMap<ProcessID, Vec<ProcessFlow>>,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
    time_slice_info: &TimeSliceInfo,
    parameters: &HashMap<ProcessID, ProcessParameter>,
    availabilities: &HashMap<ProcessID, ActivityLimitsMap>,
) -> anyhow::Result<()> {
    let params = ValidationParams {
        flows,
        region_ids,
        milestone_years,
        time_slice_info,
        parameters,
        availabilities,
    };
    for (commodity_id, commodity) in commodities {
        match commodity.kind {
            CommodityType::SupplyEqualsDemand => {
                validate_sed_commodity(commodity_id, commodity, flows)?;
            }
            CommodityType::ServiceDemand => {
                validate_svd_commodity(commodity_id, commodity, &params)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_sed_commodity(
    commodity_id: &CommodityID,
    commodity: &Rc<Commodity>,
    flows: &HashMap<ProcessID, Vec<ProcessFlow>>,
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

            if has_producer && has_consumer {
                return Ok(());
            }
        }
    }

    bail!(
        "Commodity {} of 'SED' type must have both producer and consumer processes",
        commodity_id
    );
}

fn validate_svd_commodity(
    commodity_id: &CommodityID,
    commodity: &Rc<Commodity>,
    params: &ValidationParams,
) -> Result<()> {
    for region_id in params.region_ids.iter() {
        for year in params.milestone_years.iter().copied() {
            for time_slice in params.time_slice_info.iter_ids() {
                let demand = commodity
                    .demand
                    .get((region_id.clone(), year, time_slice.clone()));
                if demand > 0.0 {
                    let mut has_producer = false;

                    // We must check for producers in every time slice, region, and year.
                    // This includes checking if flow > 0 and if availability > 0.

                    for flow in params.flows.values().flatten() {
                        if Rc::ptr_eq(&flow.commodity, commodity)
                            && flow.flow > 0.0
                            && params
                                .parameters
                                .get(&*flow.process_id)
                                .unwrap()
                                .years
                                .contains(&year)
                            && params
                                .availabilities
                                .get(&*flow.process_id)
                                .unwrap()
                                .get(time_slice)
                                .unwrap()
                                .end()
                                > &0.0
                        {
                            has_producer = true;
                            break;
                        }
                    }

                    ensure!(
                        has_producer,
                        "Commodity {} of 'SVD' type must have producer processes for region {} in year {}",
                        commodity_id,
                        region_id,
                        year
                    );
                }
            }
        }
    }

    Ok(())
}

fn create_process_map<I>(
    descriptions: I,
    mut availabilities: HashMap<ProcessID, ActivityLimitsMap>,
    mut flows: HashMap<ProcessID, Vec<ProcessFlow>>,
    mut parameters: HashMap<ProcessID, ProcessParameter>,
    mut regions: HashMap<ProcessID, RegionSelection>,
) -> Result<ProcessMap>
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
                id: id.clone(),
                description: description.description,
                activity_limits: availabilities,
                flows,
                parameter,
                regions,
            };

            Ok((description.id, process.into()))
        })
        .try_collect()
}

#[cfg(test)]
mod tests {
    use crate::commodity::{CommodityCostMap, DemandMap};
    use crate::process::FlowType;
    use crate::time_slice::TimeSliceID;
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

    use super::*;

    struct ProcessData {
        descriptions: Vec<ProcessDescription>,
        availabilities: HashMap<ProcessID, ActivityLimitsMap>,
        flows: HashMap<ProcessID, Vec<ProcessFlow>>,
        parameters: HashMap<ProcessID, ProcessParameter>,
        regions: HashMap<ProcessID, RegionSelection>,
        region_ids: HashSet<RegionID>,
    }

    /// Returns example data (without errors) for processes
    fn get_process_data() -> ProcessData {
        let descriptions = vec![
            ProcessDescription {
                id: "process1".into(),
                description: "Process 1".to_string(),
            },
            ProcessDescription {
                id: "process2".into(),
                description: "Process 2".to_string(),
            },
        ];

        let availabilities = ["process1", "process2"]
            .into_iter()
            .map(|id| {
                let mut map = ActivityLimitsMap::new();
                map.insert(
                    TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "day".into(),
                    },
                    0.1..=0.9,
                );
                (id.into(), map)
            })
            .collect();

        let flows = ["process1", "process2"]
            .into_iter()
            .map(|id| (id.into(), vec![]))
            .collect();

        let parameters = ["process1", "process2"]
            .into_iter()
            .map(|id| {
                let parameter = ProcessParameter {
                    years: 2010..=2020,
                    capital_cost: 0.0,
                    fixed_operating_cost: 0.0,
                    variable_operating_cost: 0.0,
                    lifetime: 1,
                    discount_rate: 1.0,
                    capacity_to_activity: 0.0,
                };

                (id.into(), parameter)
            })
            .collect();

        let regions = ["process1", "process2"]
            .into_iter()
            .map(|id| (id.into(), RegionSelection::All))
            .collect();

        let region_ids = HashSet::from_iter(iter::once("GBR".into()));

        ProcessData {
            descriptions,
            availabilities,
            flows,
            parameters,
            regions,
            region_ids,
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
        let data = get_process_data();
        // Create mock commodities
        let commodity_sed = Rc::new(Commodity {
            id: "commodity_sed".into(),
            description: "SED commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });

        let milestone_years = [2010, 2020];

        // Set the TimeSliceInfo
        let id = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let fractions: IndexMap<TimeSliceID, f64> = [(id.clone(), 1.0)].into_iter().collect();
        let time_slice_info = TimeSliceInfo {
            seasons: [id.season].into_iter().collect(),
            times_of_day: [id.time_of_day].into_iter().collect(),
            fractions,
        };
        let parameters = data.parameters;
        let availabilities = data.availabilities;

        // Create a dummy demand map for the non-SED commodity
        let mut demand_map = DemandMap::new();
        for region in data.region_ids.iter() {
            for year in milestone_years {
                for time_slice in time_slice_info.iter_ids() {
                    demand_map.insert((region.clone(), year, time_slice.clone()), 0.5);
                }
            }
        }
        let commodity_non_sed = Rc::new(Commodity {
            id: "commodity_non_sed".into(),
            description: "Non-SED commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: demand_map,
        });

        let commodities: CommodityMap = [
            (commodity_sed.id.clone(), Rc::clone(&commodity_sed)),
            (commodity_non_sed.id.clone(), Rc::clone(&commodity_non_sed)),
        ]
        .into_iter()
        .collect();

        // Create mock flows
        let process_flows: HashMap<ProcessID, Vec<ProcessFlow>> = [
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
                        flow: 5.0,
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
        assert!(validate_commodities(
            &commodities,
            &process_flows,
            &data.region_ids,
            &milestone_years,
            &time_slice_info,
            &parameters,
            &availabilities,
        )
        .is_ok());

        // Modify flows to make the validation fail
        let process_flows_invalid: HashMap<ProcessID, Vec<ProcessFlow>> = [(
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
        assert!(validate_commodities(
            &commodities,
            &process_flows_invalid,
            &data.region_ids,
            &milestone_years,
            &time_slice_info,
            &parameters,
            &availabilities,
        )
        .is_err());
    }
}
