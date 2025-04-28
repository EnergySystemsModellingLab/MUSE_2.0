//! Code for reading process-related information from CSV files.
use super::*;
use crate::commodity::{Commodity, CommodityID, CommodityMap, CommodityType};
use crate::process::{
    EnergyLimitsMap, Process, ProcessFlow, ProcessID, ProcessMap, ProcessParameterMap,
};
use crate::region::{RegionID, RegionSelection};
use crate::time_slice::TimeSliceInfo;
use anyhow::{bail, ensure, Context, Ok, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
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
struct ProcessRaw {
    id: ProcessID,
    description: String,
    start_year: Option<u32>,
    end_year: Option<u32>,
}
define_id_getter! {ProcessRaw, ProcessID}

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
    let year_range = milestone_years[0]..=milestone_years[milestone_years.len() - 1];
    let mut processes = read_processes_file(model_dir, &year_range)?;
    let process_ids = processes.keys().cloned().collect();

    let mut energy_limits = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let mut flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let mut parameters =
        read_process_parameters(model_dir, &process_ids, &processes, milestone_years)?;
    let mut regions = read_process_regions(model_dir, &process_ids, region_ids)?;

    // Validate commodities after the flows have been read
    validate_commodities(
        commodities,
        &flows,
        region_ids,
        milestone_years,
        time_slice_info,
        &parameters,
        &energy_limits,
    )?;

    // Add data to Process objects
    for (id, process) in processes.iter_mut() {
        process.energy_limits = energy_limits
            .remove(id)
            .with_context(|| format!("Missing availabilities for process {id}"))?;
        process.flows = flows
            .remove(id)
            .with_context(|| format!("Missing flows for process {id}"))?;
        process.parameters = parameters
            .remove(id)
            .with_context(|| format!("Missing parameters for process {id}"))?;
        process.regions = regions
            .remove(id)
            .with_context(|| format!("Missing regions for process {id}"))?;
    }

    // Create ProcessMap
    let mut process_map = ProcessMap::new();
    for (id, process) in processes {
        process_map.insert(id, process.into());
    }

    Ok(process_map)
}

fn read_processes_file(
    model_dir: &Path,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<ProcessID, Process>> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let processes_csv = read_csv(&file_path)?;
    read_processes_file_from_iter(processes_csv, year_range)
        .with_context(|| input_err_msg(&file_path))
}

fn read_processes_file_from_iter<I>(
    iter: I,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<ProcessID, Process>>
where
    I: Iterator<Item = ProcessRaw>,
{
    let mut processes = HashMap::new();
    for process_raw in iter {
        let start_year = process_raw.start_year.unwrap_or(*year_range.start());
        let end_year = process_raw.end_year.unwrap_or(*year_range.end());

        // Check year range is valid
        ensure!(
            start_year <= end_year,
            "Error in parameter for process {}: start_year > end_year",
            process_raw.id
        );

        let process = Process {
            id: process_raw.id.clone(),
            description: process_raw.description,
            years: start_year..=end_year,
            energy_limits: EnergyLimitsMap::new(),
            flows: Vec::new(),
            parameters: ProcessParameterMap::new(),
            regions: RegionSelection::default(),
        };

        ensure!(
            processes.insert(process_raw.id, process).is_none(),
            "Duplicate process ID"
        );
    }

    Ok(processes)
}

struct ValidationParams<'a> {
    flows: &'a HashMap<ProcessID, Vec<ProcessFlow>>,
    region_ids: &'a HashSet<RegionID>,
    milestone_years: &'a [u32],
    time_slice_info: &'a TimeSliceInfo,
    parameters: &'a HashMap<ProcessID, ProcessParameterMap>,
    availabilities: &'a HashMap<ProcessID, EnergyLimitsMap>,
}

/// Perform consistency checks for commodity flows.
fn validate_commodities(
    commodities: &CommodityMap,
    flows: &HashMap<ProcessID, Vec<ProcessFlow>>,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
    time_slice_info: &TimeSliceInfo,
    parameters: &HashMap<ProcessID, ProcessParameterMap>,
    availabilities: &HashMap<ProcessID, EnergyLimitsMap>,
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
                    .get(&(region_id.clone(), year, time_slice.clone()))
                    .unwrap();
                if demand > &0.0 {
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
                                .keys()
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

#[cfg(test)]
mod tests {
    use crate::commodity::{CommodityCostMap, DemandMap};
    use crate::process::{FlowType, ProcessParameter, ProcessParameterMap};
    use crate::time_slice::TimeSliceID;
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

    use super::*;

    struct ProcessData {
        availabilities: HashMap<ProcessID, EnergyLimitsMap>,
        parameters: HashMap<ProcessID, ProcessParameterMap>,
        region_ids: HashSet<RegionID>,
    }

    /// Returns example data (without errors) for processes
    fn get_process_data() -> ProcessData {
        let availabilities = ["process1", "process2"]
            .into_iter()
            .map(|id| {
                let mut map = EnergyLimitsMap::new();
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

        let parameters = ["process1", "process2"]
            .into_iter()
            .map(|id| {
                let mut parameter_map: ProcessParameterMap = HashMap::new();
                let parameter = ProcessParameter {
                    capital_cost: 0.0,
                    fixed_operating_cost: 0.0,
                    variable_operating_cost: 0.0,
                    lifetime: 1,
                    discount_rate: 1.0,
                    capacity_to_activity: 0.0,
                };
                for year in [2010, 2020] {
                    parameter_map.insert(year, parameter.clone());
                }
                (id.into(), parameter_map)
            })
            .collect();

        let region_ids = HashSet::from_iter(iter::once("GBR".into()));

        ProcessData {
            availabilities,
            parameters,
            region_ids,
        }
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
