//! Code for reading process flows file
use super::super::*;
use crate::commodity::CommodityMap;
use crate::process::{FlowType, Process, ProcessFlow, ProcessFlowsMap, ProcessID, ProcessMap};
use crate::region::parse_region_str;
use crate::year::parse_year_str;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessFlowRaw {
    process_id: String,
    commodity_id: String,
    years: String,
    regions: String,
    coeff: f64,
    #[serde(default)]
    #[serde(rename = "type")]
    kind: FlowType,
    cost: Option<f64>,
}

impl ProcessFlowRaw {
    fn validate(&self) -> Result<()> {
        // Check that flow is not infinity, nan, 0 etc.
        ensure!(
            self.coeff.is_normal(),
            "Invalid value for coeff ({})",
            self.coeff
        );

        // **TODO**: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/300
        ensure!(
            self.kind == FlowType::Fixed,
            "Commodity flexible assets are not currently supported"
        );

        // Check that flow cost is non-negative
        if let Some(cost) = self.cost {
            ensure!(
                (0.0..f64::INFINITY).contains(&cost),
                "Invalid value for flow cost ({cost}). Must be >=0."
            )
        }

        Ok(())
    }
}

/// Read process flows from a CSV file
pub fn read_process_flows(
    model_dir: &Path,
    processes: &ProcessMap,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, ProcessFlowsMap>> {
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let process_flow_csv = read_csv(&file_path)?;
    read_process_flows_from_iter(process_flow_csv, processes, commodities)
        .with_context(|| input_err_msg(&file_path))
}

/// Read 'ProcessFlowRaw' records from an iterator and convert them into 'ProcessFlow' records.
fn read_process_flows_from_iter<I>(
    iter: I,
    processes: &ProcessMap,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, ProcessFlowsMap>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    let mut map: HashMap<ProcessID, ProcessFlowsMap> = HashMap::new();
    for record in iter {
        record.validate()?;

        // Get process
        let (id, process) = processes
            .get_key_value(record.process_id.as_str())
            .with_context(|| format!("Process {} not found", record.process_id))?;

        // Get regions
        let process_regions = &process.regions;
        let record_regions =
            parse_region_str(&record.regions, process_regions).with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Get years
        let process_years = &process.years;
        let record_years = parse_year_str(&record.years, process_years).with_context(|| {
            format!("Invalid year for process {id}. Valid years are {process_years:?}")
        })?;

        // Get commodity
        let commodity = commodities
            .get(record.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &record.commodity_id))?;

        // Create ProcessFlow object
        let process_flow = ProcessFlow {
            commodity: Rc::clone(commodity),
            coeff: record.coeff,
            kind: record.kind,
            cost: record.cost.unwrap_or(0.0),
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

    for (process_id, map) in map.iter_mut() {
        let process = processes.get(process_id).unwrap();
        validate_process_flows_map(process, map)?;
    }

    Ok(map)
}

/// Validate flows for a process
fn validate_process_flows_map(process: &Process, map: &ProcessFlowsMap) -> Result<()> {
    let process_id = process.id.clone();
    for year in process.years.iter() {
        for region in process.regions.iter() {
            // Check that the process has flows for this region/year
            ensure!(
                map.contains_key(&(region.clone(), *year)),
                "Missing entry for process {process_id} in {region}/{year}"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityLevyMap, CommodityType, DemandMap};
    use crate::time_slice::TimeSliceLevel;

    use rstest::fixture;

    fn create_process_flow_raw(coeff: f64, kind: FlowType, cost: Option<f64>) -> ProcessFlowRaw {
        ProcessFlowRaw {
            process_id: "process".into(),
            commodity_id: "commodity".into(),
            years: "2020".into(),
            regions: "region".into(),
            coeff,
            kind,
            cost,
        }
    }

    #[test]
    fn test_validate_flow_raw() {
        // Valid
        let valid = create_process_flow_raw(1.0, FlowType::Fixed, Some(0.0));
        assert!(valid.validate().is_ok());

        // Invalid: Bad flow value
        let invalid = create_process_flow_raw(0.0, FlowType::Fixed, Some(0.0));
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::NAN, FlowType::Fixed, Some(0.0));
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::INFINITY, FlowType::Fixed, Some(0.0));
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(f64::NEG_INFINITY, FlowType::Fixed, Some(0.0));
        assert!(invalid.validate().is_err());

        // Invalid: Bad flow cost value
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::NAN));
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::NEG_INFINITY));
        assert!(invalid.validate().is_err());
        let invalid = create_process_flow_raw(1.0, FlowType::Fixed, Some(f64::INFINITY));
        assert!(invalid.validate().is_err());
    }

    #[fixture]
    fn commodity1() -> Commodity {
        Commodity {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: CommodityType::ServiceDemand,
            demand: DemandMap::default(),
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::default(),
        }
    }

    #[fixture]
    fn commodity2() -> Commodity {
        Commodity {
            id: "commodity2".into(),
            description: "Another commodity".into(),
            kind: CommodityType::ServiceDemand,
            demand: DemandMap::default(),
            time_slice_level: TimeSliceLevel::Annual,
            levies: CommodityLevyMap::default(),
        }
    }
}
