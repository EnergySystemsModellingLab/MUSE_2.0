//! Code for reading process-related information from CSV files.
use super::*;
use crate::commodity::{Commodity, CommodityID, CommodityMap, CommodityType};
use crate::process::{
    Process, ProcessActivityLimitsMap, ProcessFlowsMap, ProcessID, ProcessMap, ProcessParameterMap,
};
use crate::region::{parse_region_str, RegionID};
use crate::time_slice::{TimeSliceInfo, TimeSliceSelection};
use crate::units::Flow;
use anyhow::{ensure, Context, Ok, Result};
use indexmap::IndexSet;
use itertools::iproduct;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

mod availability;
use availability::read_process_availabilities;
mod flow;
use flow::read_process_flows;
mod parameter;
use crate::id::define_id_getter;
use parameter::read_process_parameters;

const PROCESSES_FILE_NAME: &str = "processes.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessRaw {
    id: ProcessID,
    description: String,
    regions: String,
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
    region_ids: &IndexSet<RegionID>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<ProcessMap> {
    let mut processes = read_processes_file(model_dir, milestone_years, region_ids)?;
    let mut activity_limits = read_process_availabilities(model_dir, &processes, time_slice_info)?;
    let mut flows = read_process_flows(model_dir, &processes, commodities)?;
    let mut parameters = read_process_parameters(model_dir, &processes)?;

    // Validate commodities after the flows have been read
    validate_commodities(
        commodities,
        &flows,
        &activity_limits,
        region_ids,
        milestone_years,
        time_slice_info,
    )?;

    // Add data to Process objects
    for (id, process) in processes.iter_mut() {
        // This will always succeed as we know there will only be one reference to the process here
        let process = Rc::get_mut(process).unwrap();
        process.activity_limits = activity_limits
            .remove(id)
            .with_context(|| format!("Missing availabilities for process {id}"))?;
        process.flows = flows
            .remove(id)
            .with_context(|| format!("Missing flows for process {id}"))?;
        process.parameters = parameters
            .remove(id)
            .with_context(|| format!("Missing parameters for process {id}"))?;
    }

    Ok(processes)
}

fn read_processes_file(
    model_dir: &Path,
    milestone_years: &[u32],
    region_ids: &IndexSet<RegionID>,
) -> Result<ProcessMap> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let processes_csv = read_csv(&file_path)?;
    read_processes_file_from_iter(processes_csv, milestone_years, region_ids)
        .with_context(|| input_err_msg(&file_path))
}

fn read_processes_file_from_iter<I>(
    iter: I,
    milestone_years: &[u32],
    region_ids: &IndexSet<RegionID>,
) -> Result<ProcessMap>
where
    I: Iterator<Item = ProcessRaw>,
{
    let mut processes = ProcessMap::new();
    for process_raw in iter {
        let start_year = process_raw.start_year.unwrap_or(milestone_years[0]);
        let end_year = process_raw
            .end_year
            .unwrap_or(*milestone_years.last().unwrap());

        // Check year range is valid
        ensure!(
            start_year <= end_year,
            "Error in parameter for process {}: start_year > end_year",
            process_raw.id
        );

        // Select process years
        let years = milestone_years
            .iter()
            .copied()
            .filter(|year| (start_year..=end_year).contains(year))
            .collect();

        // Parse region ID
        let regions = parse_region_str(&process_raw.regions, region_ids)?;

        let process = Process {
            id: process_raw.id.clone(),
            description: process_raw.description,
            years,
            activity_limits: ProcessActivityLimitsMap::new(),
            flows: ProcessFlowsMap::new(),
            parameters: ProcessParameterMap::new(),
            regions,
        };

        ensure!(
            processes.insert(process_raw.id, process.into()).is_none(),
            "Duplicate process ID"
        );
    }

    Ok(processes)
}

