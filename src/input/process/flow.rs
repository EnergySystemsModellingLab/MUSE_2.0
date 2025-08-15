//! Code for reading process flows file
use super::super::*;
use crate::commodity::{CommodityID, CommodityMap};
use crate::process::{FlowType, ProcessFlow, ProcessFlowsMap, ProcessID, ProcessMap};
use crate::region::parse_region_str;
use crate::units::{FlowPerActivity, MoneyPerFlow};
use crate::year::parse_year_str;
use anyhow::{ensure, Context, Result};
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
    years: String,
    regions: String,
    coeff: FlowPerActivity,
    #[serde(default)]
    #[serde(rename = "type")]
    kind: FlowType,
    cost: Option<MoneyPerFlow>,
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
                (0.0..f64::INFINITY).contains(&cost.value()),
                "Invalid value for flow cost ({cost}). Must be >=0."
            )
        }

        Ok(())
    }
}

/// Read process flows from a CSV file
pub fn read_process_flows(
    model_dir: &Path,
    processes: &mut ProcessMap,
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
    processes: &mut ProcessMap,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, ProcessFlowsMap>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    let mut flows_map: HashMap<ProcessID, ProcessFlowsMap> = HashMap::new();
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
            kind: FlowType::Fixed,
            cost: record.cost.unwrap_or(MoneyPerFlow(0.0)),
        };

        // Insert flow into the map
        let region_year_map = flows_map.entry(id.clone()).or_default();
        for (year, region_id) in iproduct!(record_years, record_regions.iter()) {
            let flows_map = region_year_map
                .entry((region_id.clone(), year))
                .or_default();
            let existing = flows_map
                .insert(commodity.id.clone(), process_flow.clone())
                .is_some();
            ensure!(
                !existing,
                "Duplicate process flow entry for region {}, year {} and commodity {}",
                region_id,
                year,
                commodity.id
            );
        }
    }

    check_flows_map_is_complete(processes, &flows_map)?;
    validate_flows_and_update_primary_output(processes, &flows_map)?;

    Ok(flows_map)
}

/// Returns an error if there are years/regions not covered by flows_map for different processes
fn check_flows_map_is_complete(
    processes: &mut ProcessMap,
    flows_map: &HashMap<ProcessID, ProcessFlowsMap>,
) -> Result<()> {
    for (process_id, map) in flows_map.iter() {
        let process = &processes[process_id];
        for (&year, region_id) in iproduct!(process.years.iter(), process.regions.iter()) {
            ensure!(
                map.contains_key(&(region_id.clone(), year)),
                "Missing entry for process {process_id} in {region_id}/{year}"
            );
        }
    }

    Ok(())
}

fn validate_flows_and_update_primary_output(
    processes: &mut ProcessMap,
    flows_map: &HashMap<ProcessID, ProcessFlowsMap>,
) -> Result<()> {
    for (process_id, map) in flows_map.iter() {
        let process = &processes[process_id];
        let mut iter = iproduct!(process.years.iter(), process.regions.iter());

        let primary_output = match &process.primary_output {
            Some(primary_output) => Some(primary_output.clone()),
            None => {
                let (year, region_id) = iter.next().unwrap();
                infer_primary_output(&map[&(region_id.clone(), *year)]).with_context(|| {
                    format!("Could not infer primary_output for process {process_id}")
                })?
            }
        };

        for (&year, region_id) in iter {
            // Check that the process has flows for this region/year
            let flows = &map[&(region_id.clone(), year)];

            check_flows_primary_output(flows, &primary_output).with_context(|| {
                format!(
                    "Invalid primary output configuration for process {process_id}
                    (region: {region_id}, year: {year})"
                )
            })?;
        }
    }

    Ok(())
}

/// Infer the primary output.
///
/// This is only possible if there is only one output flow for the process.
fn infer_primary_output(map: &IndexMap<CommodityID, ProcessFlow>) -> Result<Option<CommodityID>> {
    let mut iter = map
        .iter()
        .filter_map(|(commodity_id, flow)| flow.is_output().then_some(commodity_id));

    let Some(first_output) = iter.next() else {
        // If there are only input flows, then the primary output should be None
        return Ok(None);
    };

    ensure!(
        iter.next().is_none(),
        "Need to specify primary_output explicitly if there are multiple output flows"
    );

    Ok(Some(first_output.clone()))
}

