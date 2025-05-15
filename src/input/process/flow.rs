//! Code for reading process flows file
use super::super::*;
use crate::commodity::CommodityMap;
use crate::id::IDCollection;
use crate::process::{FlowType, Process, ProcessFlow, ProcessFlowsMap, ProcessID};
use crate::region::parse_region_str;
use crate::year::parse_year_str;
use anyhow::{ensure, Context, Result};
use indexmap::IndexSet;
use itertools::iproduct;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessFlowRaw {
    process_id: String,
    commodity_id: String,
    year: String,
    regions: String,
    flow: f64,
    #[serde(default)]
    flow_type: FlowType,
    flow_cost: Option<f64>,
    is_pac: bool,
}

impl ProcessFlowRaw {
    fn validate(&self) -> Result<()> {
        // Check that flow is not infinity, nan, 0 etc.
        ensure!(
            self.flow.is_normal(),
            "Invalid value for flow ({})",
            self.flow
        );

        // **TODO**: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/300
        ensure!(
            self.flow_type == FlowType::Fixed,
            "Commodity flexible assets are not currently supported"
        );

        // Check that flow cost is non-negative
        if let Some(flow_cost) = self.flow_cost {
            ensure!(
                (0.0..f64::INFINITY).contains(&flow_cost),
                "Invalid value for flow cost ({flow_cost}). Must be >=0."
            )
        }

        Ok(())
    }
}

/// Read process flows from a CSV file
pub fn read_process_flows(
    model_dir: &Path,
    process_ids: &IndexSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, ProcessFlowsMap>> {
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let process_flow_csv = read_csv(&file_path)?;
    read_process_flows_from_iter(process_flow_csv, process_ids, processes, commodities)
        .with_context(|| input_err_msg(&file_path))
}

/// Read 'ProcessFlowRaw' records from an iterator and convert them into 'ProcessFlow' records.
fn read_process_flows_from_iter<I>(
    iter: I,
    process_ids: &IndexSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, ProcessFlowsMap>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    let mut map: HashMap<ProcessID, ProcessFlowsMap> = HashMap::new();
    for record in iter {
        record.validate()?;

        // Get process
        let id = process_ids.get_id_by_str(&record.process_id)?;
        let process = processes
            .get(&id)
            .with_context(|| format!("Process {id} not found"))?;

        // Get regions
        let process_regions = process.regions.clone();
        let record_regions =
            parse_region_str(&record.regions, &process_regions).with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Get years
        let process_years = process.years.clone();
        let record_years = parse_year_str(&record.year, &process_years).with_context(|| {
            format!("Invalid year for process {id}. Valid years are {process_years:?}")
        })?;

        // Get commodity
        let commodity = commodities
            .get(record.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &record.commodity_id))?;

        // Create ProcessFlow object
        let process_flow = ProcessFlow {
            commodity: Rc::clone(commodity),
            flow: record.flow,
            flow_type: record.flow_type,
            flow_cost: record.flow_cost.unwrap_or(0.0),
            is_pac: record.is_pac,
        };

        // Insert flow into the map
        let region_year_map = map.entry(id.clone()).or_default();
        for year in record_years {
            for region in record_regions.iter() {
                let flows_map = region_year_map.entry((region.clone(), year)).or_default();
                let existing = flows_map
                    .insert(commodity.id.clone(), process_flow.clone())
                    .is_some();
                ensure!(
                    !existing,
                    "Duplicate process flow entry for region {}, year {} and commodity {}",
                    region,
                    year,
                    commodity.id
                );
            }
        }
    }

    // Validate flows and sort flows so PACs are at the start
    for (process_id, map) in map.iter_mut() {
        let process = processes.get(process_id).unwrap();
        validate_process_flows_map(process, map)?;
        sort_flows(map)?;
    }

    Ok(map)
}

/// Sort flows so PACs come first
fn sort_flows(map: &mut ProcessFlowsMap) -> Result<()> {
    for map in map.values_mut() {
        map.sort_by(|_, a, _, b| b.is_pac.cmp(&a.is_pac));

        let mut flows = map.values();
        let flow = flows.next().unwrap();
        ensure!(flow.is_pac, "No PACs defined");
        if let Some(flow) = flows.next() {
            ensure!(!flow.is_pac, "More than one PAC defined");
        }
    }

    Ok(())
}

/// Validate flows for a process
fn validate_process_flows_map(process: &Process, map: &ProcessFlowsMap) -> Result<()> {
    let process_id = process.id.clone();
    let reference_years = &process.years;
    let reference_regions = &process.regions;
    for (region, year) in iproduct!(reference_regions, reference_years.iter()) {
        // Check that the process has flows for this region/year
        ensure!(
            map.contains_key(&(region.clone(), *year)),
            format!("Missing entry for process {process_id} in {region}/{year}")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_process_flow_raw(
        flow: f64,
        flow_type: FlowType,
        flow_cost: Option<f64>,
        is_pac: bool,
    ) -> ProcessFlowRaw {
        ProcessFlowRaw {
            process_id: "process".into(),
            commodity_id: "commodity".into(),
            year: "2020".into(),
            regions: "region".into(),
            flow,
            flow_type,
            flow_cost,
            is_pac,
        }
    }

    #[test]
    fn test_validate_flow_raw() {
        // Valid
        let valid = create_process_flow_raw(1.0, FlowType::Fixed, Some(0.0), true);
        assert!(valid.validate().is_ok());

        // Invalid: Bad flow value
        let invalid = create_process_flow_raw(0.0, FlowType::Fixed, Some(0.0), true);
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::NAN, FlowType::Fixed, Some(0.0), true);
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::INFINITY, FlowType::Fixed, Some(0.0), true);
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::NEG_INFINITY, FlowType::Fixed, Some(0.0), true);
        assert!(invalid.validate().is_err());

        // Invalid: Bad flow cost value
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::NAN), true);
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::NEG_INFINITY), true);
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::INFINITY), true);
        assert!(invalid.validate().is_err());
    }
}