/// Perform consistency checks for commodity flows.
fn validate_commodities(
    commodities: &CommodityMap,
    flows: &HashMap<ProcessID, ProcessFlowsMap>,
    availabilities: &HashMap<ProcessID, ProcessActivityLimitsMap>,
    region_ids: &IndexSet<RegionID>,
    milestone_years: &[u32],
    time_slice_info: &TimeSliceInfo,
) -> Result<()> {
    for commodity in commodities.values() {
        if commodity.kind == CommodityType::Other {
            validate_other_commodity(&commodity.id, flows)?;
            continue;
        }

        for (region_id, year) in iproduct!(region_ids.iter(), milestone_years.iter().copied()) {
            match commodity.kind {
                CommodityType::SupplyEqualsDemand => {
                    validate_sed_commodity(&commodity.id, flows, region_id, year)?;
                }
                CommodityType::ServiceDemand => {
                    for ts_selection in
                        time_slice_info.iter_selections_at_level(commodity.time_slice_level)
                    {
                        validate_svd_commodity(
                            time_slice_info,
                            commodity,
                            flows,
                            availabilities,
                            region_id,
                            year,
                            &ts_selection,
                        )?;
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}

/// Check that commodities of type other are either produced or consumed but not both
fn validate_other_commodity(
    commodity_id: &CommodityID,
    flows: &HashMap<ProcessID, ProcessFlowsMap>,
) -> Result<()> {
    let mut is_producer = None;
    for flows in flows.values().flat_map(|flows| flows.values()) {
        if let Some(flow) = flows.get(commodity_id) {
            let cur_is_producer = flow.is_output();
            if let Some(is_producer) = is_producer {
                ensure!(
                    is_producer == cur_is_producer,
                    "{commodity_id} is both a producer and consumer. \
                    Commodities of type 'other' must only be consumed or produced."
                );
            } else {
                is_producer = Some(cur_is_producer);
            }
        }
    }

    ensure!(
        is_producer.is_some(),
        "Commodity {commodity_id} is neither produced or consumed."
    );

    Ok(())
}

/// Check that an SED commodity has a consumer and producer process
fn validate_sed_commodity(
    commodity_id: &CommodityID,
    flows: &HashMap<ProcessID, ProcessFlowsMap>,
    region_id: &RegionID,
    year: u32,
) -> Result<()> {
    let mut has_producer = false;
    let mut has_consumer = false;
    for flows in flows.values() {
        let flows = flows.get(&(region_id.clone(), year)).unwrap();
        if let Some(flow) = flows.get(&commodity_id.clone()) {
            if flow.is_output() {
                has_producer = true;
            } else if flow.is_input() {
                has_consumer = true;
            }
        }
    }

    ensure!(has_consumer && has_producer,
        "Commodity {} of 'SED' type must have both producer and consumer processes for region {} in year {}",
        commodity_id,
        region_id,
        year,
    );

    Ok(())
}

fn validate_svd_commodity(
    time_slice_info: &TimeSliceInfo,
    commodity: &Commodity,
    flows: &HashMap<ProcessID, ProcessFlowsMap>,
    availabilities: &HashMap<ProcessID, ProcessActivityLimitsMap>,
    region_id: &RegionID,
    year: u32,
    ts_selection: &TimeSliceSelection,
) -> Result<()> {
    // Check if the commodity has a demand in the given time slice, region and year.
    // We only need to check for producers if there is positive demand.
    let demand = *commodity
        .demand
        .get(&(region_id.clone(), year, ts_selection.clone()))
        .unwrap();
    if demand <= Flow(0.0) {
        return Ok(());
    }

    // We must check for producers in the given year, region and time slices.
    // This includes checking if flow > 0 and if availability > 0.
    for (process_id, flows) in flows.iter() {
        let flows = flows.get(&(region_id.clone(), year)).unwrap();
        let Some(flow) = flows.get(&commodity.id) else {
            // We're only interested in processes which produce this commodity
            continue;
        };
        ensure!(
            flow.is_output(),
            "SVD commodity {} is consumed by process {}. \
            SVD commodities can only be produced, not consumed.",
            commodity.id,
            process_id
        );

        // If the process has availability >0 in any time slice for this selection, we accept it
        let availabilities = availabilities.get(process_id).unwrap();
        for (ts, _) in ts_selection.iter(time_slice_info) {
            let availability = availabilities
                .get(&(region_id.clone(), year, ts.clone()))
                .unwrap();
            if *availability.end() > Dimensionless(0.0) {
                return Ok(());
            }
        }
    }

    // If we reach this point it means there is no producer, so we return an error.
    bail!(
        "Commodity {} of 'SVD' type must have a producer process for region {} in year {} and time slice(s) {}",
        commodity.id,
        region_id,
        year,
        ts_selection,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{CommodityLevyMap, DemandMap};
    use crate::fixture::{assert_error, time_slice, time_slice_info};
    use crate::process::{FlowType, ProcessFlow};
    use crate::time_slice::{TimeSliceID, TimeSliceLevel};
    use crate::units::{Dimensionless, FlowPerActivity, MoneyPerFlow};
    use indexmap::indexmap;
    use rstest::{fixture, rstest};
    use std::iter;

    #[fixture]
    fn commodity_sed() -> Commodity {
        Commodity {
            id: "commodity_sed".into(),
            description: "SED commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::new(),
            demand: DemandMap::new(),
        }
    }

    #[fixture]
    fn input_flows_sed(commodity_sed: Commodity) -> ProcessFlowsMap {
        ProcessFlowsMap::from_iter([(
            ("GBR".into(), 2010),
            indexmap! { commodity_sed.id.clone() => ProcessFlow {
                commodity: commodity_sed.into(),
                coeff: FlowPerActivity(-10.0),
                kind: FlowType::Fixed,
                cost: MoneyPerFlow(1.0),
                is_primary_output: false,
            }},
        )])
    }

    #[fixture]
    fn output_flows_sed(commodity_sed: Commodity) -> ProcessFlowsMap {
        ProcessFlowsMap::from_iter([(
            ("GBR".into(), 2010),
            indexmap! {commodity_sed.id.clone()=>ProcessFlow {
                commodity: commodity_sed.into(),
                coeff: FlowPerActivity(10.0),
                kind: FlowType::Fixed,
                cost: MoneyPerFlow(1.0),
                is_primary_output: false,
            }},
        )])
    }

    #[rstest]
    fn test_validate_sed_commodity_valid(
        commodity_sed: Commodity,
        input_flows_sed: ProcessFlowsMap,
        output_flows_sed: ProcessFlowsMap,
    ) {
        // Valid scenario
        let flows = HashMap::from_iter([
            ("process1".into(), input_flows_sed.clone()),
            ("process2".into(), output_flows_sed.clone()),
        ]);
        assert!(validate_sed_commodity(&commodity_sed.id, &flows, &"GBR".into(), 2010).is_ok());
    }

    #[rstest]
    fn test_validate_sed_commodity_invalid_no_producer(
        commodity_sed: Commodity,
        input_flows_sed: ProcessFlowsMap,
    ) {
        // Invalid scenario: no producer
        let flows = HashMap::from_iter([("process1".into(), input_flows_sed.clone())]);
        assert_error!(
            validate_sed_commodity(&commodity_sed.id, &flows, &"GBR".into(), 2010),
            "Commodity commodity_sed of 'SED' type must have both producer and consumer processes for region GBR in year 2010"
        );
    }

    #[rstest]
    fn test_validate_sed_commodity(commodity_sed: Commodity, output_flows_sed: ProcessFlowsMap) {
        // Invalid scenario: no consumer
        let flows = HashMap::from_iter([("process2".into(), output_flows_sed.clone())]);
        assert_error!(
            validate_sed_commodity(&commodity_sed.id, &flows, &"GBR".into(), 2010),
            "Commodity commodity_sed of 'SED' type must have both producer and consumer processes for region GBR in year 2010"
        );
    }

    #[fixture]
    fn commodity_svd(time_slice: TimeSliceID) -> Commodity {
        let demand = DemandMap::from_iter([(("GBR".into(), 2010, time_slice.into()), Flow(10.0))]);

        Commodity {
            id: "commodity_svd".into(),
            description: "SVD commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::new(),
            demand,
        }
    }

    #[fixture]
    fn flows_svd(commodity_svd: Commodity) -> HashMap<ProcessID, ProcessFlowsMap> {
        HashMap::from_iter([(
            "process1".into(),
            ProcessFlowsMap::from_iter([(
                ("GBR".into(), 2010),
                indexmap! { commodity_svd.id.clone() => ProcessFlow {
                    commodity: commodity_svd.into(),
                    coeff: FlowPerActivity(10.0),
                    kind: FlowType::Fixed,
                    cost: MoneyPerFlow(1.0),
                    is_primary_output: false,
                }},
            )]),
        )])
    }

    #[rstest]
    fn test_validate_svd_commodity_valid(
        commodity_svd: Commodity,
        flows_svd: HashMap<ProcessID, ProcessFlowsMap>,
        time_slice_info: TimeSliceInfo,
        time_slice: TimeSliceID,
    ) {
        let availabilities = HashMap::from_iter([(
            "process1".into(),
            ProcessActivityLimitsMap::from_iter([(
                ("GBR".into(), 2010, time_slice.clone()),
                Dimensionless(0.1)..=Dimensionless(0.9),
            )]),
        )]);

        // Valid scenario
        assert!(validate_svd_commodity(
            &time_slice_info,
            &commodity_svd,
            &flows_svd,
            &availabilities,
            &"GBR".into(),
            2010,
            &time_slice.into()
        )
        .is_ok());
    }

    #[rstest]
    fn test_validate_svd_commodity_invalid_no_availability(
        time_slice_info: TimeSliceInfo,
        commodity_svd: Commodity,
        flows_svd: HashMap<ProcessID, ProcessFlowsMap>,
        time_slice: TimeSliceID,
    ) {
        // Invalid scenario: no availability
        let availabilities = HashMap::from_iter([(
            "process1".into(),
            ProcessActivityLimitsMap::from_iter([(
                ("GBR".into(), 2010, time_slice.clone()),
                Dimensionless(0.0)..=Dimensionless(0.0),
            )]),
        )]);
        assert_error!(
            validate_svd_commodity(
                &time_slice_info,
                &commodity_svd,
                &flows_svd,
                &availabilities,
                &"GBR".into(),
                2010,
                &time_slice.into()
            ),
            "Commodity commodity_svd of 'SVD' type must have a producer process \
            for region GBR in year 2010 and time slice(s) winter.day"
        );
    }

    #[fixture]
    fn commodity_other() -> Commodity {
        Commodity {
            id: "commodity_other".into(),
            description: "Other commodity".into(),
            kind: CommodityType::Other,
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::new(),
            demand: DemandMap::new(),
        }
    }

    #[fixture]
    fn producer_flows(commodity_other: Commodity) -> ProcessFlowsMap {
        ProcessFlowsMap::from_iter([(
            ("GBR".into(), 2010),
            indexmap! { commodity_other.id.clone() => ProcessFlow {
                commodity: commodity_other.into(),
                coeff: FlowPerActivity(10.0),
                kind: FlowType::Fixed,
                cost: MoneyPerFlow(1.0),
                is_primary_output: false,
            }},
        )])
    }

    #[fixture]
    fn consumer_flows(commodity_other: Commodity) -> ProcessFlowsMap {
        ProcessFlowsMap::from_iter([(
            ("GBR".into(), 2010),
            indexmap! { commodity_other.id.clone() => ProcessFlow {
                commodity: commodity_other.into(),
                coeff: FlowPerActivity(-10.0),
                kind: FlowType::Fixed,
                cost: MoneyPerFlow(1.0),
                is_primary_output: false,
            }},
        )])
    }

    #[rstest]
    fn test_validate_other_commodity_valid_producer(
        commodity_other: Commodity,
        producer_flows: ProcessFlowsMap,
    ) {
        // Valid scenario: commodity is only produced
        let flows = HashMap::from_iter([("process1".into(), producer_flows)]);
        assert!(validate_other_commodity(&commodity_other.id, &flows).is_ok());
    }

    #[rstest]
    fn test_validate_other_commodity_valid_consumer(
        commodity_other: Commodity,
        consumer_flows: ProcessFlowsMap,
    ) {
        // Valid scenario: commodity is only consumed
        let flows = HashMap::from_iter([("process1".into(), consumer_flows)]);
        assert!(validate_other_commodity(&commodity_other.id, &flows).is_ok());
    }

    #[rstest]
    fn test_validate_other_commodity_invalid_both(
        commodity_other: Commodity,
        producer_flows: ProcessFlowsMap,
        consumer_flows: ProcessFlowsMap,
    ) {
        // Invalid scenario: commodity is both produced and consumed
        let flows = HashMap::from_iter([
            ("process1".into(), producer_flows),
            ("process2".into(), consumer_flows),
        ]);
        assert_error!(
            validate_other_commodity(&commodity_other.id, &flows),
            "commodity_other is both a producer and consumer. \
             Commodities of type 'other' must only be consumed or produced."
        );
    }

    #[rstest]
    fn test_validate_other_commodity_invalid_neither(commodity_other: Commodity) {
        // Invalid scenario: commodity is neither produced nor consumed
        let flows = HashMap::new();
        assert_error!(
            validate_other_commodity(&commodity_other.id, &flows),
            "Commodity commodity_other is neither produced or consumed."
        );
    }

    #[rstest]
    fn test_validate_svd_commodity_invalid_consumed(
        commodity_svd: Commodity,
        time_slice_info: TimeSliceInfo,
        time_slice: TimeSliceID,
    ) {
        let commodity_svd = Rc::new(commodity_svd);
        let region_id = RegionID("GBR".into());
        let availabilities = HashMap::from_iter([(
            "process1".into(),
            ProcessActivityLimitsMap::from_iter([(
                (region_id.clone(), 2010, time_slice.clone()),
                Dimensionless(0.1)..=Dimensionless(0.9),
            )]),
        )]);
        let flows = HashMap::from_iter(iter::once((
            "process1".into(),
            ProcessFlowsMap::from_iter([(
                (region_id.clone(), 2010),
                indexmap! { commodity_svd.id.clone() => ProcessFlow {
                    commodity: Rc::clone(&commodity_svd),
                    coeff: FlowPerActivity(-10.0),
                    kind: FlowType::Fixed,
                    cost: MoneyPerFlow(1.0),
                    is_primary_output: false,
                }},
            )]),
        )));
        assert_error!(
            validate_svd_commodity(
                &time_slice_info,
                &commodity_svd,
                &flows,
                &availabilities,
                &region_id,
                2010,
                &time_slice.into()
            ),
            "SVD commodity commodity_svd is consumed by process process1. \
            SVD commodities can only be produced, not consumed."
        );
    }
}