/// Check the flows are correct for the specified primary output (or lack thereof)
fn check_flows_primary_output(
    flows_map: &IndexMap<CommodityID, ProcessFlow>,
    primary_output: &Option<CommodityID>,
) -> Result<()> {
    if let Some(primary_output) = primary_output {
        let Some(flow) = flows_map.get(primary_output) else {
            bail!(
                "Primary output commodity '{}' isn't a process flow",
                primary_output
            );
        };

        ensure!(
            flow.is_output(),
            "Primary output commodity '{}' isn't an output flow",
            primary_output
        );
    } else {
        ensure!(
            flows_map.values().all(|flow| flow.is_input()),
            "First year is only inputs, but subsequent years have outputs, although no primary \
            output is specified"
        );
    }

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::commodity::Commodity;
//     use crate::fixture::svd_commodity;

//     use rstest::rstest;
//     use std::rc::Rc;

//     fn flow(commodity: Rc<Commodity>, coeff: f64) -> ProcessFlow {
//         ProcessFlow {
//             commodity,
//             coeff: FlowPerActivity(coeff),
//             kind: FlowType::Fixed,
//             cost: MoneyPerFlow(0.0),
//         }
//     }

//     fn create_process_flow_raw(
//         coeff: FlowPerActivity,
//         cost: Option<MoneyPerFlow>,
//     ) -> ProcessFlowRaw {
//         ProcessFlowRaw {
//             process_id: "process".into(),
//             commodity_id: "commodity".into(),
//             years: "2020".into(),
//             regions: "region".into(),
//             coeff,
//             kind: FlowType::Fixed,
//             cost,
//         }
//     }

//     #[test]
//     fn test_validate_flow_raw() {
//         // Valid
//         let valid = create_process_flow_raw(FlowPerActivity(1.0), Some(MoneyPerFlow(0.0)));
//         assert!(valid.validate().is_ok());

//         // Invalid: Bad flow value
//         let invalid = create_process_flow_raw(FlowPerActivity(0.0), Some(MoneyPerFlow(0.0)));
//         assert!(invalid.validate().is_err());
//         let invalid = create_process_flow_raw(FlowPerActivity(f64::NAN), Some(MoneyPerFlow(0.0)));
//         assert!(invalid.validate().is_err());
//         let invalid =
//             create_process_flow_raw(FlowPerActivity(f64::INFINITY), Some(MoneyPerFlow(0.0)));
//         assert!(invalid.validate().is_err());
//         let invalid =
//             create_process_flow_raw(FlowPerActivity(f64::NEG_INFINITY), Some(MoneyPerFlow(0.0)));
//         assert!(invalid.validate().is_err());

//         // Invalid: Bad flow cost value
//         let invalid = create_process_flow_raw(FlowPerActivity(1.0), Some(MoneyPerFlow(f64::NAN)));
//         assert!(invalid.validate().is_err());
//         let invalid =
//             create_process_flow_raw(FlowPerActivity(1.0), Some(MoneyPerFlow(f64::NEG_INFINITY)));
//         assert!(invalid.validate().is_err());
//         let invalid =
//             create_process_flow_raw(FlowPerActivity(1.0), Some(MoneyPerFlow(f64::INFINITY)));
//         assert!(invalid.validate().is_err());
//     }

//     #[rstest]
//     fn single_output_explicit_primary(#[from(svd_commodity)] commodity: Commodity) {
//         let c1 = Rc::new(commodity);
//         let mut flows = IndexMap::new();
//         flows.insert("commodity1".into(), flow(Rc::clone(&c1), 1.0));
//         let primary_outputs = vec![("commodity1".into(), Some(true))];
//         let res = check_flows_primary_output(&flows, &primary_outputs).unwrap();
//         assert_eq!(res, Some("commodity1".into()));
//     }

//     #[rstest]
//     fn multiple_outputs_one_explicit_primary(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), 1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), 2.0));
//         let primary_outputs = vec![("c1".into(), Some(true)), ("c2".into(), None)];
//         let res = check_flows_primary_output(&flows, &primary_outputs).unwrap();
//         assert_eq!(res, Some("c1".into()));
//     }

//     #[rstest]
//     fn multiple_outputs_none_explicit_should_error(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), 1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), 2.0));
//         let primary_outputs = vec![("c1".into(), None), ("c2".into(), None)];
//         let res = check_flows_primary_output(&flows, &primary_outputs);
//         assert!(res.is_err());
//     }

//     #[rstest]
//     fn multiple_outputs_all_explicit_false_should_error(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), 1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), 2.0));
//         let primary_outputs = vec![("c1".into(), Some(false)), ("c2".into(), Some(false))];
//         let res = check_flows_primary_output(&flows, &primary_outputs);
//         assert!(res.is_err());
//     }

//     #[rstest]
//     fn all_inputs(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), -1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), -2.0));
//         let primary_outputs = vec![("c1".into(), None), ("c2".into(), None)];
//         let res = check_flows_primary_output(&flows, &primary_outputs).unwrap();
//         assert_eq!(res, None);
//     }

//     #[rstest]
//     fn multiple_outputs_multiple_explicit_primaries_should_error(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), 1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), 2.0));
//         let primary_outputs = vec![("c1".into(), Some(true)), ("c2".into(), Some(true))];
//         let res = check_flows_primary_output(&flows, &primary_outputs);
//         assert!(res.is_err());
//     }

//     #[rstest]
//     fn explicit_primary_on_input_should_error(
//         #[from(svd_commodity)] commodity1: Commodity,
//         #[from(svd_commodity)] commodity2: Commodity,
//     ) {
//         let c1 = Rc::new(Commodity {
//             id: "c1".into(),
//             ..commodity1
//         });
//         let c2 = Rc::new(Commodity {
//             id: "c2".into(),
//             ..commodity2
//         });
//         let mut flows = IndexMap::new();
//         flows.insert("c1".into(), flow(Rc::clone(&c1), -1.0));
//         flows.insert("c2".into(), flow(Rc::clone(&c2), 2.0));
//         let primary_outputs = vec![("c1".into(), Some(true)), ("c2".into(), None)];
//         let res = check_flows_primary_output(&flows, &primary_outputs);
//         assert!(res.is_err());
//     }
// }
